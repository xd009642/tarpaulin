extern crate docopt;
extern crate rustc_serialize;
extern crate dwarf;

use docopt::Docopt;

const USAGE: &'static str = "
Tarpaulin - a cargo code coverage tool

Usage: 
    cargo-tarpaulin [options]
    cargo-tarpaulin (-h | --help)

Options:
    -h, --help          Show this message.
    -l, --line          Collect line coverage.
    -b, --branch        Collect branch coverage.
    -c, --condition     Collect condition coverage.
    --out ARG           Specify output type [default: Report].
    -v, --verbose       Show extra output.
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
}

fn main() {
    let args:Args = Docopt::new(USAGE)
                           .and_then(|d| d.decode())
                           .unwrap_or_else(|e| e.exit());

    println!("{:?}", args);
    
}
