use std::io;
use std::path::{PathBuf, Path};
use std::fs::File;
use std::collections::HashSet;
use object::Object;
use object::File as OFile;
use memmap::{Mmap, Protection};
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
    pub hits: u64,
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
                                        hits: 0u64
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
