use std::collections::HashMap;
use std::time::Instant;
use nix::sys::ptrace::ptrace::*;
use nix::sys::wait::*;
use nix::sys::signal;
use nix::Result;
use nix::unistd::Pid;
use tracer::*;
use breakpoint::*;
use ptrace_control::*;
use config::Config;
/// 
/// So we are either:
///     Waiting for an signal
///     Handling a stop
///     Handling an exit
///     Handling being signalled
///     Handling ptrace events
///     Handling an error
///     Continuing execution
///     Handling ptrace error
/// 
/// So we wait -> other thing -> continuing
///
/// Cannot do wait -> wait (unless non-polling, timeout?)
///
/// Or wait -> other thing -> other thing -> continue
///
/// Or wait -> other thing -> continue -> continue

/// Possible states when executing an inferior process. This is an attempt at
/// a platform agnostic abstracting to provide the potential of future
/// implementations for other operating systems and provides the implementation
/// of the test running state machine
/// T is data used to store the necessary process information to enable tracing
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TestState {
    /// Start state. Wait for test to appear and track time to enable timeout
    Start { 
        start_time: Instant, 
    },
    /// Initialise: once test process appears instrument 
    Initialise ,
    /// Waiting for breakpoint to be hit or test to end
    Waiting { 
        start_time: Instant, 
    },
    /// Test process stopped, check coverage
    Stopped,
    /// Process timed out
    Timeout,
    /// Unrecoverable error occurred
    Unrecoverable,
    /// Test exited normally
    End,
}

impl TestState {
    /// Convenience function used to check if the test has finished or errored
    pub fn is_finished(self) -> bool {
        match self {
            TestState::End | TestState::Unrecoverable => true,
            _ => false,
        }
    }

    /// Convenience function for creating start states
    fn start_state() -> TestState {
        TestState::Start{start_time: Instant::now()}
    }

    /// Convenience function for creating wait states
    fn wait_state() -> TestState {
        TestState::Waiting{start_time: Instant::now()}
    }
}

/// Trait for state machines to implement
pub trait StateMachine<T> where T:StateData {
    /// Update the states
    fn step(self, data: &mut T, config: &Config) -> TestState;
}

/// Tracing a process on an OS will have platform specific code. 
/// Structs containing the platform specific datastructures should
/// provide this trait with an implementation of the handling of 
/// the given states.
pub trait StateData {
    /// Starts the tracing. Returns None while waiting for
    /// start. Statemachine then checks timeout
    fn start(&mut self) -> Option<TestState>;
    /// Initialises test for tracing returns next state
    fn init(&mut self) -> TestState;
    /// Waits for notification from test executable that there's 
    /// something to do. Selects the next appropriate state if there's
    /// something to do otherwise None
    fn wait(&mut self) -> Option<TestState>;
    /// Handle a stop in the test executable. Coverage data will
    /// be collected here as well as other OS specific functions
    fn stop(&mut self) -> TestState;
    /// Cleanup the system state - killing processes etc
    fn cleanup(&mut self);
}


impl <T> StateMachine<T> for TestState where T:StateData {

    fn step(self, data: &mut T, config: &Config) -> TestState {
        match self {
            TestState::Start{start_time} => {
                if let Some(s) = data.start() {
                    s
                } else if start_time.elapsed() >= config.test_timeout {
                    println!("Error: Timed out when starting test");
                    TestState::Timeout
                } else {
                    TestState::Start{start_time:start_time}
                }
            },
            TestState::Initialise => {
                data.init()
            },  
            TestState::Waiting{start_time} => {
                if let Some(s) =data.wait() {
                    s
                } else if start_time.elapsed() >= config.test_timeout {
                    println!("Error: Timed out waiting for test response");
                    TestState::Timeout
                } else {
                    TestState::Waiting{start_time}
                }
            },
            TestState::Stopped => {
                data.stop()
            },
            TestState::Timeout => {
                data.cleanup();
                TestState::End
            },
            TestState::Unrecoverable => {
                data.cleanup();
                TestState::End
            },
            _ => {
                TestState::End
            }
        }
    }
}


pub fn create_state_machine<'a>(test: Pid, 
                                traces: &'a mut Vec<TracerData>, 
                                config: &'a Config) -> (TestState, LinuxData<'a>) {
    let mut data = LinuxData::new(traces, config);
    data.parent = test;
    (TestState::start_state(), data)
}


/// Handle to linux process state
#[derive(Debug)]
pub struct LinuxData<'a> {
    /// Recent result from waitpid to be handled by statemachine
    wait: WaitStatus,
    /// Current Pid to process
    current: Pid,
    /// Parent PID of test process
    parent: Pid,
    /// Map of addresses to breakpoints
    breakpoints: HashMap<u64, Breakpoint>,
    /// Instrumentation points in code with associated coverage data
    traces: &'a mut Vec<TracerData>,
    /// Program config
    config: &'a Config,
    /// Used to store error for user in the event something goes wrong
    error_message: Option<String>,
}


impl <'a> StateData for LinuxData<'a> {
    
    fn start(&mut self) -> Option<TestState> {
        match waitpid(self.current, Some(WNOHANG)) {
            Ok(WaitStatus::StillAlive) => None,
            Ok(sig @ WaitStatus::Stopped(_, signal::SIGTRAP)) => {
                if let WaitStatus::Stopped(child, _) = sig {
                    self.current = child;
                }
                self.wait = sig;
                Some(TestState::Initialise)
            },
            Ok(s) => {
                println!("Unexpected signal when starting test");
                None
            },
            Err(e) => {
                println!("Error when starting test: {}", e);
                Some(TestState::Unrecoverable)
            },
        }
    }


    fn init(&mut self) -> TestState {
        if let Err(c) = trace_children(self.current) {
            println!("Failed to trace child threads");
        }
        for trace in self.traces.iter() {
            if let Some(addr) = trace.address {
                match Breakpoint::new(self.current, addr) {
                    Ok(bp) => {
                        let _ = self.breakpoints.insert(addr, bp);
                    },
                    Err(e) => {
                        self.error_message = Some("Failed to instrument test executable".to_string());
                    },
                }
            }
        }
        if let Ok(_) = continue_exec(self.parent, None) {
            TestState::wait_state()
        } else {
            TestState::Unrecoverable
        }
    }


    fn wait(&mut self) -> Option<TestState> {
        let wait = waitpid(Pid::from_raw(-1), Some(WNOHANG | __WALL));
        match wait {
            Ok(WaitStatus::StillAlive) => {
                self.wait = WaitStatus::StillAlive;
                None
            },
            Ok(s) => {
                self.wait = s;
                Some(TestState::Stopped)
            },
            Err(e) => {
                self.error_message = Some("An error occurred while waiting for response from test".to_string());
                Some(TestState::Unrecoverable)
            },
        }
    }


    fn stop(&mut self) -> TestState {
        match self.wait {
            WaitStatus::PtraceEvent(_,_,_) => {
                match self.handle_ptrace_event() {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("Error occurred when handling ptrace event: {}", e);
                        self.error_message = Some(msg);
                        TestState::Unrecoverable
                    },
                }
            },
            WaitStatus::Stopped(c,signal::SIGTRAP) => {
                self.current = c;
                match self.collect_coverage_data() {
                    Ok(s) => s,
                    Err(e) => {
                        self.error_message = Some(format!("Error when collecting coverage: {}", e));
                        TestState::Unrecoverable
                    }
                }
            },
            WaitStatus::Stopped(child, signal::SIGSTOP) => {
                if continue_exec(child, None).is_ok() {
                    TestState::wait_state()
                } else {
                    self.error_message = Some("Error processing SIGSTOP".to_string());
                    TestState::Unrecoverable
                }
            },
            WaitStatus::Stopped(_, signal::SIGSEGV) => TestState::Unrecoverable,
            WaitStatus::Stopped(c, s) => {
                let sig = if self.config.forward_signals {
                    Some(s)
                } else {
                    None
                };
                let _ = continue_exec(c, sig);
                TestState::wait_state()
            },
            WaitStatus::Signaled(_,_,_) => {
                if let Ok(s) = self.handle_signaled() {
                    s
                } else {
                    self.error_message = Some("Error attempting to handle tarpaulin being signaled".to_string());
                    TestState::Unrecoverable
                }
            },
            WaitStatus::Exited(child, sig) => {
                if child == self.parent {
                    TestState::End
                } else {
                    let _ = continue_exec(self.parent, None);
                    TestState::wait_state()
                }
            },
            _ => TestState::Unrecoverable,
        }
    }


    fn cleanup(&mut self)  {

    }
}


impl <'a>LinuxData<'a> {
    pub fn new(traces: &'a mut Vec<TracerData>, config: &'a Config) -> LinuxData<'a> {
        LinuxData {
            wait: WaitStatus::StillAlive,
            current: Pid::from_raw(0),
            parent: Pid::from_raw(0),
            breakpoints: HashMap::new(),
            traces: traces,
            config: config,
            error_message:None
        }
    }

    fn handle_ptrace_event(&mut self) -> Result<TestState> {
        match self.wait {
            WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_CLONE) => {
                if get_event_data(child).is_ok() {
                    continue_exec(child, None)?;
                    Ok(TestState::wait_state())
                } else {
                    self.error_message = Some("Error occurred upon test executable thread creation".to_string());
                    Ok(TestState::Unrecoverable)
                }
            },
            WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_FORK) => {
                continue_exec(child, None)?;
                Ok(TestState::wait_state())
            },
            WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_VFORK) => {
                continue_exec(child, None)?;
                Ok(TestState::wait_state())
            },
            WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_EXEC) => {
                detach_child(child)?;
                Ok(TestState::wait_state())
            },
            WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_EXIT) => {
                continue_exec(child, None)?;
                Ok(TestState::wait_state())
            },
            _ => Ok(TestState::Unrecoverable),
        }
    }

    fn collect_coverage_data(&mut self) -> Result<TestState> {
        let thread_count = 1;
        let mut unwarned = true;
        if let Ok(rip) = current_instruction_pointer(self.current) {
            let rip = (rip - 1) as u64;
            if  self.breakpoints.contains_key(&rip) {
                let bp = &mut self.breakpoints.get_mut(&rip).unwrap();
                let enable = (!self.config.no_count) && (thread_count < 2);
                if !enable && unwarned {
                    println!("Code is mulithreaded, disabling hit count");
                    unwarned = false;
                }
                // Don't reenable if multithreaded as can't yet sort out segfault issue
                let updated = if let Ok(x) = bp.process(self.current, enable) {
                     x
                } else {
                    false
                };
                if updated {
                    for t in self.traces.iter_mut()
                                        .filter(|x| x.address == Some(rip)) {
                        (*t).hits += 1;
                    }
                } 
            } else {
                continue_exec(self.current, None)?;
            }
        } else {
            continue_exec(self.current, None)?;
        }
        Ok(TestState::wait_state())
    }

    fn handle_signaled(&mut self) -> Result<TestState> {
        match self.wait {
            WaitStatus::Signaled(child, signal::SIGTRAP, true) => {
                continue_exec(child, None)?; 
                Ok(TestState::wait_state())
            },
            _ => {
                self.error_message = Some("Unexpected stop".to_string());
                Ok(TestState::Unrecoverable)
            },
        }
    }
}
