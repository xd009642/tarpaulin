pub use self::types::*;

use std::env;
use std::path::{Path, PathBuf};
use std::time::Duration;

use clap::ArgMatches;
use coveralls_api::CiService;
use regex::Regex;

use self::parse::*;

mod parse;
mod types;

/// Specifies the current configuration tarpaulin is using.
#[derive(Debug)]
pub struct Config {
    /// Path to the projects cargo manifest
    pub manifest: PathBuf,
    /// Path to the projects cargo manifest
    pub root: Option<String>,
    /// Types of tests for tarpaulin to collect coverage on
    pub run_types: Vec<RunType>,
    /// Flag to also run tests with the ignored attribute
    pub run_ignored: bool,
    /// Flag to ignore test functions in coverage statistics
    pub ignore_tests: bool,
    /// Ignore panic macros in code.
    pub ignore_panics: bool,
    /// Flag to add a clean step when preparing the target project
    pub force_clean: bool,
    /// Verbose flag for printing information to the user
    pub verbose: bool,
    /// Debug flag for printing internal debugging information to the user
    pub debug: bool,
    /// Flag to count hits in coverage
    pub count: bool,
    /// Flag specifying to run line coverage (default)
    pub line_coverage: bool,
    /// Flag specifying to run branch coverage
    pub branch_coverage: bool,
    /// Output files to generate
    pub generate: Vec<OutputFile>,
    /// Directory to write output files
    pub output_directory: PathBuf,
    /// Key relating to coveralls service or repo
    pub coveralls: Option<String>,
    /// Enum representing CI tool used.
    pub ci_tool: Option<CiService>,
    /// Only valid if coveralls option is set. If coveralls option is set,
    /// as well as report_uri, then the report will be sent to this endpoint
    /// instead.
    pub report_uri: Option<String>,
    /// Forward unexpected signals back to the tracee. Used for tests which
    /// rely on signals to work.
    pub forward_signals: bool,
    /// Include all available features in target build
    pub all_features: bool,
    /// Do not include default features in target build
    pub no_default_features: bool,
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
    /// Build in release mode
    pub release: bool,
}

impl Default for Config {
    fn default() -> Config {
        Config {
            run_types: vec![RunType::Tests],
            manifest: Default::default(),
            root: Default::default(),
            run_ignored: false,
            ignore_tests: false,
            ignore_panics: false,
            force_clean: false,
            verbose: false,
            debug: false,
            count: false,
            line_coverage: true,
            branch_coverage: false,
            generate: vec![],
            output_directory: Default::default(),
            coveralls: None,
            ci_tool: None,
            report_uri: None,
            forward_signals: false,
            no_default_features: false,
            features: vec![],
            all: false,
            packages: vec![],
            exclude: vec![],
            excluded_files: vec![],
            varargs: vec![],
            test_timeout: Duration::from_secs(60),
            release: false,
            all_features: false,
        }
    }
}

impl<'a> From<&'a ArgMatches<'a>> for Config {
    fn from(args: &'a ArgMatches<'a>) -> Self {
        let debug = args.is_present("debug");
        let verbose = args.is_present("verbose") || debug;
        Config {
            manifest: get_manifest(args),
            root: get_root(args),
            run_types: get_run_types(args),
            run_ignored: args.is_present("ignored"),
            ignore_tests: args.is_present("ignore-tests"),
            ignore_panics: args.is_present("ignore-panics"),
            force_clean: args.is_present("force-clean"),
            verbose,
            debug,
            count: args.is_present("count"),
            line_coverage: get_line_cov(args),
            branch_coverage: get_branch_cov(args),
            generate: get_outputs(args),
            output_directory: get_output_directory(args),
            coveralls: get_coveralls(args),
            ci_tool: get_ci(args),
            report_uri: get_report_uri(args),
            forward_signals: args.is_present("forward"),
            all_features: args.is_present("all-features"),
            no_default_features: args.is_present("no-default-features"),
            features: get_list(args, "features"),
            all: args.is_present("all"),
            packages: get_list(args, "packages"),
            exclude: get_list(args, "exclude"),
            excluded_files: get_excluded(args),
            varargs: get_list(args, "args"),
            test_timeout: get_timeout(args),
            release: args.is_present("release"),
        }
    }
}

impl Config {
    #[inline]
    pub fn is_coveralls(&self) -> bool {
        self.coveralls.is_some()
    }

    #[inline]
    pub fn exclude_path(&self, path: &Path) -> bool {
        let project = self.strip_base_dir(path);

        self.excluded_files
            .iter()
            .any(|x| x.is_match(project.to_str().unwrap_or("")))
    }

    ///
    /// returns the relative path from the base_dir
    /// uses root if set, else env::current_dir()
    ///
    #[inline]
    pub fn get_base_dir(&self) -> PathBuf {
        if let Some(root) = &self.root {
            if Path::new(root).is_absolute() {
                PathBuf::from(root)
            } else {
                let base_dir = env::current_dir().unwrap();
                base_dir.join(root).canonicalize().unwrap()
            }
        } else {
            env::current_dir().unwrap()
        }
    }

    /// returns the relative path from the base_dir
    ///
    #[inline]
    pub fn strip_base_dir(&self, path: &Path) -> PathBuf {
        path_relative_from(path, &self.get_base_dir()).unwrap_or_else(|| path.to_path_buf())
    }

    #[inline]
    pub fn is_default_output_dir(&self) -> bool {
        self.output_directory == env::current_dir().unwrap()
    }
}

/// Gets the relative path from one directory to another, if it exists.
/// Credit to brson from this commit from 2015
/// https://github.com/rust-lang/rust/pull/23283/files
///
fn path_relative_from(path: &Path, base: &Path) -> Option<PathBuf> {
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
        let mut comps = vec![];

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
        let conf = Config::from(&matches);
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
        let conf = Config::from(&matches);
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
        let conf = Config::from(&matches);
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
        assert_eq!(
            rel_path.unwrap().to_str().unwrap(),
            "../../../b/rel/path",
            "Wrong relative path"
        );

        let path_a = Path::new("/this/should/not/form/a/rel/path/");
        let path_b = Path::new("./this/should/not/form/a/rel/path/");

        let rel_path = path_relative_from(path_b, path_a);
        assert_eq!(rel_path, None, "Did not expect relative path");

        let path_a = Path::new("./this/should/form/a/rel/path/");
        let path_b = Path::new("./this/should/form/b/rel/path/");

        let rel_path = path_relative_from(path_b, path_a);
        assert!(rel_path.is_some());
        assert_eq!(
            rel_path.unwrap().to_str().unwrap(),
            "../../../b/rel/path",
            "Wrong relative path"
        );
    }
}
