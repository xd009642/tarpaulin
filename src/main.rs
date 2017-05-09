extern crate cargo_tarpaulin;
extern crate nix;
extern crate cargo;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate fallible_iterator;
extern crate rustc_demangle;
#[macro_use]
extern crate clap;

use std::env;
use std::path::Path;
use clap::{App, Arg, SubCommand};
use cargo_tarpaulin::{get_test_coverage};
use cargo_tarpaulin::config::*;
use cargo::util::Config as CargoConfig;
use cargo::core::Workspace;
use cargo::ops;


fn is_dir(d: String) -> Result<(), String> {
    if Path::new(&d).is_dir() {
        Ok(())
    } else {
        Err(String::from("root must be a directory"))
    }
}


fn main() {
    let args = App::new("cargo-tarpaulin")
        .author("Daniel McKenna, <danielmckenna93@gmail.com>")
        .about("Tool to analyse test coverage of cargo projects")
        .version(concat!("version: ", crate_version!()))
        .bin_name("cargo")
        .subcommand(SubCommand::with_name("tarpaulin")
            .about("Tool to analyse test coverage of cargo projects")
            .args_from_usage(
                "--help -h    'Prints help information'
                 --verbose -v 'Show extra output'
                 --line -l    'Line coverage: UNSTABLE'
                 --branch -b  'Branch coverage: NOT IMPLEMENTED'")
            .args(&[
                Arg::from_usage("--out -o [FMT]   'Output format'")
                    .possible_values(&OutputFile::variants())
                    .multiple(true),
                Arg::from_usage("--root -r [DIR]  'Root directory containing Cargo.toml to use'")
                    .validator(is_dir)
            ]))
        .get_matches();
    let args = args.subcommand_matches("tarpaulin").unwrap_or(&args);
    let tarp_config = Config::from_args(args);
    
    let config = CargoConfig::default().unwrap();
    let workspace =match Workspace::new(tarp_config.manifest.as_path(), &config) {
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
