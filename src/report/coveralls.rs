use std::path::Path;
use tracer::TracerData;
use report::Report;


/// Struct representing source files and the coverage for coveralls
#[derive(Serialize)]
pub struct Source {
    /// Name of the source file. Represented as path relative to root of repo
    name: String,
    /// MD5 hash of the source file
    source_digest: String,
    /// Coverage for the source. Each element is a line with the following rules:
    /// None - not relevant to coverage
    /// 0 - not covered
    /// 1+ - covered and how often
    coverage: Vec<Option<usize>>,
}

/// Coveralls report struct
#[derive(Serialize)]
pub struct CoverallsReport {
    /// Service job ID is used for CI integration. Coveralls current supports
    /// * travis ci
    /// * travis pro
    /// * circleCI
    /// * Semaphore
    /// * JenkinsCI
    /// * Codeship
    service_job_id: Option<String>,
    /// Service name. Paired with service_job_id
    service_name: Option<String>,
    /// Other way to reference a repository. Via a repo token.
    repo_token: Option<String>,
    /// List of source files which includes coverage information.
    source_files: Vec<Source>,
}


impl Report<CoverallsReport> for CoverallsReport {
    fn export(coverage_data: &Vec<TracerData>, path: &Path) {
        unimplemented!();
    }
}
