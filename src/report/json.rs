use std::{fs, io::Write};

use crate::config::Config;
use crate::errors::*;
use crate::traces::{Trace, TraceMap};
use crate::report::safe_json;

use serde::Serialize;

#[derive(Serialize)]
struct SourceFile {
    pub path: Vec<String>,
    pub content: String,
    pub traces: Vec<Trace>,
    pub covered: usize,
    pub coverable: usize,
}

#[derive(Serialize)]
struct CoverageReport {
    pub files: Vec<SourceFile>,
}

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let mut report = CoverageReport { files: Vec::new() };
    for (path, traces) in coverage_data.iter() {
        let content = match fs::read_to_string(path) {
            Ok(k) => k,
            Err(e) => {
                return Err(RunError::Json(format!(
                    "Unable to read source file to string: {}",
                    e.to_string()
                )))
            }
        };

        report.files.push(SourceFile {
            path: path
                .components()
                .map(|c| c.as_os_str().to_string_lossy().to_string())
                .collect(),
            content,
            traces: traces.clone(),
            covered: coverage_data.covered_in_path(path),
            coverable: coverage_data.coverable_in_path(path),
        });
    }

    let file_path = config.output_directory.join("tarpaulin-report.json");
    let mut file = match fs::File::create(file_path) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Json(format!(
                "File is not writeable: {}",
                e.to_string()
            )))
        }
    };

    let report_json = match safe_json::to_string_safe(&report) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Json(format!(
                "Report isn't serializable: {}",
                e.to_string()
            )))
        }
    };

    file.write_all(report_json.as_bytes())
        .map_err(|e| RunError::Json(e.to_string()))
}
