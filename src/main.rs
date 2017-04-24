extern crate nix;
extern crate docopt;
extern crate cargo;
extern crate rustc_serialize;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate fallible_iterator;
extern crate cpp_demangle;

use std::io;
use memmap::{Mmap, Protection};
use object::Object;
use std::fs::File;
use std::ffi::CString;
use docopt::Docopt;
use std::path::Path;
use nix::sys::signal;
use nix::unistd::*;
use nix::libc::pid_t;
use nix::sys::wait::*;
use nix::sys::ptrace::*;
use nix::sys::ptrace::ptrace::*;
use cargo::util::Config;
use cargo::core::Workspace;
use fallible_iterator::FallibleIterator;
use cpp_demangle::Symbol;
use cargo::ops;
use std::ptr;

const USAGE: &'static str = "
Tarpaulin - a cargo code coverage tool

Usage: 
    cargo-tarpaulin [options]
    cargo-tarpaulin (-h | --help)

Options:
    -h, --help                  Show this message.
    -l, --line                  Collect line coverage.
    -b, --branch                Collect branch coverage.
    -c, --condition             Collect condition coverage.
    --out ARG                   Specify output type [default: Report].
    -v, --verbose               Show extra output.
    -m ARG, --manifest ARG      Path to a cargo.toml to execute tarpaulin on. 
                                Default is current directory

";

#[derive(RustcDecodable, Debug)]
enum Out {
    Json,
    Toml,
    Report
}

#[derive(RustcDecodable, Debug)]
struct Args {
    flag_line: bool,
    flag_branch: bool,
    flag_condition:bool,
    flag_verbose: bool,
    flag_out: Option<Out>,
    flag_manifest: Option<String>,
}

fn main() {
    let args:Args = Docopt::new(USAGE)
                           .and_then(|d| d.decode())
                           .unwrap_or_else(|e| e.exit());
   
    let mut path = std::env::current_dir().unwrap();

    if let Some(p) = args.flag_manifest {
        path.push(p);
    };
    path.push("Cargo.toml");
    
    let config = Config::default().unwrap();
    let workspace =match  Workspace::new(path.as_path(), &config) {
        Ok(w) => w,
        Err(_) => panic!("Invalid project directory specified"),
    };
    for m in workspace.members() {
        println!("{:?}", m.manifest_path());
    }

    let filter = ops::CompileFilter::Everything;

    let copt = ops::CompileOptions {
        config: &config,
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
    // Do I need to clean beforehand?
    if let Ok(comp) = ops::compile(&workspace, &copt) {
        for c in comp.tests.iter() {
            match fork() {
                Ok(ForkResult::Parent{ child }) => {
                    match collect_coverage(c.2.as_path(), child) {
                        Ok(_) => println!("Coverage successful"),
                        Err(e) => println!("Error occurred: \n{}", e),
                    }
                }
                Ok(ForkResult::Child) => {
                    execute_test(c.2.as_path(), true);
                }
                Err(err) => { 
                    println!("Failed to run {}", c.2.display());
                    println!("Error {}", err);
                }
            }
        }
    }
}

fn parse_object_file<Endianness>(obj: &object::File) 
    where Endianness: gimli::Endianity {

    let debug_info = obj.get_section(".debug_info").unwrap_or(&[]);
    let debug_info = gimli::DebugInfo::<Endianness>::new(debug_info);
    let debug_abbrev = obj.get_section(".debug_abbrev").unwrap_or(&[]);
    let debug_abbrev = gimli::DebugAbbrev::<Endianness>::new(debug_abbrev);
    // Used to map functions to location in source.
    let debug_line = obj.get_section(".debug_line").unwrap_or(&[]);
    let debug_line = gimli::DebugLine::<Endianness>::new(debug_line);
    let debug_string = obj.get_section(".debug_str").unwrap_or(&[]);
    let debug_string = gimli::DebugStr::<Endianness>::new(debug_string);
    // This is the root compilation unit. 
    // This should be the one for the executable. Rest should be rust-buildbot 
    // and rust core for test executables. 
    // WARNING: This is an assumption based on analysis
    if let Some(root) = debug_info.units().nth(0).unwrap() {
        println!("Searching for functions!");
        // We now follow all namespaces down and log all DW_TAG_subprograms as 
        // these are function entry points
        let abbreviations = root.abbreviations(debug_abbrev).unwrap();
        let mut cursor = root.entries(&abbreviations);
        let _ = cursor.next_entry();
        let mut accumulator: isize = 0;
        while let Some((delta, node)) = cursor.next_dfs().expect("Parsing failed") {
            accumulator += delta;
            if accumulator < 0 {
                //skipped to next CU
                break;
            }
            
            if node.tag() == gimli::DW_TAG_subprogram {
                if let Ok(Some(at)) = node.attr(gimli::DW_AT_linkage_name) {
                    if let Some(st) = at.string_value(&debug_string) {
                        match Symbol::new(st.to_bytes()) {
                            Ok(x) => {
                                println!("{}", x);
                            },
                            _ => {},
                        }
                    }
                }
            }
            
        }
        // We now have all our functions.
    } else {
        println!("Root was NONE");
    }
}

fn generate_hook_addresses(test: &Path) -> io::Result<()> {
    println!("Finding hook addresses");
    let file = File::open(test)?;
    let file = Mmap::open(&file, Protection::Read)?;
    if let Ok(obj) = object::File::parse(unsafe {file.as_slice() }) {
        
        if obj.is_little_endian() {
            parse_object_file::<gimli::LittleEndian>(&obj);
        } else {
            parse_object_file::<gimli::BigEndian>(&obj);
        }
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    }
}

fn collect_coverage(test_path: &Path, test: pid_t) -> io::Result<()> {
    generate_hook_addresses(test_path)?;
    
    match waitpid(test, None) {
        Ok(WaitStatus::Stopped(child, signal::SIGTRAP)) => {
            println!("Running test without analysing for now");
            // Use PTRACE_POKETEXT here to attach software breakpoints to lines 
            // we need to cover
            ptrace(PTRACE_CONT, child, ptr::null_mut(), ptr::null_mut())
                .ok()
                .expect("Failed to continue test");
        }
        Ok(_) => {
            println!("Unexpected grab");
        }
        Err(err) => println!("{}", err)
    }
    Ok(())
}

fn execute_test(test: &Path, backtrace_on: bool) {
    
    let exec_path = CString::new(test.to_str().unwrap()).unwrap();

    ptrace(PTRACE_TRACEME, 0, ptr::null_mut(), ptr::null_mut())
        .ok()
        .expect("Failed to trace");

    let envars: Vec<CString> = if backtrace_on {
        vec![CString::new("RUST_BACKTRACE=1").unwrap()]
    } else {
        vec![]
    };
    execve(&exec_path, &[exec_path.clone()], envars.as_slice())
        .unwrap();
}
