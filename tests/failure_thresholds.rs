use crate::utils::get_test_path;
use cargo_tarpaulin::run;
use cargo_tarpaulin::{
    config::{Config, OutputFile},
    errors::RunError,
};
use rusty_fork::rusty_fork_test;
use std::{env, fs, path::PathBuf};
use test_log::test;

rusty_fork_test! {

#[test]
fn coverage_below_threshold() {
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    config.set_current_dir(manifest.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.fail_under = Some(100.0);
    config.set_clean(false);
    config.set_include_tests(true);
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
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    config.set_current_dir(manifest.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.fail_under = Some(30.0);
    config.set_clean(false);
    config.set_include_tests(true);
    config.set_profraw_folder(PathBuf::from("coverage_above_threshold"));

    let result = run(&[config]);

    assert!(result.is_ok());
}

#[test]
fn report_coverage_fail() {
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir.clone();
    config.set_current_dir(manifest.clone());
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.fail_under = Some(10.0);
    config.set_clean(false);
    config.set_include_tests(true);
    config.set_profraw_folder(PathBuf::from("report_coverage_fail"));

    let mut report = Config::default();
    report.name = "report".to_string();
    let mut manifest = test_dir;
    config.set_current_dir(manifest.clone());
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

#[test]
fn report_generated_when_coverage_config_fails_threshold() {
    let mut config = Config::default();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    let manifest = test_dir.join("Cargo.toml");
    config.set_manifest(manifest.clone());
    config.fail_under = Some(100.0);
    config.set_clean(false);
    config.set_include_tests(true);
    config.set_profraw_folder(PathBuf::from(
        "report_generated_when_coverage_config_fails_threshold",
    ));

    let report_dir = test_dir.join("reports_fail_under");
    let _ = fs::remove_dir_all(&report_dir);

    let mut report = Config::default();
    report.name = "report".to_string();
    report.set_manifest(manifest);
    report.set_clean(false);
    report.output_directory = Some(report_dir.clone());
    report.generate.push(OutputFile::Html);

    let result = run(&[config, report]);

    assert!(matches!(result, Err(RunError::BelowThreshold(_, 100.0))));
    assert!(report_dir.join("tarpaulin-report.html").exists());

    fs::remove_dir_all(&report_dir)
        .expect("failed to remove generated fail-under report directory");
}

}
