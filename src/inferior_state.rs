use std::collections::HashMap;
use nix::sys::wait::*;
use nix::sys::signal;
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
        start_time: u64, 
    },
    /// Initialise: once test process appears instrument 
    Initialise ,
    /// Waiting for breakpoint to be hit or test to end
    Waiting { 
        start_time: u64, 
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
            org @ TestState::Start{..} => {
                if let Some(s) = data.start() {
                    s
                } else {
                    org
                }
            },
            TestState::Initialise => {
                data.init()
            },  
            org @ TestState::Waiting{..} => {
                if let Some(s) =data.wait() {
                    s
                } else {
                    org
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
    (TestState::Start{start_time:0}, data)
}


/// Handle to linux process state
#[derive(Debug)]
pub struct LinuxData<'a> {
    wait: WaitStatus,
    current: Pid,
    parent: Pid,
    breakpoints: HashMap<u64, Breakpoint>,
    traces: &'a mut Vec<TracerData>,
    config: &'a Config
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
                        println!("Failed to instrument");
                    },
                }
            }
        }
        if let Ok(_) = continue_exec(self.parent, None) {
            TestState::Waiting{start_time:0}
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
                println!("An error occurred");
                Some(TestState::Unrecoverable)
            },
        }
    }


    fn stop(&mut self) -> TestState {
        match self.wait {
            WaitStatus::PtraceEvent(_,_,_) => {
                self.handle_ptrace_event()
            },
            WaitStatus::Stopped(c,signal::SIGTRAP) => {
                self.current = c;
                self.collect_coverage_data()
            },
            WaitStatus::Stopped(child, signal::SIGSTOP) => {
                let _ =continue_exec(child, None);
                TestState::Waiting{start_time:0}
            },
            WaitStatus::Stopped(_, signal::SIGSEGV) => TestState::Unrecoverable,
            WaitStatus::Stopped(c, s) => {
                let _ = continue_exec(c, Some(s));
                TestState::Waiting{start_time:0}
            },
            WaitStatus::Signaled(_,_,_) => {
                self.handle_signaled()
            },
            WaitStatus::Exited(child, sig) => {
                if child == self.parent {
                    TestState::End
                } else {
                    let _ = continue_exec(self.parent, None);
                    TestState::Waiting{start_time:0}
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
        }
    }

    fn handle_ptrace_event(&mut self) -> TestState {
        TestState::Unrecoverable
    }

    fn collect_coverage_data(&mut self) -> TestState {
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
                continue_exec(self.current, None);
            }
        } else {
            continue_exec(self.current, None);
        }
        TestState::Waiting{start_time:0}
    }

    fn handle_signaled(&mut self) -> TestState {
        match self.wait {
            WaitStatus::Signaled(child, signal::SIGTRAP, true) => {
                continue_exec(child, None); 
                TestState::Waiting{start_time:0}
            },
            _ => {
                println!("Unexpected stop");
                TestState::Unrecoverable
            },
        }
    }
}
