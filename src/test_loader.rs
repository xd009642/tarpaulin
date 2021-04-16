use crate::config::Config;
use crate::path_utils::is_coverable_file_path;
use crate::source_analysis::*;
use crate::traces::*;
use gimli::read::Error;
use gimli::*;
use memmap::MmapOptions;
use object::{read::ObjectSection, File as OFile, Object};
use rustc_demangle::demangle;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use tracing::{debug, error, trace};

/// Describes a function as `low_pc`, `high_pc` and bool representing `is_test`.
type FuncDesc = (u64, u64, FunctionType, Option<String>);

#[derive(Debug, Clone, Copy, PartialEq)]
enum FunctionType {
    Generated,
    Test,
    Standard,
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct SourceLocation {
    pub path: PathBuf,
    pub line: u64,
}

impl From<(PathBuf, usize)> for SourceLocation {
    fn from(other: (PathBuf, usize)) -> Self {
        Self {
            path: other.0,
            line: other.1 as u64,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TracerData {
    /// Currently used to find generated __test::main and remove from coverage,
    /// may have uses in future for finding conditions etc
    pub trace_type: LineType,
    /// Start address of the line
    pub address: Option<u64>,
    /// Length of the instruction
    pub length: u64,
    /// Function name
    pub fn_name: Option<String>,
}

fn generate_func_desc<R, Offset>(
    die: &DebuggingInformationEntry<R, Offset>,
    debug_str: &DebugStr<R>,
) -> Result<FuncDesc>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    let mut func_type = FunctionType::Standard;
    let low = die.attr_value(DW_AT_low_pc)?;
    let high = die.attr_value(DW_AT_high_pc)?;
    let linkage = die.attr_value(DW_AT_linkage_name)?;
    let fn_name = die.attr_value(DW_AT_name)?;

    let fn_name: Option<String> = match fn_name {
        Some(AttributeValue::DebugStrRef(offset)) => debug_str
            .get_str(offset)
            .and_then(|r| r.to_string().map(|s| s.to_string()))
            .ok()
            .map(|r| demangle(r.as_ref()).to_string()),
        _ => None,
    };

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
        let name = debug_str
            .get_str(offset)
            .and_then(|r| r.to_string().map(|s| s.to_string()))
            .unwrap_or_else(|_| "".into());
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
    Ok((low, high, func_type, fn_name))
}

/// Finds all function entry points and returns a vector
/// This will identify definite tests, but may be prone to false negatives.
fn get_entry_points<R, Offset>(
    debug_info: &UnitHeader<R, Offset>,
    debug_abbrev: &Abbreviations,
    debug_str: &DebugStr<R>,
) -> Vec<FuncDesc>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    let mut result: Vec<FuncDesc> = Vec::new();
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

fn get_addresses_from_program<R, Offset>(
    prog: IncompleteLineProgram<R>,
    debug_strs: &DebugStr<R>,
    entries: &[(u64, LineType, &Option<String>)],
    config: &Config,
    result: &mut HashMap<SourceLocation, Vec<TracerData>>,
) -> Result<()>
where
    R: Reader<Offset = Offset>,
    Offset: ReaderOffset,
{
    let project = config.root();
    let get_string = |x: R| x.to_string().map(|y| y.to_string()).ok();
    let (cprog, seq) = prog.sequences()?;
    for s in seq {
        let mut sm = cprog.resume_from(&s);
        while let Ok(Some((header, &ln_row))) = sm.next_row() {
            // If this row isn't useful move on
            if !ln_row.is_stmt() || ln_row.line().is_none() {
                continue;
            }
            if let Some(file) = ln_row.file(header) {
                let mut path = project.clone();
                if let Some(dir) = file.directory(header) {
                    if let Some(temp) = dir.string_value(debug_strs).and_then(get_string) {
                        path.push(temp);
                    }
                }
                if let Ok(p) = path.canonicalize() {
                    path = p;
                }
                // Source is part of project so we cover it.
                if let Some(file) = ln_row.file(header) {
                    let file = file.path_name();
                    let line = ln_row.line().unwrap();
                    if let Some(file) = file.string_value(debug_strs).and_then(get_string) {
                        path.push(file);
                        if !path.is_file() {
                            // Not really a source file!
                            continue;
                        }
                        if is_coverable_file_path(&path, &project, &config.target_dir()) {
                            let address = ln_row.address();
                            let (desc, fn_name) = entries
                                .iter()
                                .filter(|&&(addr, _, _)| addr == address)
                                .map(|&(_, t, fn_name)| (t, fn_name.to_owned()))
                                .next()
                                .unwrap_or((LineType::Unknown, None));
                            let loc = SourceLocation { path, line };
                            if desc != LineType::TestMain {
                                let trace = TracerData {
                                    address: Some(address),
                                    trace_type: desc,
                                    length: 1,
                                    fn_name,
                                };
                                let tracerdata = result.entry(loc).or_default();
                                tracerdata.push(trace);
                            }
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn get_line_addresses(
    endian: RunTimeEndian,
    obj: &OFile,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
) -> Result<TraceMap> {
    let project = config.root();
    let io_err = |e| {
        error!("Io error parsing section: {}", e);
        Error::Io
    };
    let mut result = TraceMap::new();
    let debug_info = obj.section_by_name(".debug_info").ok_or(Error::Io)?;
    let debug_info = DebugInfo::new(debug_info.data().map_err(io_err)?, endian);
    let debug_abbrev = obj.section_by_name(".debug_abbrev").ok_or(Error::Io)?;
    let debug_abbrev = DebugAbbrev::new(debug_abbrev.data().map_err(io_err)?, endian);
    let debug_strings = obj.section_by_name(".debug_str").ok_or(Error::Io)?;
    let debug_strings = DebugStr::new(debug_strings.data().map_err(io_err)?, endian);
    let debug_line = obj.section_by_name(".debug_line").ok_or(Error::Io)?;
    let debug_line = DebugLine::new(debug_line.data().map_err(io_err)?, endian);

    let base_addr = obj.section_by_name(".text").ok_or(Error::Io)?;

    let mut iter = debug_info.units();
    while let Ok(Some(cu)) = iter.next() {
        let addr_size = cu.address_size();
        let abbr = match cu.abbreviations(&debug_abbrev) {
            Ok(a) => a,
            _ => continue,
        };
        let entry_points = get_entry_points(&cu, &abbr, &debug_strings);
        let entries = entry_points
            .iter()
            .map(|(a, b, c, fn_name)| match c {
                FunctionType::Test => (*a, LineType::TestEntry(*b), fn_name),
                FunctionType::Standard => (*a, LineType::FunctionEntry(*b), fn_name),
                FunctionType::Generated => (*a, LineType::TestMain, fn_name),
            })
            .collect::<Vec<_>>();

        if let Ok(Some((_, root))) = cu.entries(&abbr).next_dfs() {
            let offset = match root.attr_value(DW_AT_stmt_list) {
                Ok(Some(AttributeValue::DebugLineRef(o))) => o,
                _ => continue,
            };
            let prog = debug_line.program(offset, addr_size, None, None)?;
            let mut temp_map: HashMap<SourceLocation, Vec<TracerData>> = HashMap::new();

            if let Err(e) =
                get_addresses_from_program(prog, &debug_strings, &entries, config, &mut temp_map)
            {
                debug!("Potential issue reading test addresses {}", e);
            } else {
                // Deduplicate addresses
                for v in temp_map.values_mut() {
                    v.dedup_by_key(|x| x.address);
                }
                let temp_map = temp_map
                    .into_iter()
                    .filter(|&(ref k, _)| {
                        !(config.ignore_tests && k.path.starts_with(&project.join("tests")))
                    })
                    .filter(|&(ref k, _)| !(config.exclude_path(&k.path)))
                    .filter(|&(ref k, _)| {
                        !analysis.should_ignore(k.path.as_ref(), &(k.line as usize))
                    })
                    .map(|(k, v)| {
                        let ret = analysis.normalise(k.path.as_ref(), k.line as usize);
                        let k_n = SourceLocation::from(ret);
                        (k_n, v)
                    })
                    .collect::<HashMap<SourceLocation, Vec<TracerData>>>();

                let mut tracemap = TraceMap::new();
                for (k, val) in &temp_map {
                    let rpath = config.strip_base_dir(&k.path);
                    let mut address = HashSet::new();
                    let mut fn_name = None;
                    for v in val.iter() {
                        if let Some(a) = v.address {
                            if a < base_addr.address()
                                && a >= (base_addr.address() + base_addr.size())
                            {
                                continue;
                            }
                            address.insert(a);
                            trace!(
                                "Adding trace at address 0x{:x} in {}:{}",
                                a,
                                rpath.display(),
                                k.line
                            );
                        }
                        if fn_name.is_none() && v.fn_name.is_some() {
                            fn_name = v.fn_name.clone();
                        }
                    }
                    if address.is_empty() {
                        trace!(
                            "Adding trace with no address at {}:{}",
                            rpath.display(),
                            k.line
                        );
                    }
                    tracemap.add_trace(&k.path, Trace::new(k.line, address, 1, fn_name));
                }
                result.merge(&tracemap);
            }
        }
    }

    for (file, ref line_analysis) in analysis.iter() {
        if config.exclude_path(file) {
            continue;
        }
        for line in &line_analysis.cover {
            let line = *line as u64;
            if !result.contains_location(file, line) && !line_analysis.should_ignore(line as usize)
            {
                let rpath = config.strip_base_dir(file);
                trace!(
                    "Adding trace for potentially uncoverable line in {}:{}",
                    rpath.display(),
                    line
                );
                result.add_trace(file, Trace::new_stub(line));
            }
        }
    }
    Ok(result)
}

#[cfg(target_os = "linux")]
fn open_symbols_file(test: &Path) -> io::Result<File> {
    File::open(test)
}

#[cfg(target_os = "macos")]
fn open_symbols_file(test: &Path) -> io::Result<File> {
    let d_sym = test.with_extension("dSYM");
    File::open(&d_sym)
}

#[cfg(target_os = "windows")]
fn open_symbols_file(test: &Path) -> io::Result<File> {
    Err(io::Error::new(
        io::ErrorKind::Other,
        "Windows is not currently supported",
    ))
}

pub fn generate_tracemap(
    test: &Path,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
) -> io::Result<TraceMap> {
    let file = open_symbols_file(test)?;
    let file = unsafe { MmapOptions::new().map(&file)? };
    if let Ok(obj) = OFile::parse(&*file) {
        let endian = if obj.is_little_endian() {
            RunTimeEndian::Little
        } else {
            RunTimeEndian::Big
        };
        if let Ok(result) = get_line_addresses(endian, &obj, &analysis, config) {
            Ok(result)
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Error while parsing",
            ))
        }
    } else {
        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Unable to parse binary.",
        ))
    }
}
