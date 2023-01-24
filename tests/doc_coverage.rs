use crate::utils::get_test_path;
use cargo_tarpaulin::config::{Color, Config, RunType};
use cargo_tarpaulin::{launch_tarpaulin, setup_logging};
use rusty_fork::rusty_fork_test;
use std::time::Duration;
use std::{env, path::PathBuf};

rusty_fork_test! {
#[test]
fn doc_test_env() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_env");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.set_profraw_folder(PathBuf::from("doc_test_env"));

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());
}

#[test]
fn doc_test_coverage() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doc_coverage");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    config.run_types = vec![RunType::Doctests];
    config.set_profraw_folder(PathBuf::from("doc_test_coverage_1"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];
    config.set_profraw_folder(PathBuf::from("doc_test_coverage_2"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_panics() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_should_panic");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    config.run_types = vec![RunType::Doctests];
    config.set_profraw_folder(PathBuf::from("doc_test_panics_1"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];
    config.set_profraw_folder(PathBuf::from("doc_test_panics_2"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_panics_workspace() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_workspace_should_panic");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.set_profraw_folder(PathBuf::from("doc_test_panics_workspace_1"));

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];
    config.set_profraw_folder(PathBuf::from("doc_test_panics_workspace_2"));

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_compile_fail() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_compile_fail_fail");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    config.run_types = vec![RunType::Doctests];

    assert!(launch_tarpaulin(&config, &None).is_err());
}

#[test]
fn doc_test_no_run() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_norun");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    config.run_types = vec![RunType::Doctests];

    let (_, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
}

#[test]
fn rustdocflags_handling() {
    env::set_var("RUSTDOCFLAGS", "--cfg=foo");
    let mut config = Config::default();
    config.run_types = vec![RunType::Doctests];
    config.set_clean(false);

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("rustflags");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    let res = launch_tarpaulin(&config, &None);
    env::set_current_dir(&restore_dir).unwrap();
    env::remove_var("RUSTDOCFLAGS");
    assert!(res.is_err() || res.unwrap().1 != 0);

    let (_, ret) = launch_tarpaulin(&config, &None).unwrap();
    env::set_current_dir(&restore_dir).unwrap();
    assert_eq!(ret, 0);
}
}
