use crate::utils::get_test_path;
use cargo_tarpaulin::config::*;
use cargo_tarpaulin::traces::CoverageStat;
use cargo_tarpaulin::{launch_tarpaulin, setup_logging};
use regex::Regex;
use rusty_fork::rusty_fork_test;
use std::time::Duration;
use std::{env, fs, io};

rusty_fork_test! {
// Just for linux if we have ptrace as default
#[test]
fn sanitised_paths() {
    setup_logging(Color::Never, true, true);
    let test_dir = get_test_path("simple_project");
    let mut config = Config::default();
    config.set_engine(TraceEngine::Llvm);
    config.set_ignore_tests(false);
    config.set_clean(false);
    config.generate = vec![OutputFile::Json, OutputFile::Xml, OutputFile::Lcov];
    let report_dir = test_dir.join("reports");
    let _ = fs::remove_dir_all(&report_dir);
    let _ = fs::create_dir(&report_dir);
    config.output_directory = Some(report_dir.clone());
    let restore_dir = env::current_dir().unwrap();

    env::set_current_dir(&test_dir).unwrap();
    println!("RUN TARPAULIN");
    let (res, ret) = launch_tarpaulin(&config, &None).unwrap();
    env::set_current_dir(restore_dir).unwrap();

    println!("Look at reports");
    let mut count = 0;
    let bad_path_regex = Regex::new(r#"\\\\\?\w:\\"#).unwrap();
    for entry in fs::read_dir(&report_dir).unwrap() {
        let entry = entry.unwrap().path();
        if !entry.is_dir() {
            count += 1;
            println!("Checking: {}", entry.display());
            let f = fs::File::open(entry).unwrap();
            if let Ok(s) = io::read_to_string(f) {
                assert!(bad_path_regex.find(&s).is_none());
            } else {
                println!("Not unicode");
            }
        }
    }
    assert_eq!(count, config.generate.len());
}
}
