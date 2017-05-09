extern crate cargo_tarpaulin;
extern crate nix;
extern crate gimli;
extern crate object;
extern crate memmap;
extern crate fallible_iterator;
extern crate rustc_demangle;
#[macro_use]
extern crate clap;

use std::path::Path;
use clap::{App, Arg, SubCommand};
use cargo_tarpaulin::launch_tarpaulin;
use cargo_tarpaulin::config::*;


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
    let config = Config::from_args(args);
    launch_tarpaulin(config);
}
