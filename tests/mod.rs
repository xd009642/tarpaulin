use crate::utils::get_test_path;
use cargo_tarpaulin::event_log::EventLog;
use cargo_tarpaulin::path_utils::*;
use cargo_tarpaulin::traces::TraceMap;
use cargo_tarpaulin::{
    args::TarpaulinCli,
    config::{Config, ConfigWrapper, Mode, OutputFile, RunType, TraceEngine},
};
use cargo_tarpaulin::{launch_tarpaulin, run};
use clap::Parser;
#[cfg(windows)]
use regex::Regex;
use rusty_fork::rusty_fork_test;
use std::collections::HashSet;
#[cfg(windows)]
use std::io;
use std::path::Path;
#[cfg(windows)]
use std::path::PathBuf;
use std::time::Duration;
use std::{env, fs};
use test_log::test;

#[cfg(nightly)]
mod doc_coverage;
mod failure_thresholds;
mod failures;
mod line_coverage;
mod test_types;
mod utils;
mod workspaces;

pub fn check_percentage_with_cli_args(
    minimum_coverage: f64,
    has_lines: bool,
    args: &[String],
) -> TraceMap {
    let restore_dir = env::current_dir().unwrap();
    let args = TarpaulinCli::parse_from(args);

    let mut configs = ConfigWrapper::from(args.config).0;
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
    res
}

pub fn run_config(project_name: &str, mut config: Config) {
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path(project_name);
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    env::set_current_dir(config.root()).unwrap();
    config.set_clean(false);

    run(&[config]).unwrap();

    env::set_current_dir(restore_dir).unwrap();
}

pub fn check_percentage_with_config(
    project_name: &str,
    minimum_coverage: f64,
    has_lines: bool,
    mut config: Config,
) -> TraceMap {
    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path(project_name);
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.set_clean(false);

    // Note to contributors. If an integration test fails, uncomment this to be able to see the
    // tarpaulin logs
    //cargo_tarpaulin::setup_logging(true, true, false);
    let event_log = if config.dump_traces {
        let mut paths = HashSet::new();
        paths.insert(config.manifest());
        Some(EventLog::new(paths, &config))
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
    res
}

pub fn check_percentage(project_name: &str, minimum_coverage: f64, has_lines: bool) -> TraceMap {
    let mut config = Config::default();
    config.set_include_tests(true);
    config.set_clean(false);
    check_percentage_with_config(project_name, minimum_coverage, has_lines, config)
}

rusty_fork_test! {

#[test]
fn incorrect_manifest_path() {
    let mut config = Config::default();
    let mut invalid = config.manifest();
    invalid.push("__invalid_dir__");
    config.set_manifest(invalid);
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
    config.set_manifest(test_dir.join("Cargo.toml"));
    assert!(launch_tarpaulin(&config, &None).is_ok());
}

#[test]
fn array_coverage() {
    check_percentage("arrays", 1.0f64, true);
}

#[test]
#[ignore]
fn dependency_build_script() {
    // From issue #1297
    // TODO metatensor isn't really maintained that frequently and has broken on nightly. Find an
    // alternative project to test this (or make one)
    check_percentage("metatensor", 1.0f64, true);
}

#[test]
fn lets_coverage() {
    check_percentage("lets", 1.0f64, true);
}

#[test]
#[cfg_attr(target_os="macos", ignore)]
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
    config.set_include_tests(true);
    config.set_clean(false);

    check_percentage_with_config("structs", 1.0f64, true, config.clone());
    check_percentage_with_config("ifelse", 1.0f64, true, config.clone());
    check_percentage_with_config("returns", 1.0f64, true, config.clone());
    check_percentage_with_config("follow_exe", 1.0f64, true, config);
}

#[cfg_attr(not(ptrace_supported), test)]
#[should_panic]
fn ptrace_not_unsupported_system() {
    let config = Config::default();
    config.set_engine(TraceEngine::Ptrace);

    run_config("simple_project", config);
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
    check_percentage("breaks", 0.95f64, true);
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
//#[cfg(not(windows))] // TODO fix
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
//#[cfg(not(windows))] // TODO fix
fn examples_coverage() {
    let test = "example_test";
    check_percentage(test, 0.0f64, true);

    let mut config = Config::default();
    config.run_types = vec![RunType::Examples];
    config.set_clean(false);
    config.set_include_tests(true);
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
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    env::set_var("CARGO_HOME", new_home.display().to_string());
    let run = launch_tarpaulin(&config, &None);
    let _ = fs::remove_dir_all(&new_home);
    match previous {
        Ok(s) => env::set_var("CARGO_HOME", s),
        Err(_) => {
            env::remove_var("CARGO_HOME");
        }
    }
    let (res, _) = run.unwrap();

    env::set_current_dir(restore_dir).unwrap();

    assert_eq!(res.iter().count(), 1);
}

#[test]
fn rustflags_handling() {
    env::remove_var("RUSTFLAGS");
    check_percentage("rustflags", 1.0f64, true);
    env::set_var("RUSTFLAGS", "--cfg=foo");
    let mut config = Config::default();
    config.set_clean(false);

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("rustflags");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

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
    config.set_include_tests(true);
    config.post_test_delay = Some(Duration::from_secs(10));
    // Some false negatives on more recent compilers so lets just aim for above a reasonable threshold and 0 return code
    check_percentage_with_config("fork-test", 0.78f64, true, config);
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
    config.set_include_tests(true);

    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("not_a_file.rs");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

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
    config.set_include_tests(true);
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
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);

    config.run_types = vec![RunType::Doctests];

    env::set_var("RUSTC_BOOTSTRAP", "1");

    let (_res, ret) = launch_tarpaulin(&config, &None).unwrap();
    assert_eq!(ret, 0);
}

#[test]
#[cfg(windows)]
fn sanitised_paths() {
    let test_dir = get_test_path("assigns");
    let report_dir = test_dir.join("reports");
    let mut config = Config::default();
    config.set_engine(TraceEngine::Llvm);
    config.set_include_tests(true);
    config.set_clean(false);
    config.generate.push(OutputFile::Lcov);
    config.generate.push(OutputFile::Html);
    config.generate.push(OutputFile::Xml);
    config.generate.push(OutputFile::Json);
    let _ = fs::remove_dir_all(&report_dir);
    let _ = fs::create_dir(&report_dir);
    config.output_directory = Some(report_dir.clone());

    config.test_timeout = Duration::from_secs(60);
    let restore_dir = env::current_dir().unwrap();
    let test_dir = get_test_path("assigns");
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    env::set_current_dir(format!(r#"\\?\{}"#, config.root().display())).unwrap();
    config.set_manifest(PathBuf::from(format!(r#"\\?\{}"#, config.manifest().display())));
    config.set_clean(false);

    println!("{:#?}", config);
    run(&[config]).unwrap();

    env::set_current_dir(restore_dir).unwrap();

    println!("Look at reports");
    let mut count = 0;
    let bad_path_regex = Regex::new(r#"\\\\\?\\\w:\\"#).unwrap();
    for entry in fs::read_dir(&report_dir).unwrap() {
        let entry = entry.unwrap().path();
        if !entry.is_dir() {
            count += 1;
            println!("Checking: {}", entry.display());
            let f = fs::File::open(entry).unwrap();
            if let Ok(s) = io::read_to_string(f) {
                assert!(!s.is_empty());
                assert!(bad_path_regex.find(&s).is_none());
            } else {
                println!("Not unicode");
            }
        }
    }
    assert_eq!(count, 4);
}

#[test]
fn output_dir_workspace() {
    let test_dir = get_test_path("workspace");
    let report_dir = test_dir.join("reports");
    let mut config = Config::default();
    config.set_engine(TraceEngine::Llvm);
    config.set_include_tests(true);
    config.set_clean(false);
    config.dump_traces = true;
    config.generate.push(OutputFile::Lcov);
    config.generate.push(OutputFile::Html);
    config.generate.push(OutputFile::Xml);
    config.generate.push(OutputFile::Json);
    let _ = fs::remove_dir_all(&report_dir);
    let _ = fs::create_dir(&report_dir);
    config.output_directory = Some(report_dir.clone());

    config.test_timeout = Duration::from_secs(60);

    run_config("workspace", config);

    let mut output = HashSet::new();
    for entry in fs::read_dir(&report_dir).unwrap() {
        let entry = entry.unwrap().path();
        if !entry.is_dir() {
            let file_name = entry.file_name().unwrap().to_string_lossy().to_string();
            output.insert(file_name);
        }
    }
    println!("{:?}", output);
    assert!(output.remove("cobertura.xml"));
    assert!(output.remove("coverage.json"));
    assert!(output.remove("lcov.info"));
    assert!(output.remove("tarpaulin-report.html"));
    assert!(output.remove("tarpaulin-report.json"));
    assert_eq!(output.len(), 1);

    for event_log in &output {
        let events = report_dir.join(event_log);
        let log = fs::read(events).unwrap();
        // We can deserialize event log so it must be good
        serde_json::from_slice::<EventLog>(log.as_slice()).unwrap();
    }
}



#[test]
fn stripped_crate() {
    let mut config = Config::default();
    config.verbose = true;
    config.set_clean(false);
    config.test_timeout = Duration::from_secs(60);

    check_percentage_with_config("stripped", 0.9, true, config);
}


#[test]
fn workspace_no_fail_fast() {
    let mut config = Config::default();
    config.set_clean(false);
    config.set_include_tests(true);
    config.no_fail_fast = true;

    let test_dir = get_test_path("workspace_with_fail_tests");
    env::set_current_dir(&test_dir).unwrap();
    let mut manifest = test_dir;
    manifest.push("Cargo.toml");
    config.set_manifest(manifest);
    config.packages = vec!["bar".to_string(), "foo".to_string()];
    let result = launch_tarpaulin(&config, &None);
    let (result, ret) = result.expect("Test failed");
    let files = result.files();
    assert!(files.iter().any(|f| f.ends_with("foo/src/lib.rs")));
    assert!(ret != 0);
}

#[test]
fn warning_flags_in_config() {
    check_percentage("config_warnings", 1.0f64, true);
}

#[test]
fn workspace_default_members() {
    let mut config = Config::default();
    config.set_clean(false);
    config.set_include_tests(true);

    let only_default = check_percentage_with_config("default_members", 1.0f64, true, config.clone());

    let files = only_default.files();
    assert_eq!(files.len(), 1);
    assert!(files[0].ends_with(Path::new("workspace_1/src/lib.rs")));

    config.all= true;

    let all = check_percentage_with_config("default_members", 1.0f64, true, config);

    let files = all.files();
    assert_eq!(files.len(), 2);
    // We use a BTreeMap so they'll be alphabetically ordered
    assert!(files[0].ends_with(Path::new("workspace_1/src/lib.rs")));
    assert!(files[1].ends_with(Path::new("workspace_2/src/lib.rs")));
}

}
