use std::collections::HashMap;
use std::time::Instant;
use nix::Error as NixErr;
use nix::sys::wait::*;
use nix::sys::signal::Signal;
use nix::errno::Errno;
use nix::unistd::Pid;
use breakpoint::*;
use traces::*;
use ptrace_control::*;
use config::Config;
use errors::RunError;


#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TestState {
    /// Start state. Wait for test to appear and track time to enable timeout
    Start {
        start_time: Instant,
    },
    /// Initialise: once test process appears instrument
    Initialise,
    /// Waiting for breakpoint to be hit or test to end
    Waiting {
        start_time: Instant,
    },
    /// Test process stopped, check coverage
    Stopped,
    /// Test exited normally. Includes the exit code of the test executable.
    End(i32),
}

/// Tracing a process on an OS will have platform specific code.
/// Structs containing the platform specific datastructures should
/// provide this trait with an implementation of the handling of
/// the given states.
pub trait StateData {
    /// Starts the tracing. Returns None while waiting for
    /// start. Statemachine then checks timeout
    fn start(&mut self) -> Result<Option<TestState>, RunError>;
    /// Initialises test for tracing returns next state
    fn init(&mut self) -> Result<TestState, RunError>;
    /// Waits for notification from test executable that there's
    /// something to do. Selects the next appropriate state if there's
    /// something to do otherwise None
    fn wait(&mut self) -> Result<Option<TestState>, RunError>;
    /// Handle a stop in the test executable. Coverage data will
    /// be collected here as well as other OS specific functions
    fn stop(&mut self) -> Result<TestState, RunError>;
}


impl TestState {
    /// Convenience function used to check if the test has finished or errored
    pub fn is_finished(self) -> bool {
        match self {
            TestState::End(_) => true,
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

    /// Updates the state machine state
    pub fn step<T:StateData>(self, data: &mut T, config: &Config) -> Result<TestState, RunError> {
        match self {
            TestState::Start{start_time} => {
                if let Some(s) = data.start()? {
                    Ok(s)
                } else if start_time.elapsed() >= config.test_timeout {
                    Err(RunError::TestRuntime("Error: Timed out when starting test".to_string()))
                } else {
                    Ok(TestState::Start{start_time})
                }
            },
            TestState::Initialise => {
                data.init()
            },
            TestState::Waiting{start_time} => {
                if let Some(s) = data.wait()? {
                    Ok(s)
                } else if start_time.elapsed() >= config.test_timeout {
                    Err(RunError::TestRuntime("Error: Timed out waiting for test response".to_string()))
                } else {
                    Ok(TestState::Waiting{start_time})
                }
            },
            TestState::Stopped => {
                data.stop()
            },
            _ => {
                // Unhandled
                if config.verbose {
                    println!("Tarpaulin error: unhandled test state");
                }
                Ok(TestState::End(-1))
            }
        }
    }
}


pub fn create_state_machine<'a>(test: Pid,
                                traces: &'a mut TraceMap,
                                config: &'a Config) -> (TestState, LinuxData<'a>) {
    let mut data = LinuxData::new(traces, config);
    data.parent = test;
    (TestState::start_state(), data)
}


/// Handle to linux process state
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
    traces: &'a mut TraceMap,
    /// Program config
    config: &'a Config,
    /// Thread count. Hopefully getting rid of in future
    thread_count: isize,
    /// Used to show anomalies noticed so hit counts disabled
    force_disable_hit_count: bool
}


impl <'a> StateData for LinuxData<'a> {

    fn start(&mut self) -> Result<Option<TestState>, RunError> {
        match waitpid(self.current, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => Ok(None),
            Ok(sig @ WaitStatus::Stopped(_, Signal::SIGTRAP)) => {
                if let WaitStatus::Stopped(child, _) = sig {
                    self.current = child;
                }
                self.wait = sig;
                Ok(Some(TestState::Initialise))
            },
            Ok(_) => {
                Err(RunError::TestRuntime("Unexpected signal when starting test".to_string()))
            },
            Err(e) => {
                Err(RunError::TestRuntime(format!("Error when starting test: {}", e)))
            },
        }
    }


    fn init(&mut self) -> Result<TestState, RunError> {
        //TODO: is this recoverable?
        trace_children(self.current)?;
        for trace in self.traces.all_traces() {
            if let Some(addr) = trace.address {
                match Breakpoint::new(self.current, addr) {
                    Ok(bp) => {
                        let _ = self.breakpoints.insert(addr, bp);
                    },
                    Err(e) if e==NixErr::Sys(Errno::EIO) => {
                        return Err(RunError::TestRuntime("ERROR: Tarpaulin cannot find code addresses \
                                  check that pie is disabled for your linker. \
                                  If linking with gcc try adding -C link-args=-no-pie \
                                  to your rust flags".to_string()));
                    }
                    Err(NixErr::UnsupportedOperation) => {
                        debug!("Instrumentation address clash, ignoring 0x{:x}", addr);
                    },
                    Err(_) => {
                        return Err(RunError::TestRuntime("Failed to instrument test executable".to_string()));
                    },
                }
            }
        }

        if continue_exec(self.parent, None).is_ok() {
            return Ok(TestState::wait_state());
        }

        //TODO: Is this correct?
        Ok(TestState::start_state())
    }


    fn wait(&mut self) -> Result<Option<TestState>, RunError> {
        let wait = waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL));
        match wait {
            Ok(WaitStatus::StillAlive) => {
                self.wait = WaitStatus::StillAlive;
                Ok(None)
            },
            Ok(s) => {
                self.wait = s;
                Ok(Some(TestState::Stopped))
            },
            Err(_) => {
                //TODO: Better error message!
                Err(RunError::TestRuntime("An error occurred while waiting for response from test".to_string()))
            },
        }
    }


    fn stop(&mut self) -> Result<TestState, RunError> {
        match self.wait {
            WaitStatus::PtraceEvent(c,s,e) => {
                match self.handle_ptrace_event(c, s, e) {
                    Ok(s) => Ok(s),
                    Err(e) => {
                        Err(RunError::TestRuntime(format!("Error occurred when handling ptrace event: {}", e)))
                    },
                }
            },
            WaitStatus::Stopped(c, Signal::SIGTRAP) => {
                self.current = c;
                match self.collect_coverage_data() {
                    Ok(s) => Ok(s),
                    Err(e) => {
                        Err(RunError::TestRuntime(format!("Error when collecting coverage: {}", e)))
                    }
                }
            },
            WaitStatus::Stopped(child, Signal::SIGSTOP) => {
                match continue_exec(child, None) {
                    Ok(_) => Ok(TestState::wait_state()),
                    Err(e) => Err(RunError::TestRuntime(format!("Error processing SIGSTOP: {}", e.to_string())))
                }
            },
            WaitStatus::Stopped(_, Signal::SIGSEGV) => {
                Err(RunError::TestRuntime("A segfault occurred while executing tests".to_string()))
            },
            WaitStatus::Stopped(c, s) => {
                let sig = if self.config.forward_signals {
                    Some(s)
                } else {
                    None
                };
                let _ = continue_exec(c, sig);
                Ok(TestState::wait_state())
            },
            WaitStatus::Signaled(_,_,_) => {
                if let Ok(s) = self.handle_signaled() {
                   Ok(s)
                } else {
                    Err(RunError::TestRuntime("Attempting to handle tarpaulin being signaled".to_string()))
                }
            },
            WaitStatus::Exited(child, ec) => {
                for ref mut value in self.breakpoints.values_mut() {
                    value.thread_killed(child);
                }
                if child == self.parent {
                    Ok(TestState::End(ec))
                } else {
                    // Process may have already been destroyed. This is just incase
                    let _ = continue_exec(self.parent, None);
                    Ok(TestState::wait_state())
                }
            },
            //TODO: Seriously, improve this.
            _ => Err(RunError::TestRuntime("Something went wrong stopping the tests!".to_string()))
        }
    }
}


impl <'a>LinuxData<'a> {
    pub fn new(traces: &'a mut TraceMap, config: &'a Config) -> LinuxData<'a> {
        LinuxData {
            wait: WaitStatus::StillAlive,
            current: Pid::from_raw(0),
            parent: Pid::from_raw(0),
            breakpoints: HashMap::new(),
            traces,
            config,
            thread_count: 0,
            force_disable_hit_count: config.count
        }
    }

    fn handle_ptrace_event(&mut self, child: Pid, sig: Signal, event: i32) -> Result<TestState, RunError> {
        use nix::libc::*;

        if sig == Signal::SIGTRAP {
            match event {
                PTRACE_EVENT_CLONE => {
                    if get_event_data(child).is_ok() {
                        self.thread_count += 1;
                        continue_exec(child, None)?;
                        Ok(TestState::wait_state())
                    } else {
                        Err(RunError::TestRuntime("Error occurred upon test executable thread creation".to_string()))
                    }
                },
                PTRACE_EVENT_FORK | PTRACE_EVENT_VFORK => {
                    continue_exec(child, None)?;
                    Ok(TestState::wait_state())
                },
                PTRACE_EVENT_EXEC => {
                    detach_child(child)?;
                    Ok(TestState::wait_state())
                },
                PTRACE_EVENT_EXIT => {
                    self.thread_count -= 1;
                    continue_exec(child, None)?;
                    Ok(TestState::wait_state())
                },
                //TODO: Correct error message?
                _ => Err(RunError::TestRuntime("Something went wrong!".to_string()))
            }
        } else {
            Err(RunError::TestRuntime("Unexpected ptrace event".to_string()))
        }
    }

    fn collect_coverage_data(&mut self) -> Result<TestState, RunError> {
        if let Ok(rip) = current_instruction_pointer(self.current) {
            let rip = (rip - 1) as u64;
            if  self.breakpoints.contains_key(&rip) {
                let bp = &mut self.breakpoints.get_mut(&rip).unwrap();
                let enable = self.config.count && self.thread_count < 2;
                if !enable && self.force_disable_hit_count {
                    println!("Code is mulithreaded, disabling hit count");
                    println!("Results may be improved by not using the '--count' option when running tarpaulin");
                    self.force_disable_hit_count = false;
                }
                // Don't reenable if multithreaded as can't yet sort out segfault issue
                let updated = if let Ok(x) = bp.process(self.current, enable) {
                     x
                } else {
                    // So failed to process a breakpoint.. Still continue to avoid
                    // stalling
                    continue_exec(self.current, None)?;
                    false
                };
                if updated {
                    if let Some(ref mut t) = self.traces.get_trace_mut(rip) {
                        if let CoverageStat::Line(ref mut x) = t.stats {
                            *x += 1;
                        }
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


    fn handle_signaled(&mut self) -> Result<TestState, RunError> {
        match self.wait {
            WaitStatus::Signaled(child, Signal::SIGTRAP, true) => {
                continue_exec(child, None)?;
                Ok(TestState::wait_state())
            },
            _ => {
                Err(RunError::StateMachine("Unexpected stop".to_string()))
            },
        }
    }
}

