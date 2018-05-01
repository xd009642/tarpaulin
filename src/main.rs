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
use clap::{App, Arg, SubCommand, ArgSettings};
use cargo_tarpaulin::run;
use cargo_tarpaulin::config::*;


fn is_dir(d: String) -> Result<(), String> {
    if Path::new(&d).is_dir() {
        Ok(())
    } else {
        Err(String::from("root must be a directory"))
    }
}

const CI_SERVER_HELP: &'static str = 
"Name of service, supported services are:
travis-ci, travis-pro, circle-ci, semaphore, jenkins and codeship.
If you are interfacing with coveralls.io or another site you can \
also specify a name that they will recognise. Refer to their documentation for this.";


fn main() {
    let args = App::new("cargo-tarpaulin")
        .author("Daniel McKenna, <danielmckenna93@gmail.com>")
        .about("Tool to analyse test coverage of cargo projects")
        .version(concat!("version: ", crate_version!()))
        .bin_name("cargo")
        .subcommand(SubCommand::with_name("tarpaulin")
            .about("Tool to analyse test coverage of cargo projects")
            .version(concat!("version: ", crate_version!()))
            .args_from_usage(
                 "--verbose -v 'Show extra output'
                 --ignore-tests 'ignore lines of test functions when collecting coverage'
                 --no-count   'Disables counting line hits for a faster run'
                 --ignored -i 'Run ignored tests as well'
                 --line -l    'Line coverage'
                 --skip-clean 'Skips the clean stage to reduce build times, may affect coverage results'
                 --branch -b  'Branch coverage: NOT IMPLEMENTED'
                 --forward -f 'Forwards unexpected signals to test. Tarpaulin will still take signals it is expecting.'
                 --coveralls [KEY]  'Coveralls key, either the repo token, or if you're using travis use $TRAVIS_JOB_ID and specify travis-{ci|pro} in --ciserver'
                 --report-uri [URI] 'URI to send report to, only used if the option --coveralls is used'
                 --features [FEATURE]... 'Features to be included in the target project'
                 --all        'Build all packages in the workspace'
                 --packages -p [PACKAGE]... 'Package id specifications for which package should be build. See cargo help pkgid for more info'
                 --exclude -e [PACKAGE]... 'Package id specifications to exclude from coverage. See cargo help pkgid for more info'")
            .args(&[
                Arg::from_usage("--out -o [FMT]   'Output path'")
                    .possible_values(&OutputFile::variants())
                    .multiple(true),
                Arg::from_usage("--root -r [DIR]  'Root directory containing Cargo.toml to use'")
                    .validator(is_dir),
                Arg::from_usage("--ciserver [SERVICE] 'CI server being used'")
                    .help(CI_SERVER_HELP),
                Arg::with_name("args")
                    .set(ArgSettings::Last)
                    .multiple(true)
            ]))
        .get_matches();

    let args = args.subcommand_matches("tarpaulin").unwrap_or(&args);
    let config = Config::from_args(args);
    match run(config) {
        Ok(()) => println!("Tarpaulin finished"),
        Err(e) => {
            println!("Error during run");
            std::process::exit(e);
        },
    }
}
