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
use std::process;
use std::io::{Error, ErrorKind};
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::HashMap;
use nix::Error as NixErr;
use nix::unistd::*;
use nix::libc::pid_t;
use nix::sys::ptrace::ptrace::*;
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


const PIE_ERROR: &'static str = "ERROR: Tarpaulin cannot find code addresses check that \
pie is disabled for your linker. If linking with gcc try adding -C link-args=-no-pie \
to your rust flags";

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
        Err(_) => { 
            println!("Invalid project directory specified");
            return;
        }
    };
    
    let rustflags = "RUSTFLAGS";
    let mut value = "-C relocation-model=dynamic-no-pic -C link-dead-code ".to_string();
    {
        let env_linker = env::var(rustflags)
                            .ok()
                            .and_then(|flags| flags.split(" ")
                                                   .map(str::trim)
                                                   .filter(|s| !s.is_empty())
                                                   .skip_while(|s| !s.contains("linker="))
                                                   .next()
                                                   .map(|s| s.trim_left_matches("linker="))
                                                   .map(|s| PathBuf::from(s)));

        let target_linker = env_linker.or_else(|| {
            fn get_target_path(cargo_config: &CargoConfig, triple: &str) -> Option<PathBuf> {
                cargo_config.get_path(&format!("target.{}.linker", triple)).unwrap().map(|v| v.val)
            }

            let host = get_target_path(&cargo_config, &cargo_config.rustc().unwrap().host);
            match cargo_config.get_string("build.target").unwrap().map(|s| s.val) {
                Some(triple) => get_target_path(&cargo_config, &triple),
                None => host,
            }
        });

        // For Linux (and most everything that isn't Windows) it is fair to
        // assume the default linker is `cc` and that `cc` is GCC based.
        let mut linker_cmd = Command::new(&target_linker.unwrap_or(PathBuf::from("cc")));
        linker_cmd.arg("-v");
        if let Ok(linker_output) = linker_cmd.output() {
            if String::from_utf8_lossy(&linker_output.stderr).contains("--enable-default-pie") {
                value.push_str("-C link-arg=-no-pie ");
            }
        }
    }
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    env::set_var(rustflags, value);

    let clean_opt = ops::CleanOptions {
        config: &cargo_config,
        spec: &[],
        target: None,
        release: false,
    };
    
    let mut copt = ops::CompileOptions::default(&cargo_config, ops::CompileMode::Test); 
    copt.features = config.features.as_slice();
    copt.spec = ops::Packages::Packages(config.packages.as_slice());
    let mut result:Vec<TracerData> = Vec::new();
    if config.verbose {
        println!("Running Tarpaulin");
    }
    // Clean isn't expected to fail and if it does it likely won't have an effect
    let _ = ops::clean(&workspace, &clean_opt);
    let compilation = ops::compile(&workspace, &copt);
    match compilation {
        Ok(comp) => {
            for c in comp.tests.iter() {
                if config.verbose {
                    println!("Processing {}", c.1);
                }
                let res = get_test_coverage(workspace.root(), c.2.as_path(),
                                            config.forward_signals, false)
                    .unwrap_or(vec![]);
                merge_test_results(&mut result, &res);
                if config.run_ignored {
                    let res = get_test_coverage(workspace.root(), c.2.as_path(), 
                                                config.forward_signals, true)
                        .unwrap_or(vec![]);
                    merge_test_results(&mut result, &res);
                }
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

/// Strips the directory the project manifest is in from the path. Provides a
/// nicer path for printing to the user.
fn strip_project_path<'a>(config: &'a Config, path: &'a Path) -> &'a Path {
    if let Some(root) = config.manifest.parent() {
        &path.strip_prefix(root).unwrap_or(path)
    } else {
        path
    }
}

/// Reports the test coverage using the users preferred method. See config.rs 
/// or help text for details.
pub fn report_coverage(config: &Config, result: &Vec<TracerData>) {
    if result.len() > 0 {
        println!("Coverage Results");
        if config.verbose {
            for r in result.iter() {
                let path = strip_project_path(config, r.path.as_path());
                println!("{}:{} - hits: {}", path.display(), r.line, r.hits);
            }
            println!("");
        }
        // Hash map of files with the value (lines covered, total lines)
        let mut file_map: HashMap<&Path, (u64, u64)> = HashMap::new();
        for r in result.iter() {
            if file_map.contains_key(r.path.as_path()) {
                if let Some(v) = file_map.get_mut(r.path.as_path()) {
                    (*v).0 += (r.hits > 0) as u64;
                    (*v).1 += 1u64;
                } 
            } else {
                file_map.insert(r.path.as_path(), ((r.hits > 0) as u64, 1));
            }
        }
        for (k, v) in file_map.iter() {
            let path = strip_project_path(config, k);
            println!("{}: {}/{}", path.display(), v.0, v.1);
        }
        let covered = result.iter().filter(|&x| (x.hits > 0 )).count();
        let total = result.iter().count();
        let percent = (covered as f64)/(total as f64) * 100.0f64;
        // Put file filtering here
        println!("\n{:.2}% coverage, {}/{} lines covered", percent, covered, total);
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
pub fn get_test_coverage(root: &Path, test: &Path, forward:bool, ignored: bool) -> Option<Vec<TracerData>> {
    if !test.exists() {
        return None;
    } 
    match fork() {
        Ok(ForkResult::Parent{ child }) => {
            match collect_coverage(root, test, child, forward) {
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
            execute_test(test, ignored, true);
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
                    test: pid_t,
                    forward_signals: bool) -> io::Result<Vec<TracerData>> {
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
                    Err(e) if e == NixErr::Sys(nix::Errno::EIO) => {
                        println!("{}", PIE_ERROR);
                        process::exit(1);
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
    match run_function(test, u64::max_value(), forward_signals, 
                       &mut traces, &mut bps) {
        Err(e) => println!("Error while collecting coverage. {}", e),
        _ => {},
    }
    Ok(traces)
}

/// Starts running a test. Child must have signalled STOP or SIGNALED to show 
/// the parent it is not executing or it will be killed.
fn run_function(pid: pid_t,
                end: u64,
                forward_signals: bool,
                mut traces: &mut Vec<TracerData>,
                mut breakpoints: &mut HashMap<u64, Breakpoint>) -> Result<i8, Error> {
    let mut res = 0i8;
    // Start the function running. 
    continue_exec(pid, None)?;
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
                    let _ =continue_exec(pid, None);
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
                        continue_exec(child, None)?;
                    }
                } 
            },
            Ok(WaitStatus::Stopped(child, signal::SIGSTOP)) => {
                continue_exec(child, None)?;
            },
            Ok(WaitStatus::Stopped(child, sig)) => {
                let s = if forward_signals {
                    Some(sig)
                } else {
                    None
                };
                continue_exec(child, s)?;
            },
            Ok(WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_CLONE)) => {
                if let Ok(_) = get_event_data(child) {
                    continue_exec(child, None)?;
                }
            },
            Ok(WaitStatus::Signaled(child, signal::SIGTRAP, true)) => {
                println!("unexpected SIGTRAP attempting to continue");
                continue_exec(child, None)?;
            },
            Ok(s) => {
                println!("Unexpected stop {:?}", s);
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
fn execute_test(test: &Path, ignored: bool, backtrace_on: bool) {
    let exec_path = CString::new(test.to_str().unwrap()).unwrap();
    match personality::disable_aslr() {
        Ok(_) => {},
        Err(e) => println!("ASLR disable failed: {}", e),
    }
    request_trace().ok()
                   .expect("Failed to trace");
    
    let mut envars: Vec<CString> = vec![CString::new("RUST_TEST_THREADS=1").unwrap()];
    for (key, value) in env::vars() {
        let mut temp = String::new();
        temp.push_str(key.as_str());
        temp.push('=');
        temp.push_str(value.as_str());
        envars.push(CString::new(temp).unwrap());
    }
    if backtrace_on {
        envars.push(CString::new("RUST_BACKTRACE=1").unwrap());
    }
    let argv = if ignored {
        vec![exec_path.clone(), CString::new("--ignored").unwrap()]
    } else {
        vec![exec_path.clone()]
    };
    execve(&exec_path, &argv, envars.as_slice())
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
