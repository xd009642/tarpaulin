use std::collections::HashMap;
use std::time::Instant;
use nix::Error as NixErr;
use nix::sys::wait::*;
use nix::sys::signal::Signal;
use nix::errno::Errno;
use nix::Result;
use nix::unistd::Pid;
use breakpoint::*;
use traces::*;
use ptrace_control::*;
use config::Config;



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
    /// Test exited normally. Includes the exit code of the test executable.
    End(i32),
    /// An error occurred that indicates no future runs will succeed such as
    /// PIE issues in OS.
    Abort,
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


impl TestState {
    /// Convenience function used to check if the test has finished or errored
    pub fn is_finished(self) -> bool {
        match self {
            TestState::End(_) | TestState::Unrecoverable | TestState::Abort => true,
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
    pub fn step<T:StateData>(self, data: &mut T, config: &Config) -> TestState {
        match self {
            TestState::Start{start_time} => {
                if let Some(s) = data.start() {
                    s
                } else if start_time.elapsed() >= config.test_timeout {
                    println!("Error: Timed out when starting test");
                    TestState::Timeout
                } else {
                    TestState::Start{start_time}
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
                // Test hasn't ran all the way through. Report as error
                TestState::End(-1)
            },
            TestState::Unrecoverable => {
                data.cleanup();
                // We've gone wrong somewhere. Better report it as an issue
                TestState::End(-1)
            },
            _ => {
                // Unhandled
                if config.verbose {
                    println!("Tarpaulin error: unhandled test state");
                }
                TestState::End(-1)
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
    /// Used to store error for user in the event something goes wrong
    pub error_message: Option<String>,
    /// Thread count. Hopefully getting rid of in future
    thread_count: isize,
    /// Used to show anomalies noticed so hit counts disabled
    force_disable_hit_count: bool
}


impl <'a> StateData for LinuxData<'a> {

    fn start(&mut self) -> Option<TestState> {
        match waitpid(self.current, Some(WaitPidFlag::WNOHANG)) {
            Ok(WaitStatus::StillAlive) => None,
            Ok(sig @ WaitStatus::Stopped(_, Signal::SIGTRAP)) => {
                if let WaitStatus::Stopped(child, _) = sig {
                    self.current = child;
                }
                self.wait = sig;
                Some(TestState::Initialise)
            },
            Ok(_) => {
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
        if trace_children(self.current).is_err() {
            println!("Failed to trace child threads");
        }
        let mut instrumented = true;
        for trace in self.traces.all_traces() {
            if let Some(addr) = trace.address {
                match Breakpoint::new(self.current, addr) {
                    Ok(bp) => {
                        let _ = self.breakpoints.insert(addr, bp);
                    },
                    Err(e) if e==NixErr::Sys(Errno::EIO) => {
                        println!("ERROR: Tarpaulin cannot find code addresses \
                                  check that pie is disabled for your linker. \
                                  If linking with gcc try adding -C link-args=-no-pie \
                                  to your rust flags");
                        instrumented = false;
                        break;
                    }
                    Err(NixErr::UnsupportedOperation) => {
                        if self.config.verbose {
                            println!("Instrumentation address clash, ignoring 0x{:x}", addr);
                        }
                    },
                    Err(_) => {
                        self.error_message = Some("Failed to instrument test executable".to_string());
                    },
                }
            }
        }
        if !instrumented {
            TestState::Abort
        }
        else if continue_exec(self.parent, None).is_ok() {
            TestState::wait_state()
        } else {
            TestState::Unrecoverable
        }
    }


    fn wait(&mut self) -> Option<TestState> {
        let wait = waitpid(Pid::from_raw(-1), Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL));
        match wait {
            Ok(WaitStatus::StillAlive) => {
                self.wait = WaitStatus::StillAlive;
                None
            },
            Ok(s) => {
                self.wait = s;
                Some(TestState::Stopped)
            },
            Err(_) => {
                self.error_message = Some("An error occurred while waiting for response from test".to_string());
                Some(TestState::Unrecoverable)
            },
        }
    }


    fn stop(&mut self) -> TestState {
        match self.wait {
            WaitStatus::PtraceEvent(c,s,e) => {
                match self.handle_ptrace_event(c, s, e) {
                    Ok(s) => s,
                    Err(e) => {
                        let msg = format!("Error occurred when handling ptrace event: {}", e);
                        self.error_message = Some(msg);
                        TestState::Unrecoverable
                    },
                }
            },
            WaitStatus::Stopped(c, Signal::SIGTRAP) => {
                self.current = c;
                match self.collect_coverage_data() {
                    Ok(s) => s,
                    Err(e) => {
                        self.error_message = Some(format!("Error when collecting coverage: {}", e));
                        TestState::Unrecoverable
                    }
                }
            },
            WaitStatus::Stopped(child, Signal::SIGSTOP) => {
                if continue_exec(child, None).is_ok() {
                    TestState::wait_state()
                } else {
                    self.error_message = Some("Error processing SIGSTOP".to_string());
                    TestState::Unrecoverable
                }
            },
            WaitStatus::Stopped(_, Signal::SIGSEGV) => {
                self.error_message = Some("Error a segfault occured when executing test".to_string());
                TestState::Unrecoverable
            },
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
            WaitStatus::Exited(child, ec) => {
                for ref mut value in self.breakpoints.values_mut() {
                    value.thread_killed(child);
                }
                if child == self.parent {
                    TestState::End(ec)
                } else {
                    // Process may have already been destroyed. This is just incase
                    let _ = continue_exec(self.parent, None);
                    TestState::wait_state()
                }
            },
            _ => TestState::Unrecoverable,
        }
    }


    fn cleanup(&mut self)  {
        if let Some(ref e) = self.error_message {
            println!("An error occurred during run. Coverage results may be inaccurate.");
            println!("{}", e);
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
            error_message:None,
            thread_count: 0,
            force_disable_hit_count: config.count
        }
    }

    fn handle_ptrace_event(&mut self, child: Pid, sig: Signal, event: i32) -> Result<TestState> {
        use nix::libc::*;

        if sig == Signal::SIGTRAP {
            match event {
                PTRACE_EVENT_CLONE => {
                    if get_event_data(child).is_ok() {
                        self.thread_count += 1;
                        continue_exec(child, None)?;
                        Ok(TestState::wait_state())
                    } else {
                        self.error_message = Some("Error occurred upon test executable thread creation".to_string());
                        Ok(TestState::Unrecoverable)
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
                _ => Ok(TestState::Unrecoverable)
            }
        } else {
            self.error_message = Some("Unexpected ptrace event".to_string());
            Ok(TestState::Unrecoverable)
        }
    }

    fn collect_coverage_data(&mut self) -> Result<TestState> {
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


    fn handle_signaled(&mut self) -> Result<TestState> {
        match self.wait {
            WaitStatus::Signaled(child, Signal::SIGTRAP, true) => {
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

