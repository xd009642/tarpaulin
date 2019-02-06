use crate::config::*;
use crate::errors::*;
use crate::ptrace_control::*;
use crate::statemachine::*;
use crate::test_loader::*;
use crate::traces::*;
use cargo::core::{compiler::CompileMode, Package, Shell, Workspace};
use cargo::ops;
use cargo::util::{homedir, Config as CargoConfig};
use log::{debug, info, warn, trace};
use nix::unistd::*;
use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};

pub mod breakpoint;
pub mod config;
pub mod errors;
pub mod report;
mod source_analysis;
mod statemachine;
pub mod test_loader;
pub mod traces;

/// Should be unnecessary with a future nix crate release.
mod personality;
mod ptrace_control;

pub fn run(config: &Config) -> Result<(), RunError> {
    let tracemap = launch_tarpaulin(config)?;
    report_coverage(config, &tracemap)?;

    Ok(())
}

/// Launches tarpaulin with the given configuration.
pub fn launch_tarpaulin(config: &Config) -> Result<TraceMap, RunError> {
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
    cargo::core::enable_nightly_features();
    // This shouldn't fail so no checking the error.
    let _ = cargo_config.configure(0u32, flag_quiet, &None, false, false, &None, &[]);

    let workspace = Workspace::new(config.manifest.as_path(), &cargo_config)
        .map_err(|e| RunError::Manifest(e.to_string()))?;

    setup_environment(&config);

    let mut copt = ops::CompileOptions::new(&cargo_config, CompileMode::Test)
        .map_err(|e| RunError::Cargo(e.to_string()))?;
    if let ops::CompileFilter::Default {
        ref mut required_features_filterable,
    } = copt.filter
    {
        *required_features_filterable = true;
    }
    copt.features = config.features.clone();
    copt.all_features = config.all_features;
    copt.no_default_features = config.no_default_features;
    copt.build_config.release = config.release;
    copt.spec = match ops::Packages::from_flags(
        config.all,
        config.exclude.clone(),
        config.packages.clone(),
    ) {
        Ok(spec) => spec,
        Err(e) => {
            return Err(RunError::Packages(e.to_string()));
        }
    };

    info!("Running Tarpaulin");

    if config.force_clean {
        debug!("Cleaning project");
        // Clean isn't expected to fail and if it does it likely won't have an effect
        let clean_opt = ops::CleanOptions {
            config: &cargo_config,
            spec: vec![],
            target: None,
            release: false,
            doc: false,
        };
        let _ = ops::clean(&workspace, &clean_opt);
    }
    let mut result = TraceMap::new();
    info!("Building project");
    let compilation = ops::compile(&workspace, &copt);
    match compilation {
        Ok(comp) => {
            for &(ref package, ref _target_kind, ref name, ref path) in &comp.tests {
                debug!("Processing {}", name);
                if let Some(res) =
                    get_test_coverage(&workspace, package, path.as_path(), config, false)?
                {
                    result.merge(&res);
                }
                if config.run_ignored {
                    if let Some(res) =
                        get_test_coverage(&workspace, package, path.as_path(), config, true)?
                    {
                        result.merge(&res);
                    }
                }
            }
            result.dedup();
            Ok(result)
        }
        Err(e) => Err(RunError::TestCompile(e.to_string())),
    }
}

fn setup_environment(config: &Config) {
    let rustflags = "RUSTFLAGS";
    let mut value =
        " -C relocation-model=dynamic-no-pic -C link-dead-code -C opt-level=0 -C debuginfo=2 "
            .to_string();
    if config.release {
        value = format!("{}-C debug-assertions=off ", value);
    }
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    env::set_var(rustflags, value);
}

fn accumulate_lines(
    (mut acc, mut group): (Vec<String>, Vec<u64>),
    next: u64,
) -> (Vec<String>, Vec<u64>) {
    if let Some(last) = group.last().cloned() {
        if next == last + 1 {
            group.push(next);
            (acc, group)
        } else {
            match (group.first(), group.last()) {
                (Some(first), Some(last)) if first == last => {
                    acc.push(format!("{}", first));
                }
                (Some(first), Some(last)) => {
                    acc.push(format!("{}-{}", first, last));
                }
                (Some(_), None) | (None, _) => (),
            };
            (acc, vec![next])
        }
    } else {
        group.push(next);
        (acc, group)
    }
}

/// Reports the test coverage using the users preferred method. See config.rs
/// or help text for details.
pub fn report_coverage(config: &Config, result: &TraceMap) -> Result<(), RunError> {
    if !result.is_empty() {
        info!("Coverage Results:");
        if config.verbose {
            println!("|| Uncovered Lines:");
            for (ref key, ref value) in result.iter() {
                let path = config.strip_project_path(key);
                let mut uncovered_lines = vec![];
                for v in value.iter() {
                    match v.stats {
                        traces::CoverageStat::Line(count) if count == 0 => {
                            uncovered_lines.push(v.line);
                        }
                        _ => (),
                    }
                }
                uncovered_lines.sort();
                let (groups, last_group) = uncovered_lines
                    .into_iter()
                    .fold((vec![], vec![]), accumulate_lines);
                let (groups, _) = accumulate_lines((groups, last_group), u64::max_value());
                if !groups.is_empty() {
                    println!("|| {}: {}", path.display(), groups.join(", "));
                }
            }
        }
        println!("|| Tested/Total Lines:");
        for file in result.files() {
            let path = config.strip_project_path(file);
            println!(
                "|| {}: {}/{}",
                path.display(),
                result.covered_in_path(&file),
                result.coverable_in_path(&file)
            );
        }
        let percent = result.coverage_percentage() * 100.0f64;
        // Put file filtering here
        println!(
            "|| \n{:.2}% coverage, {}/{} lines covered",
            percent,
            result.total_covered(),
            result.total_coverable()
        );
        if config.is_coveralls() {
            report::coveralls::export(result, config)?;
            info!("Coverage data sent");
        }

        for g in &config.generate {
            match *g {
                OutputFile::Xml => {
                    report::cobertura::report(result, config)
                        .map_err(|e| RunError::XML(e))?;
                }
                OutputFile::Html => {
                    report::html::export(result, config)?;
                }
                _ => {
                    return Err(RunError::OutFormat(
                        "Format currently unsupported".to_string(),
                    ));
                }
            }
        }

        Ok(())
    } else {
        Err(RunError::CovReport(
            "No coverage results collected.".to_string(),
        ))
    }
}

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(
    project: &Workspace,
    package: &Package,
    test: &Path,
    config: &Config,
    ignored: bool,
) -> Result<Option<TraceMap>, RunError> {
    if !test.exists() {
        return Ok(None);
    }
    match fork() {
        Ok(ForkResult::Parent { child }) => match collect_coverage(project, test, child, config) {
            Ok(t) => Ok(Some(t)),
            Err(e) => Err(RunError::TestCoverage(e.to_string())),
        },
        Ok(ForkResult::Child) => {
            info!("Launching test");
            execute_test(test, package, ignored, config)?;
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
    config: &Config,
) -> Result<TraceMap, RunError> {
    let mut traces = generate_tracemap(project, test_path, config)?;
    {
        trace!("Test PID is {}", test);
        let (mut state, mut data) = create_state_machine(test, &mut traces, config);
        loop {
            state = state.step(&mut data, config)?;
            if state.is_finished() {
                if let TestState::End(i) = state {
                    if i != 0 {
                        return Err(RunError::TestCoverage(
                            "Test binary exited with non-zero return code".to_string(),
                        ));
                    }
                }
                break;
            }
        }
    }
    Ok(traces)
}

/// Launches the test executable
fn execute_test(
    test: &Path,
    package: &Package,
    ignored: bool,
    config: &Config,
) -> Result<(), RunError> {
    let exec_path = CString::new(test.to_str().unwrap()).unwrap();
    match personality::disable_aslr() {
        Ok(_) => {}
        Err(e) => return Err(RunError::TestRuntime(format!("ASLR disable failed: {}", e))),
    }
    match request_trace() {
        Ok(_) => {}
        Err(e) => return Err(RunError::Trace(e.to_string())),
    }
    info!("running {}", test.display());
    if let Some(parent) = package.manifest_path().parent() {
        let _ = env::set_current_dir(parent);
    }

    let mut envars: Vec<CString> = vec![CString::new("RUST_TEST_THREADS=1").unwrap()];
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
    } else {
        argv.push(CString::new("--quiet").unwrap());
    }
    for s in &config.varargs {
        argv.push(CString::new(s.as_bytes()).unwrap_or_default());
    }
    execve(&exec_path, &argv, envars.as_slice()).unwrap();

    Ok(())
}
