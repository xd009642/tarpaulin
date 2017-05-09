extern crate nix;
extern crate cargo;
extern crate rustc_serialize;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate fallible_iterator;
extern crate rustc_demangle;
extern crate regex;
#[macro_use]
extern crate clap;

use std::io;
use std::io::{Error, ErrorKind};
use std::ffi::CString;
use std::path::Path;
use std::collections::HashMap;
use nix::unistd::*;
use nix::libc::pid_t;
use nix::sys::signal;
use nix::sys::wait::*;

pub mod config;
pub mod tracer;
pub mod collectors;
pub mod breakpoint;
/// Should be unnecessary with a future nix crate release.
mod personality;
mod ptrace_control;

use tracer::*;
use breakpoint::*;
use ptrace_control::*;

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
           // let _ = continue_exec(child);
        },
        Ok(_) => println!("Unexpected grab"),   
        Err(err) => println!("Error on start: {}", err)
    }
    // Now we start hitting lines!
    //let e = run_function(test, u64::max_value(), &mut traces, &mut bps);
    tests_mod_coverage(test, &mut traces, &mut bps);

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
        //continue_exec(pid)?;
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


fn tests_mod_coverage(pid: pid_t,
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
