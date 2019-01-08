use crate::config::Config;
use crate::errors::*;
use crate::traces::{CoverageStat, TraceMap};
use log::info;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, Event};
use quick_xml::Writer;
use std::collections::HashSet;
use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

fn write_header<T: Write>(writer: &mut Writer<T>, config: &Config) -> Result<usize, RunError> {
    writer.write_event(Event::Start(BytesStart::borrowed(
        b"sources",
        b"sources".len(),
    )))?;
    writer.write_event(Event::Start(BytesStart::borrowed(
        b"source",
        b"source".len(),
    )))?;

    let parent_folder = match config.manifest.parent() {
        Some(s) => s.to_str().unwrap_or_default(),
        None => "",
    };
    writer.write(parent_folder.as_bytes()).unwrap();
    writer.write_event(Event::End(BytesEnd::borrowed(b"source")))?;
    Ok(writer.write_event(Event::End(BytesEnd::borrowed(b"sources")))?)
}

/// Input only from single source file
fn write_class<T: Write>(
    writer: &mut Writer<T>,
    manifest_path: &Path,
    filename: &Path,
    coverage: &TraceMap,
) -> Result<usize, RunError> {
    if !coverage.is_empty() {
        let covered = coverage.covered_in_path(filename);
        let covered = (covered as f32) / (coverage.coverable_in_path(filename) as f32);

        let tidy_filename = match filename.strip_prefix(manifest_path) {
            Ok(p) => p,
            _ => filename,
        };
        let name = filename
            .file_stem()
            .unwrap_or_default()
            .to_str()
            .unwrap_or_default();

        let mut class = BytesStart::owned(b"class".to_vec(), b"class".len());
        class.push_attribute(("name", name));
        class.push_attribute(("filename", tidy_filename.to_str().unwrap_or_default()));
        class.push_attribute(("line-rate", covered.to_string().as_ref()));
        class.push_attribute(("branch-rate", "1.0"));
        class.push_attribute(("complexity", "0.0"));
        writer.write_event(Event::Start(class))?;
        writer.write_event(Event::Empty(BytesStart::owned(
            b"methods".to_vec(),
            b"methods".len(),
        )))?;
        writer.write_event(Event::Start(BytesStart::borrowed(b"lines", b"lines".len())))?;
        for trace in coverage.get_child_traces(filename) {
            let mut line = BytesStart::owned(b"line".to_vec(), b"line".len());
            line.push_attribute(("number", trace.line.to_string().as_ref()));
            match trace.stats {
                CoverageStat::Line(hit) => {
                    line.push_attribute(("hits", hit.to_string().as_ref()));
                }
                _ => {
                    info!("Coverage statistic currently not implemented for cobertura");
                }
            }
            writer.write_event(Event::Empty(line))?;
        }
        writer.write_event(Event::End(BytesEnd::borrowed(b"lines")))?;
        Ok(writer.write_event(Event::End(BytesEnd::borrowed(b"class")))?)
    } else {
        Ok(0)
    }
}

/// Input only tracer data from a single source folder
fn write_package<T: Write>(
    mut writer: &mut Writer<T>,
    package: &Path,
    manifest_path: &Path,
    package_name: &str,
    coverage: &TraceMap,
) -> Result<usize, RunError> {
    let covered = coverage.covered_in_path(package);
    let covered = (covered as f32) / (coverage.coverable_in_path(package) as f32);

    let mut pack = BytesStart::owned(b"package".to_vec(), b"package".len());
    pack.push_attribute(("name", package_name));
    pack.push_attribute(("line-rate", covered.to_string().as_ref()));
    pack.push_attribute(("branch-rate", "1.0"));
    pack.push_attribute(("complexity", "0.0"));
    writer.write_event(Event::Start(pack))?;
    writer.write_event(Event::Start(BytesStart::borrowed(
        b"classes",
        b"classes".len(),
    )))?;

    for file in &coverage.files() {
        if file.parent() == Some(package) {
            write_class(&mut writer, manifest_path, file, coverage)?;
        }
    }

    writer.write_event(Event::End(BytesEnd::borrowed(b"classes")))?;
    Ok(writer.write_event(Event::End(BytesEnd::borrowed(b"package")))?)
}

pub fn export(coverage_data: &TraceMap, config: &Config) -> Result<(), RunError> {
    let mut file = File::create("cobertura.xml").unwrap();
    let mut writer = Writer::new(Cursor::new(Vec::new()));
    writer
        .write_event(Event::Decl(BytesDecl::new(b"1.0", None, None)))
        .unwrap();
    // Construct cobertura xml
    let line_rate = coverage_data.coverage_percentage();
    let mut cov = BytesStart::owned(b"coverage".to_vec(), b"coverage".len());
    cov.push_attribute(("line-rate", line_rate.to_string().as_ref()));
    cov.push_attribute(("branch-rate", "1.0"));
    cov.push_attribute(("version", "1.9"));

    if let Ok(s) = SystemTime::now().duration_since(UNIX_EPOCH) {
        cov.push_attribute(("timestamp", s.as_secs().to_string().as_ref()));
    } else {
        cov.push_attribute(("timestamp", "0"));
    }

    writer.write_event(Event::Start(cov)).unwrap();
    let _ = write_header(&mut writer, &config);
    // other data
    writer
        .write_event(Event::Start(BytesStart::borrowed(
            b"packages",
            b"packages".len(),
        )))
        .unwrap();

    let mut folder_set: HashSet<&Path> = HashSet::new();
    for t in &coverage_data.files() {
        let parent = match t.parent() {
            Some(x) => x,
            None => continue,
        };
        if !folder_set.contains(&parent) {
            folder_set.insert(parent);
            let manifest_path = config.manifest.parent().unwrap_or(&config.manifest);
            let package_name = match parent.strip_prefix(manifest_path) {
                Ok(p) => p,
                _ => manifest_path,
            };
            let package_name = package_name.to_str().unwrap_or_default();
            let _ = write_package(
                &mut writer,
                &parent,
                &manifest_path,
                package_name,
                &coverage_data,
            );
        }
    }

    writer
        .write_event(Event::End(BytesEnd::borrowed(b"packages")))
        .unwrap();
    writer
        .write_event(Event::End(BytesEnd::borrowed(b"coverage")))
        .unwrap();
    let result = writer.into_inner().into_inner();
    Ok(file.write_all(&result)?)
}
