use crate::config::*;
use crate::errors::RunError;
use crate::path_utils::{fix_unc_path, get_source_walker};
use cargo_metadata::{diagnostic::DiagnosticLevel, CargoOpt, Message, Metadata, MetadataCommand};
use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsStr;
use std::fs::{read_dir, read_to_string, remove_dir_all, remove_file, File};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use toml::Value;
use tracing::{debug, error, info, trace, warn};
use walkdir::{DirEntry, WalkDir};

const BUILD_PROFRAW: &str = "build_rs_cov.profraw";

cfg_if::cfg_if! {
    if #[cfg(target_os = "windows")] {
        pub const LD_PATH_VAR: &'static str ="PATH";
    } else if #[cfg(any(target_os = "macos", target_os = "ios"))] {
        pub const LD_PATH_VAR: &'static str = "DYLD_LIBRARY_PATH";
    } else {
        pub const LD_PATH_VAR: &'static str =  "LD_LIBRARY_PATH";
    }
}

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
}

impl CargoVersionInfo {
    fn supports_llvm_cov(&self) -> bool {
        (self.minor >= 50 && self.channel == Channel::Nightly) || self.minor >= 60
    }
}

#[derive(Clone, Debug, Default)]
pub struct CargoOutput {
    /// This contains all binaries we want to run to collect coverage from.
    pub test_binaries: Vec<TestBinary>,
    /// This covers binaries we don't want to run explicitly but may be called as part of tracing
    /// execution of other processes.
    pub binaries: Vec<PathBuf>,
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
    /// Linker paths used when linking the binary, this should be accessed via
    /// `Self::has_linker_paths` and `Self::ld_library_path` as there may be interaction with
    /// current environment. It's only made pub(crate) for the purpose of testing.
    pub(crate) linker_paths: Vec<PathBuf>,
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
        cfg_if::cfg_if! {
            if #[cfg(windows)] {
                const PATH_SEP: &str = ";";
            } else {
                const PATH_SEP: &str  = ":";
            }
        }

        let mut new_vals = self
            .linker_paths
            .iter()
            .map(|x| x.display().to_string())
            .collect::<Vec<String>>()
            .join(PATH_SEP);
        if let Ok(ld) = env::var(LD_PATH_VAR) {
            new_vals.push_str(PATH_SEP);
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
            r"cargo (\d)\.(\d+)\.\d+([\-betanightly]*)(\.[[:alnum:]]+)?",
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
                    Some(CargoVersionInfo {
                        major,
                        minor,
                        channel,
                    })
                } else {
                    None
                }
            })
            .unwrap_or(None)
    };
}

pub fn get_tests(config: &Config) -> Result<CargoOutput, RunError> {
    let mut result = CargoOutput::default();
    if config.force_clean() {
        let cleanup_dir = if config.release {
            config.target_dir().join("release")
        } else {
            config.target_dir().join("debug")
        };
        info!("Cleaning project");
        if cleanup_dir.exists() {
            if let Err(e) = remove_dir_all(cleanup_dir) {
                error!("Cargo clean failed: {e}");
            }
        }
    }
    let man_binding = config.manifest();
    let manifest = man_binding.as_path().to_str().unwrap_or("Cargo.toml");
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
    // Only matters for llvm cov and who knows, one day may not be needed
    let _ = remove_file(config.root().join(BUILD_PROFRAW));
    Ok(result)
}

fn run_cargo(
    metadata: &Metadata,
    manifest: &str,
    config: &Config,
    ty: Option<RunType>,
    result: &mut CargoOutput,
) -> Result<(), RunError> {
    let mut cmd = create_command(manifest, config, ty);
    if ty != Some(RunType::Doctests) {
        cmd.stdout(Stdio::piped());
    } else {
        clean_doctest_folder(config.doctest_dir());
        cmd.stdout(Stdio::null());
    }
    trace!("Running command {:?}", cmd);
    let mut child = cmd.spawn().map_err(|e| RunError::Cargo(e.to_string()))?;
    let update_from = result.test_binaries.len();
    let mut paths = match get_libdir(ty) {
        Some(path) => vec![path],
        None => vec![],
    };

    if ty != Some(RunType::Doctests) {
        let mut package_ids = vec![None; result.test_binaries.len()];
        let reader = std::io::BufReader::new(child.stdout.take().unwrap());
        let mut error = None;
        for msg in Message::parse_stream(reader) {
            match msg {
                Ok(Message::CompilerArtifact(art)) => {
                    if let Some(path) = art.executable.as_ref() {
                        if !art.profile.test && config.command == Mode::Test {
                            result.binaries.push(PathBuf::from(path));
                            continue;
                        }
                        result
                            .test_binaries
                            .push(TestBinary::new(fix_unc_path(path.as_std_path()), ty));
                        package_ids.push(Some(art.package_id.clone()));
                    }
                }
                Ok(Message::CompilerMessage(m)) => match m.message.level {
                    DiagnosticLevel::Error | DiagnosticLevel::Ice => {
                        let msg = if let Some(rendered) = m.message.rendered {
                            rendered
                        } else {
                            format!("{}: {}", m.target.name, m.message.message)
                        };
                        error = Some(RunError::TestCompile(msg));
                        break;
                    }
                    _ => {}
                },
                Ok(Message::BuildScriptExecuted(bs))
                    if !(bs.linked_libs.is_empty() && bs.linked_paths.is_empty()) =>
                {
                    let temp_paths = bs.linked_paths.iter().filter_map(|x| {
                        if x.as_std_path().exists() {
                            Some(x.as_std_path().to_path_buf())
                        } else if let Some(index) = x.as_str().find('=') {
                            Some(PathBuf::from(&x.as_str()[(index + 1)..]))
                        } else {
                            warn!("Couldn't resolve linker path: {}", x.as_str());
                            None
                        }
                    });
                    for p in temp_paths {
                        if !paths.contains(&p) {
                            paths.push(p);
                        }
                    }
                }
                Err(e) => {
                    error!("Error parsing cargo messages {e}");
                }
                _ => {}
            }
        }
        debug!("Linker paths: {:?}", paths);
        for bin in result.test_binaries.iter_mut().skip(update_from) {
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
            .test_binaries
            .iter_mut()
            .zip(package_ids.iter())
            .filter(|(_, b)| b.is_some())
        {
            if let Some(package) = package {
                let package = &metadata[package];
                res.cargo_dir = package
                    .manifest_path
                    .parent()
                    .map(|x| fix_unc_path(x.as_std_path()));
                res.pkg_name = Some(package.name.to_string());
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
        let walker = WalkDir::new(config.doctest_dir()).into_iter();
        let dir_entries = walker
            .filter_map(Result::ok)
            .filter(|e| matches!(e.metadata(), Ok(ref m) if m.is_file() && m.len() != 0))
            .filter(|e| {
                let ext = e.path().extension();
                ext != Some(OsStr::new("pdb"))
                    && ext != Some(OsStr::new("rs"))
                    && ext != Some(OsStr::new("rlib"))
            })
            .filter(|e| {
                !e.path()
                    .components()
                    .any(|x| x.as_os_str().to_string_lossy().contains("dSYM"))
            })
            .collect::<Vec<_>>();

        let should_panics = get_attribute_candidates(&dir_entries, config, "should_panic");
        let no_runs = get_attribute_candidates(&dir_entries, config, "no_run");
        for dt in &dir_entries {
            let mut tb = TestBinary::new(fix_unc_path(dt.path()), ty);

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
                    tb.cargo_dir = Some(fix_unc_path(current_dir));
                    break;
                }
                match current_dir.parent() {
                    Some(s) => {
                        current_dir = s;
                    }
                    None => break,
                }
            }
            result.test_binaries.push(tb);
        }
    }
    Ok(())
}

fn convert_to_prefix(p: &Path) -> Option<String> {
    let mut buffer = vec![];
    let mut p = Some(p);
    while let Some(path_temp) = p {
        // The only component of the path that should be lacking a filename is the final empty
        // parent of a relative path, which we don't want to include anyway.
        if let Some(name) = path_temp.file_name().and_then(|s| s.to_str()) {
            buffer.push(name.replace(['.', '-'], "_"));
        }
        p = path_temp.parent();
    }
    if buffer.is_empty() {
        None
    } else {
        buffer.reverse();
        Some(buffer.join("_"))
    }
}

fn is_prefix_match(prefix: &str, entry: &Path) -> bool {
    convert_to_prefix(entry).as_deref() == Some(prefix)
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
///
/// As of some point in June 2023 the naming convention has changed to include the package name in
/// the generated name which reduces collisions. Before it was done relative to the workspace
/// package folder not the workspace root.
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

fn start_cargo_command(ty: Option<RunType>) -> Command {
    let mut test_cmd = Command::new("cargo");
    let bootstrap = matches!(env::var("RUSTC_BOOTSTRAP").as_deref(), Ok("1"));
    let override_toolchain = if cfg!(windows) {
        let rustup_home = env::var("RUSTUP_HOME").unwrap_or(".rustup".into());
        if env::var("PATH").unwrap_or_default().contains(&rustup_home) {
            // So the specific cargo we're using is in the path var so rustup toolchains won't
            // work. This only started happening recently so special casing it for older versions
            env::remove_var("RUSTUP_TOOLCHAIN");
            false
        } else {
            true
        }
    } else {
        true
    };
    if ty == Some(RunType::Doctests) {
        if override_toolchain {
            if let Some(toolchain) = env::var("RUSTUP_TOOLCHAIN")
                .ok()
                .filter(|t| t.starts_with("nightly") || bootstrap)
            {
                test_cmd.args([format!("+{toolchain}").as_str()]);
            } else if !bootstrap && !is_nightly() {
                test_cmd.args(["+nightly"]);
            }
        }
    } else {
        if override_toolchain {
            if let Ok(toolchain) = env::var("RUSTUP_TOOLCHAIN") {
                test_cmd.arg(format!("+{toolchain}"));
            }
        }
    }
    test_cmd
}

fn get_libdir(ty: Option<RunType>) -> Option<PathBuf> {
    let mut test_cmd = start_cargo_command(ty);
    test_cmd.env("RUSTC_BOOTSTRAP", "1");
    test_cmd.args(["rustc", "-Z", "unstable-options", "--print=target-libdir"]);

    let output = match test_cmd.output() {
        Ok(output) => String::from_utf8_lossy(&output.stdout).trim().to_string(),
        Err(e) => {
            debug!("Unable to run cargo rustc command: {}", e);
            warn!("Unable to get target libdir proc macro crates in the workspace may not work. Consider adding `--exclude` to remove them from compilation");
            return None;
        }
    };
    Some(PathBuf::from(output))
}

fn create_command(manifest_path: &str, config: &Config, ty: Option<RunType>) -> Command {
    let mut test_cmd = start_cargo_command(ty);
    if ty == Some(RunType::Doctests) {
        test_cmd.args(["test"]);
    } else {
        if config.command == Mode::Test {
            test_cmd.args(["test", "--no-run"]);
        } else {
            test_cmd.arg("build");
        }
    }
    test_cmd.args(["--message-format", "json", "--manifest-path", manifest_path]);
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
        test_cmd.args(["--target", target]);
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
        test_cmd.arg(format!("-Z{feat}"));
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
        value.push_str(llvm_coverage_rustflag());
    } else if !config.no_dead_code {
        value.push_str(" -Clink-dead-code ");
    }
}

fn look_for_field_in_table(value: &Value, field: &str) -> String {
    let table = value.as_table().unwrap();

    if let Some(rustflags) = table.get(field) {
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

fn look_for_field_in_file(path: &Path, section: &str, field: &str) -> Option<String> {
    if let Ok(contents) = read_to_string(path) {
        let value = contents.parse::<Value>().ok()?;

        let value: Vec<String> = value
            .as_table()?
            .into_iter()
            .map(|(s, v)| {
                if s.as_str() == section {
                    look_for_field_in_table(v, field)
                } else {
                    String::new()
                }
            })
            .collect();

        Some(value.join(" "))
    } else {
        None
    }
}

fn look_for_field_in_section(path: &Path, section: &str, field: &str) -> Option<String> {
    let mut config_path = path.join("config");

    let value = look_for_field_in_file(&config_path, section, field);
    if value.is_some() {
        return value;
    }

    config_path.pop();
    config_path.push("config.toml");

    let value = look_for_field_in_file(&config_path, section, field);
    if value.is_some() {
        return value;
    }

    None
}

fn build_config_path(base: impl AsRef<Path>) -> PathBuf {
    let mut config_path = PathBuf::from(base.as_ref());
    config_path.push(base);
    config_path.push(".cargo");

    config_path
}

fn gather_config_field_from_section(config: &Config, section: &str, field: &str) -> String {
    if let Some(value) =
        look_for_field_in_section(&build_config_path(config.root()), section, field)
    {
        return value;
    }

    if let Ok(cargo_home_config) = env::var("CARGO_HOME") {
        if let Some(value) =
            look_for_field_in_section(&PathBuf::from(cargo_home_config), section, field)
        {
            return value;
        }
    }

    String::new()
}

pub fn rust_flags(config: &Config) -> String {
    const RUSTFLAGS: &str = "RUSTFLAGS";
    let mut value = config.rustflags.clone().unwrap_or_default();
    value.push_str(" -Cdebuginfo=2 ");
    value.push_str("-Cstrip=none ");
    if !config.avoid_cfg_tarpaulin {
        value.push_str("--cfg=tarpaulin ");
    }
    if config.release {
        value.push_str("-Cdebug-assertions=off ");
    }
    handle_llvm_flags(&mut value, config);
    lazy_static! {
        static ref DEBUG_INFO: Regex = Regex::new(r"\-C\s*debuginfo=\d").unwrap();
        static ref DEAD_CODE: Regex = Regex::new(r"\-C\s*link-dead-code").unwrap();
    }
    if let Ok(vtemp) = env::var(RUSTFLAGS) {
        let temp = DEBUG_INFO.replace_all(&vtemp, " ");
        if config.no_dead_code {
            value.push_str(&DEAD_CODE.replace_all(&temp, " "));
        } else {
            value.push_str(&temp);
        }
    } else {
        let vtemp = gather_config_field_from_section(config, "build", "rustflags");
        value.push_str(&DEBUG_INFO.replace_all(&vtemp, " "));
    }

    deduplicate_flags(&value)
}

pub fn rustdoc_flags(config: &Config) -> String {
    const RUSTDOC: &str = "RUSTDOCFLAGS";
    let common_opts = " -Cdebuginfo=2 --cfg=tarpaulin -Cstrip=none ";
    let mut value = format!(
        "{} --persist-doctests {} -Zunstable-options ",
        common_opts,
        config.doctest_dir().display()
    );
    if let Ok(vtemp) = env::var(RUSTDOC) {
        if !vtemp.contains("--persist-doctests") {
            value.push_str(vtemp.as_ref());
        }
    } else {
        let vtemp = gather_config_field_from_section(config, "build", "rustdocflags");
        value.push_str(&vtemp);
    }
    handle_llvm_flags(&mut value, config);
    deduplicate_flags(&value)
}

fn deduplicate_flags(flags: &str) -> String {
    lazy_static! {
        static ref CFG_FLAG: Regex = Regex::new(r#"\--cfg\s+"#).unwrap();
        static ref C_FLAG: Regex = Regex::new(r#"\-C\s+"#).unwrap();
        static ref Z_FLAG: Regex = Regex::new(r#"\-Z\s+"#).unwrap();
        static ref W_FLAG: Regex = Regex::new(r#"\-W\s+"#).unwrap();
        static ref A_FLAG: Regex = Regex::new(r#"\-A\s+"#).unwrap();
        static ref D_FLAG: Regex = Regex::new(r#"\-D\s+"#).unwrap();
    }

    // Going to remove the excess spaces to make it easier to filter things.
    let res = CFG_FLAG.replace_all(flags, "--cfg=");
    let res = C_FLAG.replace_all(&res, "-C");
    let res = Z_FLAG.replace_all(&res, "-Z");
    let res = W_FLAG.replace_all(&res, "-W");
    let res = A_FLAG.replace_all(&res, "-A");
    let res = D_FLAG.replace_all(&res, "-D");

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
    // https://github.com/rust-lang/rust/issues/107447
    cmd.env("LLVM_PROFILE_FILE", config.root().join(BUILD_PROFRAW));
    cmd.env("TARPAULIN", "1");
    let rustflags = "RUSTFLAGS";
    let value = rust_flags(config);
    cmd.env(rustflags, value);
    // doesn't matter if we don't use it
    let rustdoc = "RUSTDOCFLAGS";
    let value = rustdoc_flags(config);
    trace!("Setting RUSTDOCFLAGS='{}'", value);
    cmd.env(rustdoc, value);
    if let Ok(bootstrap) = env::var("RUSTC_BOOTSTRAP") {
        cmd.env("RUSTC_BOOTSTRAP", bootstrap);
    }
}

/// Taking the output of cargo version command return true if it's known to be a nightly channel
/// false otherwise.
fn is_nightly() -> bool {
    if let Some(version) = CARGO_VERSION_INFO.as_ref() {
        version.channel == Channel::Nightly
    } else {
        false
    }
}

pub fn supports_llvm_coverage() -> bool {
    if let Some(version) = CARGO_VERSION_INFO.as_ref() {
        version.supports_llvm_cov()
    } else {
        false
    }
}

pub fn llvm_coverage_rustflag() -> &'static str {
    match CARGO_VERSION_INFO.as_ref() {
        Some(v) if v.minor >= 60 => " -Cinstrument-coverage ",
        _ => " -Zinstrument-coverage ",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use toml::toml;

    #[test]
    fn can_get_libdir() {
        let path = get_libdir(Some(RunType::Tests)).unwrap();
        assert!(path.exists(), "{} doesn't exist", path.display());
    }

    #[test]
    #[cfg(not(any(windows, target_os = "macos")))]
    fn check_dead_code_flags() {
        let mut config = Config::default();
        config.set_engine(TraceEngine::Ptrace);
        assert!(rustdoc_flags(&config).contains("link-dead-code"));
        assert!(rust_flags(&config).contains("link-dead-code"));

        config.no_dead_code = true;
        assert!(!rustdoc_flags(&config).contains("link-dead-code"));
        assert!(!rust_flags(&config).contains("link-dead-code"));
    }

    #[test]
    fn parse_rustflags_from_toml() {
        let list_flags = toml! {
            rustflags = ["--cfg=foo", "--cfg=bar"]
        };
        let list_flags = toml::Value::Table(list_flags);

        assert_eq!(
            look_for_field_in_table(&list_flags, "rustflags"),
            "--cfg=foo --cfg=bar"
        );

        let string_flags = toml! {
            rustflags = "--cfg=bar --cfg=baz"
        };
        let string_flags = toml::Value::Table(string_flags);

        assert_eq!(
            look_for_field_in_table(&string_flags, "rustflags"),
            "--cfg=bar --cfg=baz"
        );
    }

    #[test]
    fn llvm_cov_compatible_version() {
        let version = CargoVersionInfo {
            major: 1,
            minor: 50,
            channel: Channel::Nightly,
        };
        assert!(version.supports_llvm_cov());
        let version = CargoVersionInfo {
            major: 1,
            minor: 60,
            channel: Channel::Stable,
        };
        assert!(version.supports_llvm_cov());
    }

    #[test]
    fn llvm_cov_incompatible_version() {
        let mut version = CargoVersionInfo {
            major: 1,
            minor: 48,
            channel: Channel::Stable,
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
