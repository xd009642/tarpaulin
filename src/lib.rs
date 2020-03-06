use crate::config::*;
use crate::errors::*;
use crate::process_handling::*;
use crate::report::report_coverage;
use crate::source_analysis::LineAnalysis;
use crate::statemachine::*;
use crate::test_loader::*;
use crate::traces::*;
use cargo::core::{
    compiler::{CompileMode, ProfileKind},
    Package, Shell, Workspace,
};
use cargo::ops;
use cargo::ops::{
    clean, compile, CleanOptions, CompileFilter, CompileOptions, FilterRule, LibRule, Packages,
    TestOptions,
};
use cargo::util::{homedir, Config as CargoConfig};
use log::{debug, info, trace, warn};
use nix::unistd::*;
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub mod breakpoint;
pub mod config;
pub mod errors;
mod process_handling;
pub mod report;
mod source_analysis;
mod statemachine;
pub mod test_loader;
pub mod traces;

mod ptrace_control;

static DOCTEST_FOLDER: &str = "target/doctests";

pub fn trace(configs: &[Config]) -> Result<TraceMap, RunError> {
    let mut tracemap = TraceMap::new();
    let mut ret = 0i32;
    let mut failure = Ok(());

    for config in configs.iter() {
        if config.name == "report" {
            continue;
        }
        //let result = result?;
        match launch_tarpaulin(config) {
            Ok((t, r)) => {
                tracemap.merge(&t);
                ret |= r;
            }
            Err(e) => {
                info!("Failure {}", e);
                if failure.is_ok() {
                    failure = Err(e);
                }
            }
        }
    }
    tracemap.dedup();
    if ret == 0 {
        Ok(tracemap)
    } else {
        Err(RunError::TestFailed)
    }
}

pub fn run(configs: &[Config]) -> Result<(), RunError> {
    let tracemap = trace(configs)?;
    if configs.len() == 1 {
        report_coverage(&configs[0], &tracemap)?;
    } else if !configs.is_empty() {
        let mut reported = false;
        for c in configs.iter() {
            if c.name == "report" {
                reported = true;
                report_coverage(c, &tracemap)?;
            }
        }
        if !reported {
            report_coverage(&configs[0], &tracemap)?;
        }
    }

    Ok(())
}

/// Launches tarpaulin with the given configuration.
pub fn launch_tarpaulin(config: &Config) -> Result<(TraceMap, i32), RunError> {
    if !config.name.is_empty() {
        info!("Running config {}", config.name);
    }
    setup_environment(&config);
    cargo::core::enable_nightly_features();
    let cwd = match config.manifest.parent() {
        Some(p) => p.to_path_buf(),
        None => PathBuf::new(),
    };
    let home = match homedir(&cwd) {
        Some(h) => h,
        None => {
            warn!("Warning failed to find home directory.");
            PathBuf::new()
        }
    };
    let mut cargo_config = CargoConfig::new(Shell::new(), cwd, home);
    let flag_quiet = if config.verbose { None } else { Some(true) };

    // This shouldn't fail so no checking the error.
    let _ = cargo_config.configure(
        0u32,
        flag_quiet,
        &None,
        config.frozen,
        config.locked,
        config.offline,
        &config.target_dir,
        &config.unstable_features,
    );

    let workspace = Workspace::new(config.manifest.as_path(), &cargo_config)
        .map_err(|e| RunError::Manifest(e.to_string()))?;

    let mut compile_options = get_compile_options(&config, &cargo_config)?;

    info!("Running Tarpaulin");

    if config.force_clean {
        debug!("Cleaning project");
        // Clean isn't expected to fail and if it does it likely won't have an effect
        let clean_opt = CleanOptions {
            config: &cargo_config,
            spec: vec![],
            target: None,
            profile_specified: config.force_clean,
            profile_kind: ProfileKind::Dev,
            doc: false,
        };
        let _ = clean(&workspace, &clean_opt);
    }
    let mut result = TraceMap::new();
    let mut return_code = 0i32;
    let project_analysis = source_analysis::get_line_analysis(&workspace, config);
    info!("Building project");
    for copt in compile_options.drain(..) {
        let run_result = match copt.build_config.mode {
            CompileMode::Build | CompileMode::Test | CompileMode::Bench => {
                run_tests(&workspace, copt, &project_analysis, config)
            }
            CompileMode::Doctest => run_doctests(&workspace, copt, &project_analysis, config),
            e => {
                debug!("Internal tarpaulin error. Unsupported compile mode {:?}", e);
                Err(RunError::Internal)
            }
        }?;
        result.merge(&run_result.0);
        return_code |= run_result.1;
    }
    result.dedup();
    Ok((result, return_code))
}

fn run_tests(
    workspace: &Workspace,
    compile_options: CompileOptions,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
) -> Result<(TraceMap, i32), RunError> {
    let mut result = TraceMap::new();
    let mut return_code = 0i32;
    let compilation = compile(&workspace, &compile_options);
    match compilation {
        Ok(comp) => {
            if config.no_run {
                info!("Project compiled successfully");
                return Ok((result, return_code));
            }
            // Examples are always in the binaries list with tests!
            if config
                .run_types
                .iter()
                .any(|x| !(*x == RunType::Tests || *x == RunType::Doctests))
            {
                // If we have binaries we have other artefacts to run
                for binary in comp.binaries {
                    if let Some(res) = get_test_coverage(
                        &workspace,
                        None,
                        binary.as_path(),
                        analysis,
                        config,
                        false,
                        false,
                    )? {
                        result.merge(&res.0);
                        return_code |= res.1;
                    }
                }
            }
            for &(ref package, ref name, ref path) in &comp.tests {
                debug!("Processing {}", name);
                if let Some(res) = get_test_coverage(
                    &workspace,
                    Some(package),
                    path.as_path(),
                    analysis,
                    config,
                    true,
                    false,
                )? {
                    result.merge(&res.0);
                    return_code |= res.1;
                }
                if config.run_ignored {
                    if let Some(res) = get_test_coverage(
                        &workspace,
                        Some(package),
                        path.as_path(),
                        analysis,
                        config,
                        true,
                        true,
                    )? {
                        result.merge(&res.0);
                        return_code |= res.1;
                    }
                }
            }
            result.dedup();
            Ok((result, return_code))
        }
        Err(e) => return Err(RunError::TestCompile(e.to_string())),
    }
}

fn run_doctests(
    workspace: &Workspace,
    compile_options: CompileOptions,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
) -> Result<(TraceMap, i32), RunError> {
    info!("Running doctests");
    let mut result = TraceMap::new();
    let mut return_code = 0i32;

    let opts = TestOptions {
        no_run: false,
        no_fail_fast: false,
        compile_opts: compile_options,
    };
    let _ = ops::run_tests(workspace, &opts, &[]);

    let mut packages: Vec<PathBuf> = workspace
        .members()
        .filter_map(|p| p.manifest_path().parent())
        .map(|x| x.join(DOCTEST_FOLDER))
        .collect();

    if packages.is_empty() {
        let doctest_dir = match config.manifest.parent() {
            Some(p) => p.join(DOCTEST_FOLDER),
            None => PathBuf::from(DOCTEST_FOLDER),
        };
        packages.push(doctest_dir);
    }

    for dir in &packages {
        let walker = WalkDir::new(dir).into_iter();
        for dt in walker
            .filter_map(|e| e.ok())
            .filter(|e| match e.metadata() {
                Ok(ref m) if m.is_file() && m.len() != 0 => true,
                _ => false,
            })
        {
            if let Some(res) =
                get_test_coverage(&workspace, None, dt.path(), analysis, config, true, false)?
            {
                result.merge(&res.0);
                return_code |= res.1;
            }
        }
    }
    result.dedup();
    Ok((result, return_code))
}

fn get_compile_options<'a>(
    config: &Config,
    cargo_config: &'a CargoConfig,
) -> Result<Vec<CompileOptions<'a>>, RunError> {
    let mut result = Vec::new();
    for run_type in &config.run_types {
        let mut copt = CompileOptions::new(cargo_config, (*run_type).into())
            .map_err(|e| RunError::Cargo(e.to_string()))?;
        if run_type == &RunType::Tests {
            if let CompileFilter::Default {
                ref mut required_features_filterable,
            } = copt.filter
            {
                *required_features_filterable = true;
            }
        } else if run_type == &RunType::Doctests {
            copt.filter = CompileFilter::new(
                LibRule::True,
                FilterRule::Just(vec![]),
                FilterRule::Just(vec![]),
                FilterRule::Just(vec![]),
                FilterRule::Just(vec![]),
            );
        } else if run_type == &RunType::Examples {
            copt.filter = CompileFilter::new(
                LibRule::True,
                FilterRule::Just(vec![]),
                FilterRule::Just(vec![]),
                FilterRule::All,
                FilterRule::Just(vec![]),
            );
        }

        copt.features = config.features.clone();
        copt.all_features = config.all_features;
        copt.no_default_features = config.no_default_features;
        copt.build_config.profile_kind = match config.release {
            true => ProfileKind::Release,
            false => ProfileKind::Dev,
        };
        copt.spec =
            match Packages::from_flags(config.all, config.exclude.clone(), config.packages.clone())
            {
                Ok(spec) => spec,
                Err(e) => {
                    return Err(RunError::Packages(e.to_string()));
                }
            };
        result.push(copt);
    }
    Ok(result)
}

fn setup_environment(config: &Config) {
    env::set_var("TARPAULIN", "1");
    let common_opts =
        " -C relocation-model=dynamic-no-pic -C link-dead-code -C opt-level=0 -C debuginfo=2 ";
    let rustflags = "RUSTFLAGS";
    let mut value = common_opts.to_string();
    if config.release {
        value = format!("{}-C debug-assertions=off ", value);
    }
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    env::set_var(rustflags, value);
    // doesn't matter if we don't use it
    let rustdoc = "RUSTDOCFLAGS";
    let mut value = format!(
        "{} --persist-doctests {} -Z unstable-options ",
        common_opts, DOCTEST_FOLDER
    );
    if let Ok(vtemp) = env::var(rustdoc) {
        if !vtemp.contains("--persist-doctests") {
            value.push_str(vtemp.as_ref());
        }
    }
    env::set_var(rustdoc, value);
}

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(
    project: &Workspace,
    package: Option<&Package>,
    test: &Path,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
    can_quiet: bool,
    ignored: bool,
) -> Result<Option<(TraceMap, i32)>, RunError> {
    if !test.exists() {
        return Ok(None);
    }
    if let Err(e) = limit_affinity() {
        warn!("Failed to set processor affinity {}", e);
    }
    match fork() {
        Ok(ForkResult::Parent { child }) => {
            match collect_coverage(project, test, child, analysis, config) {
                Ok(t) => Ok(Some(t)),
                Err(e) => Err(RunError::TestCoverage(e.to_string())),
            }
        }
        Ok(ForkResult::Child) => {
            info!("Launching test");
            execute_test(test, package, ignored, can_quiet, config)?;
            Ok(None)
        }
        Err(err) => Err(RunError::TestCoverage(format!(
            "Failed to run test {}, Error: {}",
            test.display(),
            err.to_string()
        ))),
    }
}

/// Collects the coverage data from the launched test
fn collect_coverage(
    project: &Workspace,
    test_path: &Path,
    test: Pid,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
) -> Result<(TraceMap, i32), RunError> {
    let mut ret_code = 0;
    let mut traces = generate_tracemap(project, test_path, analysis, config)?;
    {
        trace!("Test PID is {}", test);
        let (mut state, mut data) = create_state_machine(test, &mut traces, config);
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
fn execute_test(
    test: &Path,
    package: Option<&Package>,
    ignored: bool,
    can_quiet: bool,
    config: &Config,
) -> Result<(), RunError> {
    let exec_path = CString::new(test.to_str().unwrap()).unwrap();
    info!("running {}", test.display());
    if let Some(pack) = package {
        if let Some(parent) = pack.manifest_path().parent() {
            let _ = env::set_current_dir(parent);
        }
    }

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
    } else if can_quiet {
        argv.push(CString::new("--quiet").unwrap());
    }
    for s in &config.varargs {
        argv.push(CString::new(s.as_bytes()).unwrap_or_default());
    }

    execute(exec_path, &argv, envars.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_env() {
        let conf = Config::default();
        setup_environment(&conf);

        let tarp_var = env::var("TARPAULIN").unwrap();
        assert_eq!(tarp_var, "1");
    }
}
