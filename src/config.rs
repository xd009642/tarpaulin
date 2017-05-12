use std::path::PathBuf;
use std::env;
use clap::ArgMatches;

arg_enum!{
    /// Enum to represent possible output formats.
    #[derive(Debug)]
    pub enum OutputFile {
        Json,
        Toml,
        Stdout,
        Xml
    }
}
/// Specifies the current configuration tarpaulin is using.
#[derive(Debug)]
pub struct Config {
    pub manifest: PathBuf,
    pub verbose: bool,
    pub line_coverage: bool,
    pub branch_coverage: bool,
    pub generate: Vec<OutputFile>,
}


impl Config {
    /// Create configuration from clap ArgMatches.
    pub fn from_args(args: &ArgMatches) -> Config {
        let mut line = args.is_present("line");
        let mut branch = args.is_present("branch");
        let verbose = args.is_present("verbose");
        // If no coverage selected do everything!
        if !branch && !line {
            branch = true;
            line = true;
        }
        let mut root = env::current_dir().unwrap();
        if let Some(path) = args.value_of("root") {
            root.push(path);
        };
        root.push("Cargo.toml");

        let out:Vec<OutputFile> = values_t!(args.values_of("out"), OutputFile)
            .unwrap_or(vec![]);

        Config{
            manifest: root,
            verbose: verbose,
            line_coverage: line,
            branch_coverage: branch,
            generate: out
        }
    }
}
