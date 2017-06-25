extern crate nix;
extern crate cargo;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate coveralls_api;
extern crate fallible_iterator;
extern crate rustc_demangle;
extern crate regex;
#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;

use std::env;
use std::io;
use std::io::{Error, ErrorKind};
use std::ffi::CString;
use std::path::Path;
use std::collections::HashMap;
use nix::unistd::*;
use nix::libc::pid_t;
use nix::sys::signal;
use nix::sys::wait::*;
use cargo::util::Config as CargoConfig;
use cargo::core::Workspace;
use cargo::ops;

pub mod config;
pub mod tracer;
pub mod breakpoint;
pub mod report;
/// Should be unnecessary with a future nix crate release.
mod personality;
mod ptrace_control;

use config::*;
use tracer::*;
use breakpoint::*;
use ptrace_control::*;

/// Launches tarpaulin with the given configuration.
pub fn launch_tarpaulin(config: Config) {
    let cargo_config = CargoConfig::default().unwrap();
    let flag_quiet = if config.verbose {
        None
    } else {
        Some(true)
    };
    // This shouldn't fail so no checking the error.
    let _ = cargo_config.configure(0u32,
                                   flag_quiet,
                                   &None,
                                   false,
                                   false);

    let workspace = match Workspace::new(config.manifest.as_path(), &cargo_config) {
        Ok(w) => w,
        Err(_) => panic!("Invalid project directory specified"),
    };

    let filter = ops::CompileFilter::Everything;
    let rustflags = "RUSTFLAGS";
    let mut value = "-Crelocation-model=dynamic-no-pic -Clink-dead-code".to_string();
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    env::set_var(rustflags, value);
    let copt = ops::CompileOptions {
        config: &cargo_config,
        jobs: None,
        target: None,
        features: &[],
        all_features: true,
        no_default_features:false ,
        spec: ops::Packages::All,
        release: false,
        mode: ops::CompileMode::Test,
        filter: filter,
        message_format: ops::MessageFormat::Human,
        target_rustdoc_args: None,
        target_rustc_args: None,
    };
    let mut result:Vec<TracerData> = Vec::new();
    if config.verbose {
        println!("Running Tarpaulin");
    }
    // TODO Determine if I should clean the target before compiling.
    let compilation = ops::compile(&workspace, &copt);
    match compilation {
        Ok(comp) => {
            for c in comp.tests.iter() {
                if config.verbose {
                    println!("Processing {}", c.1);
                }
                let res = get_test_coverage(workspace.root(), c.2.as_path())
                    .unwrap_or(vec![]);
                merge_test_results(&mut result, &res);
            }
        },
        Err(e) => {
            if config.verbose{
                println!("Error: failed to compile: {}", e);
            }
        },
    }
    report_coverage(&config, &result);
}

/// Test artefacts may have different lines visible to them therefore for 
/// each test artefact covered we need to merge the TracerData entries to get
/// the overall coverage.
pub fn merge_test_results(master: &mut Vec<TracerData>, new: &Vec<TracerData>) {
    let mut unmerged:Vec<TracerData> = Vec::new();
    for t in new.iter() {
        let mut update = master.iter_mut()
                               .filter(|x| x.path== t.path && x.line == t.line)
                               .collect::<Vec<_>>();
        for ref mut u in update.iter_mut() {
            u.hits += t.hits;
        }

        if update.iter().count() == 0 {
            unmerged.push(t.clone());
        }
    }
    master.append(&mut unmerged);
}

/// Reports the test coverage using the users preferred method. See config.rs 
/// or help text for details.
pub fn report_coverage(config: &Config, result: &Vec<TracerData>) {
    if result.len() > 0 {
        println!("Coverage Results");
        if config.verbose {
            for r in result.iter() {
                let path = if let Some(root) = config.manifest.parent() {
                    r.path.strip_prefix(root).unwrap_or(r.path.as_path())
                } else {
                    r.path.as_path()
                };
                println!("{}:{}:x{:x} - hits: {}", path.display(), r.line, r.address, r.hits);
            }
        }
        let covered = result.iter().filter(|&x| (x.hits > 0 )).count();
        let total = result.iter().count();
        println!("Total of {}/{} lines covered", covered, total);
        if config.is_coveralls() {
            println!("Sending coverage data to coveralls.io");
            report::coveralls::export(&result, config);
            println!("Coverage data sent");
        }
    } else {
        println!("No coverage results collected.");
    }

}

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(root: &Path, test: &Path) -> Option<Vec<TracerData>> {
    match fork() {
        Ok(ForkResult::Parent{ child }) => {
            match collect_coverage(root, test, child) {
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
            execute_test(test, true);
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
fn collect_coverage(project_path: &Path, 
                    test_path: &Path, 
                    test: pid_t) -> io::Result<Vec<TracerData>> {

    let mut traces = generate_tracer_data(project_path, test_path)?;
    let mut bps: HashMap<u64, Breakpoint> = HashMap::new();
    match waitpid(test, None) {
        Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) => {
            let child_trace = trace_children(child);
            if let Err(c) = child_trace {
                println!("Failed to trace child threads: {}", c);
            }
            for trace in traces.iter() {
                match Breakpoint::new(child, trace.address) {
                    Ok(bp) => { 
                        let _ = bps.insert(trace.address, bp);
                    },
                    Err(e) => println!("Failed to instrument {}", e),
                }
            }  
        },
        Ok(_) => println!("Unexpected grab"),   
        Err(err) => println!("Error on start: {}", err)
    }
    // Now we start hitting lines!
    //run_coverage_on_all_tests(test, &mut traces, &mut bps);
    match run_function(test, u64::max_value(), &mut traces, &mut bps) {
        Err(e) => println!("Error while collecting coverage. {}", e),
        _ => {},
    }
    Ok(traces)
}

/// Starts running a test. Child must have signalled STOP or SIGNALED to show 
/// the parent it is not executing or it will be killed.
fn run_function(pid: pid_t,
                end: u64,
                mut traces: &mut Vec<TracerData>,
                mut breakpoints: &mut HashMap<u64, Breakpoint>) -> Result<i8, Error> {
    let mut res = 0i8;
    // Start the function running. 
    continue_exec(pid)?;
    loop {
        match waitpid(-1, Some(__WALL)) {
            Ok(WaitStatus::Exited(child, sig)) => {
                res = sig;
                // If test executable exiting break, else continue the program
                // to launch the next test function
                if child == pid {
                    break;
                } else {
                    // The err will be no child process and means test is over.
                    let _ =continue_exec(pid);
                }
            },
            Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) => {
                if let Ok(rip) = current_instruction_pointer(child) {
                    let rip = (rip - 1) as u64;
                    if  breakpoints.contains_key(&rip) {
                        let ref mut bp = breakpoints.get_mut(&rip).unwrap();
                        let updated = if let Ok(x) = bp.process(Some(child)) {
                            x
                        } else {
                            rip == end
                        };
                        if updated {
                            for mut t in traces.iter_mut()
                                               .filter(|ref x| x.address == rip) {
                                (*t).hits += 1;
                            }
                        } 
                    } else {
                        continue_exec(child)?;
                    }
                } 
            },
            Ok(WaitStatus::Stopped(child, signal::SIGSTOP)) => {
                continue_exec(child)?;
            },
            Ok(WaitStatus::Stopped(child, sig)) => {
                println!("Unexpected signal {:?}", sig);
                continue_exec(child)?;
            },
            Ok(WaitStatus::PtraceEvent(child, signal::SIGTRAP, 3)) => {
                if let Ok(_) = get_event_data(child) {
                    continue_exec(child)?;
                }
            },
            Ok(WaitStatus::Signaled(_, signal::SIGTRAP, true)) => break,
            Ok(_) => {
                println!("Unexpected stop");
                break;
            },
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, e))
            },
        }
    }
    Ok(res)
}


/// Launches the test executable
fn execute_test(test: &Path, backtrace_on: bool) {
    let exec_path = CString::new(test.to_str().unwrap()).unwrap();
    match personality::disable_aslr() {
        Ok(_) => {},
        Err(e) => println!("ASLR disable failed: {}", e),
    }
    request_trace().ok()
                   .expect("Failed to trace");
    
    let mut envars: Vec<CString> = vec![CString::new("RUST_TEST_THREADS=1").unwrap()];
    if backtrace_on {
        envars.push(CString::new("RUST_BACKTRACE=1").unwrap());
    }
    execve(&exec_path, &[exec_path.clone()], envars.as_slice())
        .unwrap();
}


#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use ::*;
    
    #[test]
    fn result_merge_test() {
        let mut master:Vec<TracerData> = vec![];

        master.push(TracerData { 
            path: PathBuf::from("testing/test.rs"),
            line: 2,
            address: 0,
            trace_type: LineType::Unknown,
            hits: 1
        });
        master.push(TracerData { 
            path: PathBuf::from("testing/test.rs"),
            line: 3,
            address: 1,
            trace_type: LineType::Unknown,
            hits: 1
        });
        master.push(TracerData {
            path: PathBuf::from("testing/not.rs"),
            line: 2,
            address: 0,
            trace_type: LineType::Unknown,
            hits: 7
        });

        let other:Vec<TracerData> = vec![
            TracerData {
                path:PathBuf::from("testing/test.rs"),
                line: 2,
                address: 0,
                trace_type: LineType::Unknown,
                hits: 2
            }];

        merge_test_results(&mut master, &other);
        let expected = vec![3, 1, 7];
        for (act, exp) in master.iter().zip(expected) {
            assert_eq!(act.hits, exp);
        }
    }

}
