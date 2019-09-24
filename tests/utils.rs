use std::env;
use std::path::PathBuf;

pub(crate) fn get_test_path(test_dir_name: &str) -> PathBuf {
    let mut test_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    test_dir.push("tests");
    test_dir.push("data");
    test_dir.push(test_dir_name);
    test_dir
}
