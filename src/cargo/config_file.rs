use crate::config::Config;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct CargoConfigFields {
    pub rust_doc_flags: Vec<String>,
    pub rust_flags: Vec<String>,
    pub env_vars: HashMap<String, String>,
    pub target_runner: Option<CargoTargetRunner>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CargoTargetRunner {
    pub path: PathBuf,
    pub args: Vec<OsString>,
}

fn resolve_value(path: &Path, name: &str, value: &cargo_config2::EnvConfigValue) -> Option<String> {
    if value.force || env::var(name).is_err() {
        if value.relative {
            Some(path.join(&value.value).display().to_string())
        } else {
            Some(value.value.as_os_str().to_string_lossy().to_string())
        }
    } else {
        None
    }
}

// https://doc.rust-lang.org/cargo/reference/config.html

fn host_target() -> Option<String> {
    let output = Command::new("rustc").arg("-vV").output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8(output.stdout).ok()?;
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(str::to_string)
}

pub fn get_cargo_config(config: &Config) -> CargoConfigFields {
    let cargo_config = match cargo_config2::Config::load_with_cwd(config.get_current_dir()) {
        Ok(c) => c,
        Err(e) => {
            warn!("Unable to read cargo configs: {}", e);
            return Default::default();
        }
    };

    let mut result = CargoConfigFields::default();
    if let Some(rust_flags) = cargo_config.build.rustflags.as_ref() {
        result.rust_flags = rust_flags.flags.clone();
    }
    if let Some(rust_flags) = cargo_config.build.rustdocflags.as_ref() {
        result.rust_doc_flags = rust_flags.flags.clone();
    }
    let root = config.get_current_dir();
    for (key, value) in &cargo_config.env {
        if let Some(value) = resolve_value(&root, key.as_str(), value) {
            result.env_vars.insert(key.to_string(), value);
        }
    }
    if let Some(target) = config.target.clone().or_else(host_target) {
        match cargo_config.runner(target.as_str()) {
            Ok(Some(runner)) => {
                info!("Will execute tests with {}", runner.path.display());
                result.target_runner = Some(CargoTargetRunner {
                    path: runner.path,
                    args: runner.args,
                });
            }
            Ok(None) => {}
            Err(e) => warn!("Unable to read target runner from cargo config: {}", e),
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time should be after the unix epoch")
            .as_nanos();
        let dir = env::temp_dir().join(format!("{prefix}_{timestamp}"));
        fs::create_dir_all(&dir).expect("test should create temp dir");
        dir
    }

    fn target_runner_env_key(target: &str) -> String {
        let mut target = target.replace(['-', '.'], "_");
        target.make_ascii_uppercase();
        format!("CARGO_TARGET_{target}_RUNNER")
    }

    fn temp_manifest_root(prefix: &str) -> PathBuf {
        let root = make_temp_dir(prefix);
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"runner_target_test\"\nversion = \"0.1.0\"\nedition = \"2018\"\n",
        )
        .expect("test should write a cargo manifest");
        root
    }

    fn restore_env_var(key: &str, previous: Option<OsString>) {
        unsafe {
            match previous {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }
    }

    #[test]
    fn target_runner_env_for_other_target_is_ignored() {
        let _lock = ENV_LOCK
            .lock()
            .expect("env test lock should not be poisoned");
        let root = temp_manifest_root("tarpaulin_other_target_runner");

        let runner_key = "CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER";
        let previous_runner = env::var_os(runner_key);
        unsafe {
            env::set_var(runner_key, "runner-for-the-wrong-target");
        }

        let mut matching_config = Config::default();
        matching_config.set_manifest(root.join("Cargo.toml"));
        matching_config.target = Some("wasm32-unknown-unknown".to_string());
        let matching_cargo_config = get_cargo_config(&matching_config);

        let mut other_config = Config::default();
        other_config.set_manifest(root.join("Cargo.toml"));
        other_config.target = Some("x86_64-unknown-linux-gnu".to_string());
        let other_cargo_config = get_cargo_config(&other_config);

        restore_env_var(runner_key, previous_runner);
        let _ = fs::remove_dir_all(root);

        let matching_runner = matching_cargo_config
            .target_runner
            .expect("runner env should resolve for its matching target");
        assert_eq!(
            matching_runner.path,
            PathBuf::from("runner-for-the-wrong-target")
        );
        assert_eq!(other_cargo_config.target_runner, None);
    }

    #[test]
    fn host_target_runner_env_is_used_without_explicit_target() {
        let _lock = ENV_LOCK
            .lock()
            .expect("env test lock should not be poisoned");
        let root = temp_manifest_root("tarpaulin_host_target_runner");
        let host = host_target().expect("rustc should report a host target");
        let runner_key = target_runner_env_key(&host);
        let previous_runner = env::var_os(&runner_key);
        unsafe {
            env::set_var(&runner_key, "runner-for-host-target");
        }

        let mut config = Config::default();
        config.set_manifest(root.join("Cargo.toml"));
        let cargo_config = get_cargo_config(&config);

        restore_env_var(&runner_key, previous_runner);
        let _ = fs::remove_dir_all(root);

        let runner = cargo_config
            .target_runner
            .expect("host target runner env should resolve without explicit target");
        assert_eq!(runner.path, PathBuf::from("runner-for-host-target"));
    }
}
