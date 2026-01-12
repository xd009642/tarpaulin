use crate::config::Config;
use std::env::var;
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

/// On windows removes the `\\?\\` prefix to UNC paths. For other operation systems just turns the
/// `Path` into a `PathBuf`
pub fn fix_unc_path(res: &Path) -> PathBuf {
    if cfg!(windows) {
        let res_str = res.display().to_string();
        if res_str.starts_with(r#"\\?"#) {
            PathBuf::from(res_str.replace(r#"\\?\"#, ""))
        } else {
            res.to_path_buf()
        }
    } else {
        res.to_path_buf()
    }
}

/// Returns true if the file is a rust source file
pub fn is_profraw_file(entry: &DirEntry) -> bool {
    let p = entry.path();
    p.is_file() && p.extension() == Some(OsStr::new("profraw"))
}

/// Returns true if the file is a rust source file
pub fn is_source_file(entry: &DirEntry) -> bool {
    let p = entry.path();
    p.is_file() && p.extension() == Some(OsStr::new("rs"))
}

/// Returns true if the folder is a target folder
fn is_target_folder(entry: &Path, target: &Path) -> bool {
    entry.starts_with(target)
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
                entry.starts_with(home)
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

pub fn get_source_walker(config: &Config) -> impl Iterator<Item = DirEntry> + '_ {
    let root = config.root();
    let target = config.target_dir();

    let walker = WalkDir::new(&root).into_iter();
    walker
        .filter_entry(move |e| {
            if !config.include_tests() && is_tests_folder_package(&root, e.path()) {
                return false; //Removes entire tests folder at once
            }
            is_coverable_file_path(e.path(), &root, &target)
        })
        .filter_map(Result::ok)
        .filter(move |e| !(config.exclude_path(e.path())))
        .filter(move |e| config.include_path(e.path()))
        .filter(is_source_file)
}

fn is_tests_folder_package(root: &Path, path: &Path) -> bool {
    let mut is_pkg_tests: bool = false;
    let tests_folder_name = "tests";

    // Ensure `path` is under `root`
    let relative = match path.strip_prefix(root) {
        Ok(p) => p,
        Err(_) => return false,
    };

    // check if the path contains a `tests` folder (platform independent)
    let has_tests_component = relative.components().any(|c| {
        matches!(c, Component::Normal(name) if name == tests_folder_name)
    });

    if has_tests_component {
        // Locate the actual `tests` directory in the ancestor chain. stopping at root
        if let Some(tests_dir) = path.ancestors().take_while(|anc| *anc != root).find(|anc| {
            anc.file_name().map(|n| n == tests_folder_name).unwrap_or(false)
        }) {
            if let Some(pkg_dir) = tests_dir.parent() {
                is_pkg_tests = pkg_dir.join("Cargo.toml").is_file()
                    && pkg_dir.join("src").is_dir();
            }
        }
    }

    is_pkg_tests
}

pub fn get_profile_walker(config: &Config) -> impl Iterator<Item = DirEntry> {
    let walker = WalkDir::new(config.profraw_dir()).into_iter();
    walker.filter_map(Result::ok).filter(is_profraw_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_temp_dir(prefix: &str) -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("{}_{}", prefix, n));
        let _ = fs::create_dir_all(&dir);
        dir
    }

    #[test]
    fn is_tests_folder_package_no_tests_component_returns_false() {
        let base = make_temp_dir("no_tests_component");
        let p = base.join("some").join("path").join("lib.rs");
        assert!(!is_tests_folder_package(&base, &p));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn is_tests_folder_package_testsx_component_returns_false() {
        let base = make_temp_dir("testsx_component");
        let p = base.join("some").join("testsX").join("file.rs");
        let _ = fs::create_dir_all(p.parent().unwrap());
        assert!(!is_tests_folder_package(&base, &p));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn is_tests_folder_package_tests_without_src_returns_false() {
        let base = make_temp_dir("tests_no_src");
        let pkg = base.join("pkg");
        let tests = pkg.join("tests");
        let _ = fs::create_dir_all(&tests);
        let test_file = tests.join("a.rs");
        let _ = File::create(&test_file);
        let cargo = pkg.join("Cargo.toml");
        let mut f = File::create(&cargo).expect("create Cargo.toml");
        let _ = f.write_all(b"[package]\nname = \"pkg3\"\nversion = \"0.0.0\"");
        // Ensure no src dir and no Cargo.toml
        assert!(!pkg.join("src").exists());
        assert!(pkg.join("Cargo.toml").exists());
        assert!(!is_tests_folder_package(&base, &test_file));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn is_tests_folder_package_tests_without_cargo_returns_false() {
        let base = make_temp_dir("tests_no_cargo");
        let pkg = base.join("pkg2");
        let tests = pkg.join("tests");
        let src = pkg.join("src");
        let _ = fs::create_dir_all(&src);
        let _ = fs::create_dir_all(&tests);
        let test_file = tests.join("b.rs");
        let _ = File::create(&test_file);
        // src exists but Cargo.toml does not
        assert!(src.is_dir());
        assert!(!pkg.join("Cargo.toml").exists());
        assert!(!is_tests_folder_package(&base, &test_file));
        let _ = fs::remove_dir_all(base);
    }

    #[test]
    fn is_tests_folder_package_tests_with_src_and_cargo_returns_true() {
        let base = make_temp_dir("tests_with_both");
        let pkg = base.join("pkg3");
        let tests = pkg.join("tests");
        let src = pkg.join("src");
        let _ = fs::create_dir_all(&src);
        let _ = fs::create_dir_all(&tests);
        let cargo = pkg.join("Cargo.toml");
        let mut f = File::create(&cargo).expect("create Cargo.toml");
        let _ = f.write_all(b"[package]\nname = \"pkg3\"\nversion = \"0.0.0\"");
        let test_file = tests.join("c.rs");
        let _ = File::create(&test_file);
        assert!(is_tests_folder_package(&base, &test_file));
        let _ = fs::remove_dir_all(base);
    }

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

        assert!(is_hidden(&hidden_root.join(hidden_file), hidden_root));
        assert!(is_hidden(&visible_root.join(hidden_file), visible_root));

        assert!(!is_hidden(&hidden_root.join(visible_file), hidden_root));
        assert!(!is_hidden(&visible_root.join(visible_file), visible_root));
    }
}
