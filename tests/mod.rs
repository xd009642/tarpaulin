use crate::utils::get_test_path;
use cargo_tarpaulin::config::{Config, ConfigWrapper, RunType};
use cargo_tarpaulin::launch_tarpaulin;
use cargo_tarpaulin::traces::*;
use clap::App;
use std::env;
use std::time::Duration;

mod compile_fail;
mod doc_coverage;
mod line_coverage;
mod test_types;
mod utils;

pub fn check_percentage_with_cli_args(minimum_coverage: f64, has_lines: bool, args: &[String]) {
    let restore_dir = env::current_dir().unwrap();
    let matches = App::new("tarpaulin")
        .args_from_usage(
             "--config [FILE] 'Path to a toml file specifying a list of options this will override any other options set'
             --ignore-config 'Ignore any project config files'
             --debug 'Show debug output - this is used for diagnosing issues with tarpaulin'
             --verbose -v 'Show extra output'
             --root -r [DIR] 'directory'"
        ).get_matches_from(args);

    let configs = ConfigWrapper::from(&matches).0;
    let mut res = TraceMap::new();
    for config in &configs {
        let (t, _) = launch_tarpaulin(&config).unwrap();
        res.merge(&t);
    }
    res.dedup();
    env::set_current_dir(restore_dir).unwrap();
    assert!(
        res.coverage_percentage() >= minimum_coverage,
        "Assertion failed {} >= {}",
        res.coverage_percentage(),
        minimum_coverage
    );
    if has_lines {
        assert!(res.total_coverable() > 0);
    }
}

pub fn check_percentage_with_config(
    project_name: &str,
    minimum_coverage: f64,
    has_lines: bool,
    mut config: Config,
) {
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path(project_name);
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    let (res, _) = launch_tarpaulin(&config).unwrap();

    env::set_current_dir(restore_dir).unwrap();
    assert!(
        res.coverage_percentage() >= minimum_coverage,
        "Assertion failed {} >= {}",
        res.coverage_percentage(),
        minimum_coverage
    );
    if has_lines {
        assert!(res.total_coverable() > 0);
    }
}

pub fn check_percentage(project_name: &str, minimum_coverage: f64, has_lines: bool) {
    let config = Config::default();
    check_percentage_with_config(project_name, minimum_coverage, has_lines, config);
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
fn futures_coverage() {
    check_percentage("futures", 1.0f64, true);
}

#[test]
fn breaks_expr_coverage() {
    check_percentage("breaks", 1.0f64, true);
}

#[test]
fn continues_expr_coverage() {
    check_percentage("continues", 1.0f64, true);
}

#[test]
fn boxes_coverage() {
    check_percentage("boxes", 1.0f64, true);
}

#[test]
fn method_calls_expr_coverage() {
    check_percentage("method_calls", 1.0f64, true);
}

#[test]
fn config_file_coverage() {
    let test_dir = get_test_path("configs");
    let mut args = vec![
        "tarpaulin".to_string(),
        "--root".to_string(),
        test_dir.display().to_string(),
    ];
    check_percentage_with_cli_args(1.0f64, true, &args);
    args.push("--ignore-config".to_string());
    check_percentage_with_cli_args(0.0f64, true, &args);
}

#[test]
fn match_expr_coverage() {
    check_percentage("matches", 1.0f64, true);
}

#[test]
#[ignore]
fn benchmark_coverage() {
    let test = "benchmark_coverage";
    check_percentage(test, 0.0f64, true);

    let mut config = Config::default();
    config.run_types = vec![RunType::Benchmarks];
    check_percentage_with_config(test, 1.0f64, true, config);
}

#[test]
fn examples_coverage() {
    let test = "example_test";
    check_percentage(test, 0.0f64, true);

    let mut config = Config::default();
    config.run_types = vec![RunType::Examples];
    check_percentage_with_config(test, 1.0f64, true, config);
}
