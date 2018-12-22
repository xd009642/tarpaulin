extern crate nix;
extern crate cargo;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate coveralls_api;
extern crate fallible_iterator;
extern crate rustc_demangle;
extern crate syn;
extern crate proc_macro2;
#[macro_use]
extern crate clap;
#[macro_use]
extern crate lazy_static;
extern crate serde;
extern crate serde_json;
extern crate quick_xml;
extern crate regex;
extern crate void;
extern crate walkdir;

use std::{env, io};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use nix::unistd::*;
use cargo::util::{homedir, Config as CargoConfig};
use cargo::core::{Workspace, Package, compiler::CompileMode, Shell};
use cargo::ops;


pub mod config;
pub mod test_loader;
pub mod breakpoint;
pub mod report;
pub mod traces;
mod statemachine;
mod source_analysis;

/// Should be unnecessary with a future nix crate release.
mod personality;
mod ptrace_control;

use config::*;
use test_loader::*;
use ptrace_control::*;
use statemachine::*;
use traces::*;


/// Error states that could be returned from tarpaulin
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunError {
    /// Error in cargo manifests
    Manifest,
    /// Cargo failed to run
    Cargo,
    /// Error trying to resolve package configuration in manifest
    Packages,
    /// Tests failed to compile
    TestCompileFailed,
    /// Test failed during run
    TestRuntimeFail,
    
}


pub fn run(config: &Config) -> Result<(), RunError> {
    let (result, tp) = launch_tarpaulin(config)?;
    report_coverage(config, &result);
    if tp {
        Ok(())
    } else {
        println!("Tarpaulin ran successfully");
        Err(RunError::TestRuntimeFail)
    }
}

/// Launches tarpaulin with the given configuration.
pub fn launch_tarpaulin(config: &Config) -> Result<(TraceMap, bool), RunError> {
    let cwd = match config.manifest.parent() {
        Some(p) => p.to_path_buf(),
        None => PathBuf::new(),
    };
    let home = match homedir(&cwd) {
        Some(h) => h,
        None => {
            println!("Warning failed to find home directory.");
            PathBuf::new()
        },
    };
    let mut cargo_config = CargoConfig::new(Shell::new(), cwd, home);
    let flag_quiet = if config.verbose {
        None
    } else {
        Some(true)
    };
    cargo::core::enable_nightly_features();
    // This shouldn't fail so no checking the error.
    let _ = cargo_config.configure(0u32, flag_quiet, &None, false, false, &None, &[]);
    
    let workspace = Workspace::new(config.manifest.as_path(), &cargo_config)
        .map_err(|e| {
            println!("Cargo error: {}", e);
            RunError::Manifest
        })?;
    
    setup_environment(&config);
    
    let mut copt = ops::CompileOptions::new(&cargo_config, CompileMode::Test)
        .map_err(|_| RunError::Cargo)?;
    if let ops::CompileFilter::Default{ref mut required_features_filterable} = copt.filter {
        *required_features_filterable = true;
    }
    copt.features = config.features.clone();
    copt.all_features = config.all_features;
    copt.no_default_features = config.no_default_features;
    copt.build_config.release = config.release;
    copt.spec = match ops::Packages::from_flags(config.all, 
                                                config.exclude.clone(), 
                                                config.packages.clone()) {
        Ok(spec) => spec,
        Err(e) => {
            println!("Error getting Packages from workspace {}", e);
            return Err(RunError::Packages)
        }
    };
    if config.verbose {
        println!("Running Tarpaulin");
    }
    if !config.skip_clean {
        if config.verbose {
            println!("Cleaning project");
        }
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
    println!("Building project");
    let compilation = ops::compile(&workspace, &copt);
    let mut test_passed = true;
    match compilation {
        Ok(comp) => {
            for &(ref package, ref _target_kind, ref name, ref path) in &comp.tests {
                if config.verbose {
                    println!("Processing {}", name);
                }
                if let Some((res, tp)) = get_test_coverage(&workspace, package, path.as_path(), config, false) {
                    result.merge(&res);
                    test_passed &= tp;
                }
                if config.run_ignored {
                    if let Some((res, tp)) = get_test_coverage(&workspace, package, path.as_path(),
                                                         config, true) {
                        result.merge(&res);
                        test_passed &= tp;
                    }
                }
            }
            result.dedup();
            Ok((result, test_passed))
        },
        Err(e) => {
            if config.verbose{
                println!("Error: failed to compile: {}", e);
            }
            Err(RunError::TestCompileFailed)
        },
    }
}


fn setup_environment(config: &Config) {
    let rustflags = "RUSTFLAGS";
    let mut value = " -C relocation-model=dynamic-no-pic -C link-dead-code -C opt-level=0 -C debuginfo=2 ".to_string();
    if config.release {
        value = format!("{}-C debug-assertions=off ", value);
    }
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    env::set_var(rustflags, value);
}

fn accumulate_lines((mut acc, mut group): (Vec<String>, Vec<u64>), next: u64) -> (Vec<String>, Vec<u64>) {
    if let Some(last) = group.last().cloned() {
        if next == last + 1 {
            group.push(next);
            (acc, group)
        } else {
            match (group.first(), group.last()) {
                (Some(first), Some(last)) if first == last => {
                    acc.push(format!("{}", first));
                },
                (Some(first), Some(last)) => {
                    acc.push(format!("{}-{}", first, last));
                },
                (Some(_), None) |
                (None, _) => (),
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
pub fn report_coverage(config: &Config, result: &TraceMap) {
    if !result.is_empty() {
        println!("Coverage Results");
        if config.verbose {

            println!();
            println!("Uncovered Lines:");
            for (ref key, ref value) in result.iter() {
                let path = config.strip_project_path(key);
                let mut uncovered_lines = vec![];
                for v in value.iter() {
                    match v.stats {
                        traces::CoverageStat::Line(count) if count == 0 => {
                            uncovered_lines.push(v.line);
                        },
                        _ => (),
                    }
                }
                uncovered_lines.sort();
                let (groups, last_group) =
                    uncovered_lines.into_iter()
                    .fold((vec![], vec![]), accumulate_lines);
                let (groups, _) = accumulate_lines((groups, last_group), u64::max_value());
                if ! groups.is_empty() {
                    println!("{}: {}", path.display(), groups.join(", "));
                }
            }
            println!();
        }
        println!("Tested/Total Lines:");
        for file in result.files() {
            let path = config.strip_project_path(file);
            println!("{}: {}/{}", path.display(), result.covered_in_path(&file), result.coverable_in_path(&file));
        }
        let percent = result.coverage_percentage() * 100.0f64;
        // Put file filtering here
        println!("\n{:.2}% coverage, {}/{} lines covered", percent,
                 result.total_covered(), result.total_coverable());
        if config.is_coveralls() {
            report::coveralls::export(result, config);
            println!("Coverage data sent");
        }

        for g in &config.generate {
            match *g {
                OutputFile::Xml => {
                    report::cobertura::export(result, config);
                },
                _ => {
                    println!("Format currently unsupported");
                },
            }
        }
    } else {
        println!("No coverage results collected.");
    }

}

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(project: &Workspace,
                         package: &Package,
                         test: &Path,
                         config: &Config,
                         ignored: bool) -> Option<(TraceMap, bool)> {
    if !test.exists() {
        return None;
    }
    match fork() {
        Ok(ForkResult::Parent{ child }) => {
            match collect_coverage(project, test, child, config) {
                Ok(t) => {
                    Some(t)
                },
                Err(e) => {
                    println!("Error occurred: {}", e);
                    None
                },
            }
        }
        Ok(ForkResult::Child) => {
            println!("Launching test");
            execute_test(test, package, ignored, config);
            None
        }
        Err(err) => {
            println!("Failed to run {}", test.display());
            println!("Error {}", err);
            None
        }
    }

}

/// Collects the coverage data from the launched test
fn collect_coverage(project: &Workspace,
                    test_path: &Path,
                    test: Pid,
                    config: &Config) -> io::Result<(TraceMap, bool)> {
    let mut test_passed = false;
    let mut traces = generate_tracemap(project, test_path, config)?;
    {
        let (mut state, mut data) = create_state_machine(test, &mut traces, config);
        loop {
            state = state.step(&mut data, config);
            if state.is_finished() {
                if let TestState::End(i) = state {
                    test_passed = i==0;
                }
                break;
            }
        }
        if let Some(m) = data.error_message {
            println!("{}", m);
        }
        if state == TestState::Abort  {
            println!("Can't collect coverage data. Exiting");
            std::process::exit(1);
        }
    }
    Ok((traces, test_passed))
}

/// Launches the test executable
fn execute_test(test: &Path, package: &Package, ignored: bool, config: &Config) {
    let exec_path = CString::new(test.to_str().unwrap()).unwrap();
    match personality::disable_aslr() {
        Ok(_) => {},
        Err(e) => println!("ASLR disable failed: {}", e),
    }
    request_trace().expect("Failed to trace");
    println!("running {}", test.display());
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
    execve(&exec_path, &argv, envars.as_slice())
        .unwrap();
}

