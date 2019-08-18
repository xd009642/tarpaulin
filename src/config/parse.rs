use crate::config::types::*;
use clap::{value_t, values_t, ArgMatches};
use coveralls_api::CiService;
use log::error;
use regex::Regex;
use std::env;
use std::path::PathBuf;
use std::time::Duration;

pub(super) fn get_list(args: &ArgMatches, key: &str) -> Vec<String> {
    args.values_of_lossy(key).unwrap_or_else(Vec::new)
}

pub(super) fn get_line_cov(args: &ArgMatches) -> bool {
    let cover_lines = args.is_present("line");
    let cover_branches = args.is_present("branch");

    cover_lines || !(cover_lines || cover_branches)
}

pub(super) fn get_branch_cov(args: &ArgMatches) -> bool {
    let cover_lines = args.is_present("line");
    let cover_branches = args.is_present("branch");

    cover_branches || !(cover_lines || cover_branches)
}

pub(super) fn get_manifest(args: &ArgMatches) -> PathBuf {
    if let Some(path) = args.value_of("manifest-path") {
        return PathBuf::from(path);
    }

    let mut manifest = env::current_dir().unwrap();

    if let Some(path) = args.value_of("root") {
        manifest.push(path);
    }

    manifest.push("Cargo.toml");
    manifest.canonicalize().unwrap_or(manifest)
}

pub(super) fn get_root(args: &ArgMatches) -> Option<String> {
    args.value_of("root").map(ToString::to_string)
}

pub(super) fn get_ci(args: &ArgMatches) -> Option<CiService> {
    value_t!(args, "ciserver", Ci).map(|x| x.0).ok()
}

pub(super) fn get_coveralls(args: &ArgMatches) -> Option<String> {
    args.value_of("coveralls").map(ToString::to_string)
}

pub(super) fn get_report_uri(args: &ArgMatches) -> Option<String> {
    args.value_of("report-uri").map(ToString::to_string)
}

pub(super) fn get_outputs(args: &ArgMatches) -> Vec<OutputFile> {
    values_t!(args.values_of("out"), OutputFile).unwrap_or(vec![])
}

pub(super) fn get_run_types(args: &ArgMatches) -> Vec<RunType> {
    values_t!(args.values_of("run-types"), RunType).unwrap_or(vec![RunType::Tests])
}

pub(super) fn get_excluded(args: &ArgMatches) -> Vec<Regex> {
    let mut files = vec![];

    for temp_str in &get_list(args, "exclude-files") {
        let s = &temp_str.replace(".", r"\.").replace("*", ".*");

        if let Ok(re) = Regex::new(s) {
            files.push(re);
        } else {
            error!("Invalid regex: {}", temp_str);
        }
    }

    files
}

pub(super) fn get_timeout(args: &ArgMatches) -> Duration {
    if args.is_present("timeout") {
        let duration = value_t!(args.value_of("timeout"), u64).unwrap_or(60);
        Duration::from_secs(duration)
    } else {
        Duration::from_secs(60)
    }
}
