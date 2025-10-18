use crate::config::Config;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::{env, fs::read_to_string};
use tracing::{error, warn};

#[derive(Debug, Default)]
pub struct CargoConfigFields {
    pub rust_doc_flags: Vec<String>,
    pub rust_flags: Vec<String>,
    // Because there's an order in the hierarchy
    pub env_vars: HashMap<String, String>,
}

fn find_config_files(config: &Config) -> Vec<PathBuf> {
    let mut paths = vec![];
    for path in config.root().ancestors() {
        let no_ext = path.join(".cargo/config");
        let ext = path.join(".cargo/config.toml");
        if no_ext.exists() {
            paths.push(no_ext);
        } else if ext.exists() {
            paths.push(ext);
        }
    }
    if let Ok(cargo_home_config) = env::var("CARGO_HOME") {
        let home = PathBuf::from(cargo_home_config);
        let no_ext = home.join(".cargo/config");
        let ext = home.join(".cargo/config.toml");
        if no_ext.exists() {
            paths.push(no_ext);
        } else if ext.exists() {
            paths.push(ext);
        }
    }
    paths
}

#[derive(Debug, Deserialize)]
struct CargoConfig {
    #[serde(default)]
    build: BuildSection,
    #[serde(default)]
    env: HashMap<String, EnvValue>,
}

#[derive(Debug, Default, Deserialize)]
struct BuildSection {
    rustflags: Option<Flags>,
    rustdocflags: Option<Flags>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Flags {
    Single(String),
    List(Vec<String>),
}

impl Flags {
    fn to_vec(&self) -> Vec<String> {
        match self {
            Self::Single(s) => vec![s.clone()],
            Self::List(l) => l.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EnvValue {
    Simple(String),
    Complex(EnvVar),
}

impl EnvValue {
    /// Checks if a value gets set and fixes the value if it's a path
    fn resolve(&self, path: &Path, name: &str) -> Option<String> {
        match self {
            Self::Simple(s) if env::var(name).is_err() => Some(s.clone()),
            Self::Complex(c) => c.resolve_value(path, name),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct EnvVar {
    value: String,
    #[serde(default)]
    force: bool,
    #[serde(default)]
    relative: bool,
}

impl EnvVar {
    fn resolve_value(&self, path: &Path, name: &str) -> Option<String> {
        if self.force || env::var(name).is_err() {
            Some(self.get_value_string(path))
        } else {
            None
        }
    }

    fn get_value_string(&self, path: &Path) -> String {
        if self.relative {
            if let Some(s) = path.ancestors().skip(2).next() {
                s.join(&self.value).display().to_string()
            } else {
                self.value.clone()
            }
        } else {
            self.value.clone()
        }
    }
}

// https://doc.rust-lang.org/cargo/reference/config.html

pub fn get_cargo_config(config: &Config) -> CargoConfigFields {
    let mut result = CargoConfigFields::default();

    let files = find_config_files(config);
    for config_file in &files {
        if let Ok(contents) = read_to_string(&config_file) {
            let value = match toml::from_str::<CargoConfig>(&contents) {
                Ok(c) => c,
                Err(e) => {
                    error!(
                        "Unable to read cargo config `{}`: {}",
                        config_file.display(),
                        e
                    );
                    continue;
                }
            };

            // Merge with the result
            if let Some(rustflags) = value.build.rustflags {
                result.rust_flags.append(&mut rustflags.to_vec());
            }
            if let Some(rustdocflags) = value.build.rustdocflags {
                result.rust_doc_flags.append(&mut rustdocflags.to_vec());
            }
            for (key, value) in value.env.iter() {
                if let Some(value) = value.resolve(&config_file, &key) {
                    if !result.env_vars.contains_key(key) {
                        result.env_vars.insert(key.clone(), value);
                    }
                }
            }
        } else {
            warn!("Couldn't read: {}", config_file.display());
        }
    }

    result
}
