use crate::cargo::rust_flags;
use crate::config::Config;
use crate::errors::RunError;
use crate::event_log::*;
use crate::statemachine::*;
use nix::errno::Errno;
use nix::sys::signal::Signal;
use nix::sys::wait::*;
use nix::unistd::Pid;
use nix::Error as NixErr;
use procfs::process::Process;
use std::collections::{HashMap, HashSet};
use tracing::{debug, info, trace, warn};

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
    /// Processes we're tracing
    processes: HashMap<Pid, TracedProcess>,
    /// Map from pids to their parent
    pid_map: HashMap<Pid, Pid>,
}

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
}

pub fn create_state_machine<'a>(
    test: Pid,
    traces: &'a mut TraceMap,
    config: &'a Config,
    event_log: &'a Option<EventLog>,
) -> (TestState, LinuxData<'a>) {
    let mut data = LinuxData::new(traces, config, event_log);
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
        if let Ok(maps) = proc.maps() {
            if let Some(first) = maps.first() {
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
        let traced_process = self.init_process(self.current, None)?;

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
            if let Some(log) = self.event_log.as_ref() {
                let event = TraceEvent::new_from_wait(&status);
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
                            } else {
                                warn!("Failed to merge traces from executable");
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
        for a in &actions {
            if let Some(log) = self.event_log.as_ref() {
                let event = TraceEvent::new_from_action(&a);
                log.push_trace(event);
            }
            match a {
                TracerAction::TryContinue(t) => {
                    continued = true;
                    let _ = continue_exec(t.pid, t.signal);
                }
                TracerAction::Continue(t) => {
                    continued = true;
                    continue_exec(t.pid, t.signal)?;
                }
                TracerAction::Step(t) => {
                    continued = true;
                    single_step(t.pid)?;
                }
                TracerAction::Detach(t) => {
                    continued = true;
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
        config: &'a Config,
        event_log: &'a Option<EventLog>,
    ) -> LinuxData<'a> {
        LinuxData {
            wait_queue: Vec::new(),
            processes: HashMap::new(),
            current: Pid::from_raw(0),
            parent: Pid::from_raw(0),
            traces,
            config,
            event_log,
            pid_map: HashMap::new(),
        }
    }

    fn get_traced_process_mut(&mut self, pid: Pid) -> Option<&mut TracedProcess> {
        let parent = self.pid_map.get(&pid)?;
        self.processes.get_mut(&parent)
    }

    fn get_active_trace_map(&mut self, pid: Pid) -> Option<&mut TraceMap> {
        let parent = self.pid_map.get(&pid)?;
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
        traces: Option<&mut TraceMap>,
    ) -> Result<TracedProcess, RunError> {
        let traces = match traces {
            Some(s) => s,
            None => self.traces,
        };
        let mut breakpoints = HashMap::new();
        trace_children(pid)?;
        let offset = get_offset(pid, self.config);
        trace!("Address offset: 0x{:x}", offset);
        for trace in traces.all_traces() {
            for addr in &trace.address {
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
            traces: None,
        })
    }

    fn handle_exec(
        &mut self,
        pid: Pid,
    ) -> Result<(TestState, TracerAction<ProcessInfo>), RunError> {
        trace!("Child execed other process");
        if let Ok(proc) = Process::new(pid.into()) {
            info!("{:?}", proc.exe());
        }
        Ok((TestState::wait_state(), TracerAction::Detach(pid.into())))
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
                    trace!("Caught fork event");
                    Ok((
                        TestState::wait_state(),
                        TracerAction::Continue(child.into()),
                    ))
                }
                PTRACE_EVENT_EXEC => self.handle_exec(child),
                PTRACE_EVENT_EXIT => {
                    trace!("Child exiting");
                    if let Some(proc) = self.get_traced_process_mut(child) {
                        proc.thread_count -= 1;
                    }
                    self.pid_map.remove(&child);
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
        match (sig, flag) {
            (Signal::SIGTRAP, true) => {
                Ok((TestState::wait_state(), TracerAction::Continue(pid.into())))
            }
            _ => Err(RunError::StateMachine("Unexpected stop".to_string())),
        }
    }
}
