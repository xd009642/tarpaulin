use crate::config::{coveralls::CoverallsConfig, Config};
use crate::errors::RunError;
use crate::traces::{CoverageStat, TraceMap};
use coveralls_api::*;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
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
        .map_err(|err| format!("failed to get repository head: {}", err))?;
    let branch = git2::Branch::wrap(head);
    let branch_name = branch
        .name()
        .map_err(|err| format!("failed to get branch name: {}", err))?;
    let get_string = |data: Option<&str>| match data {
        Some(str) => Ok(str.to_string()),
        None => Err("string is not valid utf-8".to_string()),
    };
    let branch_name = get_string(branch_name)?;
    let commit = repo
        .head()
        .unwrap()
        .peel_to_commit()
        .map_err(|err| format!("failed to get commit: {}", err))?;

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

fn get_identity(config: &CoverallsConfig) -> Identity {
    match &config.ci_tool {
        Some(service) => {
            let service_object = match Service::from_ci(service.clone()) {
                Some(s) => s,
                None => Service {
                    name: service.clone(),
                    job_id: Some(config.key.clone()),
                    number: None,
                    build_url: None,
                    branch: None,
                    pull_request: None,
                },
            };
            let key = if service == &CiService::Travis {
                String::new()
            } else {
                config.key.clone()
            };
            Identity::ServiceToken(key, service_object)
        }
        _ => Identity::best_match_with_token(config.key.clone()),
    }
}

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    if let Some(coveralls) = &config.coveralls {
        let id = get_identity(coveralls);

        let mut report = CoverallsReport::new(id);
        for file in &coverage_data.files() {
            let rel_path = config.strip_base_dir(file);
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

        match get_git_info(&config.manifest) {
            Ok(git_info) => {
                report.set_detailed_git_info(git_info);
                info!("Git info collected");
            }
            Err(err) => warn!("Failed to collect git info: {}", err),
        }

        if config.debug {
            if let Ok(text) = serde_json::to_string(&report) {
                info!("Attempting to write coveralls report to coveralls.json");
                let file_path = config.output_dir().join("coveralls.json");
                let _ = fs::write(file_path, text);
            } else {
                warn!("Failed to serialise coverage report");
            }
        }
        let res = if let Some(uri) = &coveralls.report_uri {
            info!("Sending report to endpoint: {}", uri);
            report.send_to_endpoint(uri)
        } else {
            info!("Sending coverage data to coveralls.io");
            report.send_to_coveralls()
        };
        match res {
            Ok(s) => {
                trace!("Coveralls response {:?}", s);
                Ok(())
            }
            Err(e) => Err(RunError::CovReport(format!("Coveralls send failed. {}", e))),
        }
    } else {
        Err(RunError::CovReport(
            "No coveralls key specified.".to_string(),
        ))
    }
}
