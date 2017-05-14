use coveralls_api::{CiService, Source, CoverallsReport};
use tracer::TracerData;
use report::Report;
use config::Config;

impl Report<CoverallsReport> for CoverallsReport {
    fn export(coverage_data: &Vec<TracerData>, config: &Config) {
        if let Some(ref key) = config.coveralls {
            // find unique file paths and get tracer data for only those paths.
            // Strip manifest root from path and convert to c_api::Source
            // Add all sources to CoverallsReport
            // Send!

            match config.ci_tool {
                Some(CiService::Travis) => {},
                Some(CiService::TravisPro) => {},
                _ => {},
            }
            
        } else {
            panic!("Not coveralls key specified.");
        }
    }
}
