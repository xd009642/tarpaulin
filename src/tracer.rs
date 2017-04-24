use std::io;
use std::path::Path;
use std::fs::File;
use object::Object;
use object::File as OFile;
use memmap::{Mmap, Protection};
use fallible_iterator::FallibleIterator;
use rustc_demangle::demangle;
use gimli::*;


fn parse_object_file<Endian>(obj: &OFile) where Endian: Endianity {

    let debug_info = obj.get_section(".debug_info").unwrap_or(&[]);
    let debug_info = DebugInfo::<Endian>::new(debug_info);
    let debug_abbrev = obj.get_section(".debug_abbrev").unwrap_or(&[]);
    let debug_abbrev = DebugAbbrev::<Endian>::new(debug_abbrev);
    // Used to map functions to location in source.
    let debug_line = obj.get_section(".debug_line").unwrap_or(&[]);
    let debug_line = DebugLine::<Endian>::new(debug_line);
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
                        println!("{}", demangle(st).to_string());
                    }
                }
            }
            
        }
    } 
}




pub fn generate_hook_addresses(test: &Path) -> io::Result<()> {
    println!("Finding hook addresses");
    let file = File::open(test)?;
    let file = Mmap::open(&file, Protection::Read)?;
    if let Ok(obj) = OFile::parse(unsafe {file.as_slice() }) {
        
        if obj.is_little_endian() {
            parse_object_file::<LittleEndian>(&obj);
        } else {
            parse_object_file::<BigEndian>(&obj);
        }
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    }
}
