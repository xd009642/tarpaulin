use crate::config::Config;
use crate::errors::*;
use crate::report::safe_json;
use crate::traces::{Trace, TraceMap};
use serde::Serialize;
use std::fs::{read_to_string, File};
use std::io::Write;

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
        let content = match read_to_string(path) {
            Ok(k) => k,
            Err(e) => {
                return Err(RunError::Html(format!(
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

    let file_path = config.output_directory.join("tarpaulin-report.html");
    let mut file = match File::create(file_path) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Html(format!(
                "File is not writeable: {}",
                e.to_string()
            )))
        }
    };

    let report_json = match safe_json::to_string_safe(&report) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Html(format!(
                "Report isn't serializable: {}",
                e.to_string()
            )))
        }
    };

    let html_write = match write!(
        file,
        r##"<!doctype html>
<html>
<head>
    <meta charset="utf-8">
    <style>{}</style>
</head>
<body>
    <div id="root"></div>
    <script>var data = {};</script>
    <script crossorigin src="https://unpkg.com/react@16/umd/react.production.min.js"></script>
    <script crossorigin src="https://unpkg.com/react-dom@16/umd/react-dom.production.min.js"></script>
    <script>{}</script>
</body>
</html>"##,
        include_str!("report_viewer.css"),
        report_json,
        include_str!("report_viewer.js")
    ) {
        Ok(_) => (),
        Err(e) => return Err(RunError::Html(e.to_string())),
    };

    Ok(html_write)
}
