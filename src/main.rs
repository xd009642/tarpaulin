extern crate nix;
extern crate libc;
extern crate docopt;
extern crate cargo;
extern crate rustc_serialize;

use std::ffi::CString;
use docopt::Docopt;
use std::path::{Path, PathBuf};
use rustc_serialize::json::Json;
use nix::sys::signal::*;
use nix::unistd::*;
use cargo::util::Config;
use cargo::core::Workspace;
use cargo::ops;

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
            analyse_coverage(c.2.as_path());
        }
    }
}


fn analyse_coverage(test: &Path) {
    let mut executable = test.to_str()
                             .unwrap()
                             .as_bytes()
                             .to_vec();
    executable.insert(0, '.' as u8);
    let exec_path = &CString::new(executable).unwrap();
    execve(exec_path, &[], &[]);
}
