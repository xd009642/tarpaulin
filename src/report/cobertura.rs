#![allow(dead_code)]
/// The XML structure for a cobatura report is roughly as follows:
/// ```xml
/// <coverage lines-valid="5" lines-covered="0" line-rate="0.0" branches-valid="0"
/// branches-covered="0" branch-rate="0.0" version="1.9" timestamp="...">
///   <sources>
///     <source>PATH</source>
///     ...
///   </sources>
///
///   <packages>
///     <package name=".."  line-rate="0.0" branch-rate="0.0" complexity="0.0">
///       <classes>
///         <class name="Main" filename="main.rs" line-rate="0.0" branch-rate="0.0" complexity="0.0">
///           <methods>
///             <method name="main" signature="()" line-rate="0.0" branch-rate="0.0">
///               <lines>
///                 <line number="1" hits="5" branch="false"/>
///                 <line number="3" hits="2" branch="true">
///                   <conditions>
///                     <condition number="0" type="jump" coverage="50%"/>
///                     ...
///                   </conditions>
///                 </line>
///               </lines>
///             </method>
///             ...
///           </methods>
///
///           <lines>
///             <line number="10" hits="4" branch="false"/>
///           </lines>
///         </class>
///         ...
///       </classes>
///     </package>
///     ...
///   </packages>
/// </coverage>
/// ```
use std::collections::HashSet;
use std::error;
use std::fmt;
use std::fs::File;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use quick_xml::{
    events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event},
    Writer,
};

use chrono::offset::Utc;

use crate::config::Config;
use crate::traces::{CoverageStat, Trace, TraceMap};

pub fn report(traces: &TraceMap, config: &Config) -> Result<(), Error> {
    let result = Report::render(config, traces)?;
    result.export(config)
}

#[derive(Debug)]
pub enum Error {
    Unknown,
    ExportError(std::io::Error),
}

impl error::Error for Error {}

impl fmt::Display for Error {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::ExportError(ref e) => write!(f, "Export Error {e}"),
            Error::Unknown => write!(f, "Unknown Error"),
        }
    }
}

#[derive(Debug)]
pub struct Report {
    timestamp: i64,
    lines_covered: usize,
    lines_valid: usize,
    line_rate: f64,
    branches_covered: usize,
    branches_valid: usize,
    branch_rate: f64,
    sources: Vec<PathBuf>,
    packages: Vec<Package>,
}

impl Report {
    pub fn render(config: &Config, traces: &TraceMap) -> Result<Self, Error> {
        let timestamp = Utc::now().timestamp();
        let sources = render_sources(config);
        let packages = render_packages(config, traces);
        let mut line_rate = 0.0;
        let mut branch_rate = 0.0;

        if !packages.is_empty() {
            line_rate = traces.coverage_percentage();
            branch_rate = 0.0;
        }

        Ok(Report {
            timestamp,
            lines_covered: traces.total_covered(),
            lines_valid: traces.total_coverable(),
            line_rate,
            branches_covered: 0,
            branches_valid: 0,
            branch_rate,
            sources,
            packages,
        })
    }

    pub fn export(&self, config: &Config) -> Result<(), Error> {
        let file_path = config.output_dir().join("cobertura.xml");
        let mut file = File::create(file_path).map_err(|e| Error::ExportError(e))?;

        let mut writer = Writer::new(Cursor::new(vec![]));
        writer
            .write_event(Event::Decl(BytesDecl::new("1.0", None, None)))
            .map_err(Error::ExportError)?;

        let cov_tag = "coverage";
        let mut cov = BytesStart::new(cov_tag);
        cov.push_attribute(("lines-covered", self.lines_covered.to_string().as_ref()));
        cov.push_attribute(("lines-valid", self.lines_valid.to_string().as_ref()));
        cov.push_attribute(("line-rate", self.line_rate.to_string().as_ref()));
        cov.push_attribute((
            "branches-covered",
            self.branches_covered.to_string().as_ref(),
        ));
        cov.push_attribute(("branches-valid", self.branches_valid.to_string().as_ref()));
        cov.push_attribute(("branch-rate", self.branch_rate.to_string().as_ref()));
        cov.push_attribute(("complexity", "0"));
        cov.push_attribute(("version", "1.9"));

        let secs = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(s) => s.as_secs().to_string(),
            Err(_) => String::from("0"),
        };
        cov.push_attribute(("timestamp", secs.as_ref()));

        writer
            .write_event(Event::Start(cov))
            .map_err(Error::ExportError)?;

        self.export_header(&mut writer)
            .map_err(Error::ExportError)?;

        self.export_packages(&mut writer)
            .map_err(Error::ExportError)?;

        writer
            .write_event(Event::End(BytesEnd::new(cov_tag)))
            .map_err(Error::ExportError)?;

        let result = writer.into_inner().into_inner();
        file.write_all(&result).map_err(|e| Error::ExportError(e))
    }

    fn export_header<T: Write>(&self, writer: &mut Writer<T>) -> Result<(), std::io::Error> {
        let sources_tag = "sources";
        let source_tag = "source";
        writer.write_event(Event::Start(BytesStart::new(sources_tag)))?;
        for source in &self.sources {
            if let Some(path) = source.to_str() {
                writer.write_event(Event::Start(BytesStart::new(source_tag)))?;
                writer.write_event(Event::Text(BytesText::new(path)))?;
                writer.write_event(Event::End(BytesEnd::new(source_tag)))?;
            }
        }
        writer
            .write_event(Event::End(BytesEnd::new(sources_tag)))
            .map(|_| ())
    }

    fn export_packages<T: Write>(&self, writer: &mut Writer<T>) -> Result<(), std::io::Error> {
        let packages_tag = "packages";
        let pack_tag = "package";

        writer.write_event(Event::Start(BytesStart::new(packages_tag)))?;
        // Export the package
        for package in &self.packages {
            let mut pack = BytesStart::new(pack_tag);
            pack.push_attribute(("name", package.name.as_str()));
            pack.push_attribute(("line-rate", package.line_rate.to_string().as_ref()));
            pack.push_attribute(("branch-rate", package.branch_rate.to_string().as_ref()));
            pack.push_attribute(("complexity", package.complexity.to_string().as_ref()));

            writer.write_event(Event::Start(pack))?;
            self.export_classes(&package.classes, writer)?;
            writer.write_event(Event::End(BytesEnd::new(pack_tag)))?;
        }

        writer
            .write_event(Event::End(BytesEnd::new(packages_tag)))
            .map(|_| ())
    }

    fn export_classes<T: Write>(
        &self,
        classes: &[Class],
        writer: &mut Writer<T>,
    ) -> Result<(), std::io::Error> {
        let classes_tag = "classes";
        let class_tag = "class";
        let methods_tag = "methods";

        writer.write_event(Event::Start(BytesStart::new(classes_tag)))?;
        for class in classes {
            let mut c = BytesStart::new(class_tag);
            c.push_attribute(("name", class.name.as_ref()));
            c.push_attribute(("filename", class.file_name.as_ref()));
            c.push_attribute(("line-rate", class.line_rate.to_string().as_ref()));
            c.push_attribute(("branch-rate", class.branch_rate.to_string().as_ref()));
            c.push_attribute(("complexity", class.complexity.to_string().as_ref()));

            writer.write_event(Event::Start(c))?;
            writer.write_event(Event::Empty(BytesStart::new(methods_tag)))?;
            self.export_lines(&class.lines, writer)?;
            writer.write_event(Event::End(BytesEnd::new(class_tag)))?;
        }
        writer
            .write_event(Event::End(BytesEnd::new(classes_tag)))
            .map(|_| ())
    }

    fn export_lines<T: Write>(
        &self,
        lines: &[Line],
        writer: &mut Writer<T>,
    ) -> Result<(), std::io::Error> {
        let lines_tag = "lines";
        let line_tag = "line";

        writer.write_event(Event::Start(BytesStart::new(lines_tag)))?;
        for line in lines {
            let mut l = BytesStart::new(line_tag);
            match line {
                Line::Plain {
                    ref number,
                    ref hits,
                } => {
                    l.push_attribute(("number", number.to_string().as_ref()));
                    l.push_attribute(("hits", hits.to_string().as_ref()));
                }
                Line::Branch { .. } => {}
            }
            writer.write_event(Event::Empty(l))?;
        }
        writer
            .write_event(Event::End(BytesEnd::new(lines_tag)))
            .map(|_| ())
    }
}

fn render_sources(config: &Config) -> Vec<PathBuf> {
    vec![config.get_base_dir()]
}

#[derive(Debug)]
struct Package {
    name: String,
    line_rate: f64,
    branch_rate: f64,
    complexity: f64,
    classes: Vec<Class>,
}

fn render_packages(config: &Config, traces: &TraceMap) -> Vec<Package> {
    let dirs: HashSet<&Path> = traces
        .files()
        .into_iter()
        .filter_map(|x| x.parent())
        .collect();

    dirs.into_iter()
        .map(|x| render_package(config, traces, x))
        .collect()
}

fn render_package(config: &Config, traces: &TraceMap, pkg: &Path) -> Package {
    let name = config.strip_base_dir(pkg).to_str().unwrap().to_string();

    let line_cover = traces.covered_in_path(pkg) as f64;
    let coverable = traces.coverable_in_path(pkg);
    let line_rate = if coverable > 0 {
        line_cover / (coverable as f64)
    } else {
        0.0
    };

    Package {
        name,
        line_rate,
        branch_rate: 0.0,
        complexity: 0.0,
        classes: render_classes(config, traces, pkg),
    }
}

#[derive(Debug)]
struct Class {
    name: String,
    file_name: String,
    line_rate: f64,
    branch_rate: f64,
    complexity: f64,
    lines: Vec<Line>,
    methods: Vec<Method>,
}

fn render_classes(config: &Config, traces: &TraceMap, pkg: &Path) -> Vec<Class> {
    traces
        .files()
        .iter()
        .filter(|x| x.parent() == Some(pkg))
        .filter_map(|x| render_class(config, traces, x))
        .collect()
}

// TODO: Cobertura distinguishes between lines outside methods, and methods
// (which also contain lines). As there is currently no way to get traces from
// a particular function only, all traces are put into lines, and the vector
// of methods is empty.
//
// Until this is fixed, the render_method function will panic if called, as it
// cannot be properly implemented.
//
fn render_class(config: &Config, traces: &TraceMap, file: &Path) -> Option<Class> {
    let name = file
        .file_stem()
        .map(|x| x.to_str().unwrap())
        .unwrap_or_default()
        .to_string();

    let file_name = config.strip_base_dir(file).to_str().unwrap().to_string();
    let coverable = traces.coverable_in_path(file);
    if coverable == 0 {
        None
    } else {
        let covered = traces.covered_in_path(file) as f64;
        let line_rate = covered / coverable as f64;
        let lines = traces.get_child_traces(file).map(render_line).collect();

        Some(Class {
            name,
            file_name,
            line_rate,
            branch_rate: 0.0,
            complexity: 0.0,
            lines,
            methods: vec![],
        })
    }
}

#[derive(Debug)]
struct Method {
    name: String,
    signature: String,
    line_rate: f64,
    branch_rate: f64,
    lines: Vec<Line>,
}

fn render_methods() -> Vec<Method> {
    unimplemented!()
}

fn render_method() -> Method {
    unimplemented!()
}

#[derive(Debug)]
enum Line {
    Plain {
        number: usize,
        hits: usize,
    },

    Branch {
        number: usize,
        hits: usize,
        conditions: Vec<Condition>,
    },
}

fn render_line(trace: &Trace) -> Line {
    match &trace.stats {
        CoverageStat::Line(hits) => Line::Plain {
            number: trace.line as usize,
            hits: *hits as usize,
        },

        // TODO: Branches in cobertura are given a fresh number as a label,
        // which would require having some form of context when rendering.
        //
        _ => panic!("Not currently supported"),
    }
}

#[derive(Debug)]
struct Condition {
    number: usize,
    cond_type: ConditionType,
    coverage: f64,
}

// Condition types
#[derive(Debug)]
enum ConditionType {
    Jump,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::traces::*;
    use std::collections::HashSet;
    use std::path::PathBuf;

    #[test]
    fn package_coverage() {
        let mut config = Config::default();
        config.set_manifest(PathBuf::from("fake/Cargo.toml"));
        let mut map = TraceMap::new();

        map.add_file(&PathBuf::from("fake/examples/foo.rs"));

        let empty_trace = Trace::new_stub(2);
        let mut address = HashSet::new();
        address.insert(2);
        let hit_trace = Trace::new(3, address, 1);

        let source_file = PathBuf::from("fake/src/lib.rs");

        map.add_trace(&source_file, empty_trace);
        map.add_trace(&source_file, hit_trace);

        let report = Report::render(&config, &map).unwrap();
        assert_eq!(report.lines_covered, 0);
        assert_eq!(report.lines_valid, 2);
        assert_eq!(report.line_rate, 0.0);
        assert_eq!(report.packages.len(), 2);
        assert_eq!(report.sources.len(), 1);

        map.increment_hit(2);

        let report = Report::render(&config, &map).unwrap();
        assert_eq!(report.lines_covered, 1);
        assert_eq!(report.lines_valid, 2);
        assert_eq!(report.line_rate, 0.5);
        assert_eq!(report.packages.len(), 2);
        assert_eq!(report.sources.len(), 1);
    }
}
