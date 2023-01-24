use crate::utils::get_test_path;
use cargo_tarpaulin::{
    config::{Color, Config},
    errors::RunError,
};
use cargo_tarpaulin::{run, setup_logging};
use rusty_fork::rusty_fork_test;
use std::{env, path::PathBuf};

rusty_fork_test! {

#[test]
fn coverage_below_threshold() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.fail_under = Some(100.0);
    config.set_clean(false);
    config.set_profraw_folder(PathBuf::from("coverage_below_threshold"));

    let result = run(&[config]);

    assert!(result.is_err());

    if let Err(RunError::BelowThreshold(a, e)) = result {
        assert!(a < e);
    } else {
        panic!("Wrong error type {}", result.unwrap_err());
    }
}

#[test]
fn coverage_above_threshold() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.fail_under = Some(30.0);
    config.set_clean(false);
    config.set_profraw_folder(PathBuf::from("coverage_above_threshold"));

    let result = run(&[config]);

    assert!(result.is_ok());
}

#[test]
fn report_coverage_fail() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.fail_under = Some(10.0);
    config.set_clean(false);
    config.set_profraw_folder(PathBuf::from("report_coverage_fail"));

    let mut report = Config::default();
    report.name = "report".to_string();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    report.set_manifest(manifest);
    report.fail_under = Some(99.0);

    let result = run(&[config, report]);

    assert!(result.is_err());
    if let Err(RunError::BelowThreshold(a, e)) = result {
        assert!(a < e);
        assert_eq!(e as usize, 99);
    } else {
        panic!("Wrong error type {}", result.unwrap_err());
    }
}

}
