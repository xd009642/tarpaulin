use crate::breakpoint::*;
use crate::cargo::rust_flags;
use crate::config::Config;
use crate::errors::RunError;
use crate::generate_tracemap;
use crate::ptrace_control::*;
use crate::source_analysis::LineAnalysis;
use crate::statemachine::*;
use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::sys::wait::*;
use nix::unistd::Pid;
use nix::Error as NixErr;
use procfs::process::{MMapPath, Process};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use tracing::{debug, trace, trace_span, warn};

/// Handle to linux process state
pub struct LinuxData<'a> {
    /// Recent results from waitpid to be handled by statemachine
    wait_queue: Vec<WaitStatus>,
    /// Parent pid of the test
    parent: Pid,
    /// Current Pid to process
    current: Pid,
    /// Program config
    config: &'a Config,
    /// Optional event log to update as the test progresses
    event_log: &'a Option<EventLog>,
    /// Instrumentation points in code with associated coverage data
    traces: &'a mut TraceMap,
    /// Source analysis, needed in case we need to follow any executables
    analysis: &'a HashMap<PathBuf, LineAnalysis>,
    /// Processes we're tracing
    processes: HashMap<Pid, TracedProcess>,
    /// Map from pids to their parent
    pid_map: HashMap<Pid, Pid>,
}

#[derive(Debug)]
pub struct TracedProcess {
    /// Map of addresses to breakpoints
    breakpoints: HashMap<u64, Breakpoint>,
    /// Thread count. Hopefully getting rid of in future
    thread_count: isize,
    /// Breakpoint offset
    offset: u64,
    /// Instrumentation points in code with associated coverage data
    /// If this is the root tracemap we don't use it...
    traces: Option<TraceMap>,
    /// Parent pid of the process
    parent: Pid,
    /// Whether the process is part of the test binary, or the result of an exec or fork
    is_test_proc: bool,
}

pub fn create_state_machine<'a>(
    test: Pid,
    traces: &'a mut TraceMap,
    source_analysis: &'a HashMap<PathBuf, LineAnalysis>,
    config: &'a Config,
    event_log: &'a Option<EventLog>,
) -> (TestState, LinuxData<'a>) {
    let mut data = LinuxData::new(traces, source_analysis, config, event_log);
    data.parent = test;
    (TestState::start_state(), data)
}

pub type UpdateContext = (TestState, TracerAction<ProcessInfo>);

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ProcessInfo {
    pub(crate) pid: Pid,
    pub(crate) signal: Option<Signal>,
}

impl ProcessInfo {
    fn new(pid: Pid, signal: Option<Signal>) -> Self {
        Self { pid, signal }
    }
}

impl From<Pid> for ProcessInfo {
    fn from(pid: Pid) -> Self {
        ProcessInfo::new(pid, None)
    }
}

impl From<&Pid> for ProcessInfo {
    fn from(pid: &Pid) -> Self {
        ProcessInfo::new(*pid, None)
    }
}

fn get_offset(pid: Pid, config: &Config) -> u64 {
    if rust_flags(config).contains("dynamic-no-pic") {
        0
    } else if let Ok(proc) = Process::new(pid.as_raw()) {
        let exe = proc.exe().ok();
        if let Ok(maps) = proc.maps() {
            let offset_info = maps.iter().find(|x| match (&x.pathname, exe.as_ref()) {
                (MMapPath::Path(p), Some(e)) => p == e,
                (MMapPath::Path(_), None) => true,
                _ => false,
            });
            if let Some(first) = offset_info {
                first.address.0
            } else {
                0
            }
        } else {
            0
        }
    } else {
        0
    }
}

impl<'a> StateData for LinuxData<'a> {
    fn start(&mut self) -> Result<Option<TestState>, RunError> {
        match waitpid(self.current, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => Ok(None),
            Ok(sig @ WaitStatus::Stopped(_, Signal::SIGTRAP)) => {
                if let WaitStatus::Stopped(child, _) = sig {
                    self.current = child;
                }
                trace!("Caught inferior transitioning to Initialise state");
                Ok(Some(TestState::Initialise))
            }
            Ok(_) => Err(RunError::TestRuntime(
                "Unexpected signal when starting test".to_string(),
            )),
            Err(e) => Err(RunError::TestRuntime(format!(
                "Error when starting test: {}",
                e
            ))),
        }
    }

    fn init(&mut self) -> Result<TestState, RunError> {
        let mut traced_process = self.init_process(self.current, None)?;
        traced_process.is_test_proc = true;

        if continue_exec(traced_process.parent, None).is_ok() {
            trace!("Initialised inferior, transitioning to wait state");
            self.processes.insert(self.current, traced_process);
            Ok(TestState::wait_state())
        } else {
            Err(RunError::TestRuntime(
                "Test didn't launch correctly".to_string(),
            ))
        }
    }

    fn wait(&mut self) -> Result<Option<TestState>, RunError> {
        let mut result = Ok(None);
        let mut running = true;
        while running {
            let wait = waitpid(
                Pid::from_raw(-1),
                Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
            );
            match wait {
                Ok(WaitStatus::StillAlive) => {
                    running = false;
                }
                Ok(WaitStatus::Exited(_, _)) => {
                    self.wait_queue.push(wait.unwrap());
                    result = Ok(Some(TestState::Stopped));
                    running = false;
                }
                Ok(WaitStatus::PtraceEvent(_, _, _)) => {
                    self.wait_queue.push(wait.unwrap());
                    result = Ok(Some(TestState::Stopped));
                    running = false;
                }
                Ok(s) => {
                    self.wait_queue.push(s);
                    result = Ok(Some(TestState::Stopped));
                }
                Err(e) => {
                    running = false;
                    result = Err(RunError::TestRuntime(format!(
                        "An error occurred while waiting for response from test: {}",
                        e
                    )))
                }
            }
        }
        if !self.wait_queue.is_empty() {
            trace!("Result queue is {:?}", self.wait_queue);
        }
        result
    }

    fn stop(&mut self) -> Result<TestState, RunError> {
        let mut actions = Vec::new();
        let mut pcs = HashMap::new();
        let mut result = Ok(TestState::wait_state());
        let pending = self.wait_queue.clone();
        self.wait_queue.clear();
        for status in &pending {
            let span = trace_span!("pending", event=?status.pid());
            let _enter = span.enter();
            if let Some(log) = self.event_log.as_ref() {
                let offset = if let Some(process) = self.get_traced_process_mut(self.current) {
                    process.offset
                } else {
                    0
                };
                let event = TraceEvent::new_from_wait(&status, offset, self.traces);
                log.push_trace(event);
            }
            let state = match status {
                WaitStatus::PtraceEvent(c, s, e) => match self.handle_ptrace_event(*c, *s, *e) {
                    Ok(s) => Ok(s),
                    Err(e) => Err(RunError::TestRuntime(format!(
                        "Error occurred when handling ptrace event: {}",
                        e
                    ))),
                },
                WaitStatus::Stopped(c, Signal::SIGTRAP) => {
                    self.current = *c;
                    match self.collect_coverage_data(&mut pcs) {
                        Ok(s) => Ok(s),
                        Err(e) => Err(RunError::TestRuntime(format!(
                            "Error when collecting coverage: {}",
                            e
                        ))),
                    }
                }
                WaitStatus::Stopped(child, Signal::SIGSTOP) => Ok((
                    TestState::wait_state(),
                    TracerAction::Continue(child.into()),
                )),
                WaitStatus::Stopped(_, Signal::SIGSEGV) => Err(RunError::TestRuntime(
                    "A segfault occurred while executing tests".to_string(),
                )),
                WaitStatus::Stopped(child, Signal::SIGILL) => {
                    let pc = current_instruction_pointer(*child).unwrap_or_else(|_| 1) - 1;
                    trace!("SIGILL raised. Child program counter is: 0x{:x}", pc);
                    Err(RunError::TestRuntime(format!(
                        "Error running test - SIGILL raised in {}",
                        child
                    )))
                }
                WaitStatus::Stopped(c, Signal::SIGCHLD) => {
                    Ok((TestState::wait_state(), TracerAction::Continue(c.into())))
                }
                WaitStatus::Stopped(c, s) => {
                    let sig = if self.config.forward_signals {
                        Some(*s)
                    } else {
                        None
                    };
                    let info = ProcessInfo::new(*c, sig);
                    Ok((TestState::wait_state(), TracerAction::TryContinue(info)))
                }
                WaitStatus::Signaled(c, s, f) => {
                    if let Ok(s) = self.handle_signaled(c, s, *f) {
                        Ok(s)
                    } else {
                        Err(RunError::TestRuntime(
                            "Attempting to handle tarpaulin being signaled".to_string(),
                        ))
                    }
                }
                WaitStatus::Exited(child, ec) => {
                    let mut parent = Pid::from_raw(0);
                    if let Some(proc) = self.get_traced_process_mut(*child) {
                        for ref mut value in proc.breakpoints.values_mut() {
                            value.thread_killed(*child);
                        }
                        parent = proc.parent;
                    }
                    if &parent == child {
                        if let Some(removed) = self.processes.remove(&parent) {
                            if parent != self.parent {
                                let traces = removed.traces.unwrap();
                                self.traces.merge(&traces);
                            }
                        }
                    }
                    trace!("Exited {:?} parent {:?}", child, self.parent);
                    if child == &self.parent {
                        Ok((TestState::End(*ec), TracerAction::Nothing))
                    } else {
                        // Process may have already been destroyed. This is just incase
                        Ok((
                            TestState::wait_state(),
                            TracerAction::TryContinue(self.parent.into()),
                        ))
                    }
                }
                _ => Err(RunError::TestRuntime(
                    "An unexpected signal has been caught by tarpaulin!".to_string(),
                )),
            };
            match state {
                Ok((TestState::Waiting { .. }, action)) => {
                    actions.push(action);
                }
                Ok((state, action)) => {
                    result = Ok(state);
                    actions.push(action);
                }
                Err(e) => result = Err(e),
            }
        }
        let mut continued = false;
        let mut actioned_pids = HashSet::new();
        for a in &actions {
            if let Some(d) = a.get_data() {
                if actioned_pids.contains(&d.pid) {
                    trace!("Skipping action '{:?}', pid already sent command", a);
                    continue;
                } else {
                    trace!("No action for {} yet", d.pid);
                }
            } else {
                trace!("No process info for action");
            }
            trace!("Action: {:?}", a);
            if let Some(log) = self.event_log.as_ref() {
                let event = TraceEvent::new_from_action(&a);
                log.push_trace(event);
            }
            match a {
                TracerAction::TryContinue(t) => {
                    continued = true;
                    actioned_pids.insert(t.pid);
                    let _ = continue_exec(t.pid, t.signal);
                }
                TracerAction::Continue(t) => {
                    continued = true;
                    actioned_pids.insert(t.pid);
                    continue_exec(t.pid, t.signal)?;
                }
                TracerAction::Step(t) => {
                    continued = true;
                    actioned_pids.insert(t.pid);
                    single_step(t.pid)?;
                }
                TracerAction::Detach(t) => {
                    continued = true;
                    actioned_pids.insert(t.pid);
                    detach_child(t.pid)?;
                }
                _ => {}
            }
        }
        if !continued {
            trace!("No action suggested to continue tracee. Attempting a continue");
            let _ = continue_exec(self.parent, None);
        }
        result
    }
}

impl<'a> LinuxData<'a> {
    pub fn new(
        traces: &'a mut TraceMap,
        analysis: &'a HashMap<PathBuf, LineAnalysis>,
        config: &'a Config,
        event_log: &'a Option<EventLog>,
    ) -> LinuxData<'a> {
        LinuxData {
            wait_queue: Vec::new(),
            processes: HashMap::new(),
            current: Pid::from_raw(0),
            parent: Pid::from_raw(0),
            traces,
            analysis,
            config,
            event_log,
            pid_map: HashMap::new(),
        }
    }

    fn get_parent(&self, pid: Pid) -> Option<Pid> {
        match self.pid_map.get(&pid) {
            Some(p) => Some(*p),
            None => {
                let mut parent_pid = None;
                'outer: for k in self.processes.keys() {
                    let proc = Process::new(k.as_raw()).ok()?;
                    if let Ok(tasks) = proc.tasks() {
                        for task in tasks.filter_map(|x| x.ok()) {
                            if task.tid == pid.as_raw() {
                                parent_pid = Some(k.clone());
                                break 'outer;
                            }
                        }
                    }
                }
                parent_pid
            }
        }
    }

    fn get_traced_process_mut(&mut self, pid: Pid) -> Option<&mut TracedProcess> {
        let parent = self.get_parent(pid)?;
        self.processes.get_mut(&parent)
    }

    fn get_active_trace_map(&mut self, pid: Pid) -> Option<&mut TraceMap> {
        let parent = self.get_parent(pid)?;
        let process = self.processes.get_mut(&parent)?;
        if process.traces.is_some() {
            process.traces.as_mut()
        } else {
            Some(self.traces)
        }
    }

    fn init_process(
        &mut self,
        pid: Pid,
        trace_map: Option<TraceMap>,
    ) -> Result<TracedProcess, RunError> {
        let traces = match trace_map.as_ref() {
            Some(s) => s,
            None => self.traces,
        };
        let mut breakpoints = HashMap::new();
        trace_children(pid)?;
        let offset = get_offset(pid, self.config);
        trace!(
            "Initialising process: {}, address offset: 0x{:x}",
            pid,
            offset
        );
        let mut clashes = HashSet::new();
        for trace in traces.all_traces() {
            for addr in &trace.address {
                if clashes.contains(&align_address(*addr)) {
                    trace!(
                        "Skipping {} as it clashes with previously disabled breakpoints",
                        addr
                    );
                    continue;
                }
                match Breakpoint::new(pid, *addr + offset) {
                    Ok(bp) => {
                        let _ = breakpoints.insert(*addr + offset, bp);
                    }
                    Err(e) if e == NixErr::Sys(Errno::EIO) => {
                        return Err(RunError::TestRuntime(
                            "Tarpaulin cannot find code addresses check your linker settings."
                                .to_string(),
                        ));
                    }
                    Err(NixErr::UnsupportedOperation) => {
                        debug!("Instrumentation address clash, ignoring 0x{:x}", addr);
                        // Now to avoid weird false positives lets get rid of the other breakpoint
                        // at this address.
                        let aligned = align_address(*addr);
                        clashes.insert(aligned);
                        let removed_keys = breakpoints
                            .keys()
                            .filter(|x| align_address(*x - offset) == aligned)
                            .copied()
                            .collect::<Vec<_>>();
                        for key in &removed_keys {
                            let breakpoint = breakpoints.remove(key).unwrap();
                            trace!("Disabling clashing breakpoint");
                            if let Err(e) = breakpoint.disable(pid) {
                                error!("Unable to disable breakpoint: {}", e);
                            }
                        }
                    }
                    Err(_) => {
                        return Err(RunError::TestRuntime(
                            "Failed to instrument test executable".to_string(),
                        ));
                    }
                }
            }
        }
        // a processes pid is it's own parent
        self.pid_map.insert(pid, pid);
        Ok(TracedProcess {
            parent: pid,
            breakpoints,
            thread_count: 0,
            offset,
            is_test_proc: false,
            traces: trace_map,
        })
    }

    fn handle_exec(
        &mut self,
        pid: Pid,
    ) -> Result<(TestState, TracerAction<ProcessInfo>), RunError> {
        trace!("Handling process exec");
        let res = Ok((TestState::wait_state(), TracerAction::Continue(pid.into())));

        if let Ok(proc) = Process::new(pid.into()) {
            let exe = match proc.exe() {
                Ok(e) if !e.starts_with(&self.config.target_dir()) => {
                    return Ok((TestState::wait_state(), TracerAction::Detach(pid.into())));
                }
                Ok(e) => e,
                _ => return Ok((TestState::wait_state(), TracerAction::Detach(pid.into()))),
            };
            match generate_tracemap(&exe, self.analysis, self.config) {
                Ok(tm) if !tm.is_empty() => match self.init_process(pid, Some(tm)) {
                    Ok(tp) => {
                        self.processes.insert(pid, tp);
                        Ok((TestState::wait_state(), TracerAction::Continue(pid.into())))
                    }
                    Err(e) => {
                        error!("Failed to init process (attempting continue): {}", e);
                        res
                    }
                },
                _ => {
                    trace!("Failed to create trace map for executable, continuing");
                    res
                }
            }
        } else {
            trace!("Failed to get process info from PID");
            res
        }
    }

    fn handle_ptrace_event(
        &mut self,
        child: Pid,
        sig: Signal,
        event: i32,
    ) -> Result<(TestState, TracerAction<ProcessInfo>), RunError> {
        use nix::libc::*;

        if sig == Signal::SIGTRAP {
            match event {
                PTRACE_EVENT_CLONE => match get_event_data(child) {
                    Ok(t) => {
                        trace!("New thread spawned {}", t);
                        let mut parent = None;
                        if let Some(proc) = self.get_traced_process_mut(child) {
                            proc.thread_count += 1;
                            parent = Some(proc.parent);
                        } else {
                            warn!("Couldn't find parent for {}", child);
                        }
                        if let Some(p) = parent {
                            self.pid_map.insert(Pid::from_raw(t as _), p);
                        }
                        Ok((
                            TestState::wait_state(),
                            TracerAction::Continue(child.into()),
                        ))
                    }
                    Err(e) => {
                        trace!("Error in clone event {:?}", e);
                        Err(RunError::TestRuntime(
                            "Error occurred upon test executable thread creation".to_string(),
                        ))
                    }
                },
                PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK => {
                    trace!("Caught fork event. Child {:?}", get_event_data(child));
                    Ok((
                        TestState::wait_state(),
                        TracerAction::Continue(child.into()),
                    ))
                }
                PTRACE_EVENT_EXEC => {
                    if self.config.follow_exec {
                        self.handle_exec(child)
                    } else {
                        Ok((TestState::wait_state(), TracerAction::Detach(child.into())))
                    }
                }
                PTRACE_EVENT_EXIT => {
                    trace!("Child exiting");
                    let mut is_parent = false;
                    if let Some(proc) = self.get_traced_process_mut(child) {
                        proc.thread_count -= 1;
                        is_parent |= proc.parent == child;
                    }
                    if !is_parent {
                        self.pid_map.remove(&child);
                    }
                    Ok((
                        TestState::wait_state(),
                        TracerAction::TryContinue(child.into()),
                    ))
                }
                _ => Err(RunError::TestRuntime(format!(
                    "Unrecognised ptrace event {}",
                    event
                ))),
            }
        } else {
            trace!("Unexpected signal with ptrace event {}", event);
            trace!("Signal: {:?}", sig);
            Err(RunError::TestRuntime("Unexpected signal".to_string()))
        }
    }

    fn collect_coverage_data(
        &mut self,
        visited_pcs: &mut HashMap<Pid, HashSet<u64>>,
    ) -> Result<UpdateContext, RunError> {
        let mut action = None;
        let current = self.current;
        let enable = self.config.count;
        let mut hits_to_increment = HashSet::new();
        if let Some(process) = self.get_traced_process_mut(current) {
            let visited = visited_pcs.entry(process.parent).or_default();
            if let Ok(rip) = current_instruction_pointer(current) {
                let rip = (rip - 1) as u64;
                trace!("Hit address 0x{:x}", rip);
                if process.breakpoints.contains_key(&rip) {
                    let bp = &mut process.breakpoints.get_mut(&rip).unwrap();
                    let updated = if visited.contains(&rip) {
                        let _ = bp.jump_to(current);
                        (true, TracerAction::Continue(current.into()))
                    } else {
                        // Don't reenable if multithreaded as can't yet sort out segfault issue
                        if let Ok(x) = bp.process(current, enable) {
                            x
                        } else {
                            // So failed to process a breakpoint.. Still continue to avoid
                            // stalling
                            (false, TracerAction::Continue(current.into()))
                        }
                    };
                    if updated.0 {
                        hits_to_increment.insert(rip - process.offset);
                    }
                    action = Some(updated.1);
                }
            }
        } else {
            warn!("Failed to find process for pid: {}", current);
        }
        if let Some(traces) = self.get_active_trace_map(current) {
            for addr in &hits_to_increment {
                traces.increment_hit(*addr);
            }
        } else {
            warn!("Failed to find traces for pid: {}", current);
        }
        let action = action.unwrap_or_else(|| TracerAction::Continue(current.into()));
        Ok((TestState::wait_state(), action))
    }

    fn handle_signaled(
        &mut self,
        pid: &Pid,
        sig: &Signal,
        flag: bool,
    ) -> Result<UpdateContext, RunError> {
        let parent = self.get_parent(*pid);
        if let Some(p) = parent {
            if let Some(proc) = self.processes.get(&p) {
                if !proc.is_test_proc {
                    let info = ProcessInfo::new(*pid, Some(*sig));
                    return Ok((TestState::wait_state(), TracerAction::TryContinue(info)));
                }
            }
        }
        match (sig, flag) {
            (Signal::SIGKILL, _) => Ok((TestState::wait_state(), TracerAction::Detach(pid.into()))),
            (Signal::SIGTRAP, true) => {
                Ok((TestState::wait_state(), TracerAction::Continue(pid.into())))
            }
            (Signal::SIGCHLD, _) => {
                Ok((TestState::wait_state(), TracerAction::Continue(pid.into())))
            }
            _ => Err(RunError::StateMachine("Unexpected stop".to_string())),
        }
    }
}
