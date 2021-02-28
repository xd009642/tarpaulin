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
fn is_hidden(entry: &Path, root: &Path) -> bool {
    let check_hidden = |e: &Path| e.iter().any(|x| x.to_string_lossy().starts_with('.'));
    match entry.strip_prefix(root) {
        Ok(e) => check_hidden(e),
        Err(_) => check_hidden(entry),
    }
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

fn is_part_of_project(e: &Path, root: &Path) -> bool {
    if e.is_absolute() && root.is_absolute() {
        e.starts_with(root)
    } else if root.is_absolute() {
        root.join(e).is_file()
    } else {
        // they're both relative and this isn't hit a lot - only really with FFI code
        true
    }
}

pub fn is_coverable_file_path(
    path: impl AsRef<Path>,
    root: impl AsRef<Path>,
    target: impl AsRef<Path>,
) -> bool {
    let e = path.as_ref();
    let ignorable_paths = !(is_target_folder(e, target.as_ref())
        || is_hidden(e, root.as_ref())
        || is_cargo_home(e, root.as_ref()));

    ignorable_paths && is_part_of_project(e, root.as_ref())
}

pub fn get_source_walker(config: &Config) -> impl Iterator<Item = DirEntry> {
    let root = config.root();
    let target = config.target_dir();

    let walker = WalkDir::new(&root).into_iter();
    walker
        .filter_entry(move |e| is_coverable_file_path(e.path(), &root, &target))
        .filter_map(|e| e.ok())
        .filter(|e| is_source_file(e))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn system_headers_not_coverable() {
        assert!(!is_coverable_file_path(
            "/usr/include/c++/9/iostream",
            "/home/ferris/rust/project",
            "/home/ferris/rust/project/target"
        ));
    }

    #[test]
    #[cfg(windows)]
    fn system_headers_not_coverable() {
        assert!(!is_coverable_file_path(
            "C:/Program Files/Visual Studio/include/c++/9/iostream",
            "C:/User/ferris/rust/project",
            "C:/User/ferris/rust/project/target"
        ));
    }

    #[test]
    fn basic_coverable_checks() {
        assert!(is_coverable_file_path(
            "/foo/src/lib.rs",
            "/foo",
            "/foo/target"
        ));
        assert!(!is_coverable_file_path(
            "/foo/target/lib.rs",
            "/foo",
            "/foo/target"
        ));
    }

    #[test]
    fn is_hidden_check() {
        // From issue#682
        let hidden_root = Path::new("/home/.jenkins/project/");
        let visible_root = Path::new("/home/jenkins/project/");

        let hidden_file = Path::new(".cargo/src/hello.rs");
        let visible_file = Path::new("src/hello.rs");

        assert!(is_hidden(&hidden_root.join(hidden_file), &hidden_root));
        assert!(is_hidden(&visible_root.join(hidden_file), &visible_root));

        assert!(!is_hidden(&hidden_root.join(visible_file), &hidden_root));
        assert!(!is_hidden(&visible_root.join(visible_file), &visible_root));
    }
}
