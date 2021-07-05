use crate::config::Config;
use crate::errors::RunError;
use crate::source_analysis::LineAnalysis;
use crate::statemachine::*;
use crate::TestHandle;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::{Child, ExitStatus};

pub fn create_state_machine<'a>(
    test: impl Into<TestHandle>,
    traces: &'a mut TraceMap,
    analysis: &'a HashMap<PathBuf, LineAnalysis>,
    config: &'a Config,
    event_log: &'a Option<EventLog>,
) -> (TestState, LlvmInstrumentedData<'a>) {
    let handle = test.into();
    if let TestHandle::Process(child) = handle {
        let pid = child.id();
        let llvm = LlvmInstrumentedData {
            parent: Some(child),
            pid,
            output: None,
            event_log,
            config,
            traces,
            analysis,
        };
        (TestState::start_state(), llvm)
    } else {
        error!("The llvm cov statemachine requires a process::Child");
        let invalid = LlvmInstrumentedData {
            parent: None,
            pid: 0,
            output: None,
            config,
            event_log,
            traces,
            analysis,
        };
        (TestState::End(1), invalid)
    }
}

/// Handle to the process for an instrumented binary. This will simply
pub struct LlvmInstrumentedData<'a> {
    /// Parent pid of the test
    parent: Option<Child>,
    /// Keep the ID as we may rely on it to identify which profdata's are ours
    pid: u32,
    /// Program outpuit
    output: Option<ExitStatus>,
    /// Program config
    config: &'a Config,
    /// Optional event log to update as the test progresses
    event_log: &'a Option<EventLog>,
    /// Instrumentation points in code with associated coverage data
    traces: &'a mut TraceMap,
    /// Source analysis, needed in case we need to follow any executables
    analysis: &'a HashMap<PathBuf, LineAnalysis>,
}

impl<'a> StateData for LlvmInstrumentedData<'a> {
    fn start(&mut self) -> Result<Option<TestState>, RunError> {
        // Nothing needs to be done at startup as this runs like a normal process
        Ok(Some(TestState::wait_state()))
    }

    fn init(&mut self) -> Result<TestState, RunError> {
        // Nothing needs to be done at init as this runs like a normal process
        unreachable!();
    }

    fn wait(&mut self) -> Result<Option<TestState>, RunError> {
        if let Some(parent) = self.parent.as_mut() {
            match parent.wait() {
                Ok(exit) => {
                    self.output = Some(exit);
                    self.parent = None;
                }
                Err(e) => {}
            }
            todo!()
        } else {
            Err(RunError::TestCoverage("Test was not launched".to_string()))
        }
    }

    fn stop(&mut self) -> Result<TestState, RunError> {
        if let Some(status) = self.output {
            todo!()
        } else {
            Err(RunError::TestCoverage(
                "No ExitStatus available for test executable".to_string(),
            ))
        }
    }
}
