use crate::cargo::TestBinary;
use crate::config::*;
use crate::errors::*;
use crate::event_log::*;
use crate::path_utils::*;
use crate::process_handling::*;
use crate::report::report_coverage;
use crate::source_analysis::{LineAnalysis, SourceAnalysis};
use crate::statemachine::*;
use crate::test_loader::*;
use crate::traces::*;
use nix::unistd::*;
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use tracing::{debug, error, info, trace_span, warn};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

pub mod branching;
pub mod breakpoint;
pub mod cargo;
pub mod config;
pub mod errors;
pub mod event_log;
mod path_utils;
mod process_handling;
pub mod report;
mod source_analysis;
mod statemachine;
pub mod test_loader;
pub mod traces;

mod ptrace_control;

const RUST_LOG_ENV: &str = "RUST_LOG";

pub fn setup_logging(color: Color, debug: bool, verbose: bool) {
    //By default, we set tarpaulin to info,debug,trace while all dependencies stay at INFO
    let base_exceptions = |env: EnvFilter| {
        if debug {
            env.add_directive("cargo_tarpaulin=trace".parse().unwrap())
        } else if verbose {
            env.add_directive("cargo_tarpaulin=debug".parse().unwrap())
        } else {
            env.add_directive("cargo_tarpaulin=info".parse().unwrap())
        }
        .add_directive(LevelFilter::INFO.into())
    };

    //If RUST_LOG is set, then first apply our default directives (which are controlled by debug an verbose).
    // Then RUST_LOG will overwrite those default directives.
    // e.g. `RUST_LOG="trace" cargo-tarpaulin` will end up printing TRACE for everything
    // `cargo-tarpaulin -v` will print DEBUG for tarpaulin and INFO for everything else.
    // `RUST_LOG="error" cargo-tarpaulin -v` will print ERROR for everything.
    let filter = match std::env::var_os(RUST_LOG_ENV).map(|s| s.into_string()) {
        Some(Ok(env)) => {
            let mut filter = base_exceptions(EnvFilter::new(""));
            for s in env.split(',').into_iter() {
                match s.parse() {
                    Ok(d) => filter = filter.add_directive(d),
                    Err(err) => println!("WARN ignoring log directive: `{}`: {}", s, err),
                };
            }
            filter
        }
        _ => base_exceptions(EnvFilter::from_env(RUST_LOG_ENV)),
    };

    let with_ansi = color != Color::Never;

    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::ERROR)
        .with_env_filter(filter)
        .with_ansi(with_ansi)
        .init();

    debug!("set up logging");
}

pub fn trace(configs: &[Config]) -> Result<TraceMap, RunError> {
    let mut tracemap = TraceMap::new();
    let mut tarpaulin_result = Ok(());
    let mut ret = 0i32;
    let logger = if configs.iter().any(|c| c.dump_traces) {
        Some(EventLog::new())
    } else {
        None
    };
    let mut bad_threshold = None;
    for config in configs.iter() {
        if config.name == "report" {
            continue;
        }
        if let Some(log) = logger.as_ref() {
            let name = if config.name.is_empty() {
                "<anonymous>".to_string()
            } else {
                config.name.clone()
            };
            log.push_config(name);
        }
        let tgt = config.target_dir();
        if !tgt.exists() {
            let create_dir_result = create_dir_all(&tgt);
            if let Err(e) = create_dir_result {
                warn!("Failed to create target-dir {}", e);
            }
        }
        match launch_tarpaulin(config, &logger) {
            Ok((t, r)) => {
                ret |= r;
                bad_threshold = check_fail_threshold(&t, config);
                tracemap.merge(&t);
            }
            Err(e) => {
                error!("{}", e);
                tarpaulin_result = tarpaulin_result.and_then(|_| Err(e));
            }
        }
    }
    tracemap.dedup();
    if let Some(bad_limit) = bad_threshold {
        Err(bad_limit)
    } else if ret == 0 {
        tarpaulin_result.map(|_| tracemap)
    } else {
        Err(RunError::TestFailed)
    }
}

fn check_fail_threshold(traces: &TraceMap, config: &Config) -> Option<RunError> {
    let percent = traces.coverage_percentage() * 100.0;
    match config.fail_under.as_ref() {
        Some(limit) if percent < *limit => {
            let error = RunError::BelowThreshold(percent, *limit);
            error!("{}", error);
            Some(error)
        }
        _ => None,
    }
}

pub fn run(configs: &[Config]) -> Result<(), RunError> {
    let mut tracemap = trace(configs)?;
    if !configs.is_empty() {
        // Assumption: all configs are for the same project
        for dir in get_source_walker(&configs[0]) {
            tracemap.add_file(dir.path());
        }
    }
    if configs.len() == 1 {
        if !configs[0].no_run {
            report_coverage(&configs[0], &tracemap)?;
        }
    } else if !configs.is_empty() {
        let mut reported = false;
        for c in configs.iter() {
            if !c.no_run && c.name == "report" {
                reported = true;
                report_coverage(c, &tracemap)?;
                if let Some(e) = check_fail_threshold(&tracemap, c) {
                    return Err(e);
                }
            }
        }
        if !configs[0].no_run && !reported {
            report_coverage(&configs[0], &tracemap)?;
        }
    }

    Ok(())
}

/// Launches tarpaulin with the given configuration.
pub fn launch_tarpaulin(
    config: &Config,
    logger: &Option<EventLog>,
) -> Result<(TraceMap, i32), RunError> {
    if !config.name.is_empty() {
        info!("Running config {}", config.name);
    }

    info!("Running Tarpaulin");

    let mut result = TraceMap::new();
    let mut return_code = 0i32;
    info!("Building project");
    let executables = cargo::get_tests(config)?;
    if !config.no_run {
        let project_analysis = SourceAnalysis::get_analysis(config);
        let project_analysis = project_analysis.lines;
        for exe in &executables {
            if exe.should_panic() {
                info!("Running a test executable that is expected to panic");
            }
            let coverage = get_test_coverage(&exe, &project_analysis, config, false, logger)?;
            if let Some(res) = coverage {
                result.merge(&res.0);
                return_code |= if exe.should_panic() {
                    (res.1 == 0) as i32
                } else {
                    res.1
                };
            }
            if config.run_ignored {
                let coverage = get_test_coverage(&exe, &project_analysis, config, true, logger)?;
                if let Some(res) = coverage {
                    result.merge(&res.0);
                    return_code |= res.1;
                }
            }
        }
        result.dedup();
    }
    Ok((result, return_code))
}

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(
    test: &TestBinary,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
    ignored: bool,
    logger: &Option<EventLog>,
) -> Result<Option<(TraceMap, i32)>, RunError> {
    if !test.path().exists() {
        return Ok(None);
    }
    if let Err(e) = limit_affinity() {
        warn!("Failed to set processor affinity {}", e);
    }
    if let Some(log) = logger.as_ref() {
        log.push_binary(test.clone());
    }
    unsafe {
        match fork() {
            Ok(ForkResult::Parent { child }) => {
                match collect_coverage(test.path(), child, analysis, config, logger) {
                    Ok(t) => Ok(Some(t)),
                    Err(e) => Err(RunError::TestCoverage(e.to_string())),
                }
            }
            Ok(ForkResult::Child) => {
                info!("Launching test");
                execute_test(test, ignored, config)?;
                Ok(None)
            }
            Err(err) => Err(RunError::TestCoverage(format!(
                "Failed to run test {}, Error: {}",
                test.path().display(),
                err.to_string()
            ))),
        }
    }
}

/// Collects the coverage data from the launched test
fn collect_coverage(
    test_path: &Path,
    test: Pid,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
    logger: &Option<EventLog>,
) -> Result<(TraceMap, i32), RunError> {
    let mut ret_code = 0;
    let mut traces = generate_tracemap(test_path, analysis, config)?;
    {
        let span = trace_span!("Collect coverage", pid=%test);
        let _enter = span.enter();
        let (mut state, mut data) =
            create_state_machine(test, &mut traces, analysis, config, logger);
        loop {
            state = state.step(&mut data, config)?;
            if state.is_finished() {
                if let TestState::End(i) = state {
                    ret_code = i;
                }
                break;
            }
        }
    }
    Ok((traces, ret_code))
}

/// Launches the test executable
fn execute_test(test: &TestBinary, ignored: bool, config: &Config) -> Result<(), RunError> {
    let exec_path = CString::new(test.path().to_str().unwrap()).unwrap();
    info!("running {}", test.path().display());
    let _ = match test.manifest_dir() {
        Some(md) => env::set_current_dir(&md),
        None => env::set_current_dir(&config.root()),
    };

    let mut envars: Vec<CString> = Vec::new();

    for (key, value) in env::vars() {
        let mut temp = String::new();
        temp.push_str(key.as_str());
        temp.push('=');
        temp.push_str(value.as_str());
        envars.push(CString::new(temp).unwrap());
    }
    let mut argv = if ignored {
        vec![exec_path.clone(), CString::new("--ignored").unwrap()]
    } else {
        vec![exec_path.clone()]
    };
    if config.verbose {
        envars.push(CString::new("RUST_BACKTRACE=1").unwrap());
    }
    for s in &config.varargs {
        argv.push(CString::new(s.as_bytes()).unwrap_or_default());
    }
    argv.push(CString::new("--color").unwrap_or_default());
    argv.push(CString::new(config.color.to_string().to_ascii_lowercase()).unwrap_or_default());

    if let Some(s) = test.pkg_name() {
        envars.push(CString::new(format!("CARGO_PKG_NAME={}", s)).unwrap_or_default());
    }
    if let Some(s) = test.pkg_version() {
        envars.push(CString::new(format!("CARGO_PKG_VERSION={}", s)).unwrap_or_default());
    }
    if let Some(s) = test.pkg_authors() {
        envars.push(CString::new(format!("CARGO_PKG_AUTHORS={}", s.join(":"))).unwrap_or_default());
    }
    if let Some(s) = test.manifest_dir() {
        envars
            .push(CString::new(format!("CARGO_MANIFEST_DIR={}", s.display())).unwrap_or_default());
    }

    execute(exec_path, &argv, envars.as_slice())
}
