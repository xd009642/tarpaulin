extern crate nix;
extern crate docopt;
extern crate cargo;
extern crate rustc_serialize;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate fallible_iterator;
extern crate rustc_demangle;
extern crate regex;

use std::io;
use std::io::{Error, ErrorKind};
use std::ffi::CString;
use std::path::Path;
use std::collections::HashMap;
use nix::unistd::*;
use nix::libc::pid_t;
use nix::sys::signal;
use nix::sys::wait::*;

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
                let file = trace.path.file_name().unwrap().to_str().unwrap();
                println!("Instrumenting {}:{} - {:X}", file, trace.line, trace.address);
                match Breakpoint::new(child, trace.address) {
                    Ok(bp) => { 
                        let _ = bps.insert(trace.address, bp);
                    },
                    Err(e) => println!("Failed to instrument {}", e),
                }
            }    
        },
        Ok(_) => println!("Unexpected grab"),   
        Err(err) => println!("{}", err)
    }
    println!("Test process: {}", test);
    // Now we start hitting lines!
    let e = run_function(test, u64::max_value(), &mut traces, &mut bps);
    println!("{:?}", e);

    for t in traces.iter() {
        let file = t.path.file_name().unwrap().to_str().unwrap();
        println!("{}:{} - hits: {}", file, t.line, t.hits);
    }
    Ok(())
}


fn run_function(pid: pid_t,
                end: u64,
                mut traces: &mut Vec<TracerData>,
                mut breakpoints: &mut HashMap<u64, Breakpoint>) -> Result<i8, Error> {
    let mut res = 0i8;
    loop {
        continue_exec(pid)?;
        match waitpid(pid, None) {
            Ok(WaitStatus::Exited(_, sig)) => {
                res = sig;
                break;
            },
            Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) | 
                Ok(WaitStatus::Signaled(child, signal::SIGTRAP, true))=> {
                if let Ok(rip) = current_instruction_pointer(child) {
                    let rip = (rip - 1) as u64;
                    if let Some(ref mut bp) = breakpoints.get_mut(&rip) {
                        for mut t in traces.iter_mut()
                                       .filter(|ref x| x.address == rip) {
                            (*t).hits += 1;
                        }
                        if rip != end {
                            let _ = bp.step();
                        } else {
                            // Hit function end.
                            break;
                        }
                    }
                }

            },
            Ok(WaitStatus::Stopped(_, sig)) => {
                println!("Stopped due to {:?}", sig);
                break;
            },
            Ok(x) => println!("Unexpected: {:?}", x),
            Err(e) => {
                println!("e {}", e);
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
    
    println!("{:?}", test_entries);
    for te in test_entries.iter() {
        println!("Setting RIP");
        let _ = set_instruction_pointer(pid, te.0);
        
        let func_length:u64 = match te.1 {
            LineType::TestEntry(len) => len as u64,
            _ => 0,
        };
        println!("Running");
        match run_function(pid, te.0 + func_length, &mut traces, &mut breakpoints) {
            Ok(r) => println!("Ran function return {}", r),
            Err(e) => println!("Error running function: {}", e),
        }

        // Disable breakpoints in that function. It's a top level test and we've
        // covered it. This is likely unnecessary. But it gives me some level of
        // verification later.
        for bp in breakpoints.values() {
            if bp.pc >= te.0 && bp.pc < (te.0 + func_length) {
                let _ = bp.disable();
            }
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
