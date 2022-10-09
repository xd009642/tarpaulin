use crate::utils::get_test_path;
use cargo_tarpaulin::{
    config::{Color, Config, Mode},
    errors::RunError,
};
use cargo_tarpaulin::{launch_tarpaulin, run, setup_logging};
use rusty_fork::rusty_fork_test;
use std::env;

rusty_fork_test! {

#[test]
fn error_if_build_script_fails() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("build_script_fail");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.set_clean(false);

    let result = launch_tarpaulin(&config, &None);

    assert!(result.is_err());

    if let Err(RunError::Cargo(_)) = result {
    } else {
        panic!("Expected a Cargo error");
    }
}

#[test]
fn error_if_compilation_fails() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("compile_fail");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.set_clean(false);

    let result = launch_tarpaulin(&config, &None);

    assert!(result.is_err());

    if let Err(RunError::TestCompile(_)) = result {
    } else {
        panic!("Expected a TestCompile error");
    }
}

#[test]
fn error_if_test_fails() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("failing_test");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.set_clean(false);

    let result = run(&[config]);

    assert!(result.is_err());

    if let Err(RunError::TestFailed) = result {
    } else {
        panic!("Expected a TestFailed error: {:?}", result);
    }
}

#[test]
fn issue_610() {
    setup_logging(Color::Never, false, false);
    let mut config = Config::default();
    let test_dir = get_test_path("issue_610");
    config.test_names.insert("foo".to_string());
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.set_clean(false);

    let result = run(&[config.clone()]);
    assert!(result.is_ok());

    config.test_names.clear();
    config.command = Mode::Build;
    let result = run(&[config]);
    assert!(result.is_err());
}

}
