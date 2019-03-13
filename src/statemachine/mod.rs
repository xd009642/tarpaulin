use crate::breakpoint::*;
use crate::config::Config;
use crate::errors::RunError;
use crate::ptrace_control::*;
use crate::traces::*;
use std::time::Instant;

#[cfg(target_os = "linux")]
pub mod linux;

#[cfg(target_os = "linux")]
pub use linux::*;

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
                    Err(RunError::TestRuntime(
                        "Error: Timed out waiting for test response".to_string(),
                    ))
                } else {
                    Ok(TestState::Waiting { start_time })
                }
            }
            TestState::Stopped => data.stop(),
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
