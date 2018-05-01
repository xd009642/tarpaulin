extern crate nix;
extern crate cargo;
extern crate gimli;
extern crate syntex_syntax;
extern crate object;
extern crate memmap;
extern crate coveralls_api;
extern crate fallible_iterator;
extern crate rustc_demangle;
#[macro_use]
extern crate clap;
extern crate serde;
extern crate serde_json;
extern crate quick_xml;
extern crate regex;


use std::env;
use std::io;
use std::process;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::collections::{BTreeMap, HashMap};
use nix::Error as NixErr;
use nix::unistd::*;
use nix::sys::ptrace::ptrace::*;
use nix::sys::signal;
use nix::sys::wait::*;
use cargo::util::Config as CargoConfig;
use cargo::core::{Workspace, Package};
use cargo::ops;


pub mod config;
pub mod tracer;
pub mod breakpoint;
pub mod report;
mod source_analysis;

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

pub fn run(config: Config) -> Result<(), i32> {
    let result = launch_tarpaulin(&config)?;
    report_coverage(&config, &result);
    Ok(())
}

/// Launches tarpaulin with the given configuration.
pub fn launch_tarpaulin(config: &Config) -> Result<Vec<TracerData>, i32> {
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
                                   false,
                                   &[]);
    
    let workspace = Workspace::new(config.manifest.as_path(), &cargo_config).map_err(|_| 1i32)?;
    
    setup_environment(&cargo_config);
        
    let mut copt = ops::CompileOptions::default(&cargo_config, ops::CompileMode::Test);
    match copt.filter {
        ops::CompileFilter::Default{ref mut required_features_filterable} => {
            *required_features_filterable = true;
        },
        _ => {},
    }
    copt.features = config.features.as_slice();
    copt.spec = match ops::Packages::from_flags(workspace.is_virtual(), config.all, &config.exclude, &config.packages) {
        Ok(spec) => spec,
        Err(e) => { 
            println!("Error getting Packages from workspace {}", e);
            return Err(-1)
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
            spec: &[],
            target: None,
            release: false,
        };
        let _ = ops::clean(&workspace, &clean_opt);
    }
    let mut result:Vec<TracerData> = Vec::new();
    println!("Building project");
    let compilation = ops::compile(&workspace, &copt);
    match compilation {
        Ok(comp) => {
            for &(ref package, ref _target_kind, ref name, ref path) in &comp.tests {
                if config.verbose {
                    println!("Processing {}", name);
                }
                let res = get_test_coverage(&workspace, package, path.as_path(),
                                            &config, false)
                    .unwrap_or_default();
                merge_test_results(&mut result, &res);
                if config.run_ignored {
                    let res = get_test_coverage(&workspace, package, path.as_path(),
                                                &config, true)
                        .unwrap_or_default();
                    merge_test_results(&mut result, &res);
                }
            }
            Ok(resolve_results(result))
        },
        Err(e) => {
            if config.verbose{
                println!("Error: failed to compile: {}", e);
            }
            Err(-1)
        },
    }
}


fn setup_environment(cargo_config: &CargoConfig) {
    let rustflags = "RUSTFLAGS";
    let mut value = "-C relocation-model=dynamic-no-pic -C link-dead-code ".to_string();
    let env_linker = env::var(rustflags)
                        .ok()
                        .and_then(|flags| flags.split(' ')
                                               .map(str::trim)
                                               .filter(|s| !s.is_empty())
                                               .skip_while(|s| !s.contains("linker="))
                                               .next()
                                               .map(|s| s.trim_left_matches("linker="))
                                               .map(PathBuf::from));

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
    let mut linker_cmd = Command::new(&target_linker.unwrap_or_else(|| PathBuf::from("cc")));
    linker_cmd.arg("-v");
    if let Ok(linker_output) = linker_cmd.output() {
        if String::from_utf8_lossy(&linker_output.stderr).contains("--enable-default-pie") {
            value.push_str("-C link-arg=-no-pie ");
        }
    }
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    env::set_var(rustflags, value);

}

fn resolve_results(raw_results: Vec<TracerData>) -> Vec<TracerData> {
    let mut result = Vec::new();
    let mut map = HashMap::new();
    for r in raw_results.iter() {
        map.entry((r.path.as_path(), r.line)).or_insert(vec![]).push(r);
    }
    for (_, v) in map.iter() {
        // Guaranteed to have at least one element
        let hits = v.iter().fold(0, |acc, &x| acc + x.hits);
        let mut temp = v[0].clone();
        temp.hits = hits;
        result.push(temp);
    }
    result.sort();
    result
}

/// Test artefacts may have different lines visible to them therefore for 
/// each test artefact covered we need to merge the `TracerData` entries to get
/// the overall coverage.
pub fn merge_test_results(master: &mut Vec<TracerData>, new: &[TracerData]) {
    let mut unmerged:Vec<TracerData> = Vec::new();
    for t in new.iter() {
        let mut update = master.iter_mut()
                               .filter(|x| x.path== t.path && x.line == t.line)
                               .collect::<Vec<_>>();
        for u in &mut update {
            u.hits += t.hits;
        }
        
        if update.is_empty() {
            unmerged.push(t.clone());
        }
    }
    master.append(&mut unmerged);
}

/// Strips the directory the project manifest is in from the path. Provides a
/// nicer path for printing to the user.
fn strip_project_path<'a>(config: &'a Config, path: &'a Path) -> &'a Path {
    if let Some(root) = config.manifest.parent() {
        path.strip_prefix(root).unwrap_or(path)
    } else {
        path
    }
}

/// Reports the test coverage using the users preferred method. See config.rs
/// or help text for details.
pub fn report_coverage(config: &Config, result: &[TracerData]) {
    if !result.is_empty() {
        println!("Coverage Results");
        if config.verbose {
            for r in result.iter() {
                let path = strip_project_path(config, r.path.as_path());
                println!("{}:{} - hits: {}", path.display(), r.line, r.hits);
            }
            println!("");
        }
        // Hash map of files with the value (lines covered, total lines)
        let mut file_map: BTreeMap<&Path, (u64, u64)> = BTreeMap::new();
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
        for (k, v) in &file_map {
            let path = strip_project_path(config, k);
            println!("{}: {}/{}", path.display(), v.0, v.1);
        }
        let covered = result.iter().filter(|&x| (x.hits > 0 )).count();
        let total = result.len();
        let percent = (covered as f64)/(total as f64) * 100.0f64;
        // Put file filtering here
        println!("\n{:.2}% coverage, {}/{} lines covered", percent, covered, total);
        if config.is_coveralls() {
            report::coveralls::export(result, config);
            println!("Coverage data sent");
        }

        for g in &config.generate {
            match g {
                &OutputFile::Xml => {
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
                         ignored: bool) -> Option<Vec<TracerData>> {
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
                    config: &Config) -> io::Result<Vec<TracerData>> {
    let mut traces = generate_tracer_data(project, test_path, config)?;
    let mut bps: HashMap<u64, Breakpoint> = HashMap::new();
    match waitpid(test, None) {
        Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) => {
            let child_trace = trace_children(child);
            if let Err(c) = child_trace {
                println!("Failed to trace child threads: {}", c);
            }
            for trace in &traces {
                if let Some(addr) = trace.address {
                    match Breakpoint::new(child, addr) {
                        Ok(bp) => { 
                            let _ = bps.insert(addr, bp);
                        },
                        Err(e) if e == NixErr::Sys(nix::Errno::EIO) => {
                            println!("{}", PIE_ERROR);
                            process::exit(1);
                        },
                        Err(e) => println!("Failed to instrument {}", e),
                    }
                }
            }  
        },
        Ok(_) => println!("Unexpected grab"),   
        Err(err) => println!("Error on start: {}", err)
    }
    // Now we start hitting lines!
    if let Err(e) = run_function(test, config.forward_signals, config.no_count,
                       &mut traces, &mut bps) {
        println!("Error while collecting coverage. {}", e);
    }
    Ok(traces)
}

/// Starts running a test. Child must have signalled STOP or SIGNALED to show 
/// the parent it is not executing or it will be killed.
fn run_function(pid: Pid,
                forward_signals: bool,
                no_count: bool,
                traces: &mut Vec<TracerData>,
                breakpoints: &mut HashMap<u64, Breakpoint>) -> Result<i8, NixErr> {
    let mut res = 0i8;
    // Thread count, don't count initial thread of execution
    let mut thread_count = 0isize;
    let mut unwarned = !no_count;
    // Start the function running. 
    continue_exec(pid, None)?;
    loop {
        match waitpid(Pid::from_raw(-1), Some(__WALL)) {
            Ok(WaitStatus::Exited(child, sig)) => {
                for (_, ref mut value) in breakpoints.iter_mut() {
                    value.thread_killed(child); 
                }
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
                        let bp = &mut breakpoints.get_mut(&rip).unwrap();
                        let enable = (!no_count) && (thread_count < 2);
                        if !enable && unwarned {
                            println!("Code is mulithreaded, disabling hit count");
                            unwarned = false;
                        }
                        // Don't reenable if multithreaded as can't yet sort out segfault issue
                        let updated = if let Ok(x) = bp.process(child, enable) {
                             x
                        } else {
                            false
                        };
                        if updated {
                            for t in traces.iter_mut()
                                               .filter(|x| x.address == Some(rip)) {
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
            Ok(WaitStatus::Stopped(_, signal::SIGSEGV)) => {
                break;
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
                if get_event_data(child).is_ok() {
                    thread_count += 1;
                    continue_exec(child, None)?;
                }
            },
            Ok(WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_FORK)) => {
                continue_exec(child, None)?;
            },
            Ok(WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_VFORK)) => {
                continue_exec(child, None)?;
            },
            Ok(WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_EXEC)) => {
                detach_child(child)?;
            },
            Ok(WaitStatus::PtraceEvent(child, signal::SIGTRAP, PTRACE_EVENT_EXIT)) => {
                thread_count -= 1;
                continue_exec(child, None)?;
            },
            Ok(WaitStatus::Signaled(child, signal::SIGTRAP, true)) => {
                continue_exec(child, None)?;
            },
            Ok(s) => {
                println!("Unexpected stop {:?}", s);
                break;
            },
            Err(e) => {
                return Err(e)
            },
        }
    }
    Ok(res)
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
            address: Some(0),
            trace_type: LineType::Unknown,
            hits: 1
        });
        master.push(TracerData { 
            path: PathBuf::from("testing/test.rs"),
            line: 3,
            address: Some(1),
            trace_type: LineType::Unknown,
            hits: 1
        });
        master.push(TracerData {
            path: PathBuf::from("testing/not.rs"),
            line: 2,
            address: Some(0),
            trace_type: LineType::Unknown,
            hits: 7
        });

        let other:Vec<TracerData> = vec![
            TracerData {
                path:PathBuf::from("testing/test.rs"),
                line: 2,
                address: Some(0),
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
