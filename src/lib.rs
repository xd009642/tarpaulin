extern crate nix;
extern crate cargo;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate fallible_iterator;
extern crate rustc_demangle;
extern crate regex;
#[macro_use]
extern crate clap;

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
pub mod collectors;
pub mod breakpoint;
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
    let workspace =match Workspace::new(config.manifest.as_path(), &cargo_config) {
        Ok(w) => w,
        Err(_) => panic!("Invalid project directory specified"),
    };
    for m in workspace.members() {
        println!("{:?}", m.manifest_path());
    }

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
    // TODO Determine if I should clean the target before compiling.
    let compilation = ops::compile(&workspace, &copt);
    match compilation {
        Ok(comp) => {
            println!("Running Tarpaulin");
            for c in comp.tests.iter() {
                println!("Processing {}", c.1);
                get_test_coverage(workspace.root(), c.2.as_path());
            }
        },
        Err(e) => println!("Failed to compile: {}", e),
    }
}


/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(root: &Path, test: &Path) {
    match fork() {
        Ok(ForkResult::Parent{ child }) => {
            match collect_coverage(root, test, child) {
                Ok(_) => println!("Coverage successful"),
                Err(e) => println!("Error occurred: {}", e),
            }
        }
        Ok(ForkResult::Child) => {
            execute_test(test, true);
        }
        Err(err) => { 
            println!("Failed to run {}", test.display());
            println!("Error {}", err);
        }
    }
}

/// Collects the coverage data from the launched test
fn collect_coverage(project_path: &Path, 
                    test_path: &Path, 
                    test: pid_t) -> io::Result<()> {
    let mut traces = generate_tracer_data(project_path, test_path)?;
    let mut bps: HashMap<u64, Breakpoint> = HashMap::new();
    
    match waitpid(test, None) {
        Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) => {
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
    run_coverage_on_all_tests(test, &mut traces, &mut bps);

    for t in traces.iter() {
        let file = t.path.file_name().unwrap().to_str().unwrap();
        println!("{}:{} - hits: {}", file, t.line, t.hits);
    } 
    let covered = traces.iter().filter(|&x| (x.hits > 0 )).count();
    let total = traces.iter().count();
    println!("{}/{} lines covered", covered, total);
    Ok(())
}

/// Starts running a test. Child must have signalled STOP or SIGNALED to show the
/// parent it is not executing or it will be killed.
fn run_function(pid: pid_t,
                end: u64,
                mut traces: &mut Vec<TracerData>,
                mut breakpoints: &mut HashMap<u64, Breakpoint>) -> Result<i8, Error> {
    let mut res = 0i8;
    // Start the function running. 
    continue_exec(pid)?;
    loop {
        match waitpid(pid, None) {
            Ok(WaitStatus::Exited(_, sig)) => {
                res = sig;
                break;
            },
            Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) => {
                if let Ok(rip) = current_instruction_pointer(child) {
                    let rip = (rip - 1) as u64;
                    
                    if let Some(ref mut bp) = breakpoints.get_mut(&rip) {
                        
                        let updated = if let Ok(x) = bp.process() {
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
                    }
                    else if rip == end {
                        // test over. Leave run function.
                        break;
                    } else {
                        continue_exec(pid)?;
                    }
                } 
            },
            Ok(WaitStatus::Stopped(_, _)) => {
                break;
            },
            Ok(WaitStatus::Signaled(_, signal::SIGTRAP, true)) => println!("Child killed"),
            Ok(x) => println!("Unexpected: {:?}", x),
            Err(e) => {
                return Err(Error::new(ErrorKind::Other, e))
            },
        }
    }
    Ok(res)
}

/// Tests the coverage of all identified tests
fn run_coverage_on_all_tests(pid: pid_t,
                      mut traces: &mut Vec<TracerData>,
                      mut breakpoints: &mut HashMap<u64, Breakpoint>) {

    let is_test = | t: &TracerData | {
        match t.trace_type {
            LineType::TestEntry(_) => true,
            _ => false,
        }
    };

    let test_entries = traces.iter()
                             .filter(|t| is_test(t))
                             .map(|t| (t.address, t.trace_type))
                             .collect::<Vec<(u64, LineType)>>();
    
    for te in test_entries.iter() {
        let _ = set_instruction_pointer(pid, te.0);
        
        let func_end:u64 = match te.1 {
            LineType::TestEntry(len) => te.0 + len as u64,
            _ => u64::max_value(),
        };
        match run_function(pid, func_end, &mut traces, &mut breakpoints) {
            Ok(_) => {},
            Err(e) => println!("Error running function: {}", e),
        }
    }
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
