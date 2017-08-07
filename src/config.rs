use std::path::PathBuf;
use std::env;
use std::str::FromStr;
use clap::ArgMatches;
use coveralls_api::CiService;


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

struct Ci(CiService);

impl FromStr for Ci {
    type Err = ();
    /// This will never error so no need to implement the error type.
    fn from_str(s: &str) -> Result<Ci, ()> {
        match s {
            "travis-ci" => Ok(Ci(CiService::Travis)),
            "travis-pro" => Ok(Ci(CiService::TravisPro)),
            "circle-ci" => Ok(Ci(CiService::Circle)),
            "semaphore" => Ok(Ci(CiService::Semaphore)),
            "jenkins" => Ok(Ci(CiService::Jenkins)),
            "codeship" => Ok(Ci(CiService::Codeship)),
            other => Ok(Ci(CiService::Other(other.to_string()))),
        }
    }
}

/// Specifies the current configuration tarpaulin is using.
#[derive(Debug)]
pub struct Config {
    pub manifest: PathBuf,
    pub run_ignored: bool,
    pub skip_clean: bool,
    pub verbose: bool,
    pub line_coverage: bool,
    pub branch_coverage: bool,
    pub generate: Vec<OutputFile>,
    /// Key relating to coveralls service or repo
    pub coveralls: Option<String>,
    /// Enum representing CI tool used.
    pub ci_tool: Option<CiService>,
    /// Forward unexpected signals back to the tracee. Used for tests which
    /// rely on signals to work. 
    pub forward_signals: bool,
    pub features: Vec<String>,
    pub packages: Vec<String>,
}


impl Config {
    /// Create configuration from clap ArgMatches.
    pub fn from_args(args: &ArgMatches) -> Config {
        let mut line = args.is_present("line");
        let mut branch = args.is_present("branch");
        let verbose = args.is_present("verbose");
        let ignored = args.is_present("ignored");
        let forward = args.is_present("forward");
        let skip_clean = args.is_present("skip-clean");
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
        if let Ok(cpath) = root.canonicalize() {
            root = cpath;
        }
        let ci_tool = match value_t!(args, "ciserver", Ci) {
            Ok(ci) => Some(ci.0),
            Err(_) => None,
        };
        let coveralls = if let Some(cio) = args.value_of("coveralls") {
            Some(cio.to_string())
        } else {
            None
        };
        let out:Vec<OutputFile> = values_t!(args.values_of("out"), OutputFile)
            .unwrap_or_default();
        let features: Vec<String> = match args.values_of_lossy("features") {
            Some(v) => v,
            None => vec![],
        };
        let packages: Vec<String> = match args.values_of_lossy("packages") {
            Some(v) => v,
            None => vec![],
        };
        Config{
            manifest: root,
            run_ignored: ignored,
            verbose: verbose,
            line_coverage: line,
            skip_clean: skip_clean,
            branch_coverage: branch,
            generate: out,
            coveralls: coveralls,
            ci_tool: ci_tool,
            forward_signals: forward,
            features: features,
            packages: packages,
        }
    }

    /// Determine whether to send data to coveralls 
    pub fn is_coveralls(&self) -> bool {
        self.coveralls.is_some()
    }
}
