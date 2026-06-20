use crate::config::Config;
use std::collections::HashMap;
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use tracing::warn;

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

pub fn get_cargo_config(config: &Config) -> CargoConfigFields {
    let cargo_config = match cargo_config2::Config::load_with_cwd(config.root()) {
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
    let root = config.root();
    for (key, value) in &cargo_config.env {
        if let Some(value) = resolve_value(&root, key.as_str(), value) {
            result.env_vars.insert(key.to_string(), value);
        }
    }
    if let Some(target) = config.target.as_ref() {
        match cargo_config.runner(target.as_str()) {
            Ok(Some(runner)) => {
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
