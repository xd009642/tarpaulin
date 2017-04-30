use std::io;
use std::path::{PathBuf, Path};
use std::fs::File;
use std::collections::HashSet;
use object::Object;
use object::File as OFile;
use memmap::{Mmap, Protection};
use fallible_iterator::FallibleIterator;
use rustc_demangle::demangle;
use gimli::*;



#[derive(Debug, Clone)]
pub struct Branch {

}

#[derive(Debug, Clone)]
pub struct TracerData {
    pub path: PathBuf,
    pub line: u64,
    pub address: u64,
    pub branch_data: Option<Branch>,
}


fn parse_object_file<Endian: Endianity>(obj: &OFile) -> Vec<TracerData> {
    
    let result: Vec<TracerData> = Vec::new();

    let debug_info = obj.get_section(".debug_info").unwrap_or(&[]);
    let debug_info = DebugInfo::<Endian>::new(debug_info);
    let debug_abbrev = obj.get_section(".debug_abbrev").unwrap_or(&[]);
    let debug_abbrev = DebugAbbrev::<Endian>::new(debug_abbrev);
    // Used to map functions to location in source.
    let debug_string = obj.get_section(".debug_str").unwrap_or(&[]);
    let debug_string = DebugStr::<Endian>::new(debug_string);
    // This is the root compilation unit. 
    // This should be the one for the executable. Rest should be rust-buildbot 
    // and rust core for test executables. 
    // WARNING: This is an assumption based on analysis
    if let Some(root) = debug_info.units().nth(0).unwrap() {
        println!("Searching for functions!");
        // We now follow all namespaces down and log all DW_TAG_subprograms as 
        // these are function entry points
        let abbreviations = root.abbreviations(debug_abbrev).unwrap();
        let mut cursor = root.entries(&abbreviations);
        let _ = cursor.next_entry();
        let mut accumulator: isize = 0;
        while let Some((delta, node)) = cursor.next_dfs().expect("Parsing failed") {
            accumulator += delta;
            if accumulator < 0 {
                //skipped to next CU
                break;
            }
            
            if node.tag() == DW_TAG_subprogram {
                if let Ok(Some(at)) = node.attr(DW_AT_linkage_name) {
                    if let Some(st) = at.string_value(&debug_string) {
                        let st = st.to_str().unwrap_or("");
                        println!("Found function {}", demangle(st).to_string());
                    }
                }
            }
        }
    }
    result
}


fn get_line_addresses<Endian: Endianity>(project: &Path, obj: &OFile) -> Vec<TracerData>  {
    let mut result: Vec<TracerData> = Vec::new();

    let debug_line = obj.get_section(".debug_line").unwrap_or(&[]);
    let debug_line = DebugLine::<Endian>::new(debug_line);
    // TODO get address size and DebugLineOffset properly! 
    if let Ok(prog) = debug_line.program(DebugLineOffset(0), 8, None, None) {
        if let Ok((cprog, seq)) = prog.sequences() {
            for s in &seq {
                let mut sm = cprog.resume_from(s);
                while let Ok(Some((ref header, &ln_row))) = sm.next_row() {
                    if let Some(file) = ln_row.file(header) {
                        let mut path = PathBuf::new();
                        
                        if let Some(dir) = file.directory(header) {
                            if let Ok(temp) = String::from_utf8(dir.to_bytes().to_vec()) {
                                path.push(temp);
                            }
                        }
                        // Source is part of project so we cover it.
                        if path.starts_with(project) {
                            if let Some(file) = ln_row.file(header) {
                                let file = file.path_name();
                                if let Ok(file) = String::from_utf8(file.to_bytes().to_vec()) {
                                    path.push(file);
                                    let data = TracerData {
                                        path: path,
                                        line: ln_row.line().unwrap_or(0),
                                        address: ln_row.address(),
                                        branch_data: None,
                                    };
                                    result.push(data);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    // Due to rust being a higher level language multiple instructions may map
    // to the same line. This prunes these to just the first instruction address
    let mut check: HashSet<(&Path, u64)> = HashSet::new();
    let result = result.iter()
                       .filter(|x| check.insert((x.path.as_path(), x.line)))
                       .map(|x| x.clone())
                       .collect::<Vec<TracerData>>();
    result
}


pub fn generate_tracer_data(manifest: &Path, test: &Path) -> io::Result<Vec<TracerData>> {
    let file = File::open(test)?;
    let file = Mmap::open(&file, Protection::Read)?;
    if let Ok(obj) = OFile::parse(unsafe {file.as_slice() }) {
        
        let data = if obj.is_little_endian() {
            get_line_addresses::<LittleEndian>(manifest, &obj)
        } else {
            get_line_addresses::<BigEndian>(manifest, &obj)
        };
        Ok(data)
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    }
}
