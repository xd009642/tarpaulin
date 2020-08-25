use crate::config::Config;
use crate::errors::*;
use crate::report::{get_previous_result, safe_json};
use crate::traces::{Trace, TraceMap};
use serde::Serialize;
use std::fs::{read_to_string, File};
use std::io::{self, Write};

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

#[derive(PartialEq)]
enum Context {
    CurrentResults,
    PreviousResults,
}

fn get_json(coverage_data: &TraceMap, context: Context) -> Result<String, RunError> {
    let mut report = CoverageReport { files: Vec::new() };

    for (path, traces) in coverage_data.iter() {
        let content = match read_to_string(path) {
            Ok(k) => k,
            Err(e) => {
                if context == Context::PreviousResults && e.kind() == io::ErrorKind::NotFound {
                    // Assume the file has been deleted since the last run.
                    continue;
                }

                return Err(RunError::Html(format!(
                    "Unable to read source file to string: {}",
                    e.to_string()
                )));
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

    safe_json::to_string_safe(&report)
        .map_err(|e| RunError::Html(format!("Report isn't serializable: {}", e)))
}

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let file_path = config.output_dir().join("tarpaulin-report.html");
    let mut file = match File::create(file_path) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Html(format!(
                "File is not writeable: {}",
                e.to_string()
            )))
        }
    };

    let report_json = get_json(coverage_data, Context::CurrentResults)?;
    let previous_report_json = match get_previous_result(&config) {
        Some(result) => get_json(&result, Context::PreviousResults)?,
        None => String::from("null"),
    };

    match write!(
        file,
        r##"<!doctype html>
<html>
<head>
    <meta charset="utf-8">
    <style>{}</style>
</head>
<body>
    <div id="root"></div>
    <script>
        var data = {};
        var previousData = {};
    </script>
    <script crossorigin>{}</script>
    <script crossorigin>{}</script>
    <script>{}</script>
</body>
</html>"##,
        include_str!("report_viewer.css"),
        report_json,
        previous_report_json,
        include_str!("react.production.min.js"),
        include_str!("react-dom.production.min.js"),
        include_str!("report_viewer.js"),
    ) {
        Ok(_) => (),
        Err(e) => return Err(RunError::Html(e.to_string())),
    };

    Ok(())
}
