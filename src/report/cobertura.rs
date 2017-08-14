use std::fs::File;
use std::io::prelude::*;
use std::io::Cursor;
use quick_xml::writer::Writer;
use quick_xml::events::{Event, BytesEnd, BytesStart};
use quick_xml::errors::Result;
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
fn write_class<T:Write>(writer: &mut Writer<T>, coverage: &[TracerData]) ->Result<usize> {
    if !coverage.is_empty() {
        let covered = coverage.iter().filter(|&x| (x.hits > 0)).count();
        let covered = (covered as f32)/(coverage.len() as f32);
        let name = coverage[0].path.file_name().unwrap_or_default().to_str().unwrap_or_default();
        let mut class = BytesStart::owned(b"class".to_vec(), b"class".len());
        class.push_attribute(("filename", name));
        class.push_attribute(("line-rate", covered.to_string().as_ref()));

        writer.write_event(Event::Start(class));
        writer.write_event(Event::Start(BytesStart::borrowed(b"lines", b"lines".len())))?;
        for trace in coverage {
            let mut line = BytesStart::owned(b"line".to_vec(), b"line".len());
            line.push_attribute(("line", "n"));
            line.push_attribute(("hits", "h"));
        }
        writer.write_event(Event::End(BytesEnd::borrowed(b"lines")))?;
        writer.write_event(Event::End(BytesEnd::borrowed(b"class")))
    } else {
        Ok(0)
    }
}

/// Input only tracer data from a single source folder
fn write_package<T:Write>(writer: &mut Writer<T>, 
                          package: &str,
                          coverage: &[TracerData], 
                          config: &Config) -> Result<usize> {
    let covered = coverage.iter().filter(|&x| (x.hits > 0)).count();
    let covered = (covered as f32)/(coverage.len() as f32);

    let mut pack = BytesStart::owned(b"package".to_vec(), b"package".len());
    pack.push_attribute(("name", package));
    pack.push_attribute(("line-rate", covered.to_string().as_ref()));
    writer.write_event(Event::Start(pack))?;
    writer.write_event(Event::Start(BytesStart::borrowed(b"classes", b"classes".len())))?;
    
    writer.write_event(Event::End(BytesEnd::borrowed(b"classes")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"package")))
}

pub fn export(coverage_data: &[TracerData], config: &Config) {
    let mut file = File::create("Cobertura.xml").unwrap();
    let mut writer = Writer::new(Cursor::new(Vec::new()));    
    // Construct cobertura xml 
    let covered = coverage_data.iter().filter(|&x| (x.hits > 0 )).count();
    let total = coverage_data.len();
    let line_rate = (covered as f32)/(total as f32);
    let mut cov = BytesStart::owned(b"coverage".to_vec(), b"coverage".len());
    cov.push_attribute(("line-rate", line_rate.to_string().as_ref()));
    writer.write_event(Event::Start(cov)).unwrap();
    let _ = write_header(&mut writer, &config);
    // other data
    writer.write_event(Event::Start(BytesStart::borrowed(b"packages", b"packages".len()))).unwrap();
    
    

    writer.write_event(Event::End(BytesEnd::borrowed(b"packages"))).unwrap();
    writer.write_event(Event::End(BytesEnd::borrowed(b"coverage"))).unwrap();
    let result = writer.into_inner().into_inner();
    file.write_all(&result).unwrap();
}
