use std::io;
use std::path::{PathBuf, Path};
use std::fs::File;
use std::cmp::{Ordering, PartialEq, Ord};
use std::collections::HashMap;
use object::{Object, File as OFile};
use memmap::MmapOptions;
use gimli::*;
use rustc_demangle::demangle;
use cargo::core::Workspace;

use config::Config;
use source_analysis::*;
use traces::*;


/// Describes a function as `low_pc`, `high_pc` and bool representing `is_test`.
type FuncDesc = (u64, u64, FunctionType);

#[derive(Clone, Copy, PartialEq)]
enum FunctionType {
    Generated,
    Test,
    Standard
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum LineType {
    /// Generated test main. Shouldn't be traced.
    TestMain,
    /// Entry of function known to be a test
    TestEntry(u64),
    /// Entry of function. May or may not be test
    FunctionEntry(u64),
    /// Standard statement
    Statement,
    /// Condition
    Condition,
    /// Unknown type
    Unknown,
    /// Unused meta-code
    UnusedGeneric,
}


#[derive(Debug, Clone, Eq, PartialOrd)]
pub struct TracerData {
    pub path: PathBuf,
    pub line: u64,
    pub address: Option<u64>,
    pub trace_type: LineType,
    pub hits: u64,
}

impl PartialEq for TracerData {
    fn eq(&self, other: &TracerData) -> bool {
        (self.path == other.path) && (self.line == other.line)
    }
}

impl Ord for TracerData {
    
    fn cmp(&self, other: &TracerData) -> Ordering {
        if self == other {
            Ordering::Equal
        } else if self.path == other.path {
            if self.line > other.line {
                Ordering::Greater
            } else {
                Ordering::Less
            }
        } else if self.path > other.path {
            Ordering::Greater
        } else {
            Ordering::Less
        }
    }
}


fn generate_func_desc<R, Offset>(die: &DebuggingInformationEntry<R, Offset>,
                                 debug_str: &DebugStr<R>) -> Result<FuncDesc>
    where R: Reader<Offset = Offset>,
          Offset: ReaderOffset
{
    let mut func_type = FunctionType::Standard;
    let low = die.attr_value(DW_AT_low_pc)?;
    let high = die.attr_value(DW_AT_high_pc)?;
    let linkage = die.attr_value(DW_AT_linkage_name)?;

    // Low is a program counter address so stored in an Addr
    let low = match low {
        Some(AttributeValue::Addr(x)) => x,
        _ => 0u64,
    };
    // High is an offset from the base pc, therefore is u64 data.
    let high = match high {
        Some(AttributeValue::Udata(x)) => x,
        _ => 0u64,
    };
    if let Some(AttributeValue::DebugStrRef(offset)) = linkage {
        let name = debug_str.get_str(offset)
            .and_then(|r| r.to_string().map(|s| s.to_string()))
            .unwrap_or("".into());
        let name = demangle(name.as_ref()).to_string();
        // Simplest test is whether it's in tests namespace.
        // Rust guidelines recommend all tests are in a tests module.
        func_type = if name.contains("tests::") {
            FunctionType::Test
        } else if name.contains("__test::main") {
            FunctionType::Generated
        } else {
            FunctionType::Standard
        };
    } 
    Ok((low, high, func_type))
}


/// Finds all function entry points and returns a vector
/// This will identify definite tests, but may be prone to false negatives.
fn get_entry_points<R, Offset>(debug_info: &CompilationUnitHeader<R, Offset>,
                               debug_abbrev: &Abbreviations,
                               debug_str: &DebugStr<R>) -> Vec<FuncDesc>
    where R: Reader<Offset = Offset>,
          Offset: ReaderOffset
{
    let mut result:Vec<FuncDesc> = Vec::new();
    let mut cursor = debug_info.entries(debug_abbrev);
    // skip compilation unit root.
    let _ = cursor.next_entry();
    while let Ok(Some((_, node))) = cursor.next_dfs() {
        // Function DIE
        if node.tag() == DW_TAG_subprogram {
            
            if let Ok(fd) = generate_func_desc(node, debug_str) {
                result.push(fd);
            }
        }
    }
    result
}

fn get_addresses_from_program<R, Offset>(prog: IncompleteLineNumberProgram<R>,
                                         entries: &[(u64, LineType)],
                                         project: &Path) -> Result<Vec<TracerData>>
    where R: Reader<Offset = Offset>,
          Offset: ReaderOffset
{
    let mut result: Vec<TracerData> = Vec::new();
    let ( cprog, seq) = prog.sequences()?;
    for s in seq {
        let mut temp_store : Vec<TracerData> = Vec::new();
        let mut sm = cprog.resume_from(&s);   
         while let Ok(Some((header, &ln_row))) = sm.next_row() {
            if let Some(file) = ln_row.file(header) {
                let mut path = PathBuf::new();
                
                if let Some(dir) = file.directory(header) {
                    if let Ok(temp) = dir.to_string() {
                        path.push(temp.as_ref());
                    }
                }
                if let Ok(p) = path.canonicalize() {
                    path = p;
                }
                let is_target = path.starts_with(project.join("target"));
                // Fix relative paths and determine if in target directory
                // Source in target directory shouldn't be covered as it's either
                // autogenerated or resulting from the projects Cargo.lock
                let is_target = if path.is_relative() {
                    path.starts_with("target")
                } else {
                    path.starts_with(project.join("target"))
                };
                // Source is part of project so we cover it.
                if !is_target && path.starts_with(project) {
                    let force_test = path.starts_with(project.join("tests"));
                    if let Some(file) = ln_row.file(header) {
                        // If we can't map to line, we can't trace it.
                        let line = match ln_row.line() {
                            Some(l) => l,
                            None => continue,
                        };
                  
                        let file = file.path_name();
                        
                        if let Ok(file) = file.to_string() {
                            path.push(file.as_ref());
                            if !path.is_file() {
                                // Not really a source file!
                                continue;
                            }
                            let address = ln_row.address();
                            
                            let desc = entries.iter()
                                              .filter(|&&(addr, _)| addr == address )
                                              .map(|&(_, t)| t)
                                              .nth(0)
                                              .unwrap_or(LineType::Unknown);
                            temp_store.push( TracerData {
                                path: path,
                                line: line,
                                address: Some(address),
                                trace_type: desc,
                                hits: 0u64
                            });
                        }
                    }
                }
            }
        }
        if !temp_store.is_empty() {
            temp_store.sort();
            temp_store.dedup();
            result.append(&mut temp_store);
        }
    }
    Ok(result)
}


fn get_line_addresses(endian: RunTimeEndian,
                      project: &Path,
                      obj: &OFile,
                      analysis: &HashMap<PathBuf, LineAnalysis>,
                      config: &Config) -> Result<Vec<TracerData>>  {
    let mut result: Vec<TracerData> = Vec::new();
    let debug_info = obj.section_data_by_name(".debug_info").unwrap_or(&[]);
    let debug_info = DebugInfo::new(debug_info, endian);
    let debug_abbrev = obj.section_data_by_name(".debug_abbrev").unwrap_or(&[]);
    let debug_abbrev = DebugAbbrev::new(debug_abbrev, endian);
    let debug_strings = obj.section_data_by_name(".debug_str").unwrap_or(&[]);
    let debug_strings = DebugStr::new(debug_strings, endian);

    let mut iter = debug_info.units();
    while let Ok(Some(cu)) = iter.next() {
        let addr_size = cu.address_size();
        let abbr = match cu.abbreviations(&debug_abbrev) {
            Ok(a) => a,
            _ => continue,
        };
        let entries = get_entry_points(&cu, &abbr, &debug_strings)
            .iter()
            .map(|&(a, b, c)| { 
                match c {
                    FunctionType::Test => (a, LineType::TestEntry(b)),
                    FunctionType::Standard => (a, LineType::FunctionEntry(b)),
                    FunctionType::Generated => (a, LineType::TestMain),
                }
            }).collect::<Vec<_>>();
        if let Ok(Some((_, root))) = cu.entries(&abbr).next_dfs() {
            let offset = match root.attr_value(DW_AT_stmt_list) {
                Ok(Some(AttributeValue::DebugLineRef(o))) => o,
                _ => continue,
            };
            let debug_line = obj.section_data_by_name(".debug_line").unwrap_or(&[]);
            let debug_line = DebugLine::new(debug_line, endian);
            
            let prog = debug_line.program(offset, addr_size, None, None)?; 
            if let Ok(mut addresses) = get_addresses_from_program(prog, &entries, project) {
                result.append(&mut addresses);
            }
        }
    }
    // Due to rust being a higher level language multiple instructions may map
    // to the same line. This prunes these to just the first instruction address
    let mut result = result.iter()
                           .filter(|x| !(config.ignore_tests && x.path.starts_with(project.join("tests"))))
                           .filter(|x| !(config.exclude_path(&x.path)))
                           .filter(|x| !analysis.should_ignore(x.path.as_ref(), &(x.line as usize)))
                           .filter(|x| x.trace_type != LineType::TestMain)
                           .cloned()
                           .collect::<Vec<TracerData>>();

    let addresses = result.iter()
                          .map(|x| x.address.unwrap())
                          .collect::<Vec<_>>();
    // TODO Probably a more idiomatic way to do this.
    {
        let test_entries = result.iter_mut()
                                 .filter(|x| match x.trace_type {
                                     LineType::TestEntry(_) => true,
                                     _ => false,
                                 });

        for test_entry in test_entries {
            let endpoint = match test_entry.trace_type {
                LineType::TestEntry(x) => x,
                _ => continue,
            };
            let addr = test_entry.address.unwrap();
            let max_address = addresses.iter()
                                       .fold(0, |acc, x| {
                                           if x > &(addr + acc) && (x-addr) <= endpoint {
                                               *x - addr
                                           } else { 
                                               acc  
                                           }
                                       });
            test_entry.trace_type = LineType::TestEntry(max_address);
        }
    }
    for (file, ref line_analysis) in analysis.iter() {
        if config.exclude_path(file) {
            continue;
        }
        for line in &line_analysis.cover {
            let line64 = *line as u64;
            let contain = result.iter().any(|ref x| &x.path == file && line64 == x.line);
            if !contain && !line_analysis.should_ignore(line) {
                result.push(TracerData {
                    line: line64,
                    path: file.to_path_buf(),
                    address: None,
                    hits: 0,
                    trace_type: LineType::UnusedGeneric,
                });
            }
        }
    }
    Ok(result)
}

pub fn generate_tracemap<'a>(project: &Workspace, test: &Path, config: &'a Config) -> io::Result<TraceMap<'a>> {
    let manifest = project.root();
    let file = File::open(test)?;
    let file = unsafe { 
        MmapOptions::new().map(&file)?
    };
    if let Ok(obj) = OFile::parse(&*file) {
        let analysis = get_line_analysis(project, config); 
        let endian = if obj.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    }
}

/// Generates a list of lines we want to trace the coverage of. Used to instrument the
/// traces into the test executable
pub fn generate_tracer_data(project: &Workspace, test: &Path, config: &Config) -> io::Result<Vec<TracerData>> {
    let manifest = project.root();
    let file = File::open(test)?;
    let file = unsafe { 
        MmapOptions::new().map(&file)?
    };
    if let Ok(obj) = OFile::parse(&*file) {
        let analysis = get_line_analysis(project, config); 
        let endian = if obj.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };
        get_line_addresses(endian,
                           manifest,
                           &obj,
                           &analysis,
                           config)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Error while parsing"))
    } else {
        Err(io::Error::new(io::ErrorKind::InvalidData, "Unable to parse binary."))
    }
}


