use std::io;
use std::io::{BufRead, BufReader};
use std::path::{PathBuf, Path};
use std::fs::File;
use std::collections::HashSet;
use object::Object;
use object::File as OFile;
use memmap::{Mmap, Protection};
use gimli::*;
use regex::Regex;

/// So far from my tests the test executable is the first compilation unit in
/// the DWARF debug information with the future ones being libraries and rustc.
const TEST_CU_OFFSET: usize = 0;

#[derive(Debug, Clone)]
pub struct Branch {

}

#[derive(Debug, Clone)]
pub struct TracerData {
    pub path: PathBuf,
    pub line: u64,
    pub address: u64,
    pub branch_data: Option<Branch>,
    pub hits: u64,
}

fn line_is_traceable(file: &PathBuf, line: u64) -> bool {
    let mut result = false;
    if line > 0 {
        // Module imports are flagged as debuggable. But are always ran so meaningless!
        let reg: Regex = Regex::new(r"(:?^|\s)mod\s+\w+;").unwrap();
        if let Ok(f) = File::open(file) {
            let reader = BufReader::new(&f);
            if let Some(Ok(l)) = reader.lines().nth((line - 1) as usize) {
                result = !reg.is_match(l.as_ref());
            }
        }
    }
    result
}

fn get_address_size<Endian: Endianity>(obj: &OFile) -> Result<u8> {
    let debug_info = obj.get_section(".debug_info").unwrap_or(&[]);
    let debug_info = DebugInfo::<Endian>::new(debug_info);

    let cu = debug_info.header_from_offset(DebugInfoOffset(TEST_CU_OFFSET))?;
    Ok(cu.address_size())
}


fn get_line_addresses<Endian: Endianity>(project: &Path, obj: &OFile) -> Result<Vec<TracerData>>  {
    let mut result: Vec<TracerData> = Vec::new();
    
    let addr_size = get_address_size::<Endian>(obj)?;

    let debug_line = obj.get_section(".debug_line").unwrap_or(&[]);
    let debug_line = DebugLine::<Endian>::new(debug_line);
    
    let prog = debug_line.program(DebugLineOffset(TEST_CU_OFFSET), addr_size, None, None)?; 
    let (cprog, seq) = prog.sequences()?;
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
                        // If we can't map to line, we can't trace it.
                        let line = ln_row.line().unwrap_or(0);
                        let file = file.path_name();
                        // We now need to filter out lines which are meaningless to trace.
                        
                        if let Ok(file) = String::from_utf8(file.to_bytes().to_vec()) {
                            path.push(file);
                            if !line_is_traceable(&path, line) {
                                continue;
                            }
                            let data = TracerData {
                                path: path,
                                line: line,
                                address: ln_row.address(),
                                branch_data: None,
                                hits: 0u64
                            };
                            result.push(data);
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
    Ok(result)
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
        data.map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Error while parsing"))
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    }
}
