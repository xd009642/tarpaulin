use crate::config::*;
use crate::errors::RunError;
use crate::path_utils::get_source_walker;
use cargo_metadata::{diagnostic::DiagnosticLevel, CargoOpt, Message, Metadata, MetadataCommand};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{read_dir, read_to_string, remove_dir_all, File};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use toml::Value;
use tracing::{debug, error, info, trace, warn};
use walkdir::{DirEntry, WalkDir};

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
enum Channel {
    Stable,
    Beta,
    Nightly,
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
struct CargoVersionInfo {
    major: usize,
    minor: usize,
    channel: Channel,
    year: usize,
    month: usize,
    day: usize,
}

impl CargoVersionInfo {
    fn supports_llvm_cov(&self) -> bool {
        self.minor >= 50 && self.channel == Channel::Nightly
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct TestBinary {
    path: PathBuf,
    ty: Option<RunType>,
    cargo_dir: Option<PathBuf>,
    pkg_name: Option<String>,
    pkg_version: Option<String>,
    pkg_authors: Option<Vec<String>>,
    should_panic: bool,
    linker_paths: Vec<PathBuf>,
}

#[derive(Clone, Debug)]
struct DocTestBinaryMeta {
    prefix: String,
    line: usize,
}

impl TestBinary {
    pub fn new(path: PathBuf, ty: Option<RunType>) -> Self {
        Self {
            path,
            ty,
            pkg_name: None,
            pkg_version: None,
            pkg_authors: None,
            cargo_dir: None,
            should_panic: false,
            linker_paths: vec![],
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn run_type(&self) -> Option<RunType> {
        self.ty
    }

    pub fn manifest_dir(&self) -> &Option<PathBuf> {
        &self.cargo_dir
    }

    pub fn pkg_name(&self) -> &Option<String> {
        &self.pkg_name
    }

    pub fn pkg_version(&self) -> &Option<String> {
        &self.pkg_version
    }

    pub fn pkg_authors(&self) -> &Option<Vec<String>> {
        &self.pkg_authors
    }

    pub fn has_linker_paths(&self) -> bool {
        !self.linker_paths.is_empty()
    }

    pub fn is_test_type(&self) -> bool {
        matches!(self.ty, None | Some(RunType::Tests))
    }

    /// Convert linker paths to an LD_LIBRARY_PATH.
    /// TODO this won't work for windows when it's implemented
    pub fn ld_library_path(&self) -> String {
        let mut new_vals = self
            .linker_paths
            .iter()
            .map(|x| x.display().to_string())
            .collect::<Vec<String>>()
            .join(":");
        if let Ok(ld) = env::var("LD_LIBRARY_PATH") {
            new_vals.push(':');
            new_vals.push_str(ld.as_str());
        }
        new_vals
    }

    /// Should be `false` for normal tests and for doctests either `true` or
    /// `false` depending on the test attribute
    pub fn should_panic(&self) -> bool {
        self.should_panic
    }

    /// Convenience function to get the file name of the binary as a string, default string if the
    /// path has no filename as this should _never_ happen
    pub fn file_name(&self) -> String {
        self.path
            .file_name()
            .map(|x| x.to_string_lossy().to_string())
            .unwrap_or_default()
    }
}

impl DocTestBinaryMeta {
    fn new<P: AsRef<Path>>(test: P) -> Option<Self> {
        if let Some(Component::Normal(folder)) = test.as_ref().components().nth_back(1) {
            let temp = folder.to_string_lossy();
            let file_end = temp.rfind("rs").map(|i| i + 2)?;
            let end = temp.rfind('_')?;
            if end > file_end + 1 {
                let line = temp[(file_end + 1)..end].parse::<usize>().ok()?;
                Some(Self {
                    prefix: temp[..file_end].to_string(),
                    line,
                })
            } else {
                None
            }
        } else {
            None
        }
    }
}

lazy_static! {
    static ref CARGO_VERSION_INFO: Option<CargoVersionInfo> = {
        let version_info = Regex::new(
            r"cargo (\d)\.(\d+)\.\d+([\-betanightly]*) \([[:alnum:]]+ (\d{4})-(\d{2})-(\d{2})\)",
        )
        .unwrap();
        Command::new("cargo")
            .arg("--version")
            .output()
            .map(|x| {
                let s = String::from_utf8_lossy(&x.stdout);
                if let Some(cap) = version_info.captures(&s) {
                    let major = cap[1].parse().unwrap();
                    let minor = cap[2].parse().unwrap();
                    // We expect a string like `cargo 1.50.0-nightly (a0f433460 2020-02-01)
                    // the version number either has `-nightly` `-beta` or empty for stable
                    let channel = match &cap[3] {
                        "-nightly" => Channel::Nightly,
                        "-beta" => Channel::Beta,
                        _ => Channel::Stable,
                    };
                    let year = cap[4].parse().unwrap();
                    let month = cap[5].parse().unwrap();
                    let day = cap[6].parse().unwrap();
                    Some(CargoVersionInfo {
                        major,
                        minor,
                        channel,
                        year,
                        month,
                        day,
                    })
                } else {
                    None
                }
            })
            .unwrap_or(None)
    };
}

pub fn get_tests(config: &Config) -> Result<Vec<TestBinary>, RunError> {
    let mut result = vec![];
    if config.force_clean() {
        let cleanup_dir = if config.release {
            config.target_dir().join("release")
        } else {
            config.target_dir().join("debug")
        };
        info!("Cleaning project");
        if cleanup_dir.exists() {
            if let Err(e) = remove_dir_all(cleanup_dir) {
                error!("Cargo clean failed: {}", e);
            }
        }
    }
    let manifest = config.manifest.as_path().to_str().unwrap_or("Cargo.toml");
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .features(CargoOpt::AllFeatures)
        .exec()
        .map_err(|e| RunError::Cargo(e.to_string()))?;

    for ty in &config.run_types {
        run_cargo(&metadata, manifest, config, Some(*ty), &mut result)?;
    }
    if config.has_named_tests() {
        run_cargo(&metadata, manifest, config, None, &mut result)?;
    } else if config.run_types.is_empty() {
        let ty = if config.command == Mode::Test {
            Some(RunType::Tests)
        } else {
            None
        };
        run_cargo(&metadata, manifest, config, ty, &mut result)?;
    }
    Ok(result)
}

fn run_cargo(
    metadata: &Metadata,
    manifest: &str,
    config: &Config,
    ty: Option<RunType>,
    result: &mut Vec<TestBinary>,
) -> Result<(), RunError> {
    let mut cmd = create_command(manifest, config, ty);
    if ty != Some(RunType::Doctests) {
        cmd.stdout(Stdio::piped());
    } else {
        clean_doctest_folder(&config.doctest_dir());
        cmd.stdout(Stdio::null());
    }
    trace!("Running command {:?}", cmd);
    let mut child = cmd.spawn().map_err(|e| RunError::Cargo(e.to_string()))?;
    let update_from = result.len();
    let mut paths = vec![];

    if ty != Some(RunType::Doctests) {
        let mut package_ids = vec![None; result.len()];
        let reader = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut error = None;
        for msg in Message::parse_stream(reader) {
            match msg {
                Ok(Message::CompilerArtifact(art)) => {
                    if let Some(path) = art.executable.as_ref() {
                        if !art.profile.test && config.command == Mode::Test {
                            continue;
                        }
                        result.push(TestBinary::new(PathBuf::from(path), ty));
                        package_ids.push(Some(art.package_id.clone()));
                    }
                }
                Ok(Message::CompilerMessage(m)) => match m.message.level {
                    DiagnosticLevel::Error | DiagnosticLevel::Ice => {
                        let msg = format!("{}: {}", m.target.name, m.message.message);
                        error = Some(RunError::TestCompile(msg));
                        break;
                    }
                    _ => {}
                },
                Ok(Message::BuildScriptExecuted(bs))
                    if !(bs.linked_libs.is_empty() || bs.linked_paths.is_empty()) =>
                {
                    let mut temp_paths = bs
                        .linked_paths
                        .iter()
                        .filter_map(|x| {
                            if x.as_std_path().exists() {
                                Some(x.as_std_path().to_path_buf())
                            } else if let Some(index) = x.as_str().find('=') {
                                Some(PathBuf::from(&x.as_str()[(index + 1)..]))
                            } else {
                                warn!("Couldn't resolve linker path: {}", x.as_str());
                                None
                            }
                        })
                        .collect::<Vec<PathBuf>>();
                    for p in temp_paths.drain(..) {
                        if !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
                Err(e) => {
                    error!("Error parsing cargo messages {}", e);
                }
                _ => {}
            }
        }
        debug!("Linker paths: {:?}", paths);
        for bin in result.iter_mut().skip(update_from) {
            bin.linker_paths = paths.clone();
        }
        let status = child.wait().unwrap();
        if let Some(error) = error {
            return Err(error);
        }
        if !status.success() {
            return Err(RunError::Cargo("cargo run failed".to_string()));
        };
        for (res, package) in result
            .iter_mut()
            .zip(package_ids.iter())
            .filter(|(_, b)| b.is_some())
        {
            if let Some(package) = package {
                let package = &metadata[package];
                res.cargo_dir = package
                    .manifest_path
                    .parent()
                    .map(|x| PathBuf::from(x.to_path_buf()));
                res.pkg_name = Some(package.name.clone());
                res.pkg_version = Some(package.version.to_string());
                res.pkg_authors = Some(package.authors.clone());
            }
        }
        child.wait().map_err(|e| RunError::Cargo(e.to_string()))?;
    } else {
        // need to wait for compiling to finish before getting doctests
        // also need to wait with output to ensure the stdout buffer doesn't fill up
        let out = child
            .wait_with_output()
            .map_err(|e| RunError::Cargo(e.to_string()))?;
        if !out.status.success() {
            error!("Building doctests failed");
            return Err(RunError::Cargo("Building doctest failed".to_string()));
        }
        let walker = WalkDir::new(&config.doctest_dir()).into_iter();
        let dir_entries = walker
            .filter_map(Result::ok)
            .filter(|e| matches!(e.metadata(), Ok(ref m) if m.is_file() && m.len() != 0))
            .collect::<Vec<_>>();
        let should_panics = get_attribute_candidates(&dir_entries, config, "should_panic");
        let no_runs = get_attribute_candidates(&dir_entries, config, "no_run");
        for dt in &dir_entries {
            let mut tb = TestBinary::new(dt.path().to_path_buf(), ty);

            if let Some(meta) = DocTestBinaryMeta::new(dt.path()) {
                if no_runs
                    .get(&meta.prefix)
                    .map(|x| x.contains(&meta.line))
                    .unwrap_or(false)
                {
                    info!("Skipping no_run doctest: {}", dt.path().display());
                    continue;
                }
                if let Some(lines) = should_panics.get(&meta.prefix) {
                    tb.should_panic |= lines.contains(&meta.line);
                }
            }
            let mut current_dir = dt.path();
            loop {
                if current_dir.is_dir() && current_dir.join("Cargo.toml").exists() {
                    tb.cargo_dir = Some(current_dir.to_path_buf());
                    break;
                }
                match current_dir.parent() {
                    Some(s) => {
                        current_dir = s;
                    }
                    None => break,
                }
            }
            result.push(tb);
        }
    }
    Ok(())
}

fn convert_to_prefix(p: &Path) -> Option<String> {
    // Need to go from directory after last one with Cargo.toml
    let convert_name = |p: &Path| {
        if let Some(s) = p.file_name() {
            s.to_str().map(|x| x.replace('.', "_")).unwrap_or_default()
        } else {
            String::new()
        }
    };
    let mut buffer = vec![convert_name(p)];
    let mut parent = p.parent();
    while let Some(path_temp) = parent {
        if !path_temp.join("Cargo.toml").exists() {
            buffer.insert(0, convert_name(path_temp));
        } else {
            break;
        }
        parent = path_temp.parent();
    }
    if buffer.is_empty() {
        None
    } else {
        Some(buffer.join("_"))
    }
}

fn is_prefix_match(prefix: &str, entry: &Path) -> bool {
    convert_to_prefix(entry)
        .map(|s| s.contains(prefix))
        .unwrap_or(false)
}

/// This returns a map of the string prefixes for the file in the doc test and a list of lines
/// which contain the string `should_panic` it makes no guarantees that all these lines are a
/// doctest attribute showing panic behaviour (but some of them will be)
///
/// Currently all doctest files take the pattern of `{name}_{line}_{number}` where name is the
/// path to the file with directory separators and dots replaced with underscores. Therefore
/// each name could potentially map to many files as `src_some_folder_foo_rs_1_1` could go to
/// `src/some/folder_foo.rs` or `src/some/folder/foo.rs` here we're going to work on a heuristic
/// that any matching file is good because we can't do any better
fn get_attribute_candidates(
    tests: &[DirEntry],
    config: &Config,
    attribute: &str,
) -> HashMap<String, Vec<usize>> {
    let mut result = HashMap::new();
    let mut checked_files = HashSet::new();
    let root = config.root();
    for test in tests {
        if let Some(test_binary) = DocTestBinaryMeta::new(test.path()) {
            for dir_entry in get_source_walker(config) {
                let path = dir_entry.path();
                if path.is_file() {
                    if let Some(p) = path_relative_from(path, &root) {
                        if is_prefix_match(&test_binary.prefix, &p) && !checked_files.contains(path)
                        {
                            checked_files.insert(path.to_path_buf());
                            let lines = find_str_in_file(path, attribute).unwrap_or_default();
                            if !result.contains_key(&test_binary.prefix) {
                                result.insert(test_binary.prefix.clone(), lines);
                            } else if let Some(current_lines) = result.get_mut(&test_binary.prefix)
                            {
                                current_lines.extend_from_slice(&lines);
                            }
                        }
                    }
                }
            }
        } else {
            warn!(
                "Invalid characters in name of doctest {}",
                test.path().display()
            );
        }
    }
    result
}

fn find_str_in_file(file: &Path, value: &str) -> io::Result<Vec<usize>> {
    let f = File::open(file)?;
    let reader = BufReader::new(f);
    let lines = reader
        .lines()
        .enumerate()
        .filter(|(_, l)| l.as_ref().map(|x| x.contains(value)).unwrap_or(false))
        .map(|(i, _)| i + 1) // Move from line index to line number
        .collect();
    Ok(lines)
}

fn create_command(manifest_path: &str, config: &Config, ty: Option<RunType>) -> Command {
    let mut test_cmd = Command::new("cargo");
    if ty == Some(RunType::Doctests) {
        if let Some(toolchain) = env::var("RUSTUP_TOOLCHAIN")
            .ok()
            .filter(|t| t.starts_with("nightly"))
        {
            test_cmd.args(&[format!("+{}", toolchain).as_str(), "test"]);
        } else {
            test_cmd.args(&["+nightly", "test"]);
        }
    } else {
        if let Ok(toolchain) = env::var("RUSTUP_TOOLCHAIN") {
            test_cmd.arg(format!("+{}", toolchain));
        }
        if config.command == Mode::Test {
            test_cmd.args(&["test", "--no-run"]);
        } else {
            test_cmd.arg("build");
        }
    }
    test_cmd.args(&["--message-format", "json", "--manifest-path", manifest_path]);
    if let Some(ty) = ty {
        match ty {
            RunType::Tests => test_cmd.arg("--tests"),
            RunType::Doctests => test_cmd.arg("--doc"),
            RunType::Benchmarks => test_cmd.arg("--benches"),
            RunType::Examples => test_cmd.arg("--examples"),
            RunType::AllTargets => test_cmd.arg("--all-targets"),
            RunType::Lib => test_cmd.arg("--lib"),
            RunType::Bins => test_cmd.arg("--bins"),
        };
    } else {
        for test in &config.test_names {
            test_cmd.arg("--test");
            test_cmd.arg(test);
        }
        for test in &config.bin_names {
            test_cmd.arg("--bin");
            test_cmd.arg(test);
        }
        for test in &config.example_names {
            test_cmd.arg("--example");
            test_cmd.arg(test);
        }
        for test in &config.bench_names {
            test_cmd.arg("--bench");
            test_cmd.arg(test);
        }
    }
    init_args(&mut test_cmd, config);
    setup_environment(&mut test_cmd, config);
    test_cmd
}

fn init_args(test_cmd: &mut Command, config: &Config) {
    if config.debug {
        test_cmd.arg("-vvv");
    } else if config.verbose {
        test_cmd.arg("-v");
    }
    if config.locked {
        test_cmd.arg("--locked");
    }
    if config.frozen {
        test_cmd.arg("--frozen");
    }
    if config.no_fail_fast {
        test_cmd.arg("--no-fail-fast");
    }
    if let Some(profile) = config.profile.as_ref() {
        test_cmd.arg("--profile");
        test_cmd.arg(profile);
    }
    if let Some(jobs) = config.jobs {
        test_cmd.arg("--jobs");
        test_cmd.arg(jobs.to_string());
    }
    if let Some(features) = config.features.as_ref() {
        test_cmd.arg("--features");
        test_cmd.arg(features);
    }
    if config.all_targets {
        test_cmd.arg("--all-targets");
    }
    if config.all_features {
        test_cmd.arg("--all-features");
    }
    if config.no_default_features {
        test_cmd.arg("--no-default-features");
    }
    if config.all {
        test_cmd.arg("--workspace");
    }
    if config.release {
        test_cmd.arg("--release");
    }
    config.packages.iter().for_each(|package| {
        test_cmd.arg("--package");
        test_cmd.arg(package);
    });
    config.exclude.iter().for_each(|package| {
        test_cmd.arg("--exclude");
        test_cmd.arg(package);
    });
    test_cmd.arg("--color");
    test_cmd.arg(config.color.to_string().to_ascii_lowercase());
    if let Some(target) = config.target.as_ref() {
        test_cmd.args(&["--target", target]);
    }
    let args = vec![
        "--target-dir".to_string(),
        format!("{}", config.target_dir().display()),
    ];
    test_cmd.args(args);
    if config.offline {
        test_cmd.arg("--offline");
    }
    for feat in &config.unstable_features {
        test_cmd.arg(format!("-Z{}", feat));
    }
    if config.command == Mode::Test && !config.varargs.is_empty() {
        let mut args = vec!["--".to_string()];
        args.extend_from_slice(&config.varargs);
        test_cmd.args(args);
    }
}

/// Old doc tests that no longer exist or where the line have changed can persist so delete them to
/// avoid confusing the results
fn clean_doctest_folder<P: AsRef<Path>>(doctest_dir: P) {
    if let Ok(rd) = read_dir(doctest_dir.as_ref()) {
        rd.flat_map(Result::ok)
            .filter(|e| {
                e.path()
                    .components()
                    .next_back()
                    .map(|e| e.as_os_str().to_string_lossy().contains("rs"))
                    .unwrap_or(false)
            })
            .for_each(|e| {
                if let Err(err) = remove_dir_all(e.path()) {
                    warn!("Failed to delete {}: {}", e.path().display(), err);
                }
            });
    }
}

fn handle_llvm_flags(value: &mut String, config: &Config) {
    if config.engine() == TraceEngine::Llvm {
        value.push_str("-Zinstrument-coverage ");
    }
    if cfg!(not(windows)) {
        value.push_str(" -Clink-dead-code ");
    }
}

pub fn rustdoc_flags(config: &Config) -> String {
    const RUSTDOC: &str = "RUSTDOCFLAGS";
    let common_opts = " -Clink-dead-code -Cdebuginfo=2 --cfg=tarpaulin ";
    let mut value = format!(
        "{} --persist-doctests {} -Zunstable-options ",
        common_opts,
        config.doctest_dir().display()
    );
    if let Ok(vtemp) = env::var(RUSTDOC) {
        if !vtemp.contains("--persist-doctests") {
            value.push_str(vtemp.as_ref());
        }
    }
    handle_llvm_flags(&mut value, config);
    deduplicate_flags(&value)
}

fn look_for_rustflags_in_table(value: &Value) -> String {
    let table = value.as_table().unwrap();

    if let Some(rustflags) = table.get("rustflags") {
        if rustflags.is_array() {
            let vec_of_flags: Vec<String> = rustflags
                .as_array()
                .unwrap()
                .iter()
                .filter_map(Value::as_str)
                .map(ToString::to_string)
                .collect();

            vec_of_flags.join(" ")
        } else if rustflags.is_str() {
            rustflags.as_str().unwrap().to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    }
}

fn look_for_rustflags_in_file(path: &Path) -> Option<String> {
    if let Ok(contents) = read_to_string(path) {
        let value = contents.parse::<Value>().ok()?;

        let rustflags_in_file: Vec<String> = value
            .as_table()?
            .into_iter()
            .map(|(s, v)| {
                if s.as_str() == "build" {
                    look_for_rustflags_in_table(v)
                } else {
                    String::new()
                }
            })
            .collect();

        Some(rustflags_in_file.join(" "))
    } else {
        None
    }
}

fn look_for_rustflags_in(path: &Path) -> Option<String> {
    let mut config_path = path.join("config");

    let rustflags = look_for_rustflags_in_file(&config_path);
    if rustflags.is_some() {
        return rustflags;
    }

    config_path.pop();
    config_path.push("config.toml");

    let rustflags = look_for_rustflags_in_file(&config_path);
    if rustflags.is_some() {
        return rustflags;
    }

    None
}

fn build_config_path(base: impl AsRef<Path>) -> PathBuf {
    let mut config_path = PathBuf::from(base.as_ref());
    config_path.push(base);
    config_path.push(".cargo");

    config_path
}

fn gather_config_rust_flags(config: &Config) -> String {
    if let Some(rustflags) = look_for_rustflags_in(&build_config_path(&config.root())) {
        return rustflags;
    }

    if let Ok(cargo_home_config) = env::var("CARGO_HOME") {
        if let Some(rustflags) = look_for_rustflags_in(&PathBuf::from(cargo_home_config)) {
            return rustflags;
        }
    }

    String::new()
}

pub fn rust_flags(config: &Config) -> String {
    const RUSTFLAGS: &str = "RUSTFLAGS";
    let mut value = config.rustflags.clone().unwrap_or_default();
    value.push_str(" -Cdebuginfo=2 ");
    if !config.avoid_cfg_tarpaulin {
        value.push_str("--cfg=tarpaulin ");
    }
    if config.release {
        value.push_str("-Cdebug-assertions=off ");
    }
    handle_llvm_flags(&mut value, config);
    lazy_static! {
        static ref DEBUG_INFO: Regex = Regex::new(r#"\-C\s*debuginfo=\d"#).unwrap();
    }
    if let Ok(vtemp) = env::var(RUSTFLAGS) {
        value.push_str(&DEBUG_INFO.replace_all(&vtemp, " "));
    } else {
        let vtemp = gather_config_rust_flags(config);
        value.push_str(&DEBUG_INFO.replace_all(&vtemp, " "));
    }
    deduplicate_flags(&value)
}

fn deduplicate_flags(flags: &str) -> String {
    lazy_static! {
        static ref CFG_FLAG: Regex = Regex::new(r#"\--cfg\s+"#).unwrap();
        static ref C_FLAG: Regex = Regex::new(r#"\-C\s+"#).unwrap();
        static ref Z_FLAG: Regex = Regex::new(r#"\-Z\s+"#).unwrap();
    }

    // Gonna remove the excess spaces to make it easier to filter things
    let res = CFG_FLAG.replace_all(flags, "--cfg=");
    let res = C_FLAG.replace_all(&res, "-C");
    let res = Z_FLAG.replace_all(&res, "-Z");

    let mut flag_set = HashSet::new();
    let mut result = vec![];
    for val in res.split_whitespace() {
        if val.starts_with("--cfg") {
            if !flag_set.contains(&val) {
                result.push(val);
                flag_set.insert(val);
            }
        } else {
            let id = val.split('=').next().unwrap();
            if !flag_set.contains(id) {
                flag_set.insert(id);
                result.push(val);
            }
        }
    }
    result.join(" ")
}

fn setup_environment(cmd: &mut Command, config: &Config) {
    cmd.env("TARPAULIN", "1");
    let rustflags = "RUSTFLAGS";
    let value = rust_flags(config);
    cmd.env(rustflags, value);
    // doesn't matter if we don't use it
    let rustdoc = "RUSTDOCFLAGS";
    let value = rustdoc_flags(config);
    trace!("Setting RUSTDOCFLAGS='{}'", value);
    cmd.env(rustdoc, value);
}

pub fn supports_llvm_coverage() -> bool {
    if let Some(version) = CARGO_VERSION_INFO.as_ref() {
        version.supports_llvm_cov()
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml::toml;

    #[test]
    fn parse_rustflags_from_toml() {
        let list_flags = toml! {
            rustflags = ["--cfg=foo", "--cfg=bar"]
        };

        assert_eq!(
            look_for_rustflags_in_table(&list_flags),
            "--cfg=foo --cfg=bar"
        );

        let string_flags = toml! {
            rustflags = "--cfg=bar --cfg=baz"
        };

        assert_eq!(
            look_for_rustflags_in_table(&string_flags),
            "--cfg=bar --cfg=baz"
        );
    }

    #[test]
    fn llvm_cov_compatible_version() {
        let version = CargoVersionInfo {
            major: 1,
            minor: 50,
            channel: Channel::Nightly,
            year: 2020,
            month: 12,
            day: 22,
        };
        assert!(version.supports_llvm_cov());
    }

    #[test]
    fn llvm_cov_incompatible_version() {
        let mut version = CargoVersionInfo {
            major: 1,
            minor: 48,
            channel: Channel::Stable,
            year: 2020,
            month: 10,
            day: 14,
        };
        assert!(!version.supports_llvm_cov());
        version.channel = Channel::Beta;
        assert!(!version.supports_llvm_cov());
        version.minor = 50;
        assert!(!version.supports_llvm_cov());
        version.minor = 58;
        version.channel = Channel::Stable;
        assert!(!version.supports_llvm_cov());
    }

    #[test]
    fn no_duplicate_flags() {
        assert_eq!(
            deduplicate_flags("--cfg=tarpaulin --cfg tarpaulin"),
            "--cfg=tarpaulin"
        );
        assert_eq!(
            deduplicate_flags("-Clink-dead-code -Zinstrument-coverage -C link-dead-code"),
            "-Clink-dead-code -Zinstrument-coverage"
        );
        assert_eq!(
            deduplicate_flags("-Clink-dead-code -Zinstrument-coverage -Zinstrument-coverage"),
            "-Clink-dead-code -Zinstrument-coverage"
        );
        assert_eq!(
            deduplicate_flags("-Clink-dead-code -Zinstrument-coverage -Cinstrument-coverage"),
            "-Clink-dead-code -Zinstrument-coverage -Cinstrument-coverage"
        );

        assert_eq!(
            deduplicate_flags("--cfg=tarpaulin --cfg tarpauline --cfg=tarp"),
            "--cfg=tarpaulin --cfg=tarpauline --cfg=tarp"
        );
    }
}
