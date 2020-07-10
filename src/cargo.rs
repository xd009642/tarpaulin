use crate::config::*;
use crate::errors::RunError;
use crate::path_utils::get_source_walker;
use cargo_metadata::{diagnostic::DiagnosticLevel, CargoOpt, Message, Metadata, MetadataCommand};
use log::{error, trace, warn};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs::{read_dir, remove_dir_all, File};
use std::io;
use std::io::{BufRead, BufReader};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::{DirEntry, WalkDir};

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Deserialize, Serialize)]
pub struct TestBinary {
    path: PathBuf,
    ty: Option<RunType>,
    cargo_dir: Option<PathBuf>,
    pkg_name: Option<String>,
    pkg_version: Option<String>,
    pkg_authors: Option<Vec<String>>,
    should_panic: bool,
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

    /// Should be `false` for normal tests and for doctests either `true` or
    /// `false` depending on the test attribute
    pub fn should_panic(&self) -> bool {
        self.should_panic
    }
}

impl DocTestBinaryMeta {
    fn new<P: AsRef<Path>>(test: P) -> Option<Self> {
        if let Some(Component::Normal(folder)) = test.as_ref().components().nth_back(1) {
            let temp = folder.to_string_lossy();
            let file_end = temp.rfind("rs").map(|i| i + 2)?;
            let end = temp.rfind("_")?;
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

pub fn get_tests(config: &Config) -> Result<Vec<TestBinary>, RunError> {
    let mut result = vec![];
    let manifest = match config.manifest.as_path().to_str() {
        Some(s) => s,
        None => "Cargo.toml",
    };
    let metadata = MetadataCommand::new()
        .manifest_path(manifest)
        .features(CargoOpt::AllFeatures)
        .exec()
        .map_err(|e| RunError::Cargo(e.to_string()))?;

    for ty in &config.run_types {
        run_cargo(&metadata, manifest, config, Some(*ty), &mut result)?;
    }
    if config.has_named_tests() {
        run_cargo(&metadata, manifest, config, None, &mut result)?
    } else if config.run_types.is_empty() {
        run_cargo(
            &metadata,
            manifest,
            config,
            Some(RunType::Tests),
            &mut result,
        )?;
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

    if ty != Some(RunType::Doctests) {
        let mut package_ids = vec![];
        let reader = std::io::BufReader::new(child.stdout.take().unwrap());
        for msg in Message::parse_stream(reader) {
            match msg {
                Ok(Message::CompilerArtifact(art)) => {
                    if let Some(path) = art.executable {
                        if !art.profile.test && ty == Some(RunType::Tests) {
                            continue;
                        }
                        result.push(TestBinary::new(path, ty));
                        package_ids.push(art.package_id.clone());
                    }
                }
                Ok(Message::CompilerMessage(m)) => match m.message.level {
                    DiagnosticLevel::Error | DiagnosticLevel::Ice => {
                        let _ = child.wait();
                        return Err(RunError::TestCompile(m.message.message));
                    }
                    _ => {}
                },
                Err(e) => {
                    error!("Error parsing cargo messages {}", e);
                }
                _ => {}
            }
        }
        for (res, package) in result.iter_mut().zip(package_ids.iter()) {
            let package = &metadata[package];
            res.cargo_dir = package.manifest_path.parent().map(|x| x.to_path_buf());
            res.pkg_name = Some(package.name.clone());
            res.pkg_version = Some(package.version.to_string());
            res.pkg_authors = Some(package.authors.clone());
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
            .filter_map(|e| e.ok())
            .filter(|e| match e.metadata() {
                Ok(ref m) if m.is_file() && m.len() != 0 => true,
                _ => false,
            })
            .collect::<Vec<_>>();

        let should_panics = get_panic_candidates(&dir_entries, config);
        for dt in &dir_entries {
            trace!("Found doctest binary {}", dt.path().display());
            let mut tb = TestBinary::new(dt.path().to_path_buf(), ty);
            // Now to do my magic!
            if let Some(meta) = DocTestBinaryMeta::new(dt.path()) {
                if let Some(lines) = should_panics.get(&meta.prefix) {
                    tb.should_panic |= lines.contains(&meta.line);
                }
            }
            result.push(tb);
        }
    }
    Ok(())
}

fn convert_to_prefix(p: &Path) -> Option<String> {
    p.to_str()
        .map(|s| s.replace(std::path::MAIN_SEPARATOR, "_").replace(".", "_"))
}

fn is_prefix_match(prefix: &str, entry: &Path) -> bool {
    convert_to_prefix(entry)
        .map(|s| s.ends_with(prefix))
        .unwrap_or(false)
}

/// This returns a map of the string prefixes for the file in the doc test and a list of lines
/// which contain the string `should_panic` it makes no guarantees that all these lines are a
/// doctest attribute showing panic behaviour (but some of them will be)
///
/// Currently all doctest files take the pattern of `{name}_{line}_{number}` where name is the
/// path to the file with directory separators and dots replaced with underscores. Therefore
/// each name could potentially map to many files as `src_some_folder_foo_rs_0_1` could go to
/// `src/some/folder_foo.rs` or `src/some/folder/foo.rs` here we're going to work on a heuristic
/// that any matching file is good because we can't do any better
fn get_panic_candidates(tests: &[DirEntry], config: &Config) -> HashMap<String, Vec<usize>> {
    let mut result = HashMap::new();
    let mut checked_files = HashSet::new();
    let root = config.root();
    for test in tests {
        if let Some(test_binary) = DocTestBinaryMeta::new(test.path()) {
            for dir_entry in get_source_walker(config) {
                let path = dir_entry.path();
                if path.is_file() {
                    if let Some(p) = path_relative_from(path, &root) {
                        if is_prefix_match(&test_binary.prefix, &p) {
                            if !checked_files.contains(path) {
                                checked_files.insert(path.to_path_buf());
                                trace!("Assessing {} for `should_panic` doctests", path.display());
                                let lines = find_panics_in_file(path).unwrap_or_default();
                                if !result.contains_key(&test_binary.prefix) {
                                    result.insert(test_binary.prefix.clone(), lines);
                                } else if let Some(current_lines) =
                                    result.get_mut(&test_binary.prefix)
                                {
                                    current_lines.extend_from_slice(&lines);
                                }
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

fn find_panics_in_file(file: &Path) -> io::Result<Vec<usize>> {
    let f = File::open(file)?;
    let reader = BufReader::new(f);
    let lines = reader
        .lines()
        .enumerate()
        .filter(|(_, l)| {
            l.as_ref()
                .map(|x| x.contains("should_panic"))
                .unwrap_or(false)
        })
        .map(|(i, _)| i + 1) // move from line index to line number
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
        test_cmd.args(&["test", "--no-run"]);
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
    if !config.varargs.is_empty() {
        let mut args = vec!["--".to_string()];
        args.extend_from_slice(&config.varargs);
        test_cmd.args(args);
    }
}

/// Old doc tests that no longer exist or where the line have changed can persist so delete them to
/// avoid confusing the results
fn clean_doctest_folder<P: AsRef<Path>>(doctest_dir: P) {
    if let Ok(rd) = read_dir(doctest_dir.as_ref()) {
        rd.flat_map(|e| e.ok())
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

fn setup_environment(cmd: &mut Command, config: &Config) {
    cmd.env("TARPAULIN", "1");
    let rustflags = "RUSTFLAGS";
    let common_opts =
        " -C relocation-model=dynamic-no-pic -C link-dead-code -C debuginfo=2 --cfg=tarpaulin ";
    let mut value = common_opts.to_string();
    if config.release {
        value = format!("{}-C debug-assertions=off ", value);
    }
    if let Ok(vtemp) = env::var(rustflags) {
        value.push_str(vtemp.as_ref());
    }
    cmd.env(rustflags, value);
    // doesn't matter if we don't use it
    let rustdoc = "RUSTDOCFLAGS";
    let mut value = format!(
        "{} --persist-doctests {} -Z unstable-options ",
        common_opts,
        config.doctest_dir().display()
    );
    if let Ok(vtemp) = env::var(rustdoc) {
        if !vtemp.contains("--persist-doctests") {
            value.push_str(vtemp.as_ref());
        }
    }
    trace!("Setting RUSTDOCFLAGS='{}'", value);
    cmd.env(rustdoc, value);
}
