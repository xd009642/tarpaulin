use std::collections::HashMap;
use coveralls_api::*;
use traces::{TraceMap, CoverageStat};
use config::Config;
use errors::RunError;

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    if let Some(ref key) = config.coveralls {
        let id = match config.ci_tool {
            Some(ref service) => Identity::ServiceToken(Service {
                service_name: service.clone(),
                service_job_id: key.clone()
            }),
            _ => Identity::RepoToken(key.clone()),
        };
        let mut report = CoverallsReport::new(id);
        for file in &coverage_data.files() {
            let rel_path = config.strip_project_path(file);
            let mut lines: HashMap<usize, usize> = HashMap::new();
            let fcov = coverage_data.get_child_traces(file);

            for c in &fcov {
                match c.stats {
                    CoverageStat::Line(hits) => {
                        lines.insert(c.line as usize, hits as usize);
                    },
                    _ => {
                        info!("Support for coverage statistic not implemented or supported for coveralls.io");
                    },
                }
            }
            if let Ok(source) = Source::new(&rel_path, file, &lines, &None, false) {
                report.add_source(source);
            }
        }

        let res = match config.report_uri {
            Some(ref uri) => {
                info!("Sending report to endpoint: {}", uri);
                report.send_to_endpoint(uri)
            },
            None => {
                info!("Sending coverage data to coveralls.io");
                report.send_to_coveralls()
            }
        };

        match res {
            Ok(_) => Ok(()),
            Err(e) => Err(RunError::CovReport(format!("Coveralls send failed. {}", e))),
        }

    } else {
        Err(RunError::CovReport("No coveralls key specified.".to_string()))
    }
}
