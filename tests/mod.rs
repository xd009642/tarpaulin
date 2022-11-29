use crate::utils::get_test_path;
use cargo_tarpaulin::config::{
    Color, Config, ConfigWrapper, Mode, OutputFile, RunType, TraceEngine,
};
use cargo_tarpaulin::event_log::EventLog;
use cargo_tarpaulin::path_utils::*;
use cargo_tarpaulin::traces::TraceMap;
use cargo_tarpaulin::{launch_tarpaulin, setup_logging};
use clap::App;
use regex::Regex;
use rusty_fork::rusty_fork_test;
use std::collections::HashSet;
use std::path::Path;
use std::time::Duration;
use std::{env, fs, io};

#[cfg(nightly)]
mod doc_coverage;
mod failure_thresholds;
mod failures;
mod line_coverage;
mod test_types;
mod utils;
mod workspaces;

pub fn check_percentage_with_cli_args(minimum_coverage: f64, has_lines: bool, args: &[String]) {
    setup_logging(Color::Never, false, false);
    let restore_dir = env::current_dir().unwrap();
    let matches = App::new("tarpaulin")
        .args_from_usage(
             "--config [FILE] 'Path to a toml file specifying a list of options this will override any other options set'
             --ignore-config 'Ignore any project config files'
             --debug 'Show debug output - this is used for diagnosing issues with tarpaulin'
             --verbose -v 'Show extra output'
             --root -r [DIR] 'directory'
             --include-tests 'include tests in your tests'
             --post-test-delay [SECONDS] 'Delay after test to collect coverage profiles'
             --implicit-test-threads 'Don't supply an explicit `--test-threads` argument to tarpaulin. By default tarpaulin will infer the default rustc would pick if not ran via tarpaulin and set it'"
        ).get_matches_from(args);

    let mut configs = ConfigWrapper::from(&matches).0;
    let mut res = TraceMap::new();
    for config in &mut configs {
        config.set_clean(false);
        let (t, _) = launch_tarpaulin(config, &None).unwrap();
        res.merge(&t);
    }
    res.dedup();
    env::set_current_dir(restore_dir).unwrap();
    if has_lines {
        assert!(
            res.coverage_percentage() >= minimum_coverage,
            "Assertion failed {} >= {}",
            res.coverage_percentage(),
            minimum_coverage
        );
        assert!(res.total_coverable() > 0);
    }
}

pub fn check_percentage_with_config(
    project_name: &str,
    minimum_coverage: f64,
    has_lines: bool,
    mut config: Config,
) {
    setup_logging(Color::Never, false, false);
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path(project_name);
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");
    config.set_clean(false);

    // Note to contributors. If an integration test fails, uncomment this to be able to see the
    // tarpaulin logs
    //cargo_tarpaulin::setup_logging(true, true);
    let event_log = if config.dump_traces {
        let mut paths = HashSet::new();
        paths.insert(config.manifest.clone());
        Some(EventLog::new(paths))
    } else {
        None
    };

    let (res, ret) = launch_tarpaulin(&config, &event_log).unwrap();
    assert_eq!(ret, 0);

    env::set_current_dir(restore_dir).unwrap();
    if has_lines {
        assert!(res.total_coverable() > 0);
        assert!(
            res.coverage_percentage() >= minimum_coverage,
            "Assertion failed {} >= {}",
            res.coverage_percentage(),
            minimum_coverage
        );
    } else {
        assert_eq!(res.total_coverable(), 0);
    }
}

pub fn check_percentage(project_name: &str, minimum_coverage: f64, has_lines: bool) {
    let mut config = Config::default();
    config.set_ignore_tests(false);
    config.set_clean(false);
    check_percentage_with_config(project_name, minimum_coverage, has_lines, config);
}

rusty_fork_test! {

#[test]
fn incorrect_manifest_path() {
    let mut config = Config::default();
    config.manifest.push("__invalid_dir__");
    config.set_clean(false);
    let launch = launch_tarpaulin(&config, &None);
    assert!(launch.is_err());
}

#[test]
fn proc_macro_link() {
    let mut config = Config::default();
    config.test_timeout = Duration::from_secs(60);
    config.set_clean(false);
    let test_dir = get_test_path("proc_macro");
    config.manifest = test_dir.join("Cargo.toml");
    assert!(launch_tarpaulin(&config, &None).is_ok());
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
#[cfg_attr(not(target_os="linux"), ignore)] // TODO So there are linker issues I can't adequately diagnose myself in windows
#[cfg(not(tarpaulin))]
fn picking_up_shared_objects() {
    // Need a project which downloads a shared object to target folder and uses build script to set
    // the linker path.
    check_percentage("torch_test", 1.0f64, true);
}

// Just for linux if we have ptrace as default
#[test]
fn llvm_sanity_test() {
    let mut config = Config::default();
    config.set_engine(TraceEngine::Llvm);
    config.follow_exec = true;
    config.set_ignore_tests(false);
    config.set_clean(false);

    check_percentage_with_config("structs", 1.0f64, true, config.clone());
    check_percentage_with_config("ifelse", 1.0f64, true, config.clone());
    check_percentage_with_config("returns", 1.0f64, true, config.clone());
    check_percentage_with_config("follow_exe", 1.0f64, true, config);
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
#[ignore]
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
fn issue_966_follow_exec() {
    let test_dir = get_test_path("follow_exec_issue966");
    let args = vec![
        "tarpaulin".to_string(),
        "--root".to_string(),
        test_dir.display().to_string(),
        "--post-test-delay".to_string(),
        10.to_string(),
    ];
    check_percentage_with_cli_args(1.0f64, true, &args);
}

#[test]
fn rustflags_config_coverage() {
    let test_dir = get_test_path("multiple_rustflags");
    let mut args = vec![
        "tarpaulin".to_string(),
        "--root".to_string(),
        test_dir.display().to_string(),
    ];
    check_percentage_with_cli_args(1.0f64, true, &args);
    args.push("--ignore-config".to_string());
    check_percentage_with_cli_args(0.0f64, false, &args);
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
    config.set_clean(false);
    config.run_types = vec![RunType::Benchmarks];
    check_percentage_with_config(test, 1.0f64, true, config);
}

#[test]
fn cargo_run_coverage() {
    let mut config = Config::default();
    config.command = Mode::Build;
    config.set_clean(false);
    check_percentage_with_config("run_coverage", 1.0f64, true, config);
}

#[test]
fn examples_coverage() {
    let test = "example_test";
    check_percentage(test, 0.0f64, true);

    let mut config = Config::default();
    config.run_types = vec![RunType::Examples];
    config.set_clean(false);
    config.set_ignore_tests(false);
    check_percentage_with_config(test, 1.0f64, true, config.clone());

    config.run_types.clear();
    config.example_names.insert("say_hello".to_string());
    check_percentage_with_config(test, 1.0f64, true, config);
}

#[test]
fn access_env_var() {
    // This test is mainly to check that expected environment variables are present
    // using `CARGO_BIN_EXE_<name>` to test
    let test = "env_var";
    check_percentage(test, 1.0f64, true);
}

#[test]
fn tarpaulin_attrs() {
    check_percentage("tarpaulin_attrs", 0.0f64, true);
}

#[test]
#[cfg(nightly)]
fn tarpaulin_tool_attr() {
    check_percentage("tool_attr", 0.0f64, false);
}

#[test]
#[cfg(nightly)]
fn filter_with_inner_attributes() {
    check_percentage("filter_inner_modules", 0.0f64, false);
}

#[test]
fn cargo_home_filtering() {
    let new_home =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/HttptestAndReqwest/new_home");
    let previous = env::var("CARGO_HOME");

    let mut config = Config::default();
    config.test_timeout = Duration::from_secs(60);
    config.set_clean(false);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("HttptestAndReqwest");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    env::set_var("CARGO_HOME", new_home.display().to_string());
    let run = launch_tarpaulin(&config, &None);
    let _ = fs::remove_dir_all(&new_home);
    match previous {
        Ok(s) => env::set_var("CARGO_HOME", s),
        Err(_) => {
            let _ = env::remove_var("CARGO_HOME");
        }
    }
    let (res, _) = run.unwrap();

    env::set_current_dir(restore_dir).unwrap();

    assert_eq!(res.iter().count(), 1);
}

#[test]
fn rustflags_handling() {
    check_percentage("rustflags", 1.0f64, true);
    env::set_var("RUSTFLAGS", "--cfg=foo");
    let mut config = Config::default();
    config.set_clean(false);

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("rustflags");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    let res = launch_tarpaulin(&config, &None);
    env::set_current_dir(&restore_dir).unwrap();
    env::remove_var("RUSTFLAGS");
    assert!(res.is_err() || res.unwrap().1 != 0);

    let (_, ret) = launch_tarpaulin(&config, &None).unwrap();
    env::set_current_dir(&restore_dir).unwrap();
    assert_eq!(ret, 0);
}

#[test]
fn follow_exes_down() {
    let mut config = Config::default();
    config.follow_exec = true;
    config.set_clean(false);
    check_percentage_with_config("follow_exe", 1.0f64, true, config);
}

#[test]
fn handle_module_level_exclude_attrs() {
    check_percentage("crate_level_ignores", 1.0f64, true);
}

#[test]
#[cfg(unix)]
fn handle_forks() {
    let mut config = Config::default();
    config.set_clean(false);
    config.set_ignore_tests(false);
    config.post_test_delay = Some(Duration::from_secs(10));
    // Some false negatives on more recent compilers so lets just aim for >90% and 0 return code
    check_percentage_with_config("fork-test", 0.85f64, true, config);
}

#[test]
fn no_test_args() {
    let test_dir = get_test_path("no_test_args");
    let args = vec![
        "tarpaulin".to_string(),
        "--root".to_string(),
        test_dir.display().to_string(),
        "--implicit-test-threads".to_string(),
        "--include-tests".to_string(),
    ];
    check_percentage_with_cli_args(1.0, true, &args);
}

#[test]
fn dot_rs_in_dir_name() {
    // issue #857
    let mut config = Config::default();
    config.set_clean(false);
    config.set_ignore_tests(false);

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("not_a_file.rs");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    let (res, _ret) = launch_tarpaulin(&config, &None).unwrap();
    env::set_current_dir(&restore_dir).unwrap();

    assert_eq!(res.files().len(), 1);

    for dir in get_source_walker(&config) {
        assert!(dir.path().is_file());
    }
}

#[test]
#[cfg(unix)]
#[cfg(not(tarpaulin))]
fn kill_used_in_test() {
    let mut config = Config::default();
    if config.engine() == TraceEngine::Llvm {
        println!("Tests using signals are not supported");
        return;
    }

    config.follow_exec = true;
    config.set_clean(false);
    config.set_ignore_tests(false);
    // Currently 2 false negatives, but if it was only covering the integration test max coverage
    // is 75% so this is high enough to prove it works
    check_percentage_with_config("kill_proc", 0.9f64, true, config);
}


#[test]
fn doc_test_bootstrap() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);
    let test_dir = get_test_path("doc_coverage");
    env::set_current_dir(&test_dir).unwrap();
    config.manifest = test_dir;
    config.manifest.push("Cargo.toml");

    config.run_types = vec![RunType::Doctests];

    env::set_var("RUSTC_BOOTSTRAP", "1");

    let (_res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
}

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
    launch_tarpaulin(&config, &None).unwrap();
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
