use std::collections::BTreeMap;
use std::collections::btree_map::Iter;
use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter, Result};
use config::Config;

/// Used to track the state of logical conditions
#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, PartialOrd)]
pub struct LogicState {
    /// Whether the condition has been observed as true
    pub been_true: bool,
    /// Whether the condition has been observed as false
    pub been_false: bool,
}

/// Shows what type of coverage data is being collected by a given trace
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd)]
pub enum CoverageStat {
    /// Line coverage data (whether line has been hit)
    Line(u64),
    /// Branch coverage data (whether branch has been true and false 
    Branch(LogicState),
    /// Condition coverage data (each boolean subcondition true and false)
    Condition(Vec<LogicState>),
}

impl Display for CoverageStat {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &CoverageStat::Line(x) => {
                write!(f, "Hits: {}", x)
            },
            _ => write!(f, ""),
        }
    }
}


#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd)]
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

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    pub fn iter(&self) -> Iter<PathBuf, Vec<Trace>> {
        self.traces.iter()
    }

    pub fn merge(&mut self, other: &'a TraceMap) {
        
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
        for val in self.all_traces_mut() {
            if val.address == Some(address) {
                return Some(val);
            }
        }
        None
    }

    pub fn contains_location(&self, file: &Path, line: u64) -> bool {
        match self.traces.get(file) {
            Some(traces) => traces.iter().any(|x| x.line == line),
            None => false,
        }
    }
    
    /// Gets all traces below a certain path
    pub fn get_child_traces(&self, root: &Path) -> Vec<&Trace> {
        self.traces.iter()
                   .filter(|&(ref k, _)| k.starts_with(root))
                   .flat_map(|(_, ref v)| v.iter())
                   .collect()
    }

    /// Gets all traces
    pub fn all_traces(&self) -> Vec<&Trace> {
        self.traces.values().flat_map(|ref x| x.iter()).collect()
    }

    /// Gets a vector of all the traces to mutate
    fn all_traces_mut(&mut self) -> Vec<&mut Trace> {
        self.traces.values_mut().flat_map(|x| x.iter_mut()).collect()
    }

    pub fn files(&self) -> Vec<&PathBuf> {
        self.traces.keys().collect()
    }

    pub fn coverable_in_path(&self, path: &Path) -> usize {
        let mut result = 0usize;
        for t in self.get_child_traces(path) {
            result += match t.stats {
                CoverageStat::Branch(x) => {
                    2usize
                },
                CoverageStat::Condition(x) => {
                    x.len() * 2usize
                }
                _ => 1usize
            };
        }
        result
    }

    pub fn covered_in_path(&self, path: &Path) -> usize {
        let mut result = 0usize;
        for t in self.get_child_traces(path) {
            result += match t.stats {
                CoverageStat::Branch(x) => {
                    (x.been_true as usize) + (x.been_false as usize)
                },
                CoverageStat::Condition(x) => {
                    x.iter()
                     .fold(0, |acc, &x| acc + (x.been_true as usize) + (x.been_false as usize))
                }
                CoverageStat::Line(x) => {
                    (x > 0) as usize
                }
            };
        }
        result
    }

    /// Give the total amount of coverable points in the code. This will vary
    /// based on the statistics available for line coverage it will be total
    /// line whereas for condition or decision it will count the number of 
    /// conditions available
    pub fn total_coverable(&self) -> usize {
        let mut result = 0usize;
        for t in self.all_traces() {
            result += match t.stats {
                CoverageStat::Branch(x) => {
                    2usize
                },
                CoverageStat::Condition(x) => {
                    x.len() * 2usize
                }
                _ => 1usize
            };
        }
        result
    }

    pub fn total_covered(&self) -> usize {
        let mut result = 0usize;
        for t in self.all_traces() {
            result += match t.stats {
                CoverageStat::Branch(x) => {
                    (x.been_true as usize) + (x.been_false as usize)
                },
                CoverageStat::Condition(x) => {
                    x.iter()
                     .fold(0, |acc, &x| acc + (x.been_true as usize) + (x.been_false as usize))
                }
                CoverageStat::Line(x) => {
                    (x > 0) as usize
                }
            };
        }
        result
    }

}
