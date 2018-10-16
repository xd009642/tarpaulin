extern crate cargo_tarpaulin;

use cargo_tarpaulin::launch_tarpaulin;
use cargo_tarpaulin::config::Config;
use std::env;
use std::time::Duration;


pub fn check_percentage(project_name: &str, minimum_coverage: f64) {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let mut test_dir = env::current_dir().unwrap();
    test_dir.push("tests");
    test_dir.push("data");
    test_dir.push(project_name);
    env::set_current_dir(test_dir.clone()).unwrap();
    config.manifest = test_dir.clone();
    config.manifest.push("Cargo.toml");
    
    let (res, tp) = launch_tarpaulin(&config).unwrap();

    env::set_current_dir(restore_dir).unwrap();
    assert!(tp);
    assert!(res.coverage_percentage() >= minimum_coverage);
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
    let mut test_dir = env::current_dir().unwrap();
    test_dir.push("tests");
    test_dir.push("data");
    test_dir.push("proc_macro");
    config.manifest = test_dir.join("Cargo.toml");
    assert!(launch_tarpaulin(&config).is_ok());
}

#[test]
fn array_coverage() {
    check_percentage("arrays", 1.0f64);
}

#[test]
fn lets_coverage() {
    check_percentage("lets", 1.0f64);
}

#[test]
fn struct_expr_coverage() {
    check_percentage("structs", 1.0f64);
}

#[test]
fn ifelse_expr_coverage() {
    check_percentage("ifelse", 1.0f64);
}
