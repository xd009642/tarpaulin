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
enum State<T> {
    /// Start state. Wait for test to appear and track time to enable timeout
    Start { 
        hnd: T,
        start_time: u64, 
    },
    /// Initialise: once test process appears instrument 
    Initialise {
        hnd: T,
    },
    /// Waiting for breakpoint to be hit or test to end
    Waiting { 
        hnd: T,
        start_time: u64, 
    },
    /// Test process stopped, check coverage
    Stopped { 
        hnd: T,
        stop_state:T,
    },
    /// Unrecoverable error occurred
    Unrecoverable,
    /// Test exited normally
    End,
}

/// Trait for state machines to implement
trait StateMachine<T> {
    /// Update the states
    fn step(self, traces: &mut TracerData, config: &Config) -> State<T>;
}

/// Handle to linux process state
#[derive(Debug, Clone, Copy)]
struct LinuxHnd {
    wait: Option<WaitStatus>,
    parent: Pid,
}


impl StateMachine<LinuxHnd> for State<LinuxHnd> {
    
    fn step(self, traces: &mut TracerData, config: &Config) -> State<LinuxHnd> {
        match self {
            State::Start{hnd, start_time} => {
                if let Some(res) = self.start(hnd) {
                    res
                } else if start_time > 1000 {
                    State::Unrecoverable::<_>
                } else {
                    State::Start{hnd: hnd, start_time:start_time+1}
                }
            },
            _ => State::Unrecoverable::<_>
        }
    }
}

impl State<LinuxHnd> {
    
    fn start(self, hnd: LinuxHnd) -> Option<State<LinuxHnd>> {
        match waitpid(hnd.parent, Some(WNOHANG)) {
            Ok(WaitStatus::Stopped(_, signal::SIGTRAP)) => {
                Some(State::Initialise::<LinuxHnd>{hnd})
            },
            Ok(_) => {
                println!("Unexpected signal from test");
                None
            },
            Err(e) => {
                println!("Error on start: {}", e);
                Some(State::Unrecoverable::<_>)
            },
        }
    }
    
}


