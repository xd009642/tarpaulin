use std::env;
use std::fs;
use std::process::{self, Command};

fn main() {
    let mut args = env::args_os().skip(1);
    let test_binary = args.next().expect("runner needs a test binary argument");
    let marker = env::var_os("TARPAULIN_RUNNER_MARKER")
        .expect("TARPAULIN_RUNNER_MARKER must be set for the runner fixture");
    fs::write(marker, b"runner executed").expect("runner should write its marker file");

    let status = Command::new(test_binary)
        .args(args)
        .status()
        .expect("runner should execute the test binary");

    process::exit(status.code().unwrap_or(1));
}
