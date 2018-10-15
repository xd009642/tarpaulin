extern crate cargo_tarpaulin;

use cargo_tarpaulin::launch_tarpaulin;
use cargo_tarpaulin::config::Config;
use cargo_tarpaulin::traces::CoverageStat;
use std::env;
use std::time::Duration;


#[test]
fn array_coverage() {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    let mut test_dir = env::current_dir().unwrap();
    test_dir.push("tests");
    test_dir.push("data");
    test_dir.push("arrays");
    env::set_current_dir(test_dir.clone()).unwrap();
    config.manifest = test_dir.clone();
    config.manifest.push("Cargo.toml");
    
    let (res, tp) = launch_tarpaulin(&config).unwrap();

    assert!(tp);
    // Float rounding errors shouldn't be an issue for 1.0
    assert_eq!(res.coverage_percentage(), 1.0f64);
}
