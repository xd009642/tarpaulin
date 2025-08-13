use crate::config::Config;
use crate::errors::*;
use crate::report::get_previous_result;
use crate::traces::TraceMap;
use std::fs::File;
use std::io::Write;
use tracing::info;

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    // Initialize the markdown file we'll write to
    let file_path = config.output_dir().join("tarpaulin-report.md");
    let mut file = match File::create(&file_path) {
        Ok(k) => k,
        Err(e) => return Err(RunError::Html(format!("File is not writeable: {e}"))),
    };

    // Make sure we have previous results available so we can compare when appropriate
    let previous_result = get_previous_result(config);

    // Get the total covered lines and total coverable lines so we can calculate percentage covered
    let total_covered: usize = coverage_data
        .iter()
        .map(|(path, _)| coverage_data.covered_in_path(path))
        .sum();

    let total_coverable: usize = coverage_data
        .iter()
        .map(|(path, _)| coverage_data.coverable_in_path(path))
        .sum();

    // If we have no coverable lines, we can't calculate a percentage
    let coverage_percentage = if total_coverable > 0 {
        (total_covered as f64 / total_coverable as f64) * 100.0
    } else {
        0.0
    };

    // Take the previous result if it exists and compare it to the current result
    let coverage_info = match previous_result {
        Some(ref previous_data) => {
            // First, get the total covered/total coverable
            let prev_total_covered: usize = previous_data
                .iter()
                .map(|(path, _)| previous_data.covered_in_path(path))
                .sum();
            let prev_total_coverable: usize = previous_data
                .iter()
                .map(|(path, _)| previous_data.coverable_in_path(path))
                .sum();

            // Calculate the coverage percentage for the previous result
            let prev_coverage_percentage = if prev_total_coverable > 0 {
                (prev_total_covered as f64 / prev_total_coverable as f64) * 100.0
            } else {
                0.0
            };

            let diff = coverage_percentage - prev_coverage_percentage;
            if diff.abs() < 0.01 {
                format!(
                    "{}/{} ({:.2}%)",
                    total_covered, total_coverable, coverage_percentage
                )
            } else {
                let sign = if diff > 0.0 { "+" } else { "-" };
                format!(
                    "{}/{} ({:.2}% {}{:.2}%)",
                    total_covered,
                    total_coverable,
                    coverage_percentage,
                    sign,
                    diff.abs()
                )
            }
        }
        None => format!(
            "{}/{} ({:.2}%)",
            total_covered, total_coverable, coverage_percentage
        ),
    };

    // Add the base coverage info to the markdown
    let mut markdown_content = format!(
        "# Coverage Report\n\nOverall coverage: {}\n\n",
        coverage_info
    );

    // Next, add file-by-file coverage table
    markdown_content.push_str("## File Coverage\n\n");
    // Wrap the table in a details element for collapsibility
    markdown_content.push_str("<details>\n<summary>Show file coverage details</summary>\n\n");
    markdown_content.push_str("| File | Coverage | Percentage | Change |\n");
    markdown_content.push_str("|------|----------|------------|--------|\n");

    // Loop through each file and add its coverage info to the markdown
    for (path, _) in coverage_data.iter() {
        let covered = coverage_data.covered_in_path(path);
        let coverable = coverage_data.coverable_in_path(path);
        let file_percentage = if coverable > 0 {
            (covered as f64 / coverable as f64) * 100.0
        } else {
            0.0
        };

        let change_info = match previous_result {
            Some(ref previous_data) => {
                let prev_covered = previous_data.covered_in_path(path);
                let prev_coverable = previous_data.coverable_in_path(path);
                let prev_file_percentage = if prev_coverable > 0 {
                    (prev_covered as f64 / prev_coverable as f64) * 100.0
                } else {
                    0.0
                };

                let diff = file_percentage - prev_file_percentage;
                if diff.abs() < 0.01 {
                    "-".to_string()
                } else {
                    let sign = if diff > 0.0 { "+" } else { "-" };
                    format!("{}{:.2}%", sign, diff.abs())
                }
            }
            None => "-".to_string(),
        };

        // Calculate the relative path
        let path_str = if path.is_absolute() {
            path.strip_prefix(std::env::current_dir().unwrap_or_default())
                .unwrap_or(path) // If we can't find the relative path just use the absolute path
                .to_string_lossy()
        } else {
            path.to_string_lossy()
        };

        // Push the file result to the markdown table
        markdown_content.push_str(&format!(
            "| {} | {}/{} | {:.2}% | {} |\n",
            path_str, covered, coverable, file_percentage, change_info
        ));
    }

    // Close the details element
    markdown_content.push_str("\n</details>\n\n");

    // Finally, write the markdown content to the file
    file.write_all(markdown_content.as_bytes())
        .map_err(|e| RunError::Html(format!("Failed to write to file: {e}")))?;
    info!("Markdown content written to file: {}", file_path.display());

    Ok(())
}
