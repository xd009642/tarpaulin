use std::path::{PathBuf, Path};
use std::env;
use std::time::Duration;
use std::str::FromStr;
use clap::ArgMatches;
use coveralls_api::CiService;
use regex::Regex;

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
#[derive(Debug, Default)]
pub struct Config {
    /// Path to the projects cargo manifest
    pub manifest: PathBuf,
    /// Flag to also run tests with the ignored attribute
    pub run_ignored: bool,
    /// Flag to ignore test functions in coverage statistics
    pub ignore_tests: bool,
    /// Flag to skip the clean step when preparing the target project
    pub skip_clean: bool,
    /// Verbose flag for printing information to the user
    pub verbose: bool,
    /// Flag to disable counting line hits in line coverage mode
    pub no_count: bool,
    /// Flag specifying to run line coverage (default)
    pub line_coverage: bool,
    /// Flag specifying to run branch coverage
    pub branch_coverage: bool,
    /// Output files to generate
    pub generate: Vec<OutputFile>,
    /// Key relating to coveralls service or repo
    pub coveralls: Option<String>,
    /// Enum representing CI tool used.
    pub ci_tool: Option<CiService>,
    /// Forward unexpected signals back to the tracee. Used for tests which
    /// rely on signals to work. 
    pub forward_signals: bool,
    /// Include all available features in target build
    pub all_features: bool,
    /// Features to include in the target project build
    pub features: Vec<String>,
    /// Build all packages in the workspace
    pub all: bool,
    /// Packages to include when building the target project
    pub packages: Vec<String>,
    /// Packages to exclude from testing
    pub exclude: Vec<String>,
    /// Files to exclude from testing
    excluded_files: Vec<Regex>,
    /// Varargs to be forwarded to the test executables.
    pub varargs: Vec<String>,
    /// Duration to wait before a timeout occurs
    pub test_timeout: Duration,
}

impl Config {
    fn get_list_from_args(args: &ArgMatches, key: &str) -> Vec<String> {
        match args.values_of_lossy(key) {
            Some(v) => v,
            None => vec![],
        }
    }

    /// Create configuration from clap ArgMatches.
    pub fn from_args(args: &ArgMatches) -> Config {
        let mut line = args.is_present("line");
        let mut branch = args.is_present("branch");
        let verbose = args.is_present("verbose");
        let ignored = args.is_present("ignored");
        let forward = args.is_present("forward");
        let skip_clean = args.is_present("skip-clean");
        let no_count = args.is_present("no-count");
        let ignore_tests = args.is_present("ignore-tests");
        let all_features = args.is_present("all-features");
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
        let features: Vec<String> = Config::get_list_from_args(args, "features");
        let all = args.is_present("all");
        let packages: Vec<String> = Config::get_list_from_args(args, "packages");
        let exclude: Vec<String> = Config::get_list_from_args(args, "exclude");
        let varargs: Vec<String> = Config::get_list_from_args(args, "args");
        let mut ex_files:Vec<Regex> = vec![]; 
        for temp_str in &Config::get_list_from_args(args, "exclude-files") {
            let s =  &temp_str.replace(".", r"\.").replace("*", ".*");
            if let Ok(re) = Regex::new(s) {
                ex_files.push(re);
            } else if verbose {
                println!("Error in wildcard expression: {}", temp_str);
            }
        }

        let timeout = if args.is_present("timeout") {
            match value_t!(args.value_of("timeout"), u64) {
                Ok(s) => s,
                Err(_) => {
                    println!("Invalid value for timeout. Setting to 1 minute");
                    60u64
                }
            }
        } else {
            60u64
        };
        Config{
            manifest: root,
            run_ignored: ignored,
            ignore_tests: ignore_tests,
            verbose: verbose,
            no_count: no_count,
            line_coverage: line,
            skip_clean: skip_clean,
            branch_coverage: branch,
            generate: out,
            coveralls: coveralls,
            ci_tool: ci_tool,
            forward_signals: forward,
            all_features: all_features,
            features: features,
            all: all,
            packages: packages,
            exclude: exclude,
            excluded_files: ex_files,
            varargs: varargs,
            test_timeout: Duration::from_secs(timeout),
        }
    }

    /// Determine whether to send data to coveralls 
    pub fn is_coveralls(&self) -> bool {
        self.coveralls.is_some()
    }


    pub fn exclude_path(&self, path: &Path) -> bool {
        let path = self.strip_project_path(path);
        self.excluded_files.iter()
                           .any(|x| x.is_match(path.to_str().unwrap_or("")))
    }
    
    /// Strips the directory the project manifest is in from the path. Provides a
    /// nicer path for printing to the user.
    pub fn strip_project_path<'a>(&'a self, path: &'a Path) -> PathBuf {
        if let Some(root) = self.manifest.parent() {
            path_relative_from(path, root).unwrap_or(path.to_path_buf())
        } else {
            path.to_path_buf()
        }
    }
}

/// Gets the relative path from one directory to another, if it exists.
/// Credit to brson from this commit from 2015
/// https://github.com/rust-lang/rust/pull/23283/files
pub(crate) fn path_relative_from(path: &Path, base: &Path) -> Option<PathBuf> {
    use std::path::Component;

    if path.is_absolute() != base.is_absolute() {
        if path.is_absolute() {
            Some(path.to_path_buf())
        } else {
            None
        }
    } else {
        let mut ita = path.components();
        let mut itb = base.components();
        let mut comps: Vec<Component> = vec![];
        loop {
            match (ita.next(), itb.next()) {
                (None, None) => break,
                (Some(a), None) => {
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
                (None, _) => comps.push(Component::ParentDir),
                (Some(a), Some(b)) if comps.is_empty() && a == b => (),
                (Some(a), Some(b)) if b == Component::CurDir => comps.push(a),
                (Some(_), Some(b)) if b == Component::ParentDir => return None,
                (Some(a), Some(_)) => {
                    comps.push(Component::ParentDir);
                    for _ in itb {
                        comps.push(Component::ParentDir);
                    }
                    comps.push(a);
                    comps.extend(ita.by_ref());
                    break;
                }
            }
        }
        Some(comps.iter().map(|c| c.as_os_str()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::App;


    #[test]
    fn exclude_paths() {
        let matches = App::new("tarpaulin")
            .args_from_usage("--exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'")
            .get_matches_from_safe(vec!["tarpaulin", "--exclude-files", "*module*"])
            .unwrap();
        let conf = Config::from_args(&matches);
        assert!(conf.exclude_path(Path::new("src/module/file.rs")));
        assert!(!conf.exclude_path(Path::new("src/mod.rs")));
        assert!(!conf.exclude_path(Path::new("unrelated.rs")));
        assert!(conf.exclude_path(Path::new("module.rs")));
    }
    
    
    #[test]
    fn no_exclusions() {
        let matches = App::new("tarpaulin")
            .args_from_usage("--exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'")
            .get_matches_from_safe(vec!["tarpaulin"])
            .unwrap();
        let conf = Config::from_args(&matches);
        assert!(!conf.exclude_path(Path::new("src/module/file.rs")));
        assert!(!conf.exclude_path(Path::new("src/mod.rs")));
        assert!(!conf.exclude_path(Path::new("unrelated.rs")));
        assert!(!conf.exclude_path(Path::new("module.rs")));
    }

    
    #[test]
    fn exclude_exact_file() {
        let matches = App::new("tarpaulin")
            .args_from_usage("--exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'")
            .get_matches_from_safe(vec!["tarpaulin", "--exclude-files", "*/lib.rs"])
            .unwrap();
        let conf = Config::from_args(&matches);
        assert!(conf.exclude_path(Path::new("src/lib.rs")));
        assert!(!conf.exclude_path(Path::new("src/mod.rs")));
        assert!(!conf.exclude_path(Path::new("src/notlib.rs")));
        assert!(!conf.exclude_path(Path::new("lib.rs")));
    }

    #[test]
    fn relative_path_test() {
        let path_a = Path::new("/this/should/form/a/rel/path/");
        let path_b = Path::new("/this/should/form/b/rel/path/");

        let rel_path = path_relative_from(path_b, path_a);
        assert!(rel_path.is_some());
        assert_eq!(rel_path.unwrap().to_str().unwrap(), "../../../b/rel/path", "Wrong relative path");

        let path_a = Path::new("/this/should/not/form/a/rel/path/");
        let path_b = Path::new("./this/should/not/form/a/rel/path/");

        let rel_path = path_relative_from(path_b, path_a);
        assert_eq!(rel_path, None, "Did not expect relative path");
        

        let path_a = Path::new("./this/should/form/a/rel/path/");
        let path_b = Path::new("./this/should/form/b/rel/path/");

        let rel_path = path_relative_from(path_b, path_a);
        assert!(rel_path.is_some());
        assert_eq!(rel_path.unwrap().to_str().unwrap(), "../../../b/rel/path", "Wrong relative path");
    }
}
