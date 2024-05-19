use crate::config::{Config, TraceEngine};
use crate::errors::RunError;
use crate::event_log::*;
use crate::traces::*;
use crate::LineAnalysis;
use crate::TestHandle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;
use tracing::error;

pub mod instrumented;
cfg_if::cfg_if! {
    if #[cfg(ptrace_supported)] {
        pub mod linux;
        pub use linux::ProcessInfo;
    }
}
pub fn create_state_machine<'a>(
    test: impl Into<TestHandle>,
    traces: &'a mut TraceMap,
    source_analysis: &'a HashMap<PathBuf, LineAnalysis>,
    config: &'a Config,
    event_log: &'a Option<EventLog>,
) -> (TestState, Box<dyn StateData + 'a>) {
    match config.engine() {
        TraceEngine::Ptrace => {
            cfg_if::cfg_if! {
                if #[cfg(ptrace_supported)] {
                    let (state, machine) = linux::create_state_machine(test, traces, source_analysis, config, event_log);
                    (state, Box::new(machine))
                } else {
                    error!("The ptrace backend is not supported on this system");
                    panic!()
                }
            }
        }
        // Should never be auto so ignore our normal rules
        TraceEngine::Llvm | TraceEngine::Auto => {
            let (state, machine) = instrumented::create_state_machine(
                test,
                traces,
                source_analysis,
                config,
                event_log,
            );
            (state, Box::new(machine))
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum TestState {
    /// Start state. Wait for test to appear and track time to enable timeout
    Start { start_time: Instant },
    /// Initialise: once test process appears instrument
    Initialise,
    /// Waiting for breakpoint to be hit or test to end
    Waiting { start_time: Instant },
    /// Test process stopped, check coverage
    Stopped,
    /// Test exited normally. Includes the exit code of the test executable.
    End(i32),
}

/// This enum represents a generic action for the process tracing API to take
/// along with any form of ID or handle to the underlying thread or process
/// i.e. a PID in Unix.
#[derive(Debug, Clone, Eq, PartialEq)]
pub enum TracerAction<T> {
    /// Try continue is for times when you don't know if there is something
    /// paused but if there is you want it to move on.
    TryContinue(T),
    Continue(T),
    Step(T),
    Detach(T),
    Nothing,
}

impl<T> TracerAction<T> {
    pub fn get_data(&self) -> Option<&T> {
        match self {
            TracerAction::Continue(d) => Some(d),
            TracerAction::Step(d) => Some(d),
            TracerAction::Detach(d) => Some(d),
            TracerAction::TryContinue(d) => Some(d),
            _ => None,
        }
    }
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
    /// This is here for the times when we're about to mark the attempted coverage collection as a
    /// failure i.e. timeout, but there's an alternative to that which can see if we're actually in
    /// a "finished" state but are still waiting on resource cleanup so we don't lose the results.
    fn last_wait_attempt(&mut self) -> Result<Option<TestState>, RunError>;
    /// Handle a stop in the test executable. Coverage data will
    /// be collected here as well as other OS specific functions
    fn stop(&mut self) -> Result<TestState, RunError>;
}

impl<'a> StateData for Box<dyn StateData + 'a> {
    fn start(&mut self) -> Result<Option<TestState>, RunError> {
        self.as_mut().start()
    }

    fn init(&mut self) -> Result<TestState, RunError> {
        self.as_mut().init()
    }

    fn wait(&mut self) -> Result<Option<TestState>, RunError> {
        self.as_mut().wait()
    }

    fn last_wait_attempt(&mut self) -> Result<Option<TestState>, RunError> {
        self.as_mut().last_wait_attempt()
    }

    fn stop(&mut self) -> Result<TestState, RunError> {
        self.as_mut().stop()
    }
}

impl TestState {
    /// Convenience function used to check if the test has finished or errored
    pub fn is_finished(self) -> bool {
        matches!(self, TestState::End(_))
    }

    /// Convenience function for creating start states
    fn start_state() -> TestState {
        TestState::Start {
            start_time: Instant::now(),
        }
    }

    /// Convenience function for creating wait states
    fn wait_state() -> TestState {
        TestState::Waiting {
            start_time: Instant::now(),
        }
    }

    /// Updates the state machine state
    pub fn step<T: StateData>(self, data: &mut T, config: &Config) -> Result<TestState, RunError> {
        match self {
            TestState::Start { start_time } => {
                if let Some(s) = data.start()? {
                    Ok(s)
                } else if start_time.elapsed() >= config.test_timeout {
                    Err(RunError::TestRuntime(
                        "Error: Timed out when starting test".to_string(),
                    ))
                } else {
                    Ok(TestState::Start { start_time })
                }
            }
            TestState::Initialise => data.init(),
            TestState::Waiting { start_time } => {
                if let Some(s) = data.wait()? {
                    Ok(s)
                } else if start_time.elapsed() >= config.test_timeout {
                    if let Some(s) = data.last_wait_attempt()? {
                        Ok(s)
                    } else {
                        Err(RunError::TestRuntime(
                            "Error: Timed out waiting for test response".to_string(),
                        ))
                    }
                } else {
                    Ok(TestState::Waiting { start_time })
                }
            }
            TestState::Stopped => data.stop(),
            TestState::End(e) => Ok(TestState::End(e)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    impl StateData for () {
        fn start(&mut self) -> Result<Option<TestState>, RunError> {
            Ok(None)
        }

        fn init(&mut self) -> Result<TestState, RunError> {
            Err(RunError::StateMachine(
                "No valid coverage collector".to_string(),
            ))
        }

        fn wait(&mut self) -> Result<Option<TestState>, RunError> {
            Ok(None)
        }

        fn last_wait_attempt(&mut self) -> Result<Option<TestState>, RunError> {
            Err(RunError::StateMachine(
                "No valid coverage collector".to_string(),
            ))
        }
        fn stop(&mut self) -> Result<TestState, RunError> {
            Err(RunError::StateMachine(
                "No valid coverage collector".to_string(),
            ))
        }
    }

    #[test]
    fn hits_timeouts() {
        let mut config = Config::default();
        config.test_timeout = Duration::from_secs(5);

        let start_time = Instant::now() - Duration::from_secs(6);

        let state = TestState::Start { start_time };

        assert!(state.step(&mut (), &config).is_err());

        let state = TestState::Waiting { start_time };

        assert!(state.step(&mut (), &config).is_err());
    }
}
