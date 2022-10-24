use clap::arg_enum;
use coveralls_api::CiService;
use serde::{Deserialize, Serialize};
use std::str::FromStr;

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum Color {
        Auto,
        Always,
        Never,
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum TraceEngine {
        Auto,
        Ptrace,
        Llvm,
    }
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

impl Default for TraceEngine {
    fn default() -> Self {
        cfg_if::cfg_if! {
            if #[cfg(ptrace_supported)] {
                TraceEngine::Ptrace
            } else {
                TraceEngine::Llvm
            }
        }
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum Mode {
        Test,
        Build
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum RunType {
        Tests,
        Doctests,
        Benchmarks,
        Examples,
        Lib,
        Bins,
        AllTargets,
    }
}

arg_enum! {
    #[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
    #[non_exhaustive]
    pub enum OutputFile {
        Json,
        Stdout,
        Xml,
        Html,
        Lcov,
    }
}

impl Default for OutputFile {
    #[inline]
    fn default() -> Self {
        OutputFile::Stdout
    }
}

#[derive(Debug, Eq, PartialEq, Ord, PartialOrd, Deserialize, Serialize)]
pub struct Ci(pub CiService);

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
