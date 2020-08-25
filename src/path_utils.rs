use crate::config::Config;
use std::env::var;
use std::ffi::OsStr;
use std::path::Path;
use walkdir::{DirEntry, WalkDir};

/// Returns true if the file is a rust source file
pub fn is_source_file(entry: &DirEntry) -> bool {
    let p = entry.path();
    p.extension() == Some(OsStr::new("rs"))
}

/// Returns true if the folder is a target folder
fn is_target_folder(entry: &Path, target: &Path) -> bool {
    entry.starts_with(&target)
}

/// Returns true if the file or folder is hidden
fn is_hidden(entry: &Path) -> bool {
    entry.iter().any(|x| x.to_string_lossy().starts_with('.'))
}

/// If `CARGO_HOME` is set filters out all folders within `CARGO_HOME`
fn is_cargo_home(entry: &Path, root: &Path) -> bool {
    match var("CARGO_HOME") {
        Ok(s) => {
            let path = Path::new(&s);
            if path.is_absolute() && entry.starts_with(path) {
                true
            } else {
                let home = root.join(path);
                entry.starts_with(&home)
            }
        }
        _ => false,
    }
}

pub fn is_coverable_file_path(
    e: &Path,
    root: &Path,
    target: &Path,
    config: Option<&Config>,
) -> bool {
    let valid_package = if e == root {
        true
    } else if let Some(c) = config {
        if !c.all {
            true
        } else {
            let check = |x| e.starts_with(root.join(x));
            let excluded = c.exclude.iter().any(check);
            let included = c.packages.iter().any(check);
            // If it is explicitly included or we've passed --workspace and not --exclude
            included || (c.all && !excluded)
        }
    } else {
        true
    };
    let project_file = !(is_target_folder(e, target) || is_hidden(e) || is_cargo_home(e, root));
    valid_package && project_file
}

pub fn get_source_walker(config: &Config) -> impl Iterator<Item = DirEntry> {
    let root = config.root();
    let target = config.target_dir();
    // TODO this is a bit annoying...
    let config = Some(config.clone());
    let walker = WalkDir::new(&root).into_iter();
    walker
        .filter_entry(move |e| is_coverable_file_path(e.path(), &root, &target, config.as_ref()))
        .filter_map(|e| e.ok())
        .filter(|e| is_source_file(e))
}
