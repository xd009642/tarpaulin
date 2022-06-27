#![allow(dead_code)]
use crate::config::Config;
use crate::errors::RunError;
use crate::path_utils::{get_profile_walker, is_coverable_file_path};
use crate::process_handling::RunningProcessHandle;
use crate::source_analysis::LineAnalysis;
use crate::statemachine::*;
use crate::TestHandle;
use llvm_profparser::*;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::info;

pub fn create_state_machine<'a>(
    test: impl Into<TestHandle>,
    traces: &'a mut TraceMap,
    analysis: &'a HashMap<PathBuf, LineAnalysis>,
    config: &'a Config,
    event_log: &'a Option<EventLog>,
) -> (TestState, LlvmInstrumentedData<'a>) {
    let handle = test.into();
    if let TestHandle::Process(process) = handle {
        let llvm = LlvmInstrumentedData {
            process: Some(process),
            event_log,
            config,
            traces,
            analysis,
        };
        (TestState::start_state(), llvm)
    } else {
        error!("The llvm cov statemachine requires a process::Child");
        let invalid = LlvmInstrumentedData {
            process: None,
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
    process: Option<RunningProcessHandle>,
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

    fn last_wait_attempt(&mut self) -> Result<Option<TestState>, RunError> {
        unreachable!();
    }

    fn wait(&mut self) -> Result<Option<TestState>, RunError> {
        if let Some(parent) = self.process.as_mut() {
            match parent.child.wait() {
                Ok(exit) => {
                    let profraws = get_profile_walker(self.config)
                        .map(|x| x.path().to_path_buf())
                        .filter(|x| !parent.existing_profraws.contains(&x))
                        .collect::<Vec<_>>();

                    info!(
                        "For binary: {}",
                        self.config.strip_base_dir(&parent.path).display()
                    );
                    for prof in &profraws {
                        info!("Generated: {}", self.config.strip_base_dir(prof).display());
                    }

                    let binary_path = parent.path.clone();

                    let instrumentation = merge_profiles(&profraws)?;
                    // Panics due to a todo!();
                    let mapping =
                        CoverageMapping::new(&[binary_path], &instrumentation).map_err(|e| {
                            error!("Failed to get coverage: {}", e);
                            RunError::TestCoverage(e.to_string())
                        })?;
                    let report = mapping.generate_report();

                    if self.traces.is_empty() {
                        let target = self.config.target_dir();
                        let root = self.config.root();
                        for (file, result) in report
                            .files
                            .iter()
                            .filter(|(k, _)| is_coverable_file_path(k, &root, &target))
                        {
                            for (loc, hits) in result.hits.iter() {
                                let mut trace = Trace::new_stub(loc.line_start as u64);
                                trace.stats = CoverageStat::Line(*hits as u64);
                                self.traces.add_trace(file, trace);
                            }
                        }
                    } else {
                        self.traces.dedup();

                        for (file, result) in report.files.iter() {
                            if let Some(traces) = self.traces.file_traces_mut(&file) {
                                for trace in traces.iter_mut() {
                                    if let Some(hits) = result.hits_for_line(trace.line as usize) {
                                        if let CoverageStat::Line(ref mut x) = trace.stats {
                                            *x = hits as _;
                                        }
                                    }
                                }
                            } else {
                                println!(
                                    "Couldn't find {} in {:?}",
                                    file.display(),
                                    self.traces.files()
                                );
                            }
                        }
                    }

                    self.process = None;
                    let code = exit.code().unwrap_or(1);
                    Ok(Some(TestState::End(code)))
                }
                Err(e) => Err(e.into()),
            }
        } else {
            Err(RunError::TestCoverage("Test was not launched".to_string()))
        }
    }

    fn stop(&mut self) -> Result<TestState, RunError> {
        unreachable!();
    }
}
