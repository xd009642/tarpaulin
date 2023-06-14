use crate::utils::get_test_path;
use cargo_tarpaulin::config::{types::RunType, Color, Config};
use cargo_tarpaulin::{launch_tarpaulin, setup_logging};
use rusty_fork::rusty_fork_test;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

rusty_fork_test! {

#[test]
fn mix_test_types() {
    setup_logging(Color::Never, false, false);
    // Issue 747 the clean would delete old tests leaving to only one run type effectively being
    // ran. This test covers against that mistake
    let mut config = Config::default();
    config.set_clean(true);
    config.set_ignore_tests(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Tests, RunType::Examples];
    config.set_profraw_folder(PathBuf::from("mix_test_types"));

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

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
fn only_test_coverage() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Tests];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
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
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::AllTargets];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.set_profraw_folder(PathBuf::from("only_example_coverage"));

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
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Examples];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
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
#[ignore]
fn only_bench_coverage() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Benchmarks];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.set_profraw_folder(PathBuf::from("only_bench_coverage"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();

    for f in res.files() {
        let f_name = f.file_name().unwrap().to_str().unwrap();
        if f_name.contains("bench") {
            assert!(res.covered_in_path(f) > 0);
        } else {
            assert_eq!(res.covered_in_path(f), 0);
        }
    }
}

#[test]
#[cfg(feature = "nightly")]
fn only_doctest_coverage() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Doctests];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
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
