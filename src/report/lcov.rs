use crate::config::Config;
use crate::errors::RunError;
use crate::traces::{CoverageStat, TraceMap};
use std::fs::File;
use std::io::Write;

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let file_path = config.output_dir().join("lcov.info");
    let mut file = match File::create(file_path) {
        Ok(k) => k,
        Err(e) => {
            return Err(RunError::Lcov(format!(
                "File is not writeable: {}",
                e.to_string()
            )))
        }
    };

    for (path, traces) in coverage_data.iter() {
        if traces.is_empty() {
            continue;
        }
        writeln!(file, "TN:")?;
        writeln!(file, "SF:{}", path.to_str().unwrap())?;

        let mut fns: Vec<String> = vec![];
        let mut fnda: Vec<String> = vec![];
        let mut da: Vec<(u64, u64)> = vec![];

        for trace in traces {
            if trace.fn_name.is_some() {
                let fn_name = trace.fn_name.clone().unwrap();
                let fn_hits = match trace.stats {
                    CoverageStat::Line(hits) => hits,
                    _ => {
                        return Err(RunError::Lcov(
                            "Function doesn't have hits number".to_string(),
                        ))
                    }
                };

                fns.push(format!("FN:{},{}", trace.line, fn_name));
                fnda.push(format!("FNDA:{},{}", fn_hits, fn_name));
            }

            match trace.stats {
                CoverageStat::Line(hits) => da.push((trace.line, hits)),
                _ => (),
            };
        }

        for fn_line in fns.iter() {
            writeln!(file, "{}", fn_line)?;
        }

        writeln!(file, "FNF:{}", fns.len())?;

        for fnda_line in fnda {
            writeln!(file, "{}", fnda_line)?;
        }

        for (line, hits) in da.iter() {
            writeln!(file, "DA:{},{}", line, hits)?;
        }

        writeln!(file, "LF:{}", da.len())?;
        writeln!(
            file,
            "LH:{}",
            da.iter().filter(|(_, hits)| *hits != 0).count()
        )?;

        // TODO: add support for branching
        // BRDA (BRDA:<line number>,<block number>,<branch number>,<hits>)
        // BRF (branches found)
        // BRH (branches hit)
        // More at http://ltp.sourceforge.net/coverage/lcov/geninfo.1.php

        writeln!(file, "end_of_record")?;
    }

    Ok(())
}
