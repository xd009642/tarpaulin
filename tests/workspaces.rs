use crate::utils::get_test_path;
use cargo_tarpaulin::config::Config;
use cargo_tarpaulin::launch_tarpaulin;
use std::env;

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
