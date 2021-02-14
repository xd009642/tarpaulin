use crate::utils::get_test_path;
use cargo_tarpaulin::config::Config;
use cargo_tarpaulin::launch_tarpaulin;
use std::env;
use std::path::PathBuf;

#[test]
fn package_exclude() {
    let mut config = Config::default();
    let test_dir = get_test_path("workspace");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.all = true;
    let result = launch_tarpaulin(&config, &None);
    let result = result.expect("Test failed").0;
    let files = result.files();
    files.iter().for_each(|f| {
        println!("File: {}", f.display());
    });
    assert!(files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(files.iter().any(|f| f.ends_with("bar/src/lib.rs")));

    config.exclude = vec!["foo".to_string()];
    let result = launch_tarpaulin(&config, &None);
    let result = result.expect("Test failed").0;
    let files = result.files();
    assert!(!files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(files.iter().any(|f| f.ends_with("bar/src/lib.rs")));

    config.exclude = vec!["bar".to_string()];
    let result = launch_tarpaulin(&config, &None);
    let result = result.expect("Test failed").0;
    let files = result.files();
    assert!(files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(!files.iter().any(|f| f.ends_with("bar/src/lib.rs")));
}

#[test]
fn specify_package() {
    let mut config = Config::default();
    let test_dir = get_test_path("workspace");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.packages = vec!["foo".to_string()];
    let result = launch_tarpaulin(&config, &None);
    let result = result.expect("Test failed").0;
    let files = result.files();
    assert!(files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(!files.iter().any(|f| f.ends_with("bar/src/lib.rs")));

    config.packages = vec!["bar".to_string()];
    let result = launch_tarpaulin(&config, &None);
    let result = result.expect("Test failed").0;
    let files = result.files();
    assert!(!files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(files.iter().any(|f| f.ends_with("bar/src/lib.rs")));

    config.packages = vec!["bar".to_string(), "foo".to_string()];
    let result = launch_tarpaulin(&config, &None);
    let result = result.expect("Test failed").0;
    let files = result.files();
    assert!(files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(files.iter().any(|f| f.ends_with("bar/src/lib.rs")));
}

#[test]
fn config_relative_pathing() {
    let mut test_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    test_dir.push("tests");
    test_dir.push("data");
    let base_path = test_dir.clone();
    test_dir.push("tarpaulin.toml");
    // This test added because if it doesn't work it can mess up using --features
    // in workspace roots
    let configs = Config::load_config_file(&test_dir).unwrap();

    assert_eq!(configs.len(), 2);
    assert_eq!(configs[0].manifest, base_path.join("lib/Cargo.toml"));
    assert_eq!(configs[1].manifest, base_path.join("bin/Cargo.toml"));
    assert_eq!(configs[1].target_dir(), base_path.join("targ"));
}
