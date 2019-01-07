use std::fs::{read_to_string, File};
use std::io::Write;
use serde::{Serialize};
use traces::{TraceMap, Trace};
use config::Config;

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

pub fn export(coverage_data: &TraceMap, _config: &Config) {
    let mut report = CoverageReport { files: Vec::new() };
    for (path, traces) in coverage_data.iter() {
        report.files.push(SourceFile {
            path: path.components().map(|c| c.as_os_str().to_string_lossy().to_string()).collect(),
            content: read_to_string(path).expect("Source file exists and is a text"),
            traces: traces.clone(),
            covered: coverage_data.covered_in_path(path),
            coverable: coverage_data.coverable_in_path(path),
        });
    }

    let mut file = File::create("tarpaulin-report.html").expect("File is writable");
    write!(file, r##"<!doctype html>
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
        serde_json::to_string(&report).expect("Report is serializable"),
        include_str!("report_viewer.js")
    ).expect("Report is written");
}
