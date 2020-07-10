pub use self::types::*;

use self::parse::*;
use cargo_metadata::{Metadata, MetadataCommand, Package};
use clap::ArgMatches;
use coveralls_api::CiService;
use humantime_serde::deserialize as humantime_serde;
use indexmap::IndexMap;
use log::{error, info, warn};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cell::{Ref, RefCell};
use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::io::{Error, ErrorKind, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

mod parse;
pub mod types;

pub struct ConfigWrapper(pub Vec<Config>);

/// Specifies the current configuration tarpaulin is using.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct Config {
    pub name: String,
    /// Path to the projects cargo manifest
    #[serde(rename = "manifest-path")]
    pub manifest: PathBuf,
    /// Path to a tarpaulin.toml config file
    pub config: Option<PathBuf>,
    /// Path to the projects cargo manifest
    pub root: Option<String>,
    /// Flag to also run tests with the ignored attribute
    #[serde(rename = "ignored")]
    pub run_ignored: bool,
    /// Flag to ignore test functions in coverage statistics
    #[serde(rename = "ignore-tests")]
    pub ignore_tests: bool,
    /// Ignore panic macros in code.
    #[serde(rename = "ignore-panics")]
    pub ignore_panics: bool,
    /// Flag to add a clean step when preparing the target project
    #[serde(rename = "force-clean")]
    pub force_clean: bool,
    #[serde(rename = "all-targets")]
    pub all_targets: bool,
    /// Verbose flag for printing information to the user
    pub verbose: bool,
    /// Debug flag for printing internal debugging information to the user
    pub debug: bool,
    /// Enable the event logger
    #[serde(rename = "dump-traces")]
    pub dump_traces: bool,
    /// Flag to count hits in coverage
    pub count: bool,
    /// Flag specifying to run line coverage (default)
    #[serde(rename = "line")]
    pub line_coverage: bool,
    /// Flag specifying to run branch coverage
    #[serde(rename = "branch")]
    pub branch_coverage: bool,
    /// Directory to write output files
    #[serde(rename = "output-dir")]
    pub output_directory: Option<PathBuf>,
    /// Key relating to coveralls service or repo
    pub coveralls: Option<String>,
    /// Enum representing CI tool used.
    #[serde(rename = "ciserver", deserialize_with = "deserialize_ci_server")]
    pub ci_tool: Option<CiService>,
    /// Only valid if coveralls option is set. If coveralls option is set,
    /// as well as report_uri, then the report will be sent to this endpoint
    /// instead.
    #[serde(rename = "report-uri")]
    pub report_uri: Option<String>,
    /// Forward unexpected signals back to the tracee. Used for tests which
    /// rely on signals to work.
    #[serde(rename = "forward")]
    pub forward_signals: bool,
    /// Include all available features in target build
    #[serde(rename = "all-features")]
    pub all_features: bool,
    /// Do not include default features in target build
    #[serde(rename = "no-default-features")]
    pub no_default_features: bool,
    /// Build all packages in the workspace
    #[serde(alias = "workspace")]
    pub all: bool,
    /// Duration to wait before a timeout occurs
    #[serde(deserialize_with = "humantime_serde", rename = "timeout")]
    pub test_timeout: Duration,
    /// Build in release mode
    pub release: bool,
    /// Build the tests only don't run coverage
    #[serde(rename = "no-run")]
    pub no_run: bool,
    /// Don't update `Cargo.lock`.
    pub locked: bool,
    /// Don't update `Cargo.lock` or any caches.
    pub frozen: bool,
    /// Build for the target triple.
    pub target: Option<String>,
    /// Directory for generated artifacts
    #[serde(rename = "target-dir")]
    target_dir: Option<PathBuf>,
    /// Run tarpaulin on project without accessing the network
    pub offline: bool,
    /// Types of tests for tarpaulin to collect coverage on
    #[serde(rename = "run-types")]
    pub run_types: Vec<RunType>,
    /// Packages to include when building the target project
    pub packages: Vec<String>,
    /// Packages to exclude from testing
    pub exclude: Vec<String>,
    /// Files to exclude from testing in their compiled form
    #[serde(skip_deserializing, skip_serializing)]
    excluded_files: RefCell<Vec<Regex>>,
    /// Files to exclude from testing in uncompiled form (for serde)
    #[serde(rename = "exclude-files")]
    excluded_files_raw: Vec<String>,
    /// Varargs to be forwarded to the test executables.
    #[serde(rename = "args")]
    pub varargs: Vec<String>,
    /// Features to include in the target project build, e.g. "feature1 feature2"
    pub features: Option<String>,
    /// Unstable cargo features to use
    #[serde(rename = "Z")]
    pub unstable_features: Vec<String>,
    /// Output files to generate
    #[serde(rename = "out")]
    pub generate: Vec<OutputFile>,
    /// Names of tests to run corresponding to `cargo --test <NAME>...`
    #[serde(rename = "test")]
    pub test_names: HashSet<String>,
    /// Names of binaries to run corresponding to `cargo --bin <NAME>...`
    #[serde(rename = "bin")]
    pub bin_names: HashSet<String>,
    /// Names of examples to run corresponding to `cargo --example <NAME>...`
    #[serde(rename = "example")]
    pub example_names: HashSet<String>,
    /// Names of benches to run corresponding to `cargo --bench <NAME>...`
    #[serde(rename = "bench")]
    pub bench_names: HashSet<String>,
    /// Whether to carry on or stop when a test failure occurs
    #[serde(rename = "no-fail-fast")]
    pub no_fail_fast: bool,
    /// Run with the given profile
    pub profile: Option<String>,
    /// Result of cargo_metadata ran on the crate
    #[serde(skip_deserializing, skip_serializing)]
    pub metadata: RefCell<Option<Metadata>>,
}

fn default_test_timeout() -> Duration {
    Duration::from_secs(60)
}

impl Default for Config {
    fn default() -> Config {
        Config {
            name: String::new(),
            run_types: vec![RunType::Tests],
            manifest: default_manifest(),
            config: None,
            root: Default::default(),
            run_ignored: false,
            all_targets: false,
            ignore_tests: false,
            ignore_panics: false,
            force_clean: false,
            verbose: false,
            debug: false,
            dump_traces: false,
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
            features: None,
            unstable_features: vec![],
            all: false,
            packages: vec![],
            exclude: vec![],
            excluded_files: RefCell::new(vec![]),
            excluded_files_raw: vec![],
            varargs: vec![],
            test_timeout: default_test_timeout(),
            release: false,
            all_features: false,
            no_run: false,
            locked: false,
            frozen: false,
            target: None,
            target_dir: None,
            offline: false,
            test_names: HashSet::new(),
            example_names: HashSet::new(),
            bin_names: HashSet::new(),
            bench_names: HashSet::new(),
            no_fail_fast: false,
            profile: None,
            metadata: RefCell::new(None),
        }
    }
}

impl<'a> From<&'a ArgMatches<'a>> for ConfigWrapper {
    fn from(args: &'a ArgMatches<'a>) -> Self {
        info!("Creating config");
        let debug = args.is_present("debug");
        let dump_traces = debug || args.is_present("dump-traces");
        let verbose = args.is_present("verbose") || debug;
        let excluded_files = get_excluded(args);
        let excluded_files_raw = get_list(args, "exclude-files");
        let features = get_list(args, "features");
        let features = if features.is_empty() {
            None
        } else {
            Some(features.join(" "))
        };

        let args_config = Config {
            name: String::new(),
            manifest: get_manifest(args),
            config: None,
            root: get_root(args),
            run_types: get_run_types(args),
            run_ignored: args.is_present("ignored"),
            ignore_tests: args.is_present("ignore-tests"),
            ignore_panics: args.is_present("ignore-panics"),
            force_clean: args.is_present("force-clean"),
            no_fail_fast: args.is_present("no-fail-fast"),
            all_targets: args.is_present("all-targets"),
            verbose,
            debug,
            dump_traces,
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
            features,
            unstable_features: get_list(args, "Z"),
            all: args.is_present("all") | args.is_present("workspace"),
            packages: get_list(args, "packages"),
            exclude: get_list(args, "exclude"),
            excluded_files: RefCell::new(excluded_files),
            excluded_files_raw,
            varargs: get_list(args, "args"),
            test_timeout: get_timeout(args),
            release: args.is_present("release"),
            no_run: args.is_present("no-run"),
            locked: args.is_present("locked"),
            frozen: args.is_present("frozen"),
            target: get_target(args),
            target_dir: get_target_dir(args),
            offline: args.is_present("offline"),
            test_names: get_list(args, "test").iter().cloned().collect(),
            bin_names: get_list(args, "bin").iter().cloned().collect(),
            bench_names: get_list(args, "bench").iter().cloned().collect(),
            example_names: get_list(args, "example").iter().cloned().collect(),
            profile: get_profile(args),
            metadata: RefCell::new(None),
        };
        if args.is_present("ignore-config") {
            Self(vec![args_config])
        } else if args.is_present("config") {
            let mut path = PathBuf::from(args.value_of("config").unwrap());
            if path.is_relative() {
                path = env::current_dir()
                    .unwrap()
                    .join(path)
                    .canonicalize()
                    .unwrap();
            }
            let confs = Config::load_config_file(&path);
            Config::get_config_vec(confs, args_config)
        } else if let Some(cfg) = args_config.check_for_configs() {
            let confs = Config::load_config_file(&cfg);
            Config::get_config_vec(confs, args_config)
        } else {
            Self(vec![args_config])
        }
    }
}

impl Config {
    pub fn target_dir(&self) -> PathBuf {
        if let Some(s) = &self.target_dir {
            s.clone()
        } else {
            match *self.get_metadata() {
                Some(ref meta) => meta.target_directory.clone(),
                _ => self
                    .manifest
                    .parent()
                    .map(|x| x.to_path_buf())
                    .unwrap_or_default()
                    .join("target"),
            }
        }
    }

    pub fn doctest_dir(&self) -> PathBuf {
        let mut result = self.target_dir();
        result.push("doctests");
        result
    }

    fn get_metadata(&self) -> Ref<Option<Metadata>> {
        if self.metadata.borrow().is_none() {
            match MetadataCommand::new().manifest_path(&self.manifest).exec() {
                Ok(meta) => {
                    self.metadata.replace(Some(meta));
                }
                Err(e) => warn!("Couldn't get project metadata {}", e),
            }
        }
        self.metadata.borrow()
    }
    pub fn root(&self) -> PathBuf {
        match *self.get_metadata() {
            Some(ref meta) => meta.workspace_root.clone(),
            _ => self
                .manifest
                .parent()
                .map(|x| x.to_path_buf())
                .unwrap_or_default(),
        }
    }

    pub fn get_packages(&self) -> Vec<Package> {
        match *self.get_metadata() {
            Some(ref meta) => meta.packages.clone(),
            None => vec![],
        }
    }

    pub fn output_dir(&self) -> PathBuf {
        if let Some(ref path) = self.output_directory {
            path.clone()
        } else {
            env::current_dir().unwrap()
        }
    }

    pub fn get_config_vec(file_configs: std::io::Result<Vec<Self>>, backup: Self) -> ConfigWrapper {
        if let Ok(mut confs) = file_configs {
            for c in confs.iter_mut() {
                c.merge(&backup);
            }
            if confs.is_empty() {
                ConfigWrapper(vec![backup])
            } else {
                ConfigWrapper(confs)
            }
        } else {
            warn!("Failed to deserialize config file falling back to provided args");
            ConfigWrapper(vec![backup])
        }
    }

    /// Taking an existing config look for any relevant config files
    pub fn check_for_configs(&self) -> Option<PathBuf> {
        if let Some(root) = &self.root {
            Self::check_path_for_configs(&root)
        } else if let Some(root) = self.manifest.clone().parent() {
            Self::check_path_for_configs(&root)
        } else {
            None
        }
    }

    fn check_path_for_configs<P: AsRef<Path>>(path: P) -> Option<PathBuf> {
        let mut path_1 = PathBuf::from(path.as_ref());
        let mut path_2 = path_1.clone();
        path_1.push("tarpaulin.toml");
        path_2.push(".tarpaulin.toml");
        if path_1.exists() {
            Some(path_1)
        } else if path_2.exists() {
            Some(path_2)
        } else {
            None
        }
    }

    pub fn load_config_file<P: AsRef<Path>>(file: P) -> std::io::Result<Vec<Self>> {
        let mut f = File::open(file.as_ref())?;
        let mut buffer = Vec::new();
        f.read_to_end(&mut buffer)?;
        let mut res = Self::parse_config_toml(&buffer);
        if let Ok(cfs) = res.as_mut() {
            for mut c in cfs.iter_mut() {
                c.config = Some(file.as_ref().to_path_buf());
            }
        }
        res
    }

    pub fn parse_config_toml(buffer: &[u8]) -> std::io::Result<Vec<Self>> {
        let mut map: IndexMap<String, Self> = toml::from_slice(&buffer).map_err(|e| {
            error!("Invalid config file {}", e);
            Error::new(ErrorKind::InvalidData, format!("{}", e))
        })?;

        let mut result = Vec::new();
        for (name, mut conf) in map.iter_mut() {
            conf.name = name.to_string();
            result.push(conf.clone());
        }
        if result.is_empty() {
            Err(Error::new(ErrorKind::InvalidData, "No config tables"))
        } else {
            Ok(result)
        }
    }

    /// Given a config made from args ignoring the config file take the
    /// relevant settings that should be carried across and move them
    pub fn merge(&mut self, other: &Config) {
        if other.debug {
            self.debug = other.debug;
            self.verbose = other.verbose;
        } else if other.verbose {
            self.verbose = other.verbose;
        }
        self.no_run |= other.no_run;
        self.no_default_features |= other.no_default_features;
        self.ignore_panics |= other.ignore_panics;
        self.forward_signals |= other.forward_signals;
        self.run_ignored |= other.run_ignored;
        self.release |= other.release;
        self.count |= other.count;
        self.all_features |= other.all_features;
        self.all_targets |= other.all_targets;
        self.line_coverage |= other.line_coverage;
        self.branch_coverage |= other.branch_coverage;
        self.dump_traces |= other.dump_traces;
        self.offline |= other.offline;
        self.manifest = other.manifest.clone();
        self.root = Config::pick_optional_config(&self.root, &other.root);
        self.coveralls = Config::pick_optional_config(&self.coveralls, &other.coveralls);
        self.ci_tool = Config::pick_optional_config(&self.ci_tool, &other.ci_tool);
        self.report_uri = Config::pick_optional_config(&self.report_uri, &other.report_uri);
        self.target = Config::pick_optional_config(&self.target, &other.target);
        self.target_dir = Config::pick_optional_config(&self.target_dir, &other.target_dir);
        self.output_directory =
            Config::pick_optional_config(&self.output_directory, &other.output_directory);
        self.all |= other.all;
        self.frozen |= other.frozen;
        self.locked |= other.locked;
        self.force_clean |= other.force_clean;
        self.ignore_tests |= other.ignore_tests;
        self.no_fail_fast |= other.no_fail_fast;

        if other.test_timeout != default_test_timeout() {
            self.test_timeout = other.test_timeout.clone();
        }

        if self.profile.is_none() && other.profile.is_some() {
            self.profile = other.profile.clone();
        }
        if other.features.is_some() {
            if self.features.is_none() {
                self.features = other.features.clone();
            } else if let Some(features) = self.features.as_mut() {
                features.push(' ');
                features.push_str(other.features.as_ref().unwrap());
            }
        }

        let additional_packages = other
            .packages
            .iter()
            .filter(|package| !self.packages.contains(package))
            .cloned()
            .collect::<Vec<String>>();
        self.packages.extend(additional_packages);

        let additional_outs = other
            .generate
            .iter()
            .filter(|out| !self.generate.contains(out))
            .copied()
            .collect::<Vec<_>>();
        self.generate.extend(additional_outs);

        let additional_excludes = other
            .exclude
            .iter()
            .filter(|package| !self.exclude.contains(package))
            .cloned()
            .collect::<Vec<String>>();
        self.exclude.extend(additional_excludes);

        let additional_varargs = other
            .varargs
            .iter()
            .filter(|package| !self.varargs.contains(package))
            .cloned()
            .collect::<Vec<String>>();
        self.varargs.extend(additional_varargs);

        let additional_z_opts = other
            .unstable_features
            .iter()
            .filter(|package| !self.unstable_features.contains(package))
            .cloned()
            .collect::<Vec<String>>();
        self.unstable_features.extend(additional_z_opts);

        let exclude = &self.exclude;
        self.packages.retain(|package| {
            let keep = !exclude.contains(package);
            if !keep {
                info!("{} is in exclude list removing from packages", package);
            }
            keep
        });

        for test in &other.test_names {
            self.test_names.insert(test.clone());
        }
        for test in &other.bin_names {
            self.bin_names.insert(test.clone());
        }
        for test in &other.example_names {
            self.example_names.insert(test.clone());
        }
        for test in &other.bench_names {
            self.bench_names.insert(test.clone());
        }
        for ty in &other.run_types {
            if !self.run_types.contains(ty) {
                self.run_types.push(*ty);
            }
        }

        if !other.excluded_files_raw.is_empty() {
            self.excluded_files_raw
                .extend_from_slice(&other.excluded_files_raw);

            // Now invalidated the compiled regex cache so clear it
            let mut excluded_files = self.excluded_files.borrow_mut();
            excluded_files.clear();
        }
    }

    pub fn pick_optional_config<T: Clone>(
        base_config: &Option<T>,
        override_config: &Option<T>,
    ) -> Option<T> {
        if override_config.is_some() {
            override_config.clone()
        } else {
            base_config.clone()
        }
    }

    pub fn has_named_tests(&self) -> bool {
        !(self.test_names.is_empty()
            && self.bin_names.is_empty()
            && self.example_names.is_empty()
            && self.bench_names.is_empty())
    }

    #[inline]
    pub fn is_coveralls(&self) -> bool {
        self.coveralls.is_some()
    }

    #[inline]
    pub fn exclude_path(&self, path: &Path) -> bool {
        if self.excluded_files.borrow().len() != self.excluded_files_raw.len() {
            let mut excluded_files = self.excluded_files.borrow_mut();
            let mut compiled = regexes_from_excluded(&self.excluded_files_raw);
            excluded_files.clear();
            excluded_files.append(&mut compiled);
        }
        let project = self.strip_base_dir(path);

        self.excluded_files
            .borrow()
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
        self.output_directory.is_none()
    }
}

/// Gets the relative path from one directory to another, if it exists.
/// Credit to brson from this commit from 2015
/// https://github.com/rust-lang/rust/pull/23283/files
///
pub fn path_relative_from(path: &Path, base: &Path) -> Option<PathBuf> {
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
    fn features_args() {
        let matches = App::new("tarpaulin")
            .args_from_usage(
                "--features [FEATURES]... 'Features to be included in the target project'
                             --ignore-config 'Ignore any project config files'",
            )
            .get_matches_from_safe(vec![
                "tarpaulin",
                "--ignore-config",
                "--features",
                "a",
                "--features",
                "b",
            ])
            .unwrap();
        let conf = ConfigWrapper::from(&matches).0;
        assert_eq!(conf.len(), 1);
        assert_eq!(conf[0].features, Some("a b".to_string()));

        let matches = App::new("tarpaulin")
            .args_from_usage(
                "--features [FEATURES]... 'Features to be included in the target project'
                             --ignore-config 'Ignore any project config files'",
            )
            .get_matches_from_safe(vec!["tarpaulin", "--ignore-config", "--features", "a b"])
            .unwrap();
        let conf = ConfigWrapper::from(&matches).0;
        assert_eq!(conf.len(), 1);
        assert_eq!(conf[0].features, Some("a b".to_string()))
    }

    #[test]
    fn exclude_paths() {
        let matches = App::new("tarpaulin")
            .args_from_usage("--exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'")
            .get_matches_from_safe(vec!["tarpaulin", "--exclude-files", "*module*"])
            .unwrap();
        let conf = ConfigWrapper::from(&matches).0;
        assert_eq!(conf.len(), 1);
        assert!(conf[0].exclude_path(Path::new("src/module/file.rs")));
        assert!(!conf[0].exclude_path(Path::new("src/mod.rs")));
        assert!(!conf[0].exclude_path(Path::new("unrelated.rs")));
        assert!(conf[0].exclude_path(Path::new("module.rs")));
    }

    #[test]
    fn no_exclusions() {
        let matches = App::new("tarpaulin")
            .args_from_usage("--exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'")
            .get_matches_from_safe(vec!["tarpaulin"])
            .unwrap();
        let conf = ConfigWrapper::from(&matches).0;
        assert_eq!(conf.len(), 1);
        assert!(!conf[0].exclude_path(Path::new("src/module/file.rs")));
        assert!(!conf[0].exclude_path(Path::new("src/mod.rs")));
        assert!(!conf[0].exclude_path(Path::new("unrelated.rs")));
        assert!(!conf[0].exclude_path(Path::new("module.rs")));
    }

    #[test]
    fn exclude_exact_file() {
        let matches = App::new("tarpaulin")
            .args_from_usage("--exclude-files [FILE]... 'Exclude given files from coverage results has * wildcard'")
            .get_matches_from_safe(vec!["tarpaulin", "--exclude-files", "*/lib.rs"])
            .unwrap();
        let conf = ConfigWrapper::from(&matches).0;
        assert_eq!(conf.len(), 1);
        assert!(conf[0].exclude_path(Path::new("src/lib.rs")));
        assert!(!conf[0].exclude_path(Path::new("src/mod.rs")));
        assert!(!conf[0].exclude_path(Path::new("src/notlib.rs")));
        assert!(!conf[0].exclude_path(Path::new("lib.rs")));
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

    #[test]
    fn config_toml() {
        let toml = "[global]
        ignored= true
        coveralls= \"hello\"

        [other]
        run-types = [\"Doctests\", \"Tests\"]";

        let configs = Config::parse_config_toml(toml.as_bytes()).unwrap();
        assert_eq!(configs.len(), 2);
        for c in &configs {
            if c.name == "global" {
                assert_eq!(c.run_ignored, true);
                assert_eq!(c.coveralls, Some("hello".to_string()));
            } else if c.name == "other" {
                assert_eq!(c.run_types, vec![RunType::Doctests, RunType::Tests]);
            } else {
                panic!("Unexpected name {}", c.name);
            }
        }
    }

    #[test]
    fn excluded_merge() {
        let toml = r#"[a]
        exclude-files = ["target/*"]
        [b]
        exclude-files = ["foo.rs"]
        "#;

        let mut configs = Config::parse_config_toml(toml.as_bytes()).unwrap();
        let mut config = configs.remove(0);
        config.merge(&configs[0]);
        assert!(config.excluded_files_raw.contains(&"target/*".to_string()));
        assert!(config.excluded_files_raw.contains(&"foo.rs".to_string()));

        assert_eq!(config.excluded_files_raw.len(), 2);
        assert_eq!(configs[0].excluded_files_raw.len(), 1);
    }

    #[test]
    fn target_merge() {
        let toml_a = r#""#;
        let toml_b = r#"target = "wasm32-unknown-unknown""#;
        let toml_c = r#"target = "x86_64-linux-gnu""#;

        let mut a: Config = toml::from_slice(toml_a.as_bytes()).unwrap();
        let mut b: Config = toml::from_slice(toml_b.as_bytes()).unwrap();
        let c: Config = toml::from_slice(toml_c.as_bytes()).unwrap();

        assert_eq!(a.target, None);
        assert_eq!(b.target, Some(String::from("wasm32-unknown-unknown")));
        assert_eq!(c.target, Some(String::from("x86_64-linux-gnu")));

        b.merge(&c);
        assert_eq!(b.target, Some(String::from("x86_64-linux-gnu")));

        a.merge(&b);
        assert_eq!(a.target, Some(String::from("x86_64-linux-gnu")));
    }

    #[test]
    fn workspace_merge() {
        let toml_a = r#"workspace = false"#;
        let toml_b = r#"workspace = true"#;

        let mut a: Config = toml::from_slice(toml_a.as_bytes()).unwrap();
        let b: Config = toml::from_slice(toml_b.as_bytes()).unwrap();

        assert_eq!(a.all, false);
        assert_eq!(b.all, true);

        a.merge(&b);
        assert_eq!(a.all, true);
    }

    #[test]
    fn packages_merge() {
        let toml_a = r#"packages = []"#;
        let toml_b = r#"packages = ["a"]"#;
        let toml_c = r#"packages = ["b", "a"]"#;

        let mut a: Config = toml::from_slice(toml_a.as_bytes()).unwrap();
        let mut b: Config = toml::from_slice(toml_b.as_bytes()).unwrap();
        let c: Config = toml::from_slice(toml_c.as_bytes()).unwrap();

        assert_eq!(a.packages, Vec::<String>::new());
        assert_eq!(b.packages, vec![String::from("a")]);
        assert_eq!(c.packages, vec![String::from("b"), String::from("a")]);

        a.merge(&c);
        assert_eq!(a.packages, vec![String::from("b"), String::from("a")]);

        b.merge(&c);
        assert_eq!(b.packages, vec![String::from("a"), String::from("b")]);
    }

    #[test]
    fn exclude_packages_merge() {
        let toml_a = r#"packages = []
                        exclude = ["a"]"#;
        let toml_b = r#"packages = ["a"]
                        exclude = ["b"]"#;
        let toml_c = r#"packages = ["b", "a"]
                        exclude = ["c"]"#;

        let mut a: Config = toml::from_slice(toml_a.as_bytes()).unwrap();
        let mut b: Config = toml::from_slice(toml_b.as_bytes()).unwrap();
        let c: Config = toml::from_slice(toml_c.as_bytes()).unwrap();

        assert_eq!(a.exclude, vec![String::from("a")]);
        assert_eq!(b.exclude, vec![String::from("b")]);
        assert_eq!(c.exclude, vec![String::from("c")]);

        a.merge(&c);
        assert_eq!(a.packages, vec![String::from("b")]);
        assert_eq!(a.exclude, vec![String::from("a"), String::from("c")]);

        b.merge(&c);
        assert_eq!(b.packages, vec![String::from("a")]);
        assert_eq!(b.exclude, vec![String::from("b"), String::from("c")]);
    }

    #[test]
    fn coveralls_merge() {
        let toml = r#"[a]
        coveralls = "abcd"
        report-uri = "https://example.com/report"

        [b]
        coveralls = "xyz"
        ciserver = "coveralls-ruby"
        "#;

        let configs = Config::parse_config_toml(toml.as_bytes()).unwrap();
        let mut a_config = configs.iter().find(|x| x.name == "a").unwrap().clone();
        let b_config = configs.iter().find(|x| x.name == "b").unwrap();
        a_config.merge(b_config);
        assert_eq!(a_config.coveralls, Some("xyz".to_string()));
        assert_eq!(
            a_config.ci_tool,
            Some(CiService::Other("coveralls-ruby".to_string()))
        );
        assert_eq!(
            a_config.report_uri,
            Some("https://example.com/report".to_string())
        );
    }

    #[test]
    fn output_dir_merge() {
        let toml = r#"[has_dir]
        output-dir = "foo"

        [no_dir]
        coveralls = "xyz"
        
        [other_dir]
        output-dir = "bar"
        "#;

        let configs = Config::parse_config_toml(toml.as_bytes()).unwrap();
        let has_dir = configs
            .iter()
            .find(|x| x.name == "has_dir")
            .unwrap()
            .clone();
        let no_dir = configs.iter().find(|x| x.name == "no_dir").unwrap().clone();
        let other_dir = configs
            .iter()
            .find(|x| x.name == "other_dir")
            .unwrap()
            .clone();

        let mut merged_into_has_dir = has_dir.clone();
        merged_into_has_dir.merge(&no_dir);
        assert_eq!(merged_into_has_dir.output_dir(), PathBuf::from("foo"));

        let mut merged_into_no_dir = no_dir.clone();
        merged_into_no_dir.merge(&has_dir);
        assert_eq!(merged_into_no_dir.output_dir(), PathBuf::from("foo"));

        let mut neither_merged_dir = no_dir.clone();
        neither_merged_dir.merge(&no_dir);
        assert_eq!(neither_merged_dir.output_dir(), env::current_dir().unwrap());

        let mut both_merged_dir = has_dir;
        both_merged_dir.merge(&other_dir);
        assert_eq!(both_merged_dir.output_dir(), PathBuf::from("bar"));
    }

    #[test]
    fn all_toml_options() {
        let toml = r#"[all]
        debug = true
        verbose = true
        ignore-panics = true
        count = true
        ignored = true
        force-clean = true
        branch = true
        forward = true
        coveralls = "hello"
        report-uri = "http://hello.com"
        no-default-features = true
        features = "a b"
        all-features = true
        workspace = true
        packages = ["pack_1"]
        exclude = ["pack_2"]
        exclude-files = ["fuzz/*"]
        timeout = "5s"
        release = true
        no-run = true
        locked = true
        frozen = true
        target = "wasm32-unknown-unknown"
        target-dir = "/tmp"
        offline = true
        Z = ["something-nightly"]
        out = ["Html"]
        run-types = ["Doctests"]
        root = "/home/rust"
        manifest-path = "/home/rust/foo/Cargo.toml"
        ciserver = "travis-ci"
        args = ["--nocapture"]
        test = ["test1", "test2"]
        bin = ["bin"]
        example = ["example"]
        bench = ["bench"]
        no-fail-fast = true
        profile = "Release"
        dump-traces = true
        all-targets = true
        "#;
        let mut configs = Config::parse_config_toml(toml.as_bytes()).unwrap();
        assert_eq!(configs.len(), 1);
        let config = configs.remove(0);
        assert!(config.debug);
        assert!(config.verbose);
        assert!(config.dump_traces);
        assert!(config.all_targets);
        assert!(config.ignore_panics);
        assert!(config.count);
        assert!(config.run_ignored);
        assert!(config.force_clean);
        assert!(config.branch_coverage);
        assert!(config.forward_signals);
        assert_eq!(config.coveralls, Some("hello".to_string()));
        assert_eq!(config.report_uri, Some("http://hello.com".to_string()));
        assert!(config.no_default_features);
        assert!(config.all_features);
        assert!(config.all);
        assert!(config.release);
        assert!(config.no_run);
        assert!(config.locked);
        assert!(config.frozen);
        assert_eq!(Some(String::from("wasm32-unknown-unknown")), config.target);
        assert_eq!(Some(Path::new("/tmp").to_path_buf()), config.target_dir);
        assert!(config.offline);
        assert_eq!(config.test_timeout, Duration::from_secs(5));
        assert_eq!(config.unstable_features.len(), 1);
        assert_eq!(config.unstable_features[0], "something-nightly");
        assert_eq!(config.varargs.len(), 1);
        assert_eq!(config.varargs[0], "--nocapture");
        assert_eq!(config.features, Some(String::from("a b")));
        assert_eq!(config.excluded_files_raw.len(), 1);
        assert_eq!(config.excluded_files_raw[0], "fuzz/*");
        assert_eq!(config.packages.len(), 1);
        assert_eq!(config.packages[0], "pack_1");
        assert_eq!(config.exclude.len(), 1);
        assert_eq!(config.exclude[0], "pack_2");
        assert_eq!(config.generate.len(), 1);
        assert_eq!(config.generate[0], OutputFile::Html);
        assert_eq!(config.run_types.len(), 1);
        assert_eq!(config.run_types[0], RunType::Doctests);
        assert_eq!(config.ci_tool, Some(CiService::Travis));
        assert_eq!(config.root, Some("/home/rust".to_string()));
        assert_eq!(config.manifest, PathBuf::from("/home/rust/foo/Cargo.toml"));
        assert_eq!(config.profile, Some("Release".to_string()));
        assert!(config.no_fail_fast);
        assert!(config.test_names.contains("test1"));
        assert!(config.test_names.contains("test2"));
        assert!(config.bin_names.contains("bin"));
        assert!(config.example_names.contains("example"));
        assert!(config.bench_names.contains("bench"));
    }
}
