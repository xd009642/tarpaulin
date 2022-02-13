use crate::utils::get_test_path;
use cargo_tarpaulin::config::{Config, RunType};
use cargo_tarpaulin::launch_tarpaulin;
use rusty_fork::rusty_fork_test;
use std::env;
use std::time::Duration;

rusty_fork_test! {
#[test]
fn doc_test_env() {
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_env");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());
}

#[test]
fn doc_test_coverage() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doc_coverage");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_panics() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_should_panic");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_panics_workspace() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_workspace_should_panic");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_compile_fail() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_compile_fail_fail");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    assert!(launch_tarpaulin(&config, &None).is_err());
}

#[test]
fn doc_test_no_run() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_norun");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (_, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
}
}
