use std::fs::read_to_string;
use crate::traces::TraceMap;
use crate::config::Config;
use crate::errors::RunError;

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
  for (path, traces) in coverage_data.iter() {
    let content = match read_to_string(path) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Info(format!(
                "Unable to read source file to string: {}",
                e.to_string()
            )))
        }
    };

    println!("{:?}", traces);
  }

  Ok(())
}