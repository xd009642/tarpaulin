use clap::{value_t, ArgMatches};
use coveralls_api::CiService;
use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CoverallsConfig {
    #[serde(rename = "coveralls")]
    pub key: String,
    /// Only valid if coveralls option is set. Represents CI tool used.
    #[serde(rename = "ciserver", deserialize_with = "deserialize_ci_server")]
    pub ci_tool: Option<CiService>,
    /// Only valid if coveralls option is set. If coveralls option is set,
    /// as well as report_uri, then the report will be sent to this endpoint
    /// instead.
    #[serde(rename = "report-uri")]
    pub report_uri: Option<String>,
}

pub fn deserialize_ci_server<'de, D>(d: D) -> Result<Option<CiService>, D::Error>
where
    D: Deserializer<'de>,
{
    struct CiServerVisitor;

    impl<'de> de::Visitor<'de> for CiServerVisitor {
        type Value = Option<CiService>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(formatter, "A string containing the ci-service name")
        }

        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if v.is_empty() {
                Ok(None)
            } else {
                Ok(Some(v.parse::<Ci>().unwrap().0))
            }
        }
    }

    d.deserialize_any(CiServerVisitor)
}

#[derive(Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub struct Ci(pub CiService);

impl std::str::FromStr for Ci {
    /// This can never fail, so the error type is uninhabited.
    type Err = std::convert::Infallible;

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

pub(super) fn get_coveralls(args: &ArgMatches) -> Option<CoverallsConfig> {
    Some(CoverallsConfig {
        key: args.value_of("coveralls").map(ToString::to_string)?,
        ci_tool: value_t!(args, "ciserver", Ci).map(|Ci(x)| x).ok(),
        report_uri: args.value_of("report-uri").map(ToString::to_string)
    })
}
