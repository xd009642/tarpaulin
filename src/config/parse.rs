use crate::config::types::*;
use crate::path_utils::fix_unc_path;
use coveralls_api::CiService;
use serde::de::{self, Deserializer};
use std::env;
use std::fmt;
use std::fs::create_dir_all;
use std::path::PathBuf;
use std::str::FromStr;
use tracing::error;

pub(super) fn globs_from_excluded(strs: &[String]) -> Vec<glob::Pattern> {
    let mut files = vec![];
    for temp_str in strs {
        if let Ok(glob) = glob::Pattern::new(temp_str) {
            files.push(glob);
        } else {
            error!("Ignoring invalid glob pattern: '{}'", temp_str);
        }
    }
    files
}

pub(super) fn process_manifest(
    opt_manifest_path: Option<PathBuf>,
    opt_root: Option<PathBuf>,
) -> PathBuf {
    if let Some(path) = opt_manifest_path {
        return canonicalize_path(path);
    }

    let mut manifest = env::current_dir().unwrap();
    if let Some(path) = opt_root {
        manifest.push(path);
    }
    manifest.push("Cargo.toml");
    canonicalize_path(manifest)
}

pub(super) fn default_manifest() -> PathBuf {
    let mut manifest = env::current_dir().unwrap();
    manifest.push("Cargo.toml");
    fix_unc_path(&manifest.canonicalize().unwrap_or(manifest))
}

pub(super) fn process_target_dir(opt_path: Option<PathBuf>) -> Option<PathBuf> {
    let path = if let Some(path) = opt_path {
        path
    } else if let Some(envvar) = env::var_os("CARGO_TARPAULIN_TARGET_DIR") {
        PathBuf::from(envvar)
    } else {
        return None;
    };

    if !path.exists() {
        let _ = create_dir_all(&path);
    }
    Some(canonicalize_path(path))
}

pub(super) fn canonicalize_paths(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.into_iter().map(canonicalize_path).collect()
}

pub(super) fn canonicalize_path(mut path: PathBuf) -> PathBuf {
    if path.is_relative() {
        path = env::current_dir()
            .unwrap()
            .canonicalize()
            .unwrap()
            .join(&path);
        path = fix_unc_path(&path);
    }
    path
}

pub fn deserialize_ci_server<'de, D>(d: D) -> Result<Option<CiService>, D::Error>
where
    D: Deserializer<'de>,
{
    struct CiServerVisitor;

    impl<'de> de::Visitor<'de> for CiServerVisitor {
        type Value = Option<CiService>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("A string containing the ci-service name")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.is_empty() {
                Ok(None)
            } else {
                Ok(Some(Ci::from_str(v).unwrap().0))
            }
        }
    }

    d.deserialize_any(CiServerVisitor)
}
