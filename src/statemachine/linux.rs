use crate::breakpoint::*;
use crate::cargo::rust_flags;
use crate::config::Config;
use crate::errors::RunError;
use crate::generate_tracemap;
use crate::ptrace_control::*;
use crate::source_analysis::LineAnalysis;
use crate::statemachine::*;
use crate::TestHandle;
use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::sys::wait::*;
use nix::unistd::Pid;
use nix::Error as NixErr;
use procfs::process::{MMapPath, Process};
use std::collections::{HashMap, HashSet};
use std::ops::RangeBounds;
use std::path::PathBuf;
use tracing::{debug, info, trace, trace_span, warn};

/// Handle to linux process state
pub struct LinuxData<'a> {
    /// Recent results from waitpid to be handled by statemachine
    wait_queue: Vec<WaitStatus>,
    /// Pending action queue, this is for actions where we need to wait one cycle before we can
    /// apply them :sobs:
    pending_actions: Vec<TracerAction<ProcessInfo>>,
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
    /// So if we have the exit code but we're also waiting for all the spawned processes to end
    exit_code: Option<i32>,
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
    test: impl Into<TestHandle>,
    traces: &'a mut TraceMap,
    source_analysis: &'a HashMap<PathBuf, LineAnalysis>,
    config: &'a Config,
    event_log: &'a Option<EventLog>,
) -> (TestState, LinuxData<'a>) {
    let mut data = LinuxData::new(traces, source_analysis, config, event_log);
    let handle = test.into();
    match handle {
        TestHandle::Id(test) => {
            data.parent = test;
        }
        _ => unreachable!("Test handle must be a PID for ptrace engine"),
    }
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
                "Error when starting test: {e}"
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

    fn last_wait_attempt(&mut self) -> Result<Option<TestState>, RunError> {
        if let Some(ec) = self.exit_code {
            let parent = self.parent;
            for (_, process) in self.processes.iter().filter(|(k, _)| **k != parent) {
                if let Some(tm) = process.traces.as_ref() {
                    self.traces.merge(tm);
                }
            }
            Ok(Some(TestState::End(ec)))
        } else {
            Ok(None)
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
                Err(_) if self.exit_code.is_some() => {
                    running = false;
                    result = self.last_wait_attempt();
                }
                Err(e) => {
                    running = false;
                    result = Err(RunError::TestRuntime(format!(
                        "An error occurred while waiting for response from test: {e}"
                    )));
                }
            }
        }
        if !self.wait_queue.is_empty() {
            trace!("Result queue is {:?}", self.wait_queue);
        } else {
            self.apply_pending_actions(..);
        }
        result
    }

    fn stop(&mut self) -> Result<TestState, RunError> {
        let mut actions = Vec::new();
        let mut pcs = HashMap::new();
        let mut result = Ok(TestState::wait_state());
        let pending = self.wait_queue.clone();
        let pending_action_len = self.pending_actions.len();
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
                let traces = match self.get_active_trace_map(self.current) {
                    Some(tm) => tm,
                    None => self.traces,
                };
                let event = TraceEvent::new_from_wait(status, offset, traces);
                log.push_trace(event);
            }
            let state = match status {
                WaitStatus::PtraceEvent(c, s, e) => match self.handle_ptrace_event(*c, *s, *e) {
                    Ok(s) => Ok(s),
                    Err(e) => Err(RunError::TestRuntime(format!(
                        "Error occurred when handling ptrace event: {e}"
                    ))),
                },
                WaitStatus::Stopped(c, Signal::SIGTRAP) => {
                    self.current = *c;
                    match self.collect_coverage_data(&mut pcs) {
                        Ok(s) => Ok(s),
                        Err(e) => Err(RunError::TestRuntime(format!(
                            "Error when collecting coverage: {e}"
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
                    let pc = current_instruction_pointer(*child).unwrap_or(1) - 1;
                    trace!("SIGILL raised. Child program counter is: 0x{:x}", pc);
                    Err(RunError::TestRuntime(format!(
                        "Error running test - SIGILL raised in {child}"
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
                        if self.processes.is_empty() || !self.config.follow_exec {
                            Ok((TestState::End(*ec), TracerAction::Nothing))
                        } else {
                            self.exit_code = Some(*ec);
                            info!("Test process exited, but spawned processes still running. Continuing tracing");
                            Ok((TestState::wait_state(), TracerAction::Nothing))
                        }
                    } else {
                        match self.exit_code {
                            Some(ec) if self.processes.is_empty() => return Ok(TestState::End(ec)),
                            _ => {
                                // Process may have already been destroyed. This is just in case
                                Ok((
                                    TestState::wait_state(),
                                    TracerAction::TryContinue(self.parent.into()),
                                ))
                            }
                        }
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
                let event = TraceEvent::new_from_action(a);
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
                    let _ = detach_child(t.pid);
                }
                TracerAction::Nothing => {}
            }
        }
        // Here we assume that no pending actions will exist for things that have currently
        // signaled as stopped in this iteration. Currently, the pending actions are just fork
        // parents that ptrace will stall until child returns so this will hold true. But if that
        // behaviour changes in future the pending action list may need to be pruned more
        // thoroughly
        self.apply_pending_actions(..pending_action_len);

        if !continued && self.exit_code.is_none() {
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
            pending_actions: Vec::new(),
            processes: HashMap::new(),
            current: Pid::from_raw(0),
            parent: Pid::from_raw(0),
            traces,
            analysis,
            config,
            event_log,
            pid_map: HashMap::new(),
            exit_code: None,
        }
    }

    fn get_parent(&self, pid: Pid) -> Option<Pid> {
        self.pid_map.get(&pid).copied().or_else(|| {
            let mut parent_pid = None;
            'outer: for k in self.processes.keys() {
                let proc = Process::new(k.as_raw()).ok()?;
                if let Ok(tasks) = proc.tasks() {
                    for task in tasks.filter_map(Result::ok) {
                        if task.tid == pid.as_raw() {
                            parent_pid = Some(*k);
                            break 'outer;
                        }
                    }
                }
            }
            parent_pid
        })
    }

    fn get_traced_process_mut(&mut self, pid: Pid) -> Option<&mut TracedProcess> {
        let parent = self.get_parent(pid)?;
        self.processes.get_mut(&parent)
    }

    fn get_active_trace_map_mut(&mut self, pid: Pid) -> Option<&mut TraceMap> {
        let parent = self.get_parent(pid)?;
        let process = self.processes.get_mut(&parent)?;
        if process.traces.is_some() {
            process.traces.as_mut()
        } else {
            Some(self.traces)
        }
    }

    fn get_active_trace_map(&mut self, pid: Pid) -> Option<&TraceMap> {
        let parent = self.get_parent(pid)?;
        let process = self.processes.get(&parent)?;
        Some(process.traces.as_ref().unwrap_or(self.traces))
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
                    Err(Errno::EIO) => {
                        return Err(RunError::TestRuntime(
                            "Tarpaulin cannot find code addresses check your linker settings."
                                .to_string(),
                        ));
                    }
                    Err(NixErr::UnknownErrno) => {
                        debug!("Instrumentation address clash, ignoring 0x{:x}", addr);
                        // Now to avoid weird false positives lets get rid of the other breakpoint
                        // at this address.
                        let aligned = align_address(*addr);
                        clashes.insert(aligned);
                        breakpoints.retain(|address, breakpoint| {
                            if align_address(*address - offset) == aligned {
                                trace!("Disabling clashing breakpoint");
                                if let Err(e) = breakpoint.disable(pid) {
                                    error!("Unable to disable breakpoint: {}", e);
                                }
                                false
                            } else {
                                true
                            }
                        });
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
        match self.pid_map.insert(pid, pid) {
            Some(old) if old != pid => {
                debug!("{} being promoted to parent. Old parent {}", pid, old)
            }
            _ => {}
        }
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
                Ok(e) if !e.starts_with(self.config.target_dir()) => {
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
                PTRACE_EVENT_FORK => {
                    if let Ok(fork_child) = get_event_data(child) {
                        trace!("Caught fork event. Child {}", fork_child);
                        let parent = if let Some(process) = self.get_traced_process_mut(child) {
                            // Counting a fork as a new thread ?
                            process.thread_count += 1;
                            Some(process.parent)
                        } else {
                            None
                        };
                        if let Some(parent) = parent {
                            self.pid_map.insert(Pid::from_raw(fork_child as _), parent);
                        }
                    } else {
                        trace!("No event data for child");
                    }
                    Ok((
                        TestState::wait_state(),
                        TracerAction::Continue(child.into()),
                    ))
                }
                PTRACE_EVENT_VFORK => {
                    // So VFORK used to be handled the same place as FORK however, from the man
                    // page for posix_spawn:
                    //
                    // > [The] posix_spawn() function commences by calling clone with CLONE_VM and CLONE_VFORK flags.
                    //
                    // This suggests that Command::new().spawn() will result in a
                    // `PTRACE_EVENT_VFORK` not `PTRACE_EVENT_EXEC`
                    if let Ok(fork_child) = get_event_data(child) {
                        let fork_child = Pid::from_raw(fork_child as _);
                        if self.config.follow_exec {
                            // So I've seen some recursive bin calls with vforks... Maybe just assume
                            // every vfork is an exec :thinking:
                            let (state, action) = self.handle_exec(fork_child)?;
                            if self.config.forward_signals {
                                self.pending_actions
                                    .push(TracerAction::Continue(child.into()));
                            }
                            Ok((state, action))
                        } else {
                            Ok((
                                TestState::wait_state(),
                                TracerAction::Continue(child.into()),
                            ))
                        }
                    } else {
                        Ok((
                            TestState::wait_state(),
                            TracerAction::Continue(child.into()),
                        ))
                    }
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
                    "Unrecognised ptrace event {event}"
                ))),
            }
        } else {
            trace!("Unexpected signal with ptrace event {event}");
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
            if let Ok(pc) = current_instruction_pointer(current) {
                let pc = (pc - 1) as u64;
                trace!("Hit address {:#x}", pc);
                if process.breakpoints.contains_key(&pc) {
                    let bp = process.breakpoints.get_mut(&pc).unwrap();
                    let updated = if visited.contains(&pc) {
                        let _ = bp.jump_to(current);
                        (true, TracerAction::Continue(current.into()))
                    } else {
                        // Don't re-enable if multithreaded as can't yet sort out segfault issue
                        if let Ok(x) = bp.process(current, enable) {
                            x
                        } else {
                            // So failed to process a breakpoint.. Still continue to avoid
                            // stalling
                            (false, TracerAction::Continue(current.into()))
                        }
                    };
                    if updated.0 {
                        hits_to_increment.insert(pc - process.offset);
                    }
                    action = Some(updated.1);
                }
            }
        } else {
            warn!("Failed to find process for pid: {}", current);
        }
        if let Some(traces) = self.get_active_trace_map_mut(current) {
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
            (Signal::SIGTERM, _) => {
                let info = ProcessInfo {
                    pid: *pid,
                    signal: Some(Signal::SIGTERM),
                };
                Ok((TestState::wait_state(), TracerAction::TryContinue(info)))
            }
            _ => Err(RunError::StateMachine("Unexpected stop".to_string())),
        }
    }

    fn apply_pending_actions(&mut self, range: impl RangeBounds<usize>) {
        for a in self.pending_actions.drain(range) {
            if let Some(log) = self.event_log.as_ref() {
                let event = TraceEvent::new_from_action(&a);
                log.push_trace(event);
            }
            match a {
                TracerAction::Continue(t) | TracerAction::TryContinue(t) => {
                    let _ = continue_exec(t.pid, t.signal);
                }
                e => {
                    error!("Pending actions should only be continues: {:?}", e);
                }
            }
        }
    }
}
