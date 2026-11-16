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
        self.wait()
    }

    fn wait(&mut self) -> Result<Option<TestState>, RunError> {
        let should_panic = self.should_panic();
        if let Some(parent) = self.process.as_mut() {
            match parent.child.try_wait() {
                Ok(Some(exit)) => {
                    if !exit.success() && !should_panic {
                        self.process = None;
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
                        warn!(
                            "profraw file has no records after merging. If this is unexpected it may be caused by a panic or signal used in a test that prevented the LLVM instrumentation runtime from serialising results"
                        );
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
                        CoverageMapping::new(&binaries, &instrumentation, true).map_err(|e| {
                            error!("Failed to get coverage: {}", e);
                            RunError::TestCoverage(e.to_string())
                        })?;

                    let root = self.config.root();
                    let report = mapping
                        .generate_subreport(|paths| {
                            paths.iter().any(|path| path.starts_with(&root))
                        })
                        .map_err(|e| {
                            error!("Failed to generate report: {}", e);
                            RunError::TestCoverage(e.to_string())
                        })?;

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
                                let result = report.files.get(file);
                                for line in analysis.cover.iter() {
                                    if !self.traces.contains_location(file, *line as u64) {
                                        let hits = result
                                            .and_then(|result| {
                                                function_entry_hits(analysis, result, *line)
                                            })
                                            .unwrap_or_default();
                                        let mut trace = Trace::new_stub(*line as u64);
                                        trace.stats = CoverageStat::Line(hits as u64);
                                        self.traces.add_trace(file, trace);
                                    }
                                }
                            }
                        }
                    } else {
                        self.traces.dedup();

                        for (file, result) in report.files.iter() {
                            if let Some(traces) = self.traces.file_traces_mut(file) {
                                let analysis = self.analysis.get(file);
                                for trace in traces.iter_mut() {
                                    let line = trace.line as usize;
                                    let hits = line_hits(result, line).or_else(|| {
                                        analysis.and_then(|analysis| {
                                            function_entry_hits(analysis, result, line)
                                        })
                                    });
                                    if let Some(hits) = hits {
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
                Ok(None) => {
                    sleep(std::time::Duration::from_millis(10));
                    Ok(None)
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

impl Drop for LlvmInstrumentedData<'_> {
    fn drop(&mut self) {
        if let Some(process) = self.process.as_mut() {
            let _ = process.child.kill();
            let _ = process.child.wait();
        }
    }
}

/// Hits of `line` the way `llvm-cov` counts them: the most of any region on the line.
///
/// A line can hold several regions, such as a call and a closure passed to it that never runs,
/// and the line ran when any of them did.
fn line_hits(result: &CoverageResult, line: usize) -> Option<usize> {
    result
        .hits
        .iter()
        .filter(|(loc, _)| loc.line_start <= line && loc.line_end >= line)
        .map(|(_, hits)| *hits)
        .max()
}

/// Hits of a line the compiler gave no region, inside a function it did map: the times that
/// function was entered.
///
/// The debug info and the forced lines of generic, inline and generic impl functions hold lines
/// no region covers. Since rustc 1.100 these include match arm patterns and `let` patterns, so
/// leaving them at zero reports arms whose bodies run as uncovered. A function with no region
/// at all gets `None`, and its lines stay uncovered.
fn function_entry_hits(
    analysis: &LineAnalysis,
    result: &CoverageResult,
    line: usize,
) -> Option<usize> {
    let (start, end) = analysis
        .functions
        .values()
        .filter(|(start, end)| (*start..=*end).contains(&line))
        .max_by_key(|(start, _)| *start)?;
    result
        .hits
        .iter()
        .filter(|(loc, _)| (*start..=*end).contains(&loc.line_start))
        .min_by_key(|(loc, _)| (loc.line_start, loc.column_start))
        .map(|(_, hits)| *hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn result(regions: &[(usize, usize, usize, usize)]) -> CoverageResult {
        let mut result = CoverageResult::default();
        for (line_start, column_start, line_end, hits) in regions {
            let loc = SourceLocation {
                line_start: *line_start,
                line_end: *line_end,
                column_start: *column_start,
                column_end: 80,
            };
            result.insert("f", loc, *hits);
        }
        result
    }

    fn analysis(functions: &[(usize, usize)]) -> LineAnalysis {
        let mut analysis = LineAnalysis::default();
        for (i, function) in functions.iter().enumerate() {
            analysis.functions.insert(format!("f{i}"), *function);
        }
        analysis
    }

    #[test]
    fn lines_take_the_most_hits_of_their_regions() {
        let result = result(&[(1, 1, 2, 3), (2, 20, 2, 0), (4, 1, 4, 0)]);
        assert_eq!(line_hits(&result, 2), Some(3));
        assert_eq!(line_hits(&result, 4), Some(0));
        assert_eq!(line_hits(&result, 3), None);
    }

    #[test]
    fn lines_without_regions_take_the_entries_of_their_mapped_function() {
        let analysis = analysis(&[(1, 10)]);
        let result = result(&[(5, 9, 5, 0), (1, 1, 1, 4), (1, 30, 1, 2)]);
        assert_eq!(function_entry_hits(&analysis, &result, 3), Some(4));
    }

    #[test]
    fn lines_of_unmapped_functions_get_no_hits() {
        let analysis = analysis(&[(1, 10), (12, 20)]);
        let result = result(&[(1, 1, 1, 4)]);
        assert_eq!(function_entry_hits(&analysis, &result, 15), None);
        assert_eq!(function_entry_hits(&analysis, &result, 11), None);
    }

    #[test]
    fn the_innermost_function_decides() {
        // A function nested in another has its own mapping
        let analysis = analysis(&[(1, 20), (5, 10)]);
        let result = result(&[(1, 1, 1, 4), (5, 1, 5, 0), (15, 1, 15, 4)]);
        assert_eq!(function_entry_hits(&analysis, &result, 7), Some(0));
        assert_eq!(function_entry_hits(&analysis, &result, 12), Some(4));
    }
}
