use crate::utils::get_test_path;
use cargo_tarpaulin::config::Config;
use cargo_tarpaulin::launch_tarpaulin;
use std::env;
use std::time::Duration;

mod compile_fail;
mod line_coverage;
mod doc_coverage;
mod utils;

pub fn check_percentage(project_name: &str, minimum_coverage: f64, has_lines: bool) {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path(project_name);
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    let (res, _) = launch_tarpaulin(&config).unwrap();

    env::set_current_dir(restore_dir).unwrap();
    assert!(res.coverage_percentage() >= minimum_coverage);
    if has_lines {
        assert!(res.total_coverable() > 0);
    }
}

#[test]
fn incorrect_manifest_path() {
    let mut config = Config::default();
    config.manifest.push("__invalid_dir__");
    assert!(launch_tarpaulin(&config).is_err());
}

#[test]
fn proc_macro_link() {
    let mut config = Config::default();
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("proc_macro");
    config.manifest = test_dir.join("Cargo.toml");
    assert!(launch_tarpaulin(&config).is_ok());
}

#[test]
fn array_coverage() {
    check_percentage("arrays", 1.0f64, true);
}

#[test]
fn lets_coverage() {
    check_percentage("lets", 1.0f64, true);
}

#[test]
fn struct_expr_coverage() {
    check_percentage("structs", 1.0f64, true);
}

#[test]
fn ifelse_expr_coverage() {
    check_percentage("ifelse", 1.0f64, true);
}

#[test]
fn returns_expr_coverage() {
    check_percentage("returns", 1.0f64, true);
}

#[test]
fn loops_expr_coverage() {
    check_percentage("loops", 1.0f64, true);
}

#[test]
fn loops_assigns_coverage() {
    check_percentage("assigns", 1.0f64, true);
}

#[test]
fn paths_coverage() {
    check_percentage("paths", 1.0f64, true);
}

#[test]
#[ignore]
fn futures_coverage() {
    check_percentage("futures", 1.0f64, true);
}
