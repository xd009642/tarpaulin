use clap_derive::ArgEnum;
use coveralls_api::CiService;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, ArgEnum, Deserialize, Serialize,
)]
pub enum Color {
    Auto,
    Always,
    Never,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Always => write!(f, "always"),
            Self::Never => write!(f, "never"),
        }
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, ArgEnum, Deserialize, Serialize,
)]
pub enum TraceEngine {
    Auto,
    Ptrace,
    Llvm,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, ArgEnum, Deserialize, Serialize,
)]
pub enum Mode {
    Test,
    Build,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, ArgEnum, Deserialize, Serialize,
)]
pub enum RunType {
    Tests,
    Doctests,
    Benchmarks,
    Examples,
    Lib,
    Bins,
    AllTargets,
}

#[derive(
    Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, ArgEnum, Deserialize, Serialize,
)]
pub enum OutputFile {
    Json,
    Stdout,
    Xml,
    Html,
    Lcov,
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
pub struct Ci(pub CiService);

impl Default for OutputFile {
    #[inline]
    fn default() -> Self {
        OutputFile::Stdout
    }
}

impl FromStr for Color {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as clap::ArgEnum>::from_str(s, true)
    }
}

impl FromStr for TraceEngine {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as clap::ArgEnum>::from_str(s, true)
    }
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as clap::ArgEnum>::from_str(s, true)
    }
}

impl FromStr for RunType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as clap::ArgEnum>::from_str(s, true)
    }
}

impl FromStr for OutputFile {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        <Self as clap::ArgEnum>::from_str(s, true)
    }
}

impl FromStr for Ci {
    /// This can never fail, so the error type is uninhabited.
    type Err = std::convert::Infallible;

    #[inline]
    fn from_str(x: &str) -> Result<Ci, Self::Err> {
        match x {
            "circle-ci" => Ok(Ci(CiService::Circle)),
            "codeship" => Ok(Ci(CiService::Codeship)),
            "jenkins" => Ok(Ci(CiService::Jenkins)),
            "semaphore" => Ok(Ci(CiService::Semaphore)),
            "travis-ci" => Ok(Ci(CiService::Travis)),
            "travis-pro" => Ok(Ci(CiService::TravisPro)),
            other => Ok(Ci(CiService::Other(other.to_string()))),
        }
    }
}
