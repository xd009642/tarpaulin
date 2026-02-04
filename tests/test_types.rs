use crate::utils::get_test_path;
use cargo_tarpaulin::config::{types::RunType, Config};
use cargo_tarpaulin::launch_tarpaulin;
use rusty_fork::rusty_fork_test;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use test_log::test;

rusty_fork_test! {

#[test]
fn mix_test_types() {
    // Issue 747 the clean would delete old tests leaving to only one run type effectively being
    // ran. This test covers against that mistake
    let mut config = Config::default();
    config.set_clean(true);
    config.set_include_tests(true);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Tests, RunType::Examples];
    config.set_profraw_folder(PathBuf::from("mix_test_types"));

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    config.set_current_dir(test_dir.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    let mut target = test_dir;
    target.push("mix_target");
    config.set_target_dir(target);

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();

    for f in res.files() {
        let f_name = f.file_name().unwrap().to_str().unwrap();
        if f_name.contains("example") || (f_name.contains("test") && !f_name.contains("doc")) {
            assert!(res.covered_in_path(f) > 0);
        } else {
            assert_eq!(res.covered_in_path(f), 0);
        }
    }
}

#[test]
fn only_test_coverage() {
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Tests];
    config.set_include_tests(true);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    config.set_current_dir(test_dir.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    let mut target = test_dir;
    target.push("only_test_target");
    config.set_target_dir(target);
    config.set_profraw_folder(PathBuf::from("only_test_coverage"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();

    for f in res.files() {
        let f_name = f.file_name().unwrap().to_str().unwrap();
        if f_name.contains("test") && !f_name.contains("doc") {
            assert!(res.covered_in_path(f) > 0);
        } else {
            assert_eq!(res.covered_in_path(f), 0);
        }
    }
}

#[test]
fn all_targets_coverage() {
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::AllTargets];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    config.set_current_dir(test_dir.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    let mut target = test_dir;
    target.push("all_target");
    config.set_target_dir(target);
    config.set_profraw_folder(PathBuf::from("all_targets_coverage"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();

    for f in res.files() {
        let f_name = f.file_name().unwrap().to_str().unwrap();
        if f_name.contains("doc") {
            assert_eq!(res.covered_in_path(f), 0);
        } else {
            assert!(res.covered_in_path(f) > 0);
        }
    }
}


#[test]
fn only_example_coverage() {
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Examples];
    config.set_include_tests(true);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    config.set_current_dir(test_dir.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    let mut target = test_dir;
    target.push("example_target");
    config.set_target_dir(target);
    config.set_profraw_folder(PathBuf::from("only_example_coverage"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();

    for f in res.files() {
        let f_name = f.file_name().unwrap().to_str().unwrap();
        if f_name.contains("example") {
            assert!(res.covered_in_path(f) > 0);
        } else {
            assert_eq!(res.covered_in_path(f), 0);
        }
    }
}

#[test]
#[cfg(nightly)]
fn only_doctest_coverage() {
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Doctests];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    let mut target = test_dir;
    target.push("doc_target");
    config.set_target_dir(target);
    config.set_profraw_folder(PathBuf::from("only_doctest_coverage"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();

    for f in res.files() {
        let f_name = f.file_name().unwrap().to_str().unwrap();
        if f_name.contains("doc") {
            assert!(res.covered_in_path(f) > 0);
        } else {
            assert_eq!(res.covered_in_path(f), 0);
        }
    }
}

}
