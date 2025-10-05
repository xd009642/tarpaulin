use clap::ValueEnum;
#[cfg(feature = "coveralls")]
use coveralls_api::CiService;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize, ValueEnum,
)]
#[value(rename_all = "PascalCase")]
pub enum Color {
    Auto,
    Always,
    Never,
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Color::Auto => write!(f, "Auto"),
            Color::Always => write!(f, "Always"),
            Color::Never => write!(f, "Never"),
        }
    }
}

#[derive(
    Debug,
    Default,
    Copy,
    Clone,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
    Deserialize,
    Serialize,
    ValueEnum,
)]
#[value(rename_all = "PascalCase")]
pub enum TraceEngine {
    Auto,
    #[cfg_attr(ptrace_supported, default)]
    Ptrace,
    #[cfg_attr(not(ptrace_supported), default)]
    Llvm,
}

impl TraceEngine {
    pub const fn supported() -> &'static [TraceEngine] {
        cfg_if::cfg_if! {
            if #[cfg(ptrace_supported)] {
                &[TraceEngine::Ptrace, TraceEngine::Llvm]
            } else {
                &[TraceEngine::Llvm]
            }
        }
    }
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize, ValueEnum,
)]
#[value(rename_all = "PascalCase")]
pub enum Mode {
    Test,
    Build,
}

#[derive(
    Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize, ValueEnum,
)]
#[value(rename_all = "PascalCase")]
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
    Debug,
    Default,
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Deserialize,
    Serialize,
    ValueEnum,
)]
#[value(rename_all = "PascalCase")]
#[non_exhaustive]
pub enum OutputFile {
    Json,
    #[default]
    Stdout,
    Xml,
    Html,
    Lcov,
    Markdown,
}

#[cfg(feature = "coveralls")]
#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
pub struct Ci(pub CiService);

#[cfg(feature = "coveralls")]
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
