use crate::config::*;
use crate::errors::RunError;
use cargo_metadata::{
    diagnostic::DiagnosticLevel, parse_messages, CargoOpt, Message, MetadataCommand,
};
use log::{error, trace};
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use walkdir::WalkDir;

static DOCTEST_FOLDER: &str = "target/doctests";

#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TestBinary {
    path: PathBuf,
    ty: RunType,
    cargo_dir: Option<PathBuf>,
    pkg_name: Option<String>,
    pkg_version: Option<String>,
    pkg_authors: Option<Vec<String>>,
}

impl TestBinary {
    pub fn new(path: PathBuf, ty: RunType) -> Self {
        Self {
            path,
            ty,
            pkg_name: None,
            pkg_version: None,
            pkg_authors: None,
            cargo_dir: None,
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn run_type(&self) -> RunType {
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
        let mut cmd = create_command(manifest, config, ty);
        cmd.stdout(Stdio::piped());
        if !config.verbose {
            cmd.stderr(Stdio::null());
        }
        trace!("Running command {:?}", cmd);
        let mut child = cmd.spawn().map_err(|e| RunError::Cargo(e.to_string()))?;

        if ty != &RunType::Doctests {
            let mut package_ids = vec![];
            for msg in parse_messages(child.stdout.take().unwrap()) {
                match msg {
                    Ok(Message::CompilerArtifact(art)) => {
                        if let Some(path) = art.executable {
                            if !art.profile.test && ty == &RunType::Tests {
                                continue;
                            }
                            result.push(TestBinary::new(path, *ty));
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
        } else {
            // Need to get the packages...
            let package_roots = config
                .get_packages()
                .iter()
                .filter_map(|x| x.manifest_path.parent())
                .map(|x| x.join(DOCTEST_FOLDER))
                .collect::<Vec<PathBuf>>();

            for dir in &package_roots {
                let walker = WalkDir::new(dir).into_iter();
                for dt in walker
                    .filter_map(|e| e.ok())
                    .filter(|e| match e.metadata() {
                        Ok(ref m) if m.is_file() && m.len() != 0 => true,
                        _ => false,
                    })
                {
                    result.push(TestBinary::new(dt.path().to_path_buf(), *ty));
                }
            }
        }
        child.wait().map_err(|e| RunError::Cargo(e.to_string()))?;
    }
    Ok(result)
}

fn create_command(manifest_path: &str, config: &Config, ty: &RunType) -> Command {
    let mut test_cmd = Command::new("cargo");
    if *ty == RunType::Doctests {
        test_cmd.args(&["+nightly", "test"]);
    } else {
        if let Ok(toolchain) = env::var("RUSTUP_TOOLCHAIN") {
            if toolchain.starts_with("nightly") {
                test_cmd.arg("+nightly");
            } else if toolchain.starts_with("beta") {
                test_cmd.arg("+beta");
            }
        }
        if *ty != RunType::Examples {
            test_cmd.args(&["test", "--no-run"]);
        } else {
            test_cmd.arg("build");
        }
    }
    test_cmd.args(&["--message-format", "json", "--manifest-path", manifest_path]);
    match ty {
        RunType::Tests => test_cmd.arg("--tests"),
        RunType::Doctests => test_cmd.arg("--doc"),
        RunType::Benchmarks => test_cmd.arg("--benches"),
        RunType::Examples => test_cmd.arg("--examples"),
    };
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
    if !config.features.is_empty() {
        let mut args = vec!["--features".to_string()];
        args.extend_from_slice(&config.features);
        test_cmd.args(args);
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
    if let Some(ref target_dir) = config.target_dir {
        let args = vec![
            "--target-dir".to_string(),
            format!("{}", target_dir.display()),
        ];
        test_cmd.args(args);
    }
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

fn setup_environment(cmd: &mut Command, config: &Config) {
    cmd.env("TARPAULIN", "1");
    let rustflags = "RUSTFLAGS";
    let common_opts = " -C relocation-model=dynamic-no-pic -C link-dead-code -C debuginfo=2 ";
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
        common_opts, DOCTEST_FOLDER
    );
    if let Ok(vtemp) = env::var(rustdoc) {
        if !vtemp.contains("--persist-doctests") {
            value.push_str(vtemp.as_ref());
        }
    }
    cmd.env(rustdoc, value);
}
