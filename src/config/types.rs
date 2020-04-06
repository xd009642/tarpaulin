use clap::arg_enum;
use coveralls_api::CiService;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use void::Void;

arg_enum! {
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
    pub enum RunType {
        Tests,
        Doctests,
        Benchmarks,
        Examples,
    }
}

arg_enum! {
    #[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Deserialize, Serialize)]
    pub enum OutputFile {
        Json,
        Toml,
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
    type Err = Void;

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
