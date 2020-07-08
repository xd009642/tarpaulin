use crate::config::*;
use crate::errors::*;
use crate::test_loader::TracerData;
use crate::traces::*;
use log::{error, info};
use serde::Serialize;
use std::fs::{create_dir_all, File};
use std::io::BufReader;

pub mod cobertura;
pub mod coveralls;
pub mod html;
pub mod json;
pub mod lcov;
mod safe_json;
/// Trait for report formats to implement.
/// Currently reports must be serializable using serde
pub trait Report<Out: Serialize> {
    /// Export coverage report
    fn export(coverage_data: &[TracerData], config: &Config);
}

/// Reports the test coverage using the users preferred method. See config.rs
/// or help text for details.
pub fn report_coverage(config: &Config, result: &TraceMap) -> Result<(), RunError> {
    if !result.is_empty() {
        info!("Coverage Results:");
        if config.verbose {
            print_missing_lines(config, result);
        }
        print_summary(config, result);
        generate_requested_reports(config, result)?;
        let mut report_dir = config.target_dir();
        report_dir.push("tarpaulin");
        if !report_dir.exists() {
            let _ = create_dir_all(&report_dir);
        }
        report_dir.push("coverage.json");
        let file = File::create(&report_dir)
            .map_err(|_| RunError::CovReport("Failed to create run report".to_string()))?;
        serde_json::to_writer(&file, &result)
            .map_err(|_| RunError::CovReport("Failed to save run report".to_string()))?;
        Ok(())
    } else if !config.no_run {
        Err(RunError::CovReport(
            "No coverage results collected.".to_string(),
        ))
    } else {
        Ok(())
    }
}

fn generate_requested_reports(config: &Config, result: &TraceMap) -> Result<(), RunError> {
    if config.is_coveralls() {
        coveralls::export(result, config)?;
        info!("Coverage data sent");
    }

    if !config.is_default_output_dir() && create_dir_all(&config.output_dir()).is_err() {
        return Err(RunError::OutFormat(format!(
            "Failed to create or locate custom output directory: {:?}",
            config.output_directory,
        )));
    }

    for g in &config.generate {
        match *g {
            OutputFile::Xml => {
                cobertura::report(result, config).map_err(RunError::XML)?;
            }
            OutputFile::Html => {
                html::export(result, config)?;
            }
            OutputFile::Lcov => {
                lcov::export(result, config)?;
            }
            OutputFile::Json => {
                json::export(result, config)?;
            }
            _ => {
                return Err(RunError::OutFormat(
                    "Output format is currently not supported!".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn print_missing_lines(config: &Config, result: &TraceMap) {
    println!("|| Uncovered Lines:");
    for (ref key, ref value) in result.iter() {
        let path = config.strip_base_dir(key);
        let mut uncovered_lines = vec![];
        for v in value.iter() {
            match v.stats {
                CoverageStat::Line(count) if count == 0 => {
                    uncovered_lines.push(v.line);
                }
                _ => (),
            }
        }
        uncovered_lines.sort();
        let (groups, last_group) = uncovered_lines
            .into_iter()
            .fold((vec![], vec![]), accumulate_lines);
        let (groups, _) = accumulate_lines((groups, last_group), u64::max_value());
        if !groups.is_empty() {
            println!("|| {}: {}", path.display(), groups.join(", "));
        }
    }
}

fn get_previous_result(config: &Config) -> Option<TraceMap> {
    // Check for previous report
    let mut report_dir = config.target_dir();
    report_dir.push("tarpaulin");
    if report_dir.exists() {
        // is report there?
        report_dir.push("coverage.json");
        let file = File::open(&report_dir).ok()?;
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).ok()
    } else {
        // make directory
        create_dir_all(&report_dir)
            .unwrap_or_else(|e| error!("Failed to create report directory: {}", e));
        None
    }
}

fn print_summary(config: &Config, result: &TraceMap) {
    let last = match get_previous_result(config) {
        Some(l) => l,
        None => TraceMap::new(),
    };
    println!("|| Tested/Total Lines:");
    for file in result.files() {
        if result.coverable_in_path(&file) == 0 {
            continue;
        }
        let path = config.strip_base_dir(file);
        if last.contains_file(file) && last.coverable_in_path(&file) > 0 {
            let last_percent = coverage_percentage(&last.get_child_traces(file));
            let current_percent = coverage_percentage(&result.get_child_traces(file));
            let delta = 100.0f64 * (current_percent - last_percent);
            println!(
                "|| {}: {}/{} {:+}%",
                path.display(),
                result.covered_in_path(&file),
                result.coverable_in_path(&file),
                delta
            );
        } else {
            println!(
                "|| {}: {}/{}",
                path.display(),
                result.covered_in_path(&file),
                result.coverable_in_path(&file)
            );
        }
    }
    let percent = result.coverage_percentage() * 100.0f64;
    if last.is_empty() {
        println!(
            "|| \n{:.2}% coverage, {}/{} lines covered",
            percent,
            result.total_covered(),
            result.total_coverable()
        );
    } else {
        let delta = percent - 100.0f64 * last.coverage_percentage();
        println!(
            "|| \n{:.2}% coverage, {}/{} lines covered, {:+}% change in coverage",
            percent,
            result.total_covered(),
            result.total_coverable(),
            delta
        );
    }
}

fn accumulate_lines(
    (mut acc, mut group): (Vec<String>, Vec<u64>),
    next: u64,
) -> (Vec<String>, Vec<u64>) {
    if let Some(last) = group.last().cloned() {
        if next == last + 1 {
            group.push(next);
            (acc, group)
        } else {
            match (group.first(), group.last()) {
                (Some(first), Some(last)) if first == last => {
                    acc.push(format!("{}", first));
                }
                (Some(first), Some(last)) => {
                    acc.push(format!("{}-{}", first, last));
                }
                (Some(_), None) | (None, _) => (),
            };
            (acc, vec![next])
        }
    } else {
        group.push(next);
        (acc, group)
    }
}
