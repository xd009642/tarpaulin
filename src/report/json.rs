use std::convert::From;
use std::{fs, io::Write};

use crate::config::Config;
use crate::errors::*;
use crate::traces::{Trace, TraceMap};
use std::slice::Iter;

use serde::Serialize;

#[derive(Serialize)]
struct SourceFile {
    path: Vec<String>,
    content: String,
    traces: Vec<Trace>,
    covered: usize,
    coverable: usize,
}

#[derive(Serialize)]
pub struct CoverageReport {
    files: Vec<SourceFile>,
}

impl CoverageReport {
    fn iter(&self) -> Iter<SourceFile> {
        self.files.iter()
    }

    pub fn covered(&self) -> Vec<usize> {
        self.iter().map(|r| r.covered).collect()
    }

    pub fn coverable(&self) -> Vec<usize> {
        self.iter().map(|r| r.coverable).collect()
    }
}

impl From<&TraceMap> for Vec<SourceFile> {
    fn from(coverage_data: &TraceMap) -> Self {
        coverage_data
            .iter()
            .map(|(path, traces)| -> Result<SourceFile, RunError> {
                let content = fs::read_to_string(path).map_err(RunError::from)?;
                Ok(SourceFile {
                    path: path
                        .components()
                        .map(|c| c.as_os_str().to_string_lossy().to_string())
                        .collect(),
                    content,
                    traces: traces.clone(),
                    covered: coverage_data.covered_in_path(path),
                    coverable: coverage_data.coverable_in_path(path),
                })
            })
            .filter_map(Result::ok)
            .collect()
    }
}

impl From<&TraceMap> for CoverageReport {
    fn from(coverage_data: &TraceMap) -> Self {
        CoverageReport {
            files: Vec::<SourceFile>::from(coverage_data),
        }
    }
}

type JsonStringResult = Result<String, serde_json::error::Error>;

impl Into<JsonStringResult> for &TraceMap {
    fn into(self) -> JsonStringResult {
        serde_json::to_string(&CoverageReport::from(self))
    }
}

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let file_path = config.output_dir().join("tarpaulin-report.json");
    let report: JsonStringResult = coverage_data.into();
    fs::File::create(file_path)?
        .write_all(report?.as_bytes())
        .map_err(RunError::from)
}
