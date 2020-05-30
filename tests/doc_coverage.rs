use crate::utils::get_test_path;
use cargo_tarpaulin::config::{Config, RunType};
use cargo_tarpaulin::launch_tarpaulin;
use std::env;
use std::time::Duration;

#[test]
fn doc_test_coverage() {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doc_coverage");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];

    let (res, ret) = launch_tarpaulin(&config).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}

#[test]
fn doc_test_panics() {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doctest_should_panic");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    let (res, ret) = launch_tarpaulin(&config).unwrap();

    assert_eq!(ret, 0);
    assert!(res.total_covered() > 0);
    assert_eq!(res.total_covered(), res.total_coverable());

    config.run_types = vec![RunType::Tests];

    let (res, ret) = launch_tarpaulin(&config).unwrap();

    assert_eq!(ret, 0);
    assert_eq!(res.total_covered(), 0);
}
