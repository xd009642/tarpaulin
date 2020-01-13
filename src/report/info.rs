use crate::config::Config;
use crate::errors::RunError;
use crate::traces::{TraceMap, CoverageStat};
use std::fs::File;
use std::io::Write;

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let file_path = config.output_directory.join("lcov.info");
    let mut file = match File::create(file_path) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Info(format!(
                "File is not writeable: {}",
                e.to_string()
            )))
        }
    };

    for (path, traces) in coverage_data.iter() {
        writeln!(file, "TN:")?;
        writeln!(file, "SF:{}", path.to_str().unwrap())?;

        let mut fns: Vec<String> = vec![];
        let mut fnda: Vec<String> = vec![];
        let mut da: Vec<String> = vec![];

        for trace in traces {
            if trace.fn_name.is_some() {
                let fn_name = trace.fn_name.clone().unwrap();
                let fn_hits = match trace.stats {
                    CoverageStat::Line(hits) => hits,
                    _ => return Err(RunError::Info(
                        "Function doesn't have hits number".to_string(),
                    )),
                };

                fns.push(format!("FN:{},{}", trace.line, fn_name));
                fnda.push(format!("FNDA:{},{}", fn_hits, fn_name));
            }

            match trace.stats {
                CoverageStat::Line(hits) => {
                    da.push(format!("DA:{},{}", trace.line, hits))
                },
                _ => ()
            };
        }

        for fn_line in fns.iter() {
            writeln!(file, "{}", fn_line)?;
        }

        writeln!(file, "FNF:{}", fns.len())?;

        for fnda_line in fnda {
            writeln!(file, "{}", fnda_line)?;
        }

        for da_line in da {
            writeln!(file, "{}", da_line)?;
        }

        writeln!(file, "end_of_record")?;
    }

    Ok(())
}
