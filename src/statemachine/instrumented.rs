#![allow(dead_code)]
use crate::path_utils::{get_profile_walker, get_source_walker};
use crate::process_handling::RunningProcessHandle;
use crate::statemachine::*;
use llvm_profparser::*;
use std::thread::sleep;
use tracing::{info, warn};

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

impl<'a> LlvmInstrumentedData<'a> {
    fn should_panic(&self) -> bool {
        match &self.process {
            Some(hnd) => hnd.should_panic,
            None => false,
        }
    }
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
        let should_panic = self.should_panic();
        if let Some(parent) = self.process.as_mut() {
            match parent.child.wait() {
                Ok(exit) => {
                    if !exit.success() && !should_panic {
                        return Err(RunError::TestFailed);
                    }
                    if let Some(delay) = self.config.post_test_delay {
                        sleep(delay);
                    }
                    let profraws = get_profile_walker(self.config)
                        .map(|x| x.path().to_path_buf())
                        .filter(|x| !parent.existing_profraws.contains(x))
                        .collect::<Vec<_>>();

                    info!(
                        "For binary: {}",
                        self.config.strip_base_dir(&parent.path).display()
                    );
                    for prof in &profraws {
                        let profraw_name = self.config.strip_base_dir(prof);
                        info!("Generated: {}", profraw_name.display());
                    }

                    let binary_path = parent.path.clone();
                    info!("Merging coverage reports");
                    let instrumentation = merge_profiles(&profraws)?;
                    if instrumentation.is_empty() {
                        warn!("profraw file has no records after merging. If this is unexpected it may be caused by a panic or signal used in a test that prevented the LLVM instrumentation runtime from serialising results");
                        self.process = None;
                        let code = exit.code().unwrap_or(1);
                        return Ok(Some(TestState::End(code)));
                    }

                    let mut binaries = parent
                        .extra_binaries
                        .iter()
                        .filter(|path| {
                            // extra binaries might not exist yet and be created
                            // later by the test suite
                            if path.exists() {
                                true
                            } else {
                                info!(
                                    "Skipping additional object '{}' since the file does not exist",
                                    path.display()
                                );
                                false
                            }
                        })
                        .cloned()
                        .collect::<Vec<_>>();

                    binaries.push(binary_path);
                    info!("Mapping coverage data to source");
                    let mapping =
                        CoverageMapping::new(&binaries, &instrumentation).map_err(|e| {
                            error!("Failed to get coverage: {}", e);
                            RunError::TestCoverage(e.to_string())
                        })?;
                    let report = mapping.generate_report();

                    if self.traces.is_empty() {
                        for source_file in get_source_walker(self.config) {
                            let file = source_file.path();
                            let analysis = self.analysis.get(file);
                            if let Some(result) = report.files.get(file) {
                                for (loc, hits) in result.hits.iter() {
                                    for line in loc.line_start..(loc.line_end + 1) {
                                        let include = match analysis.as_ref() {
                                            Some(analysis) => !analysis.should_ignore(line),
                                            None => true,
                                        };
                                        if include {
                                            let mut trace = Trace::new_stub(line as u64);
                                            trace.stats = CoverageStat::Line(*hits as u64);
                                            self.traces.add_trace(file, trace);
                                        }
                                    }
                                }
                            }
                            if let Some(analysis) = analysis {
                                for line in analysis.cover.iter() {
                                    if !self.traces.contains_location(file, *line as u64) {
                                        let mut trace = Trace::new_stub(*line as u64);
                                        trace.stats = CoverageStat::Line(0);
                                        self.traces.add_trace(file, trace);
                                    }
                                }
                            }
                        }
                    } else {
                        self.traces.dedup();

                        for (file, result) in report.files.iter() {
                            if let Some(traces) = self.traces.file_traces_mut(file) {
                                for trace in traces.iter_mut() {
                                    if let Some(hits) = result.hits_for_line(trace.line as usize) {
                                        if let CoverageStat::Line(ref mut x) = trace.stats {
                                            *x = hits as _;
                                        }
                                    }
                                }
                            } else {
                                warn!(
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
