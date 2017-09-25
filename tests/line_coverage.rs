extern crate cargo_tarpaulin;

use cargo_tarpaulin::launch_tarpaulin;
use cargo_tarpaulin::config::Config;
use std::env;

#[test]
fn incorrect_manifest_path() {
    let mut config = Config::default();
    config.manifest.push("__invalid_dir__");
    assert!(launch_tarpaulin(&config).is_err());
}

#[test]
fn simple_project_coverage() {
    let mut config = Config::default();
    config.manifest = env::current_dir().unwrap();
    config.manifest.push("tests");
    config.manifest.push("data");
    config.manifest.push("simple_project");
    config.manifest.push("Cargo.toml");
    let res = launch_tarpaulin(&config).unwrap();
    
    let unused_hits = res.iter()
                         .filter(|x| x.path.file_name().unwrap() == "unused.rs")
                         .fold(0, |acc, ref x| acc + x.hits);
    assert_eq!(unused_hits, 0);
}
