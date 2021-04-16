use cargo_tarpaulin::cargo::{rust_flags, rustdoc_flags};
use cargo_tarpaulin::config::{Color, Config, ConfigWrapper, Mode, OutputFile, RunType};
use cargo_tarpaulin::{run, setup_logging};
use clap::{crate_version, App, Arg, ArgSettings, SubCommand};
use std::collections::HashMap;
use std::path::Path;
use tracing::{info, trace};

fn is_dir(d: String) -> Result<(), String> {
    if Path::new(&d).is_dir() {
        Ok(())
    } else {
        Err(String::from("root must be a directory"))
    }
}

fn print_env(seen_rustflags: HashMap<String, Vec<String>>, prefix: &str, default_val: &str) {
    info!("Printing `{}`", prefix);
    if seen_rustflags.is_empty() {
        info!("No configs provided printing default RUSTFLAGS");
        println!("{}={}", prefix, default_val);
    } else if seen_rustflags.len() == 1 {
        let flags = seen_rustflags.keys().next().unwrap();
        println!(r#"{}="{}""#, prefix, flags);
    } else {
        for (k, v) in seen_rustflags.iter() {
            info!("RUSTFLAGS for configs {:?}", v);
            println!(r#"{}="{}""#, prefix, k);
        }
    }
}

const CI_SERVER_HELP: &str = "Name of service, supported services are:
travis-ci, travis-pro, circle-ci, semaphore, jenkins and codeship.
If you are interfacing with coveralls.io or another site you can \
also specify a name that they will recognise. Refer to their documentation for this.";

fn main() -> Result<(), String> {
    let args = App::new("cargo-tarpaulin")
        .author("Daniel McKenna, <danielmckenna93@gmail.com>")
        .about("Tool to analyse test coverage of cargo projects")
        .version(concat!("version: ", crate_version!()))
        .bin_name("cargo")
        .subcommand(SubCommand::with_name("tarpaulin")
            .about("Tool to analyse test coverage of cargo projects")
            .version(concat!("version: ", crate_version!()))
            .args_from_usage(
                 "--config [FILE] 'Path to a toml file specifying a list of options this will override any other options set'
                 --ignore-config 'Ignore any project config files'
                 --lib 'Test only this package's library unit tests'
                 --bin [NAME]... 'Test only the specified binary`
                 --bins 'Test all binaries'
                 --example [NAME]... 'Test only the specified example'
                 --examples 'Test all examples'
                 --test [NAME]... 'Test only the specified test target'
                 --tests 'Test all tests'
                 --bench [NAME]... 'Test only the specified bench target'
                 --benches 'Test all benches'
                 --doc 'Test only this library's documentation'
                 --all-targets 'Test all targets'
                 --no-fail-fast 'Run all tests regardless of failure'
                 --profile [NAME] 'Build artefacts with the specified profile'
                 --debug 'Show debug output - this is used for diagnosing issues with tarpaulin'
                 --dump-traces 'Log tracing events and save to a json file. Also, enabled when --debug is used'
                 --verbose -v 'Show extra output'
                 --ignore-tests 'Ignore lines of test functions when collecting coverage'
                 --ignore-panics 'Ignore panic macros in tests'
                 --count   'Counts the number of hits during coverage'
                 --ignored -i 'Run ignored tests as well'
                 --line -l    'Line coverage'
                 --skip-clean 'The opposite of --force-clean'
                 --force-clean 'Adds a clean stage to work around cargo bugs that may affect coverage results'
                 --fail-under [PERCENTAGE] 'Sets a percentage threshold for failure ranging from 0-100, if coverage is below exit with a non-zero code'
                 --branch -b  'Branch coverage: NOT IMPLEMENTED'
                 --forward -f 'Forwards unexpected signals to test. Tarpaulin will still take signals it is expecting.'
                 --coveralls [KEY]  'Coveralls key, either the repo token, or if you're using travis use $TRAVIS_JOB_ID and specify travis-{ci|pro} in --ciserver'
                 --report-uri [URI] 'URI to send report to, only used if the option --coveralls is used'
                 --no-default-features 'Do not include default features'
                 --features [FEATURES]... 'Features to be included in the target project'
                 --all-features 'Build all available features'
                 --all        'Alias for --workspace (deprecated)'
                 --workspace 'Test all packages in the workspace'
                 --packages -p [PACKAGE]... 'Package id specifications for which package should be build. See cargo help pkgid for more info'
                 --exclude -e [PACKAGE]... 'Package id specifications to exclude from coverage. See cargo help pkgid for more info'
                 --exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'
                 --timeout -t [SECONDS] 'Integer for the maximum time in seconds without response from test before timeout (default is 1 minute).'
                 --follow-exec 'Follow executed processes capturing coverage information if they're part of your project.'
                 --release   'Build in release mode.'
                 --no-run 'Compile tests but don't run coverage'
                 --locked 'Do not update Cargo.lock'
                 --frozen 'Do not update Cargo.lock or any caches'
                 --target [TRIPLE] 'Compilation target triple'
                 --target-dir [DIR] 'Directory for all generated artifacts'
                 --offline 'Run without accessing the network'
                 --print-rust-flags 'Print the RUSTFLAGS options that tarpaulin will compile your program with and exit'
                 --print-rustdoc-flags 'Print the RUSTDOCFLAGS options that tarpaulin will compile any doctests with and exit'
                 --avoid-cfg-tarpaulin 'Remove --cfg=tarpaulin from the RUSTFLAG'
                 -j --jobs [N] 'Number of parallel jobs, defaults to # of CPUs'
                 --rustflags [FLAGS] 'rustflags to add when building project (can also be set via RUSTFLAGS env var)'
                 -Z [FEATURES]...   'List of unstable nightly only flags'")
            .args(&[
                Arg::from_usage("--out -o [FMT]   'Output format of coverage report'")
                    .possible_values(&OutputFile::variants())
                    .case_insensitive(true)
                    .multiple(true),
                Arg::from_usage("--output-dir [PATH] 'Specify a custom directory to write report files'"),
                Arg::from_usage("--run-types [TYPE]... 'Type of the coverage run'")
                    .possible_values(&RunType::variants())
                    .case_insensitive(true)
                    .multiple(true),
                Arg::from_usage("--color [WHEN] 'Coloring: auto, always, never'")
                    .case_insensitive(true)
                    .possible_values(&Color::variants()),
                Arg::from_usage("--command [CMD] 'cargo subcommand to run. So far only test and build are supported'")
                    .case_insensitive(true)
                    .possible_values(&Mode::variants()),
                Arg::from_usage("--root -r [DIR]  'Calculates relative paths to root directory. If --manifest-path isn't specified it will look for a Cargo.toml in root'")
                    .validator(is_dir),
                Arg::from_usage("--manifest-path [PATH] 'Path to Cargo.toml'"),
                Arg::from_usage("--ciserver [SERVICE] 'CI server being used, if unspecified tarpaulin may automatically infer for coveralls uploads'")
                    .help(CI_SERVER_HELP),
                Arg::with_name("args")
                    .set(ArgSettings::Last)
                    .multiple(true)
                    .help("Arguments to be passed to the test executables can be used to filter or skip certain tests")
            ]))
        .get_matches();

    let args = args.subcommand_matches("tarpaulin").unwrap_or(&args);

    let config = ConfigWrapper::from(args);
    setup_logging(
        config.0[0].color,
        args.is_present("debug"),
        args.is_present("verbose"),
    );
    let mut run_coverage = true;
    if args.is_present("print-rust-flags") {
        run_coverage = false;
        let mut seen_rustflags = HashMap::new();
        for config in &config.0 {
            let rustflags = rust_flags(config);
            seen_rustflags
                .entry(rustflags)
                .or_insert(vec![])
                .push(config.name.clone());
        }
        let default = Config::default();
        print_env(seen_rustflags, "RUSTFLAGS", &rust_flags(&default));
    }
    if args.is_present("print-rustdoc-flags") {
        run_coverage = false;
        let mut seen_rustflags = HashMap::new();
        for config in &config.0 {
            let rustflags = rustdoc_flags(config);
            seen_rustflags
                .entry(rustflags)
                .or_insert(vec![])
                .push(config.name.clone());
        }
        let default = Config::default();
        print_env(seen_rustflags, "RUSTDOCFLAGS", &rustdoc_flags(&default));
    }
    if run_coverage {
        trace!("Debug mode activated");
        // Since this is the last function we run and don't do any error mitigations (other than
        // printing the error to the user it's fine to unwrap here
        run(&config.0).map_err(|e| e.to_string())
    } else {
        Ok(())
    }
}
