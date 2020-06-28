use crate::utils::get_test_path;
use cargo_tarpaulin::config::{types::*, Config};
use cargo_tarpaulin::launch_tarpaulin;
use std::env;
use std::time::Duration;

#[test]
fn only_test_coverage() {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Tests];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

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
fn only_example_coverage() {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Examples];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

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
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Benchmarks];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

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
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    config.run_types = vec![RunType::Doctests];
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("all_test_types");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

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
