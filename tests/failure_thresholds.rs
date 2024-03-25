use crate::utils::get_test_path;
use cargo_tarpaulin::{
    config::{Color, Config},
    errors::RunError,
};
use cargo_tarpaulin::{run, setup_logging};
use rusty_fork::rusty_fork_test;
use std::{env, fs, path::PathBuf};

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

#[test]
fn coverage_increasing() {
    // NOTE: This test uses a seperate target directory, or else it clashes with other test and
    // produces random failures, further more it removes data from previous runs for the same
    // reason to give it a clean environment.
    setup_logging(Color::Never, false, false);

    let test_dir = get_test_path("configs");
    env::set_current_dir(&test_dir).unwrap();

    let mut target_dir = test_dir.clone();
    target_dir.push("target/coverage_increasing");

    let mut manifest = test_dir;
    manifest.push("Cargo.toml");

    // Prepare our steps
    let step1 = String::from("feature1");
    let step2 = String::from("feature1 feature2");

    // Prepare our configuration
    let mut config = Config::default();
    config.set_manifest(manifest);
    config.fail_decreasing = true;
    config.set_clean(false);
    config.set_target_dir(target_dir.clone());

    // Set a clean base line
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).ok();
    }

    // Start step #1
    config.features = Some(step1);
    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    // Start step #2
    config.features = Some(step2);
    let result = run(&[config]);
    assert!(result.is_ok());
}

#[test]
fn coverage_decreasing() {
    // NOTE: This test uses a seperate target directory, or else it clashes with other test and
    // produces random failures, further more it removes data from previous runs for the same
    // reason to give it a clean environment.
    setup_logging(Color::Never, false, false);

    let test_dir = get_test_path("configs");
    env::set_current_dir(&test_dir).unwrap();

    let mut target_dir = test_dir.clone();
    target_dir.push("target/coverage_decreasing");

    let mut manifest = test_dir;
    manifest.push("Cargo.toml");

    // Prepare our steps
    let step1 = String::from("feature1 feature2");
    let step2 = String::from("feature1");

    // Prepare our configuration
    let mut config = Config::default();
    config.set_manifest(manifest);
    config.fail_decreasing = true;
    config.set_clean(false);
    config.set_target_dir(target_dir.clone());

    // Set a clean base line
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).ok();
    }

    // Start step #1
    config.features = Some(step1);
    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    // Start step #2
    config.features = Some(step2);
    let result = run(&[config]);
    assert!(result.is_err());
    if let Err(RunError::CoverageDecreasing(delta)) = result {
        assert!(delta < 0.0f64);
        assert_eq!(delta as isize, -50);
    } else {
        panic!("Wrong error type {}", result.unwrap_err());
    }
}

#[test]
fn coverage_increasing_below_threshold() {
    // NOTE: This test uses a seperate target directory, or else it clashes with other test and
    // produces random failures, further more it removes data from previous runs for the same
    // reason to give it a clean environment.
    setup_logging(Color::Never, false, false);

    let test_dir = get_test_path("configs");
    env::set_current_dir(&test_dir).unwrap();

    let mut target_dir = test_dir.clone();
    target_dir.push("target/coverage_increasing_below_threshold");

    let mut manifest = test_dir;
    manifest.push("Cargo.toml");

    // Prepare our steps
    let step1 = String::from("feature1");
    let step2 = String::from("feature1 feature2");

    // Prepare our configuration
    let mut config = Config::default();
    config.set_manifest(manifest);
    config.fail_under = Some(70.0f64);
    config.fail_decreasing = true;
    config.set_clean(false);
    config.set_target_dir(target_dir.clone());

    // Set a clean base line
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).ok();
    }

    // Start step 1
    config.features = Some(step1);
    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    // Start step 2
    config.features = Some(step2);
    let result = run(&[config]);
    assert!(result.is_ok());
}

#[test]
fn coverage_decreasing_below_threshold() {
    // NOTE: This test uses a seperate target directory, or else it clashes with other test and
    // produces random failures, further more it removes data from previous runs for the same
    // reason to give it a clean environment.
    setup_logging(Color::Never, false, false);

    let test_dir = get_test_path("configs");
    env::set_current_dir(&test_dir).unwrap();

    let mut target_dir = test_dir.clone();
    target_dir.push("target/coverage_decreasing_below_threshold");

    let mut manifest = test_dir.clone();
    manifest.push("Cargo.toml");

    // Prepare our steps
    let step1 = String::from("feature1 feature2");
    let step2 = String::from("feature1");

    // Prepare our configuration
    let mut config = Config::default();
    config.set_manifest(manifest);
    config.fail_under = Some(70.0f64);
    config.fail_decreasing = true;
    config.set_target_dir(target_dir.clone());

    // Set a clean base line
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).ok();
    }

    // Start step 1
    config.features = Some(step1);
    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    // Start step 2
    config.features = Some(step2);
    let result = run(&[config]);
    if let Err(RunError::CoverageDecreasing(delta)) = result {
        assert!(delta < 0.0f64);
        assert_eq!(delta as isize, -50);
    } else {
        panic!("Wrong error type {}", result.unwrap_err());
    }
}

#[test]
fn coverage_increasing_above_threshold() {
    // NOTE: This test uses a seperate target directory, or else it clashes with other test and
    // produces random failures, further more it removes data from previous runs for the same
    // reason to give it a clean environment.
    setup_logging(Color::Never, false, false);

    let test_dir = get_test_path("configs");
    env::set_current_dir(&test_dir).unwrap();

    let mut target_dir = test_dir.clone();
    target_dir.push("target/coverage_increasing_above_threshold");

    let mut manifest = test_dir;
    manifest.push("Cargo.toml");

    // Prepare our steps
    let step1 = String::from("feature1");
    let step2 = String::from("feature1 feature2");

    // Prepare our configuration
    let mut config = Config::default();
    config.set_manifest(manifest);
    config.fail_under = Some(30.0f64);
    config.fail_decreasing = true;
    config.set_clean(false);
    config.set_target_dir(target_dir.clone());

    // Set a clean base line
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).ok();
    }

    // Start step 1
    config.features = Some(step1);
    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    // Start step 2
    config.features = Some(step2);
    let result = run(&[config]);
    assert!(result.is_ok());
}

#[test]
fn coverage_decreasing_above_threshold() {
    // NOTE: This test uses a seperate target directory, or else it clashes with other test and
    // produces random failures, further more it removes data from previous runs for the same
    // reason to give it a clean environment.
    setup_logging(Color::Never, false, false);

    let test_dir = get_test_path("configs");
    env::set_current_dir(&test_dir).unwrap();

    let mut target_dir = test_dir.clone();
    target_dir.push("target/coverage_decreasing_above_threshold");

    let mut manifest = test_dir;
    manifest.push("Cargo.toml");

    // Prepare our steps
    let step1 = String::from("feature1");
    let step2 = String::from("feature1 feature2");

    // Prepare our configuration
    let mut config = Config::default();
    config.set_manifest(manifest);
    config.fail_under = Some(30.0f64);
    config.fail_decreasing = true;
    config.set_clean(false);
    config.set_target_dir(target_dir.clone());

    // Set a clean base line
    if target_dir.exists() {
        fs::remove_dir_all(target_dir).ok();
    }

    // Start step 1
    config.features = Some(step1);
    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    // Start step 2
    config.features = Some(step2);
    let result = run(&[config]);
    assert!(result.is_ok());
}

}
