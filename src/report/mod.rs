#![allow(unreachable_patterns)] // We may want to add more warnings and keep error logs stable
use crate::config::*;
use crate::errors::*;
use crate::test_loader::TracerData;
use crate::traces::*;
use cargo_metadata::Metadata;
use serde::Serialize;
use std::fs::{create_dir_all, File};
use std::io::{self, BufReader, Write};
use tracing::{error, info};

pub mod cobertura;
#[cfg(feature = "coveralls")]
pub mod coveralls;
pub mod html;
pub mod json;
pub mod lcov;
pub mod markdown;
mod safe_json;
/// Trait for report formats to implement.
/// Currently reports must be serializable using serde
pub trait Report<Out: Serialize> {
    /// Export coverage report
    fn export(coverage_data: &[TracerData], config: &Config);
}

fn coverage_report_name(config: &Config) -> String {
    // separate reports by package to prevent clashing statistics
    let mut joined = String::from("-");
    if !config.packages.is_empty() {
        let mut packages = config.packages.clone();
        packages.sort();
        joined = String::from("-") + &packages.join("-") + "-";
    }

    config
        .get_metadata()
        .as_ref()
        .and_then(Metadata::root_package)
        .map(|x| format!("{}{}coverage.json", x.name, joined))
        .unwrap_or_else(|| "coverage.json".to_string())
}

/// Reports the test coverage using the users preferred method. See config.rs
/// or help text for details.
pub fn report_coverage(config: &Config, result: &TraceMap) -> Result<(), RunError> {
    if !result.is_empty() {
        generate_requested_reports(config, result)?;
        let report_dir = config.output_dir();
        if !report_dir.exists() {
            let _ = create_dir_all(&report_dir);
        }

        let mut report_dir = config.target_dir().join("tarpaulin");
        report_dir.push(coverage_report_name(config));
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
    #[cfg(feature = "coveralls")]
    if config.is_coveralls() {
        coveralls::export(result, config)?;
        info!("Coverage data sent");
    }
    info!("Coverage Results:");

    if !config.is_default_output_dir() && create_dir_all(config.output_dir()).is_err() {
        return Err(RunError::OutFormat(format!(
            "Failed to create or locate custom output directory: {:?}",
            config.output_directory,
        )));
    }

    if config.verbose || config.generate.is_empty() {
        print_missing_lines(config, result);
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
            OutputFile::Markdown => {
                markdown::export(result, config)?;
            }
            OutputFile::Stdout => {
                // Already reported the missing lines
                if !config.verbose {
                    print_missing_lines(config, result);
                }
            }
            _ => {
                return Err(RunError::OutFormat(
                    "Output format is currently not supported!".to_string(),
                ));
            }
        }
    }
    // We always want to report the short summary
    print_summary(config, result);
    Ok(())
}

fn print_missing_lines(config: &Config, result: &TraceMap) {
    let mut w: Box<dyn Write> = if config.stderr {
        Box::new(io::stderr().lock())
    } else {
        Box::new(io::stdout().lock())
    };
    writeln!(w, "|| Uncovered Lines:").unwrap();
    for (key, value) in result.iter() {
        let path = config.strip_base_dir(key);
        let mut uncovered_lines = vec![];
        for v in value.iter() {
            if let CoverageStat::Line(0) = v.stats {
                uncovered_lines.push(v.line);
            }
        }
        uncovered_lines.sort_unstable();
        let (groups, last_group) = uncovered_lines
            .into_iter()
            .fold((vec![], vec![]), accumulate_lines);
        let (groups, _) = accumulate_lines((groups, last_group), u64::max_value());
        if !groups.is_empty() {
            writeln!(w, "|| {}: {}", path.display(), groups.join(", ")).unwrap();
        }
    }
}

fn get_previous_result(config: &Config) -> Option<TraceMap> {
    // Check for previous report
    let mut report_dir = config.target_dir();
    report_dir.push("tarpaulin");
    if report_dir.exists() {
        // is report there?
        report_dir.push(coverage_report_name(config));
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
    let mut w: Box<dyn Write> = if config.stderr {
        Box::new(io::stderr().lock())
    } else {
        Box::new(io::stdout().lock())
    };
    let last = match get_previous_result(config) {
        Some(l) => l,
        None => TraceMap::new(),
    };
    // All the `writeln` unwraps are fine, it's basically what the `println` macro does
    writeln!(w, "|| Tested/Total Lines:").unwrap();
    for file in result.files() {
        if result.coverable_in_path(file) == 0 {
            continue;
        }
        let path = config.strip_base_dir(file);
        if last.contains_file(file) && last.coverable_in_path(file) > 0 {
            let last_percent = coverage_percentage(last.get_child_traces(file));
            let current_percent = coverage_percentage(result.get_child_traces(file));
            let delta = 100.0f64 * (current_percent - last_percent);
            writeln!(
                w,
                "|| {}: {}/{} {:+.2}%",
                path.display(),
                result.covered_in_path(file),
                result.coverable_in_path(file),
                delta
            )
            .unwrap();
        } else {
            writeln!(
                w,
                "|| {}: {}/{}",
                path.display(),
                result.covered_in_path(file),
                result.coverable_in_path(file)
            )
            .unwrap();
        }
    }
    let percent = result.coverage_percentage() * 100.0f64;
    if result.total_coverable() == 0 {
        writeln!(w, "No coverable lines found").unwrap();
    } else if last.is_empty() {
        writeln!(
            w,
            "|| \n{:.2}% coverage, {}/{} lines covered",
            percent,
            result.total_covered(),
            result.total_coverable()
        )
        .unwrap();
    } else {
        let delta = percent - 100.0f64 * last.coverage_percentage();
        writeln!(
            w,
            "|| \n{:.2}% coverage, {}/{} lines covered, {:+.2}% change in coverage",
            percent,
            result.total_covered(),
            result.total_coverable(),
            delta
        )
        .unwrap();
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
                    acc.push(format!("{first}"));
                }
                (Some(first), Some(last)) => {
                    acc.push(format!("{first}-{last}"));
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

#[cfg(test)]
mod tests {
    use crate::{config::Config, report::coverage_report_name};

    #[test]
    fn coverage_report_name_no_package() {
        let config = Config::default();

        let name_report = coverage_report_name(&config);
        assert_eq!(
            name_report, "cargo-tarpaulin-coverage.json",
            "Suffix should have been added and name should be in title"
        );
    }

    #[test]
    fn coverage_report_name_1_package() {
        let mut config = Config::default();
        config.packages = vec![String::from("bintest")];

        let name_report = coverage_report_name(&config);
        assert_eq!(name_report, "cargo-tarpaulin-bintest-coverage.json", "Suffix should have been added, name should be in title, and also package should be present");
    }

    #[test]
    fn coverage_report_name_3_packages() {
        let mut config = Config::default();
        config.packages = vec![
            String::from("pizza"),
            String::from("bintest"),
            String::from("fur"),
        ];

        let name_report = coverage_report_name(&config);
        assert_eq!(name_report, "cargo-tarpaulin-bintest-fur-pizza-coverage.json", "Suffix should have been added, name should be in title, and also packages should be present");
    }

    #[test]
    fn coverage_report_name_3_packages_diff() {
        let mut config = Config::default();
        config.packages = vec![
            String::from("pizza"),
            String::from("fur"),
            String::from("bintest"),
        ];

        let name_report = coverage_report_name(&config);
        assert_eq!(name_report, "cargo-tarpaulin-bintest-fur-pizza-coverage.json", "Suffix should have been added, name should be in title, and also packages should be present in alphabetical order");
    }
}
