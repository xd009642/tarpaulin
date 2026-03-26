use crate::process_handling::event_source::*;
use crate::statemachine::TestState;
use crate::Config;
use crate::RunError;
use libc::c_long;
use nix::sys::signal::Signal;
use nix::sys::wait::{WaitPidFlag, WaitStatus};
use nix::unistd::Pid;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};

/// One logical step in a thread's execution.
/// Each Step produces a WaitStatus once the thread is continued/stepped to it.
#[derive(Debug, Clone)]
pub enum ThreadStep {
    /// Nothing happens in this step (can also use this before the TID has started)
    None,
    /// Hit a breakpoint at this address, fires SIGTRAP
    Breakpoint(u64),
    /// Fires a SIGTRAP from a single-step completing (no address change needed)
    Step,
    /// Thread receives an OS signal (forwarded signal scenario)
    Signal(Signal),
    /// Thread exits cleanly
    Exit(i32),
    /// Thread clones a new thread with given child pid
    Clone(Pid),
    /// Thread execs a new process (follow-exec scenario)
    Exec(Pid),
    /// Thread has forked a child process
    Fork(Pid),
}

/// Describes one thread's complete execution sequence.
#[derive(Debug, Clone)]
pub struct ThreadScenario {
    pub pid: Pid,
    pub steps: VecDeque<ThreadStep>,
}

impl ThreadScenario {
    pub fn new(pid: Pid, steps: impl IntoIterator<Item = ThreadStep>) -> Self {
        Self {
            pid,
            steps: steps.into_iter().collect(),
        }
    }
}

/// Fluent builder for constructing mock scenarios.
///
/// Example:
/// ```
/// let source = MockEventSource::build()
///     .process(100)
///         .breakpoint(0xdead)
///         .breakpoint(0xbeef)
///         .exit(0)
///     .process(101)           // child spawned via clone from 100
///         .parent(100)
///         .breakpoint(0xdead) // same bp, races with parent
///         .exit(0)
///     .finish();
/// ```
pub struct MockBuilder {
    processes: Vec<(ThreadScenario, Option<Pid> /* parent */)>,
    current_pid: Option<Pid>,
    // address -> original byte, shared across all pids (same binary image)
    image: HashMap<u64, c_long>,
}

impl MockBuilder {
    pub fn new() -> Self {
        Self {
            processes: Vec::new(),
            current_pid: None,
            image: HashMap::new(),
        }
    }

    /// Start describing a new process/thread with this pid.
    pub fn process(mut self, raw_pid: i32) -> Self {
        self.processes
            .push((ThreadScenario::new(Pid::from_raw(raw_pid), []), None));
        self.current_pid = Some(Pid::from_raw(raw_pid));
        self
    }

    /// Set this thread's parent pid (for get_ppid / follow-exec).
    pub fn parent(mut self, raw_pid: i32) -> Self {
        if let Some(last) = self.processes.last_mut() {
            last.1 = Some(Pid::from_raw(raw_pid));
        }
        self
    }

    /// Thread hits a breakpoint at addr.
    pub fn breakpoint(mut self, addr: u64) -> Self {
        self.push_step(ThreadStep::Breakpoint(addr));
        // pre-populate the image so read_address returns something non-zero
        self.image.entry(addr).or_insert(0xDEAD_BEEF); // ends with INT3
        self
    }

    /// Thread receives a non-SIGTRAP signal.
    pub fn signal(mut self, sig: Signal) -> Self {
        self.push_step(ThreadStep::Signal(sig));
        self
    }

    /// Thread exits with this code.
    pub fn exit(mut self, code: i32) -> Self {
        self.push_step(ThreadStep::Exit(code));
        self
    }

    /// Thread clones a new child.
    pub fn clone_child(mut self, child_raw_pid: i32) -> Self {
        self.push_step(ThreadStep::Clone(Pid::from_raw(child_raw_pid)));
        self
    }

    pub fn fork_child(mut self, child_raw_pid: i32) -> Self {
        self.push_step(ThreadStep::Fork(Pid::from_raw(child_raw_pid)));
        self
    }

    /// Add a known address to the binary image (for read/write_address testing).
    pub fn with_image_byte(mut self, addr: u64, value: c_long) -> Self {
        self.image.insert(addr, value);
        self
    }

    pub fn noop(mut self) -> Self {
        self.push_step(ThreadStep::None);
        self
    }

    pub fn finish(self) -> MockEventSource {
        MockEventSource::from_builder(self)
    }

    fn push_step(&mut self, step: ThreadStep) {
        if let Some(last) = self.processes.last_mut() {
            last.0.steps.push_back(step);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal per-process state
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum PidState {
    /// Waiting for continue_pid or single_step to advance it
    Stopped,
    /// Running; will produce its next step when next_events drains
    Running,
    /// No more steps, process has exited
    Dead,
}

#[derive(Debug)]
struct ProcessState {
    scenario: ThreadScenario,
    state: PidState,
    parent: Option<Pid>,
    instruction_pointer: u64,
    /// Steps that have fired but not yet drained by next_events
    pending_wait: Option<WaitStatus>,
}

/// A fully in-process mock implementation of `EventSource`.
///
/// All mutable state is behind `RefCell` so it can satisfy the shared-borrow
/// signature of the real trait (`&self` receivers throughout).
#[derive(Debug)]
pub struct MockEventSource {
    inner: RefCell<MockInner>,
}

#[derive(Debug)]
struct MockInner {
    processes: HashMap<Pid, ProcessState>,
    /// Ordered so tests can assert on what the tracer did
    action_log: Vec<TraceAction>,
    image: HashMap<u64, c_long>,
    /// breakpoint addr -> original byte before INT3 was written
    breakpoints: HashMap<u64, c_long>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TraceAction {
    ContinuePid(Pid, Option<Signal>),
    SingleStep(Pid),
    WriteAddress(Pid, u64, c_long),
    ReadAddress(Pid, u64),
    SetInstructionPointer(Pid, u64),
    DetachChild(Pid),
}

impl MockEventSource {
    pub fn build() -> MockBuilder {
        MockBuilder::new()
    }

    fn from_builder(builder: MockBuilder) -> Self {
        let mut processes = HashMap::new();
        for (scenario, parent) in builder.processes {
            let pid = scenario.pid;
            processes.insert(
                pid,
                ProcessState {
                    scenario,
                    state: PidState::Stopped, // starts stopped (just exec'd / TRACE_ME)
                    parent,
                    instruction_pointer: 0,
                    pending_wait: None,
                },
            );
        }
        MockEventSource {
            inner: RefCell::new(MockInner {
                processes,
                action_log: Vec::new(),
                image: builder.image,
                breakpoints: HashMap::new(),
            }),
        }
    }

    /// Advance a running process: pop its next step and park a WaitStatus.
    fn advance_process(proc: &mut ProcessState) {
        if proc.state != PidState::Running {
            //    return;
        }
        let Some(step) = proc.scenario.steps.pop_front() else {
            proc.state = PidState::Dead;
            proc.pending_wait = Some(WaitStatus::Exited(proc.scenario.pid, 0));
            return;
        };
        let pid = proc.scenario.pid;
        match step {
            ThreadStep::None => {}
            ThreadStep::Breakpoint(addr) => {
                proc.instruction_pointer = addr + 1; // after INT3
                proc.state = PidState::Stopped;
                proc.pending_wait = Some(WaitStatus::Stopped(pid, Signal::SIGTRAP));
            }
            ThreadStep::Step => {
                proc.state = PidState::Stopped;
                proc.pending_wait = Some(WaitStatus::Stopped(pid, Signal::SIGTRAP));
            }
            ThreadStep::Signal(sig) => {
                proc.state = PidState::Stopped;
                proc.pending_wait = Some(WaitStatus::Stopped(pid, sig));
            }
            ThreadStep::Exit(code) => {
                proc.state = PidState::Dead;
                proc.pending_wait = Some(WaitStatus::Exited(pid, code));
            }
            ThreadStep::Clone(_child_pid) => {
                // The clone event itself is a ptrace stop on the parent
                proc.state = PidState::Stopped;
                proc.pending_wait = Some(WaitStatus::PtraceEvent(
                    pid,
                    Signal::SIGTRAP,
                    nix::libc::PTRACE_EVENT_CLONE as i32,
                ));
            }
            ThreadStep::Exec(_new_pid) => {
                proc.state = PidState::Stopped;
                proc.pending_wait = Some(WaitStatus::PtraceEvent(
                    pid,
                    Signal::SIGTRAP,
                    nix::libc::PTRACE_EVENT_EXEC as i32,
                ));
            }
            ThreadStep::Fork(_new_pid) => {
                proc.state = PidState::Stopped;
                proc.pending_wait = Some(WaitStatus::PtraceEvent(
                    pid,
                    Signal::SIGTRAP,
                    nix::libc::PTRACE_EVENT_FORK,
                ));
            }
        }
    }

    /// Test helper: get a snapshot of all tracer actions so far.
    pub fn action_log(&self) -> Vec<TraceAction> {
        self.inner.borrow().action_log.clone()
    }

    /// Test helper: how many times was this address written with INT3?
    pub fn breakpoint_set_count(&self, addr: u64) -> usize {
        self.inner
            .borrow()
            .action_log
            .iter()
            .filter(|a| {
                matches!(a, TraceAction::WriteAddress(_, a2, v)
                if *a2 == addr && (*v & 0xff) == 0xcc)
            })
            .count()
    }

    /// Test helper: is there any pid still stopped with a dangling INT3 written?
    /// A dangling breakpoint = INT3 is in the image but no pending step to restore it.
    pub fn has_dangling_breakpoints(&self) -> bool {
        let inner = self.inner.borrow();
        inner.breakpoints.keys().any(|addr| {
            // If the breakpoint is still in the active set and all pids are dead/running,
            // something failed to restore it before continuing.
            inner
                .processes
                .values()
                .all(|p| p.state != PidState::Stopped)
                && inner.image.get(addr).copied().unwrap_or(0) & 0xff == 0xcc
        })
    }
}

// ---------------------------------------------------------------------------
// EventSource implementation
// ---------------------------------------------------------------------------

impl EventSource for MockEventSource {
    fn request_trace(&self) -> nix::Result<()> {
        Ok(())
    }

    fn get_offset(&self, _pid: Pid, _config: &Config) -> u64 {
        0 // no ASLR in tests
    }

    fn trace_children(&self, _pid: Pid) -> nix::Result<()> {
        Ok(())
    }

    fn waitpid(&self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        println!("Waiting (pid={}): {:?}", pid, options);
        println!("{:?}", self);
        assert_eq!(pid, Pid::from_raw(0));
        assert!(matches!(options, None | Some(WaitPidFlag::WNOHANG)));

        let mut inner = self.inner.borrow_mut();
        for proc in inner.processes.values_mut() {
            if let Some(status) = proc.pending_wait.take() {
                return Ok(status);
            }
        }
        Ok(WaitStatus::StillAlive)
    }

    fn next_events(&self, wait_queue: &mut Vec<WaitStatus>) -> Result<Option<TestState>, RunError> {
        println!("{:?}", self);

        let mut result = None;

        let mut inner = self.inner.borrow_mut();
        for proc in inner.processes.values_mut() {
            if let Some(status) = proc.pending_wait.take() {
                if !matches!(status, WaitStatus::StillAlive) {
                    result = Some(TestState::Stopped);
                }
                wait_queue.push(status);
            }
        }
        println!("Wait queue {:?}", wait_queue);
        Ok(result)
    }

    fn get_event_data(&self, pid: Pid) -> nix::Result<c_long> {
        println!("Get event data(pid={})", pid);
        Ok(0)
    }

    fn continue_pid(&self, pid: Pid, signal: Option<Signal>) -> nix::Result<()> {
        println!("Continue(pid={}) signal: {:?}", pid, signal);
        let mut inner = self.inner.borrow_mut();
        inner.action_log.push(TraceAction::ContinuePid(pid, signal));

        // Find the root process pid — either this pid is itself a root,
        // or it's a thread whose parent is the root.
        let root = inner
            .processes
            .get(&pid)
            .and_then(|p| p.parent)
            .unwrap_or(pid);

        // Advance all threads belonging to this process group
        let pids: Vec<Pid> = inner
            .processes
            .iter()
            .filter(|(_, p)| p.scenario.pid == root || p.parent == Some(root))
            .map(|(pid, _)| *pid)
            .collect();

        for pid in pids {
            let proc = inner.processes.get_mut(&pid).unwrap();
            if proc.state == PidState::Stopped {
                proc.state = PidState::Running;
                MockEventSource::advance_process(proc);
            }
        }

        Ok(())
    }

    fn single_step(&self, pid: Pid) -> nix::Result<()> {
        println!("Stepping(pid={})", pid);
        let mut inner = self.inner.borrow_mut();
        inner.action_log.push(TraceAction::SingleStep(pid));
        if let Some(proc) = inner.processes.get_mut(&pid) {
            if proc.state == PidState::Stopped {
                proc.state = PidState::Running;
                // A single_step immediately queues a Step event on the next advance
                proc.scenario.steps.push_front(ThreadStep::Step);
                MockEventSource::advance_process(proc);
            }
        }
        Ok(())
    }

    fn detach_child(&self, pid: Pid) -> nix::Result<()> {
        println!("Detach(pid={})", pid);
        let mut inner = self.inner.borrow_mut();
        inner.action_log.push(TraceAction::DetachChild(pid));
        if let Some(proc) = inner.processes.get_mut(&pid) {
            proc.state = PidState::Dead;
        }
        Ok(())
    }

    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long> {
        println!("Read RIP(pid={})", pid);
        let inner = self.inner.borrow();
        Ok(inner
            .processes
            .get(&pid)
            .map(|p| p.instruction_pointer as c_long)
            .unwrap_or(0))
    }

    fn set_current_instruction_pointer(&self, pid: Pid, addr: u64) -> nix::Result<c_long> {
        println!("Write RIP(pid={}) address: {}", pid, addr);
        let mut inner = self.inner.borrow_mut();
        inner
            .action_log
            .push(TraceAction::SetInstructionPointer(pid, addr));
        let old = inner
            .processes
            .get(&pid)
            .map(|p| p.instruction_pointer as c_long)
            .unwrap_or(0);
        if let Some(proc) = inner.processes.get_mut(&pid) {
            proc.instruction_pointer = addr;
        }
        Ok(old)
    }

    fn write_address(&self, pid: Pid, addr: u64, data: c_long) -> nix::Result<()> {
        println!("write address(pid={}), *{}={}", pid, addr, data);
        let mut inner = self.inner.borrow_mut();
        inner
            .action_log
            .push(TraceAction::WriteAddress(pid, addr, data));
        // Track whether this is setting or clearing a breakpoint
        if data & 0xff == 0xcc {
            // Placing INT3: save original if we haven't already
            let original = inner.image.get(&addr).copied().unwrap_or(data);
            inner.breakpoints.insert(addr, original);
        } else {
            // Restoring original byte
            inner.breakpoints.remove(&addr);
        }
        inner.image.insert(addr, data);
        Ok(())
    }

    fn read_address(&self, pid: Pid, addr: u64) -> nix::Result<c_long> {
        println!("Read address(pid={}), {}", pid, addr);
        let mut inner = self.inner.borrow_mut();
        inner.action_log.push(TraceAction::ReadAddress(pid, addr));
        inner
            .image
            .get(&addr)
            .copied()
            .map(Ok)
            .unwrap_or(Err(nix::errno::Errno::EIO))
    }

    fn get_tids(&self, pid: Pid) -> Box<dyn Iterator<Item = Pid> + '_> {
        println!("Get TIDs(pid={})", pid);
        // Return all live pids that are children of this pid, plus pid itself
        let inner = self.inner.borrow();
        let tasks: Vec<Pid> = inner
            .processes
            .values()
            .filter(|p| p.scenario.pid == pid || p.parent == Some(pid))
            .filter(|p| p.state != PidState::Dead)
            .map(|p| p.scenario.pid)
            .collect();
        // Can't return a borrow, collect eagerly
        Box::new(tasks.into_iter())
    }

    fn get_ppid(&self, pid: Pid) -> Option<Pid> {
        println!("Get parent process id(pid={})", pid);
        self.inner.borrow().processes.get(&pid)?.parent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::statemachine::{
        linux::{create_state_machine, LinuxData},
        *,
    };
    use crate::traces::{CoverageStat, Trace, TraceMap};
    use crate::TestHandle;
    use std::path::Path;
    use std::rc::Rc;

    fn run_to_completion(test_state: &mut TestState, data: &mut LinuxData<'_>, config: &Config) {
        for i in 0..100 {
            println!("Step {}", i);
            *test_state = test_state.step(data, config).unwrap();
            if test_state.is_finished() {
                return;
            }
        }
        panic!("Test didn't end in 100 steps. Unexpectedly long simulation");
    }

    #[test]
    fn simple_mock_continuation() {
        // let (mut state, mut data) =
        //     create_state_machine(test, &mut traces, analysis, config, logger);

        let source = MockEventSource::build()
            .process(100)
            .signal(Signal::SIGTRAP)
            .breakpoint(1000)
            .exit(0)
            .finish();

        source.continue_pid(Pid::from_raw(100), None).unwrap();

        let mut tracemap = TraceMap::new();
        tracemap.add_file(&Path::new("src/lib.rs"));
        let addresses = [1000u64].iter().copied().collect();
        tracemap.add_trace(&Path::new("src/main.rs"), Trace::new(5, addresses, 1));

        let test_handle = TestHandle::Id(Pid::from_raw(100));
        let analysis = HashMap::new();
        let config = Config::default();
        let (mut test_state, mut linux_data) =
            create_state_machine(test_handle, &mut tracemap, &analysis, &config, &None);

        linux_data.event_source = Rc::new(source);

        run_to_completion(&mut test_state, &mut linux_data, &config);

        let stat = tracemap
            .all_traces()
            .find(|x| x.line == 5 && x.address.contains(&1000))
            .unwrap();

        assert_eq!(stat.stats, CoverageStat::Line(1));
    }
}
