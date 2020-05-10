use crate::report::cobertura;
use std::fmt::{self, Display, Formatter};
/// Error states that could be returned from tarpaulin
#[derive(Debug)]
pub enum RunError {
    /// Error in cargo manifests
    Manifest(String),
    /// Cargo failed to run
    Cargo(String),
    /// Error trying to resolve package configuration in manifest
    Packages(String),
    /// Tests failed to compile
    TestCompile(String),
    /// Test failed during run
    TestRuntime(String),
    TestFailed,
    /// Failed to parse
    Parse(std::io::Error),
    /// Failed to get test coverage
    TestCoverage(String),
    Trace(String),
    CovReport(String),
    OutFormat(String),
    IO(std::io::Error),
    StateMachine(String),
    NixError(nix::Error),
    Html(String),
    XML(cobertura::Error),
    Lcov(String),
    Json(String),
    Internal,
}

impl Display for RunError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            Self::Manifest(e) => write!(f, "Failed to parse Cargo.toml! Error: {}", e),
            Self::Cargo(e) => write!(f, "Cargo failed to run! Error: {}", e),
            Self::Packages(e) => write!(f, "Failed to resolve package in manifest! Error: {}", e),
            Self::TestCompile(e) => write!(f, "Failed to compile tests! Error: {}", e),
            Self::TestRuntime(e) => write!(f, "Failed to run tests: {}", e),
            Self::TestFailed => write!(f, "Test failed during run"),
            Self::Parse(e) => write!(f, "Error while parsing: {}", e),
            Self::TestCoverage(e) => write!(f, "Failed to get test coverage! Error: {}", e),
            // TODO: Better error message!
            Self::Trace(e) => write!(f, "Failed to trace! Error: {}", e),
            Self::CovReport(e) => write!(f, "Failed to report coverage! Error: {}", e),
            Self::OutFormat(e) => write!(f, "{}", e),
            Self::IO(e) => write!(f, "{}", e),
            Self::StateMachine(e) => write!(f, "Error running test: {}", e),
            Self::NixError(e) => write!(f, "{}", e),
            Self::Html(e) => write!(f, "Failed to generate HTML report! Error: {}", e),
            Self::XML(e) => write!(f, "Failed to generate XML report! Error: {}", e),
            Self::Lcov(e) => write!(f, "Failed to generate Lcov report! Error: {}", e),
            Self::Json(e) => write!(f, "Failed to generate JSON report! Error: {}", e),
            Self::Internal => write!(f, "Tarpaulin experienced an internal error"),
        }
    }
}

impl From<std::io::Error> for RunError {
    fn from(e: std::io::Error) -> Self {
        RunError::IO(e)
    }
}

impl From<nix::Error> for RunError {
    fn from(e: nix::Error) -> Self {
        RunError::NixError(e)
    }
}

impl From<cobertura::Error> for RunError {
    fn from(e: cobertura::Error) -> Self {
        RunError::XML(e)
    }
}

impl From<serde_json::error::Error> for RunError {
    fn from(e: serde_json::error::Error) -> Self {
        RunError::Json(e.to_string())
    }
}
