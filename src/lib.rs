use crate::cargo::TestBinary;
use crate::config::*;
use crate::errors::*;
use crate::event_log::*;
use crate::path_utils::*;
use crate::process_handling::*;
use crate::report::report_coverage;
use crate::source_analysis::{LineAnalysis, SourceAnalysis};
use crate::test_loader::*;
use crate::traces::*;
use std::ffi::OsString;
use std::fs::{create_dir_all, remove_dir_all};
use tracing::{debug, error, info, warn};
use tracing_subscriber::{filter::LevelFilter, EnvFilter};

pub mod branching;
pub mod cargo;
pub mod config;
pub mod errors;
pub mod event_log;
pub mod path_utils;
mod process_handling;
pub mod report;
pub mod source_analysis;
pub mod statemachine;
pub mod test_loader;
pub mod traces;

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
    let filter = match std::env::var_os(RUST_LOG_ENV).map(OsString::into_string) {
        Some(Ok(env)) => {
            let mut filter = base_exceptions(EnvFilter::new(""));
            for s in env.split(',') {
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
    let logger = create_logger(configs);
    let mut tracemap = TraceMap::new();
    let mut ret = 0;
    let mut tarpaulin_result = Ok(());
    let mut bad_threshold = Ok(());
    for config in configs.iter() {
        if config.name == "report" {
            continue;
        }

        if let Some(log) = logger.as_ref() {
            let name = config_name(config);
            log.push_config(name);
        }

        create_target_dir(config);

        match launch_tarpaulin(config, &logger) {
            Ok((t, r)) => {
                ret |= r;
                if configs.len() > 1 {
                    // Otherwise threshold is a global one and we'll let the caller handle it
                    bad_threshold = check_fail_threshold(&t, config);
                }
                tracemap.merge(&t);
            }
            Err(e) => {
                error!("{}", e);
                tarpaulin_result = tarpaulin_result.and(Err(e));
            }
        }
    }

    tracemap.dedup();

    // It's OK that bad_threshold, tarpaulin_result may be overwritten in a loop
    if let Err(bad_limit) = bad_threshold {
        // Failure threshold probably more important than reporting failing
        let _ = report_coverage(&configs[0], &tracemap);
        Err(bad_limit)
    } else if ret == 0 {
        tarpaulin_result.map(|_| tracemap)
    } else {
        Err(RunError::TestFailed)
    }
}

fn create_logger(configs: &[Config]) -> Option<EventLog> {
    if configs.iter().any(|c| c.dump_traces) {
        Some(EventLog::new(configs.iter().map(|x| x.root()).collect()))
    } else {
        None
    }
}

fn create_target_dir(config: &Config) {
    let path = config.target_dir();
    if !path.exists() {
        if let Err(e) = create_dir_all(&path) {
            warn!("Failed to create target-dir {}", e);
        }
    }
}

fn config_name(config: &Config) -> String {
    if config.name.is_empty() {
        "<anonymous>".to_string()
    } else {
        config.name.clone()
    }
}

fn check_fail_threshold(traces: &TraceMap, config: &Config) -> Result<(), RunError> {
    let percent = traces.coverage_percentage() * 100.0;
    match config.fail_under.as_ref() {
        Some(limit) if percent < *limit => {
            let error = RunError::BelowThreshold(percent, *limit);
            error!("{}", error);
            Err(error)
        }
        _ => Ok(()),
    }
}

pub fn run(configs: &[Config]) -> Result<(), RunError> {
    if configs.iter().any(|x| x.engine() == TraceEngine::Llvm) {
        let profraw_dir = configs[0].profraw_dir();
        let _ = remove_dir_all(&profraw_dir);
        if let Err(e) = create_dir_all(&profraw_dir) {
            warn!(
                "Unable to create profraw directory in tarpaulin's target folder: {}",
                e
            );
        }
    }
    let tracemap = collect_tracemap(configs)?;
    report_tracemap(configs, tracemap)
}

fn collect_tracemap(configs: &[Config]) -> Result<TraceMap, RunError> {
    let mut tracemap = trace(configs)?;
    if !configs.is_empty() {
        // Assumption: all configs are for the same project
        for dir in get_source_walker(&configs[0]) {
            tracemap.add_file(dir.path());
        }
    }

    Ok(tracemap)
}

fn report_tracemap(configs: &[Config], tracemap: TraceMap) -> Result<(), RunError> {
    let mut reported = false;
    for c in configs.iter() {
        if c.no_run || c.name != "report" {
            continue;
        }

        report_coverage_with_check(c, &tracemap)?;
        reported = true;
    }

    if !reported && !configs.is_empty() && !configs[0].no_run {
        report_coverage_with_check(&configs[0], &tracemap)?;
    }

    Ok(())
}

fn report_coverage_with_check(c: &Config, tracemap: &TraceMap) -> Result<(), RunError> {
    report_coverage(c, tracemap)?;
    check_fail_threshold(tracemap, c)
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
        for exe in &executables.test_binaries {
            if exe.should_panic() {
                info!("Running a test executable that is expected to panic");
            }
            let coverage = get_test_coverage(
                exe,
                &executables.binaries,
                &project_analysis,
                config,
                false,
                logger,
            )?;
            if let Some(res) = coverage {
                result.merge(&res.0);
                return_code |= if exe.should_panic() {
                    (res.1 == 0).into()
                } else {
                    res.1
                };
            }
            if config.run_ignored {
                let coverage = get_test_coverage(
                    exe,
                    &executables.binaries,
                    &project_analysis,
                    config,
                    true,
                    logger,
                )?;
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
