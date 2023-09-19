use std::path::PathBuf;

use clap::{Args, Parser};
use glob::Pattern;

use crate::config::{Ci, Color, Mode, OutputFile, RunType, TraceEngine};

#[derive(Debug, Parser)]
#[command(name = "cargo-tarpaulin")]
#[command(bin_name = "cargo")]
#[command(author, version, about, long_about = None)]
pub enum CargoTarpaulinCli {
    Tarpaulin(TarpaulinCli),
}

impl CargoTarpaulinCli {
    pub fn from_args() -> TarpaulinCli {
        match TarpaulinCli::try_parse() {
            Ok(tarpaulin_cli) => tarpaulin_cli,
            Err(err) if !err.use_stderr() => err.exit(),
            Err(_) => match CargoTarpaulinCli::parse() {
                CargoTarpaulinCli::Tarpaulin(tarpaulin_cli) => tarpaulin_cli,
            },
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "tarpaulin")]
#[command(author, version, about, long_about = None)]
pub struct TarpaulinCli {
    #[clap(flatten)]
    pub print_flags: PrintFlagsArgs,
    #[clap(flatten)]
    pub config: ConfigArgs,
}

#[derive(Debug, Clone, Args)]
pub struct ConfigArgs {
    #[clap(flatten)]
    pub logging: LoggingArgs,
    #[clap(flatten)]
    pub run_types: RunTypesArgs,

    /// Path to a toml file specifying a list of options this will override any other options set
    #[arg(long, value_name = "FILE")]
    pub config: Option<PathBuf>,
    /// Ignore any project config files
    #[arg(long)]
    pub ignore_config: bool,
    /// Test only the specified binary
    #[arg(long, value_name = "NAME", num_args = 0..)]
    pub bin: Vec<String>,
    /// Test only the specified example
    #[arg(long, value_name = "NAME", num_args = 0..)]
    pub example: Vec<String>,
    /// Test only the specified test target
    #[arg(long, value_name = "NAME", num_args = 0..)]
    pub test: Vec<String>,
    /// Test only the specified bench target
    #[arg(long, value_name = "NAME", num_args = 0..)]
    pub bench: Vec<String>,
    /// Run all tests regardless of failure
    #[arg(long)]
    pub no_fail_fast: bool,
    /// Build artefacts with the specified profile
    #[arg(long, value_name = "NAME")]
    pub profile: Option<String>,
    /// Ignore lines of test functions when collecting coverage (default)
    #[arg(long)]
    pub ignore_tests: bool,
    /// Stops tarpaulin from building projects with -Clink-dead-code
    #[arg(long)]
    pub no_dead_code: bool,
    /// Include lines of test functions when collecting coverage
    #[arg(long)]
    pub include_tests: bool,
    /// Ignore panic macros in tests
    #[arg(long)]
    pub ignore_panics: bool,
    /// Counts the number of hits during coverage
    #[arg(long)]
    pub count: bool,
    /// Run ignored tests as well
    #[arg(long, short)]
    pub ignored: bool,
    /// Line coverage
    #[arg(long, short)]
    pub line: bool,
    /// The opposite of --force-clean
    #[arg(long)]
    pub skip_clean: bool,
    /// Adds a clean stage to work around cargo bugs that may affect coverage results
    #[arg(long)]
    pub force_clean: bool,
    /// Sets a percentage threshold for failure ranging from 0-100, if coverage is below exit with a non-zero code
    #[arg(long, value_name = "PERCENTAGE")]
    pub fail_under: Option<f64>,
    /// Branch coverage: NOT IMPLEMENTED
    #[arg(long, short)]
    pub branch: bool,
    /// Forwards unexpected signals to test. This is now the default behaviour
    #[arg(long, short)]
    pub forward: bool,
    /// Coveralls key, either the repo token, or if you're using travis use $TRAVIS_JOB_ID and specify travis-{ci|pro} in --ciserver
    #[arg(long, value_name = "KEY")]
    pub coveralls: Option<String>,
    /// URI to send report to, only used if the option --coveralls is used
    #[arg(long, value_name = "URI")]
    pub report_uri: Option<String>,
    /// Do not include default features
    #[arg(long)]
    pub no_default_features: bool,
    /// Features to be included in the target project
    #[arg(long, value_name = "FEATURES", num_args = 0..)]
    pub features: Vec<String>,
    /// Build all available features
    #[arg(long)]
    pub all_features: bool,
    /// Alias for --workspace (deprecated)
    #[arg(long)]
    pub all: bool,
    /// Test all packages in the workspace
    #[arg(long)]
    pub workspace: bool,
    /// Package id specifications for which package should be build. See cargo help pkgid for more info
    #[arg(long, short, value_name = "PACKAGE", num_args = 0..)]
    pub packages: Vec<String>,
    /// Package id specifications to exclude from coverage. See cargo help pkgid for more info
    #[arg(long, short, value_name = "PACKAGE", num_args = 0..)]
    pub exclude: Vec<String>,
    /// Exclude given files from coverage results has * wildcard
    #[arg(long, value_name = "FILE", num_args = 0..)]
    pub exclude_files: Vec<Pattern>,
    /// Integer for the maximum time in seconds without response from test before timeout (default is 1 minute).
    #[arg(long, short, value_name = "SECONDS")]
    pub timeout: Option<u64>,
    /// Delay after test to collect coverage profiles
    #[arg(long, value_name = "SECONDS")]
    pub post_test_delay: Option<u64>,
    /// Follow executed processes capturing coverage information if they're part of your project.
    #[arg(long)]
    pub follow_exec: bool,
    /// Build in release mode.
    #[arg(long)]
    pub release: bool,
    /// Compile tests but don't run coverage
    #[arg(long)]
    pub no_run: bool,
    /// 'Don't supply an explicit `--test-threads` argument to test executable. By default tarpaulin will infer the default rustc would pick if not ran via tarpaulin and set it
    #[arg(long)]
    pub implicit_test_threads: bool,
    /// Do not update Cargo.lock
    #[arg(long)]
    pub locked: bool,
    /// Do not update Cargo.lock or any caches
    #[arg(long)]
    pub frozen: bool,
    /// Compilation target triple
    #[arg(long, value_name = "TRIPLE")]
    pub target: Option<String>,
    /// Directory for all generated artifacts
    #[arg(long, value_name = "DIR")]
    pub target_dir: Option<PathBuf>,
    /// Run without accessing the network
    #[arg(long)]
    pub offline: bool,
    /// Remove --cfg=tarpaulin from the RUSTFLAG
    #[arg(long)]
    pub avoid_cfg_tarpaulin: bool,
    /// Number of parallel jobs, defaults to # of CPUs
    #[arg(long, short, value_name = "N")]
    pub jobs: Option<usize>,
    /// Rustflags to add when building project (can also be set via RUSTFLAGS env var)
    #[arg(long, value_name = "FLAGS")]
    pub rustflags: Option<String>,
    /// Other object files to load which contain information for llvm coverage - must have been compiled with llvm coverage instrumentation (ignored for ptrace)
    #[arg(long, value_name = "objects", num_args = 0..)]
    pub objects: Vec<PathBuf>,
    /// List of unstable nightly only flags
    #[arg(short = 'Z', value_name = "FEATURES", num_args = 0..)]
    pub unstable_features: Vec<String>,
    /// Output format of coverage report
    #[arg(long, short, value_enum, value_name = "FMT", num_args = 0..)]
    pub out: Vec<OutputFile>,
    /// Coverage tracing backend to use
    #[arg(long, value_enum, value_name = "ENGINE")]
    pub engine: Option<TraceEngine>,
    /// Specify a custom directory to write report files
    #[arg(long, value_name = "PATH")]
    pub output_dir: Option<PathBuf>,
    /// cargo subcommand to run. So far only test and build are supported
    #[arg(long, value_enum, value_name = "CMD")]
    pub command: Option<Mode>,
    /// Calculates relative paths to root directory. If --manifest-path isn't specified it will look for a Cargo.toml in root
    #[arg(long, short, value_name = "DIR")]
    pub root: Option<PathBuf>,
    /// Path to Cargo.toml
    #[arg(long, value_name = "PATH")]
    pub manifest_path: Option<PathBuf>,
    /// CI server being used, if unspecified tarpaulin may automatically infer for coveralls uploads
    #[arg(long, value_name = "SERVICE")]
    pub ciserver: Option<Ci>,
    /// Option to fail immediately after a single test fails
    #[arg(long)]
    pub fail_immediately: bool,
    /// Arguments to be passed to the test executables can be used to filter or skip certain tests
    #[arg(last = true)]
    pub args: Vec<String>,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct LoggingArgs {
    /// Coloring: auto, always, never
    #[arg(long, value_enum, value_name = "WHEN")]
    pub color: Option<Color>,
    /// Show debug output - this is used for diagnosing issues with tarpaulin
    #[arg(long)]
    pub debug: bool,
    /// Show extra output
    #[arg(long, short)]
    pub verbose: bool,
    /// Log tracing events and save to a json file. Also, enabled when --debug is used
    #[arg(long)]
    pub dump_traces: bool,
}

#[derive(Debug, Clone, Copy, Args)]
pub struct PrintFlagsArgs {
    /// Print the RUSTFLAGS options that tarpaulin will compile your program with and exit
    #[arg(long)]
    pub print_rust_flags: bool,
    /// Print the RUSTDOCFLAGS options that tarpaulin will compile any doctests with and exit
    #[arg(long)]
    pub print_rustdoc_flags: bool,
}

#[derive(Debug, Clone, Args)]
pub struct RunTypesArgs {
    /// Type of the coverage run
    #[arg(long, value_enum, value_name = "TYPE")]
    pub run_types: Vec<RunType>,
    /// Test all benches
    #[arg(long)]
    pub benches: bool,
    /// Test only this library's documentation
    #[arg(long)]
    pub doc: bool,
    /// Test all targets (excluding doctests)
    #[arg(long)]
    pub all_targets: bool,
    /// Test only this package's library unit tests
    #[arg(long)]
    pub lib: bool,
    /// Test all binaries
    #[arg(long)]
    pub bins: bool,
    /// Test all examples
    #[arg(long)]
    pub examples: bool,
    /// Test all tests
    #[arg(long)]
    pub tests: bool,
}

impl RunTypesArgs {
    pub fn collect(self) -> Vec<RunType> {
        let mut run_types = self.run_types;
        if self.lib && !run_types.contains(&RunType::Lib) {
            run_types.push(RunType::Lib);
        }
        if self.all_targets && !run_types.contains(&RunType::AllTargets) {
            run_types.push(RunType::AllTargets);
        }
        if self.benches && !run_types.contains(&RunType::Benchmarks) {
            run_types.push(RunType::Benchmarks);
        }
        if self.bins && !run_types.contains(&RunType::Bins) {
            run_types.push(RunType::Bins);
        }
        if self.examples && !run_types.contains(&RunType::Examples) {
            run_types.push(RunType::Examples);
        }
        if self.doc && !run_types.contains(&RunType::Doctests) {
            run_types.push(RunType::Doctests);
        }
        if self.tests && !run_types.contains(&RunType::Tests) {
            run_types.push(RunType::Tests);
        }
        run_types
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::CargoTarpaulinCli;

    #[test]
    fn verify_args() {
        CargoTarpaulinCli::command().debug_assert()
    }

    #[test]
    #[ignore = "Manual use only"]
    fn show_help() {
        let help = CargoTarpaulinCli::command().render_help();
        println!("{help}");
    }
}
