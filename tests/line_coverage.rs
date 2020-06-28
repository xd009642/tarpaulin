use crate::utils::get_test_path;
use cargo_tarpaulin::config::Config;
use cargo_tarpaulin::launch_tarpaulin;
use cargo_tarpaulin::traces::CoverageStat;
use std::env;
use std::time::Duration;

#[test]
fn simple_project_coverage() {
    let mut config = Config::default();
    config.verbose = true;
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("simple_project");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir.clone();
    config.manifest.push("Cargo.toml");

    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
    env::set_current_dir(restore_dir).unwrap();
    let unused_file = test_dir.join("src/unused.rs");
    let unused_hits = res.covered_in_path(&unused_file);
    let unused_lines = res.coverable_in_path(&unused_file);
    assert_eq!(unused_hits, 0);
    assert_eq!(unused_lines, 3);
    let unused_hits = res
        .get_child_traces(&unused_file)
        .iter()
        .map(|x| x.line)
        .collect::<Vec<_>>();

    assert_eq!(unused_hits.len(), 3);
    assert!(unused_hits.contains(&4));
    assert!(unused_hits.contains(&5));
    assert!(unused_hits.contains(&6));

    let lib_file = test_dir.join("src/lib.rs");
    let lib_traces = res.get_child_traces(&lib_file);
    for l in &lib_traces {
        if l.line == 6 {
            assert_eq!(CoverageStat::Line(0), l.stats);
        } else if l.line == 8 {
            assert_eq!(CoverageStat::Line(1), l.stats);
        }
    }
}
