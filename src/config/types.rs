use std::str::{FromStr};

use coveralls_api::{CiService};
use void::{Void};


arg_enum! {

    #[derive(Debug)]
    pub enum OutputFile {
        Json,
        Toml,
        Stdout,
        Xml,
    }
}

impl Default for OutputFile {

    #[inline]
    fn default() -> Self {
        OutputFile::Stdout
    }
}


pub struct Ci(pub CiService);

impl FromStr for Ci {
    /// This can never fail, so the error type is uninhabited.
    type Err = Void;

    #[inline]
    fn from_str(x: &str) -> Result<Ci, Self::Err> {
        match x {
            "circle-ci"     => Ok(Ci(CiService::Circle)),
            "codeship"      => Ok(Ci(CiService::Codeship)),
            "jenkins"       => Ok(Ci(CiService::Jenkins)),
            "semaphore"     => Ok(Ci(CiService::Semaphore)),
            "travis-ci"     => Ok(Ci(CiService::Travis)),
            "travis-pro"    => Ok(Ci(CiService::TravisPro)),
            other           => Ok(Ci(CiService::Other(other.to_string()))),
        }
    }
}

