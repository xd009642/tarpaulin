use crate::utils::get_test_path;
use cargo_tarpaulin::run;
use cargo_tarpaulin::{config::Config, errors::RunError};
use std::env;

#[test]
fn coverage_below_threshold() {
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.fail_under = Some(100.0);
    config.force_clean = false;

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
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.fail_under = Some(30.0);
    config.force_clean = false;

    let result = run(&[config]);

    assert!(result.is_ok());
}

#[test]
fn report_coverage_fail() {
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir.clone();
    config.manifest.push("Cargo.toml");
    config.fail_under = Some(10.0);
    config.force_clean = false;

    let mut report = Config::default();
    report.name = "report".to_string();
    report.manifest = test_dir;
    report.manifest.push("Cargo.toml");
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
