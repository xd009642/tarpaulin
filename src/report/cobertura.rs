use std::time::{SystemTime, UNIX_EPOCH};
use std::fs::File;
use std::ffi::OsStr;
use std::path::Path;
use std::io::prelude::*;
use std::io::Cursor;
use std::collections::HashSet;
use quick_xml::Writer;
use quick_xml::events::{Event, BytesEnd, BytesStart, BytesDecl};
use quick_xml::Result;
use tracer::TracerData;
use config::Config;



fn write_header<T:Write>(writer: &mut Writer<T>, config: &Config) -> Result<usize> {
    
    writer.write_event(Event::Start(BytesStart::borrowed(b"sources", b"sources".len())))?;
    writer.write_event(Event::Start(BytesStart::borrowed(b"source", b"source".len())))?;
    
    let parent_folder = match config.manifest.parent() {
        Some(s) => s.to_str().unwrap_or_default(),
        None => "",
    };
    writer.write(parent_folder.as_bytes()).unwrap();
    writer.write_event(Event::End(BytesEnd::borrowed(b"source")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"sources")))
}

/// Input only from single source file
fn write_class<T:Write>(writer: &mut Writer<T>, manifest_path: &Path, coverage: &[&TracerData]) ->Result<usize> {
    if !coverage.is_empty() {
        let covered = coverage.iter().filter(|&x| (x.hits > 0)).count();
        let covered = (covered as f32)/(coverage.len() as f32);
        //let filename = coverage[0].path.file_name().unwrap_or_default().to_str().unwrap_or_default();
        let filename = match coverage[0].path.strip_prefix(manifest_path) {
            Ok(p) => p,
            _ => coverage[0].path.as_path()
        };
        let name = coverage[0].path.file_stem().unwrap_or_default().to_str().unwrap_or_default();
        
        let mut class = BytesStart::owned(b"class".to_vec(), b"class".len());
        class.push_attribute(("name", name));
        class.push_attribute(("filename", filename.to_str().unwrap_or_default()));
        class.push_attribute(("line-rate", covered.to_string().as_ref()));
        class.push_attribute(("branch-rate", "1.0"));
        class.push_attribute(("complexity", "0.0"));
        writer.write_event(Event::Start(class))?;
        writer.write_event(Event::Empty(BytesStart::owned(b"methods".to_vec(), b"methods".len())))?;
        writer.write_event(Event::Start(BytesStart::borrowed(b"lines", b"lines".len())))?;
        for trace in coverage {
            let mut line = BytesStart::owned(b"line".to_vec(), b"line".len());
            line.push_attribute(("number", trace.line.to_string().as_ref()));
            line.push_attribute(("hits", trace.hits.to_string().as_ref()));
            writer.write_event(Event::Empty(line))?;
        }
        writer.write_event(Event::End(BytesEnd::borrowed(b"lines")))?;
        writer.write_event(Event::End(BytesEnd::borrowed(b"class")))
    } else {
        Ok(0)
    }
}

/// Input only tracer data from a single source folder
fn write_package<T:Write>(mut writer: &mut Writer<T>, 
                          package: &Path,
                          package_name: &str,
                          coverage: &[&TracerData]) -> Result<usize> {
    let covered = coverage.iter().filter(|&x| (x.hits > 0)).count();
    let covered = (covered as f32)/(coverage.len() as f32);

    let mut pack = BytesStart::owned(b"package".to_vec(), b"package".len());
    pack.push_attribute(("name", package_name));
    pack.push_attribute(("line-rate", covered.to_string().as_ref()));
    pack.push_attribute(("branch-rate", "1.0"));
    pack.push_attribute(("complexity", "0.0"));
    writer.write_event(Event::Start(pack))?;
    writer.write_event(Event::Start(BytesStart::borrowed(b"classes", b"classes".len())))?;
    let mut file_set: HashSet<&OsStr> = HashSet::new();

    for t in coverage.iter() {
        let filename = t.path.file_name();
        if !file_set.contains(filename.unwrap_or_default()) {
            file_set.insert(filename.unwrap_or_default());
            let class = coverage.iter()
                                .filter(|x| x.path.file_name() == filename)
                                .map(|x| *x)
                                .collect::<Vec<_>>();

            write_class(&mut writer, package, &class)?;
        }
    }

    writer.write_event(Event::End(BytesEnd::borrowed(b"classes")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"package")))
}

pub fn export(coverage_data: &[TracerData], config: &Config) {
    let mut file = File::create("cobertura.xml").unwrap();
    let mut writer = Writer::new(Cursor::new(Vec::new()));    
    writer.write_event(Event::Decl(BytesDecl::new(b"1.0", None, None))).unwrap();
    // Construct cobertura xml 
    let covered = coverage_data.iter().filter(|&x| (x.hits > 0 )).count();
    let total = coverage_data.len();
    let line_rate = (covered as f32)/(total as f32);
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
    writer.write_event(Event::Start(BytesStart::borrowed(b"packages", b"packages".len()))).unwrap();
    
    let mut folder_set: HashSet<&Path> = HashSet::new();
    for t in coverage_data.iter() {
        let parent = match t.path.parent() {
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
            let package = coverage_data.iter().filter(|x| x.path.parent() == Some(&parent)).collect::<Vec<_>>();
            let _ = write_package(&mut writer, &manifest_path, package_name, &package);
        }
    }

    writer.write_event(Event::End(BytesEnd::borrowed(b"packages"))).unwrap();
    writer.write_event(Event::End(BytesEnd::borrowed(b"coverage"))).unwrap();
    let result = writer.into_inner().into_inner();
    file.write_all(&result).unwrap();
}
