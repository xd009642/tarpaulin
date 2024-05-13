use crate::config::Config;
use crate::errors::RunError;
use crate::traces::{CoverageStat, TraceMap};
use coveralls_api::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::{info, trace, warn};

fn get_git_info(manifest_path: &Path) -> Result<GitInfo, String> {
    let dir_path = manifest_path
        .parent()
        .ok_or_else(|| format!("failed to get parent for path: {}", manifest_path.display()))?;
    let repo = git2::Repository::discover(dir_path).map_err(|err| {
        format!(
            "failed to open git repository: {}: {}",
            dir_path.display(),
            err
        )
    })?;

    let head = repo
        .head()
        .map_err(|err| format!("failed to get repository head: {err}"))?;
    let branch = git2::Branch::wrap(head);
    let branch_name = branch
        .name()
        .map_err(|err| format!("failed to get branch name: {err}"))?;
    let get_string = |data: Option<&str>| match data {
        Some(str) => Ok(str.to_string()),
        None => Err("string is not valid utf-8".to_string()),
    };
    let branch_name = get_string(branch_name)?;
    let commit = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .map_err(|err| format!("failed to get commit: {err}"))?;

    let author = commit.author();
    let committer = commit.committer();
    Ok(GitInfo {
        head: Head {
            id: commit.id().to_string(),
            author_name: get_string(author.name())?,
            author_email: get_string(author.email())?,
            committer_name: get_string(committer.name())?,
            committer_email: get_string(committer.email())?,
            message: get_string(commit.message())?,
        },
        branch: branch_name,
        remotes: Vec::new(),
    })
}

fn get_identity(ci_tool: &Option<CiService>, key: &str) -> Identity {
    match ci_tool {
        Some(ref service) => {
            let service_object = match Service::from_ci(service.clone()) {
                Some(s) => s,
                None => Service {
                    name: service.clone(),
                    job_id: Some(key.to_string()),
                    number: None,
                    build_url: None,
                    branch: None,
                    pull_request: None,
                },
            };
            let key = if service == &CiService::Travis {
                String::new()
            } else {
                key.to_string()
            };
            Identity::ServiceToken(key, service_object)
        }
        _ => Identity::best_match_with_token(key.to_string()),
    }
}

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    if let Some(ref key) = config.coveralls {
        let id = get_identity(&config.ci_tool, key);

        let mut report = CoverallsReport::new(id);
        for file in &coverage_data.files() {
            let rel_path = get_rel_path(config, file);
            let mut lines: HashMap<usize, usize> = HashMap::new();
            let fcov = coverage_data.get_child_traces(file);

            for c in fcov {
                match c.stats {
                    CoverageStat::Line(hits) => {
                        lines.insert(c.line as usize, hits as usize);
                    }
                    _ => {
                        info!("Support for coverage statistic not implemented or supported for coveralls.io");
                    }
                }
            }
            if !lines.is_empty() {
                if let Ok(source) = Source::new(&rel_path, file, &lines, &None, false) {
                    report.add_source(source);
                }
            }
        }

        match get_git_info(&config.manifest()) {
            Ok(git_info) => {
                report.set_detailed_git_info(git_info);
                info!("Git info collected");
            }
            Err(err) => warn!("Failed to collect git info: {}", err),
        }

        let res = if let Some(uri) = &config.report_uri {
            info!("Sending report to endpoint: {}", uri);
            report.send_to_endpoint(uri)
        } else {
            info!("Sending coverage data to coveralls.io");
            report.send_to_coveralls()
        };
        if config.debug {
            if let Ok(text) = serde_json::to_string(&report) {
                info!("Attempting to write coveralls report to coveralls.json");
                let file_path = config.output_dir().join("coveralls.json");
                let _ = fs::write(file_path, text);
            } else {
                warn!("Failed to serialise coverage report");
            }
        }
        match res {
            Ok(s) => {
                trace!("Coveralls response {:?}", s);
                Ok(())
            }
            Err(e) => Err(RunError::CovReport(format!("Coveralls send failed. {e}"))),
        }
    } else {
        Err(RunError::CovReport(
            "No coveralls key specified.".to_string(),
        ))
    }
}

fn get_rel_path(config: &Config, file: &&PathBuf) -> PathBuf {
    if cfg!(windows) {
        let rel_path_with_windows_path_separator = config.strip_base_dir(file);
        let rel_path_with_windows_path_separator_as_str =
            String::from(rel_path_with_windows_path_separator.to_str().unwrap());
        let rel_path_with_linux_path_separator =
            rel_path_with_windows_path_separator_as_str.replace('\\', "/");

        PathBuf::from(rel_path_with_linux_path_separator)
    } else {
        config.strip_base_dir(file)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{path::PathBuf, process::Command};

    #[test]
    fn git_info_correct() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
        let res = match get_git_info(&manifest) {
            Ok(r) => r,
            Err(e) => {
                if e.starts_with("failed to get branch name:") {
                    // Pull requests don't get access to working git env
                    return;
                } else {
                    panic!("Unexpected failure to get git info:\n{}", e);
                }
            }
        };

        let git_output = Command::new("git")
            .args(["log", "-1", "--pretty=format:%H %an %ae"])
            .output()
            .unwrap();

        let output = String::from_utf8(git_output.stdout).unwrap();

        let expected = format!(
            "{} {} {}",
            res.head.id, res.head.author_name, res.head.author_email
        );

        assert_eq!(output, expected);
    }

    #[test]
    fn error_if_no_git() {
        let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml");
        println!("{:?}", manifest);
        assert!(get_git_info(&manifest).is_err());
    }

    #[test]
    #[cfg_attr(target_family = "windows", ignore)]
    fn get_rel_path_coveralls_friendly_on_linux() {
        let config = Config::default();
        let file = PathBuf::from("src/report/coveralls.rs");
        let rel_path = get_rel_path(&config, &&file);

        assert_eq!(rel_path, PathBuf::from("src/report/coveralls.rs"));
    }

    #[test]
    #[cfg_attr(not(target_family = "windows"), ignore)]
    fn get_rel_path_coveralls_friendly_on_windows() {
        let config = Config::default();
        let file = PathBuf::from("src\\report\\coveralls.rs");
        let rel_path = get_rel_path(&config, &&file);

        assert_eq!(rel_path, PathBuf::from("src/report/coveralls.rs"));
    }
}
