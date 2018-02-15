use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use config::Config;

/// Used to track the state of logical conditions
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, PartialOrd)]
pub struct LogicState {
    /// Whether the condition has been observed as true
    pub beenTrue: bool,
    /// Whether the condition has been observed as false
    pub beenFalse: bool,
}

/// Shows what type of coverage data is being collected by a given trace
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, PartialOrd)]
pub enum CoverageStat {
    /// Line coverage data (whether line has been hit)
    Line(u64),
    /// Branch coverage data (whether branch has been true and false 
    Branch(LogicState),
    /// Condition coverage data (each boolean subcondition true and false)
    Condition(Vec<LogicState>),
}


#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, PartialOrd)]
pub struct Trace {
    /// Line the trace is on in the file
    pub line: u64,
    /// Optional address showing location in the test artefact 
    pub address: Option<u64>,
    /// Length of the instruction (useful to get entire condition/branch)
    pub length: usize,
    /// Coverage stats
    pub stats: CoverageStat,
}

/// Stores all the program traces mapped to files and provides an interface to
/// add, query and change traces.
pub struct TraceMap<'a> {
    /// Traces in the program mapped to the given file
    traces: BTreeMap<PathBuf, Vec<Trace>>,
    /// Tarpaulin config - is this needed?
    config: &'a Config
}

impl<'a> TraceMap<'a> {
    /// Create a new TraceMap 
    pub fn new(config: &'a Config) -> TraceMap<'a> {
        TraceMap {
            config: config,
            traces: BTreeMap::new(),
        }
    }

    /// Add a trace to the tracemap for the given file
    pub fn add_trace(&mut self, file: &Path, trace: Trace) {
        if self.traces.contains_key(file) {
            if let Some(trace_vec) = self.traces.get_mut(file) {
                trace_vec.push(trace);
            }
        } else {
            self.traces.insert(file.to_path_buf(), vec![trace]);
        }
    }

    /// Gets an immutable reference to a trace from an address. Returns None if
    /// there is no trace at that address
    pub fn get_trace(&self, address: u64) -> Option<&Trace> {
        self.all_traces()
            .iter()
            .find(|x| x.address == Some(address))
            .map(|x| *x)
    }
    
    /// Gets a mutable reference to a trace at a given address
    /// Returns None if there is no trace at that address
    pub fn get_trace_mut(&mut self, address: u64) -> Option<&mut Trace> {
        None
    }
    
    /// Gets all traces below a certain path
    pub fn get_child_traces(&self, root: &Path) -> Vec<&Trace> {
        self.traces.iter()
                   .filter(|&(ref k, _)| k.starts_with(root))
                   .flat_map(|(_, ref v)| v.iter())
                   .collect()
    }

    /// Gets all traces
    fn all_traces(&self) -> Vec<&Trace> {
        self.traces.values().flat_map(|ref x| x.iter()).collect()
    }

}
