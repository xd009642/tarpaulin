use std::collections::BTreeMap;
use std::collections::btree_map::Iter;
use std::path::{PathBuf, Path};
use std::fmt::{Display, Formatter, Result};
use std::ops::Add;
use std::cmp::{Ord, Ordering};

/// Used to track the state of logical conditions
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd)]
pub struct LogicState {
    /// Whether the condition has been observed as true
    pub been_true: bool,
    /// Whether the condition has been observed as false
    pub been_false: bool,
}

impl<'a> Add for &'a LogicState {
    type Output = LogicState;

    fn add(self, other: &'a LogicState) -> LogicState {
        LogicState {
            been_true: self.been_true | other.been_true,
            been_false: self.been_false | other.been_false,
        }
    }
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

impl Add for CoverageStat {
    type Output = CoverageStat;

    fn add(self, other: CoverageStat) -> CoverageStat {
        match (self, other) {
            (CoverageStat::Line(ref l), CoverageStat::Line(ref r)) => {
                CoverageStat::Line(l+r)
            },
            (CoverageStat::Branch(ref l), CoverageStat::Branch(ref r)) => {
                CoverageStat::Branch(l + r)
            },
            t @ _ => t.0,
        }
    }
}

impl Display for CoverageStat {
    fn fmt(&self, f: &mut Formatter) -> Result {
        match self {
            &CoverageStat::Line(x) => {
                write!(f, "hits: {}", x)
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

/// Implemented to allow Traces to be sorted by line number
impl Ord for Trace {
    fn cmp(&self, other: &Trace) -> Ordering {
        self.line.cmp(&other.line)
    }
    fn max(self, other: Trace) -> Trace {
        if self.line > other.line {
            self
        } else {
            other
        }
    }
    fn min(self, other: Trace) -> Trace {
        if self.line < other.line {
            self
        } else {
            other
        }
    }
}

/// Stores all the program traces mapped to files and provides an interface to
/// add, query and change traces.
#[derive(Debug)]
pub struct TraceMap {
    /// Traces in the program mapped to the given file
    traces: BTreeMap<PathBuf, Vec<Trace>>,
}

impl TraceMap {
    /// Create a new TraceMap 
    pub fn new() -> TraceMap {
        TraceMap {
            traces: BTreeMap::new(),
        }
    } 

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    pub fn iter(&self) -> Iter<PathBuf, Vec<Trace>> {
        self.traces.iter()
    }

    pub fn merge(&mut self, other: &TraceMap) {
        for ( k,  values) in other.iter() {
            if !self.traces.contains_key(k) {
                self.traces.insert(k.to_path_buf(), values.to_vec());
            } else {
                let mut existing = self.traces.get_mut(k).unwrap();
                for ref v in values.iter() {
                    let mut added = false;
                    if let Some(ref mut t) = existing.iter_mut().find(|ref x| x.line == v.line) {
                        t.stats = t.stats.clone() + v.stats.clone();
                        added = true;
                    }
                    if !added {
                        existing.push((*v).clone());
                        existing.sort_unstable();
                    }
                }
            }
        }
    }

    /// Add a trace to the tracemap for the given file
    pub fn add_trace(&mut self, file: &Path, trace: Trace) {
        if self.traces.contains_key(file) {
            if let Some(trace_vec) = self.traces.get_mut(file) {
                trace_vec.push(trace);
                trace_vec.sort_unstable();
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
    
    /// Returns true if the location described by file and line number is present
    /// in the tracemap
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
                CoverageStat::Branch(_) => {
                    2usize
                },
                CoverageStat::Condition(ref x) => {
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
                CoverageStat::Branch(ref x) => {
                    (x.been_true as usize) + (x.been_false as usize)
                },
                CoverageStat::Condition(ref x) => {
                    x.iter()
                     .fold(0, |acc, ref x| acc + (x.been_true as usize) + (x.been_false as usize))
                }
                CoverageStat::Line(ref x) => {
                    (x > &0) as usize
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
                CoverageStat::Branch(_) => {
                    2usize
                },
                CoverageStat::Condition(ref x) => {
                    x.len() * 2usize
                }
                _ => 1usize
            };
        }
        result
    }
    
    /// From all the coverable data return the amount covered
    pub fn total_covered(&self) -> usize {
        let mut result = 0usize;
        for t in self.all_traces() {
            result += match t.stats {
                CoverageStat::Branch(ref x) => {
                    (x.been_true as usize) + (x.been_false as usize)
                },
                CoverageStat::Condition(ref x) => {
                    x.iter()
                     .fold(0, |acc, ref x| acc + (x.been_true as usize) + (x.been_false as usize))
                }
                CoverageStat::Line(ref x) => {
                    (x > &0) as usize
                }
            };
        }
        result
    }

    /// Returns coverage percentage ranging from 0.0-1.0
    pub fn coverage_percentage(&self) -> f64 {
        (self.total_covered() as f64) / (self.total_coverable() as f64)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stat_addition() {
        let x = CoverageStat::Line(0);
        let y = CoverageStat::Line(5);
        let z = CoverageStat::Line(7);
        let xy = x.clone() + y.clone();
        let yx = y.clone() + x.clone();
        let yy = y.clone() + y.clone();
        let zy = z.clone() + y.clone();
        assert_eq!(&xy, &CoverageStat::Line(5));
        assert_eq!(&yx, &xy);
        assert_eq!(&yy, &CoverageStat::Line(10));
        assert_eq!(&zy, &CoverageStat::Line(12));

        let tf = LogicState{been_true:true, been_false:true};
        let t = LogicState{been_true:true, been_false:false};
        let f = LogicState{been_true:false, been_false:true};
        let n = LogicState{been_true:false, been_false:false};

        assert_eq!(&t+&f, tf);
        assert_eq!(&t+&t, t);
        assert_eq!(&tf+&f, tf);
        assert_eq!(&tf+&t, tf);
        assert_eq!(&t+&n, t);
        assert_eq!(&n+&f, f);
        assert_eq!(&n+&n,n);
    }
}
