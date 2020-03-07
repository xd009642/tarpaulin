use crate::config::*;
use cargo_metadata::parse_messages;
use std::path::PathBuf;
use std::process::Command;

static DOCTEST_FOLDER: &str = "target/doctests";

fn get_tests(config: &Config) -> Result<Vec<PathBuf>, String> {
    let manifest = match config.manifest.as_path().to_str() {
        Some(s) => s,
        None => "Cargo.toml",
    };
    let cmd = create_command(manifest, config)
        .spawn()
        .map_err(|e| format!("Cargo error: {}", e))?;

    for msg in parse_messages(cmd.stdout.take().unwrap()) {}
}

fn create_command(manifest_path: &str, config: &Config) -> Command {
    let mut test_cmd = Command::new("cargo").args(&[
        "test",
        "--message-format",
        "json",
        "--no-run",
        "--manifest-path",
        manifest_path,
    ]);

    // TODO Missing +nightly etc commands, flag_quiet/verbosity

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
    if config.target_dir.is_some() {
        let args = vec![
            "--target-dir".to_string(),
            format!("{}", config.target_dir.unwrap().display()),
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
    test_cmd
}
