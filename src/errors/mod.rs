use crate::report::cobertura;
use failure::Fail;
/// Error states that could be returned from tarpaulin
#[derive(Fail, Debug)]
pub enum RunError {
    /// Error in cargo manifests
    #[fail(display = "Failed to parse Cargo.toml! Error: {}", _0)]
    Manifest(String),
    /// Cargo failed to run
    #[fail(display = "Cargo failed to run! Error: {}", _0)]
    Cargo(String),
    /// Error trying to resolve package configuration in manifest
    #[fail(display = "Failed to resolve package in manifest! Error: {}", _0)]
    Packages(String),
    /// Tests failed to compile
    #[fail(display = "Failed to compile tests! Error: {}", _0)]
    TestCompile(String),
    /// Test failed during run
    #[fail(display = "Failed to run tests: {}", _0)]
    TestRuntime(String),
    #[fail(display = "Test failed during run")]
    TestFailed,
    /// Failed to parse
    #[fail(display = "Error while parsing: {}", _0)]
    Parse(std::io::Error),
    /// Failed to get test coverage
    #[fail(display = "Failed to get test coverage! Error: {}", _0)]
    TestCoverage(String),
    #[fail(display = "Failed to trace! Error: {}", _0)]
    Trace(String),
    #[fail(display = "Failed to report coverage! Error: {}", _0)]
    CovReport(String),
    #[fail(display = "Output format {} is currently not supported!", _0)]
    OutFormat(String),
    #[fail(display = "{}", _0)]
    IO(std::io::Error),
    #[fail(display = "Error running test: {}", _0)]
    StateMachine(String),
    //TODO: Better error message!
    #[fail(display = "{}", _0)]
    NixError(nix::Error),
    #[fail(display = "Failed to generate HTML report! Error: {}", _0)]
    Html(String),
    #[fail(display = "Failed to generate XML report! Error: {}", _0)]
    XML(cobertura::Error),
    #[fail(display = "Failed to generate JSON report! Error: {}", _0)]
    Json(String),
    #[fail(display = "Tarpaulin experienced an internal error")]
    Internal,
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
