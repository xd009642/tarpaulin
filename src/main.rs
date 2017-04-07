extern crate docopt;
extern crate rustc_serialize;
extern crate dwarf;

use docopt::Docopt;
use std::env;

const USAGE: &'static str = "
Tarpaulin - a cargo code coverage tool
Usage: cargo tarpaulin [Options]
       cargo tarpaulin --help

Options:
    -h, --help          Show this message.
    -t --type CTYPE     Coverage algorithm to run.
    -o --output OTYPE   Output type.
";

#[derive(RustcDecodable, Debug)]
enum CoverageType {
    Line,
    Branch,
    Condition,
}

#[derive(RustcDecodable, Debug)]
enum OutputType {
    Json,
    Toml,
    Report
}

#[derive(RustcDecodable, Debug)]
struct Args {
    arg_ctype: Option<CoverageType>,
    arg_otype: Option<OutputType>,
}

fn main() {
    let args: Vec<String> = env::args().collect();
    
    let args:Args = Docopt::new(USAGE)
                           .and_then(|d| d.argv(argv()).decode())
                           .unwrap_or_else(|e| e.exit());

    println!("{:?}", args);
    
}
