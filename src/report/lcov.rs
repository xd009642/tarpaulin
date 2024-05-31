use crate::config::Config;
use crate::errors::RunError;
use crate::traces::{CoverageStat, TraceMap};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let file_path = config.output_dir().join("lcov.info");
    let file = match File::create(file_path) {
        Ok(k) => k,
        Err(e) => return Err(RunError::Lcov(format!("File is not writeable: {e}"))),
    };

    write_lcov(file, coverage_data)
}

fn write_lcov(mut file: impl Write, coverage_data: &TraceMap) -> Result<(), RunError> {
    for (path, traces) in coverage_data.iter() {
        if traces.is_empty() {
            continue;
        }
        writeln!(file, "TN:")?;
        writeln!(file, "SF:{}", path.to_str().unwrap())?;

        let mut fns: Vec<String> = vec![];
        let mut fnda: Vec<String> = vec![];
        let mut da: Vec<(u64, u64)> = vec![];

        let mut fn_locs = coverage_data
            .get_functions(&path)
            .map(|x| ((x.start, x.end), &x.name))
            .collect::<BTreeMap<_, _>>();

        let mut first_fn = fn_locs.pop_first();

        for trace in traces {
            match &first_fn {
                Some(((start, end), name)) if (*start..*end).contains(&trace.line) => {
                    let fn_hits = match trace.stats {
                        CoverageStat::Line(hits) => hits,
                        _ => {
                            return Err(RunError::Lcov(
                                "Function doesn't have hits number".to_string(),
                            ))
                        }
                    };

                    fns.push(format!("FN:{},{}", trace.line, name));
                    fnda.push(format!("FNDA:{fn_hits},{name}"));

                    first_fn = fn_locs.pop_first();
                }
                _ => {}
            }
            /*if trace.fn_name.is_some() {
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
                fnda.push(format!("FNDA:{fn_hits},{fn_name}"));
            }*/

            if let CoverageStat::Line(hits) = trace.stats {
                da.push((trace.line, hits));
            }
        }

        for fn_line in &fns {
            writeln!(file, "{fn_line}",)?;
        }

        writeln!(file, "FNF:{}", fns.len())?;

        for fnda_line in fnda {
            writeln!(file, "{fnda_line}")?;
        }

        for (line, hits) in &da {
            writeln!(file, "DA:{line},{hits}")?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source_analysis::Function;
    use crate::traces::*;
    use lcov::{record::Record, Reader};
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::path::{Path, PathBuf};

    #[test]
    fn generate_valid_lcov() {
        let mut traces = TraceMap::new();
        traces.add_trace(
            Path::new("foo.rs"),
            Trace {
                line: 4,
                stats: CoverageStat::Line(1),
                address: Default::default(),
                length: 0,
            },
        );
        traces.add_trace(
            Path::new("foo.rs"),
            Trace {
                line: 5,
                stats: CoverageStat::Line(0),
                address: Default::default(),
                length: 0,
            },
        );

        traces.add_trace(
            Path::new("bar.rs"),
            Trace {
                line: 14,
                stats: CoverageStat::Line(9),
                address: Default::default(),
                length: 0,
            },
        );

        let mut functions = HashMap::new();
        functions.insert(
            PathBuf::from("bar.rs"),
            vec![Function {
                name: "baz".to_string(),
                start: 14,
                end: 20,
            }],
        );
        traces.set_functions(functions);

        let mut data = vec![];
        let cursor = Cursor::new(&mut data);

        write_lcov(cursor, &traces).unwrap();

        let reader = Reader::new(data.as_slice());
        let mut items = 0;
        let mut files_seen = 0;

        let mut current_source = PathBuf::new();
        for item in reader {
            let record = item.unwrap();

            match record {
                Record::SourceFile { path } => {
                    current_source = path.clone();
                    // We know files are presented sorted
                    if files_seen == 0 {
                        assert_eq!(path, Path::new("bar.rs"));
                    } else if files_seen == 1 {
                        assert_eq!(path, Path::new("foo.rs"));
                    } else {
                        panic!("Too many files");
                    }

                    files_seen += 1;
                }
                Record::EndOfRecord => {
                    current_source = PathBuf::new();
                }
                Record::FunctionName { name, start_line } => {
                    assert_eq!(name, "baz");
                    assert_eq!(start_line, 14);
                }
                Record::LineData {
                    line,
                    count,
                    checksum: _,
                } => {
                    if current_source == Path::new("bar.rs") {
                        assert_eq!(line, 14);
                        assert_eq!(count, 9);
                    } else if current_source == Path::new("foo.rs") {
                        assert!((line == 4 && count == 1) || (line == 5 && count == 0));
                    } else {
                        panic!("Line data not attached to file");
                    }
                }
                Record::LinesFound { found } => {
                    if current_source == Path::new("bar.rs") {
                        assert_eq!(found, 1);
                    } else if current_source == Path::new("foo.rs") {
                        assert_eq!(found, 2);
                    } else {
                        panic!("Lines found not attached to file");
                    }
                }
                Record::LinesHit { hit } => {
                    if current_source == Path::new("bar.rs") {
                        assert_eq!(hit, 1);
                    } else if current_source == Path::new("foo.rs") {
                        assert_eq!(hit, 1);
                    } else {
                        panic!("Lines found not attached to file");
                    }
                }
                _ => {}
            }

            items += 1;
        }
        assert!(items > 0);
    }
}
