use crate::source_analysis::Function;
use serde::{Deserialize, Serialize};
use std::cmp::{Ord, Ordering};
use std::collections::btree_map::Iter;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ops::Add;
use std::path::{Path, PathBuf};
use tracing::trace;

/// Used to track the state of logical conditions
#[derive(Debug, Clone, Copy, Default, Hash, PartialEq, Eq, PartialOrd, Deserialize, Serialize)]
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
            been_true: self.been_true || other.been_true,
            been_false: self.been_false || other.been_false,
        }
    }
}

/// Shows what type of coverage data is being collected by a given trace
#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Deserialize, Serialize)]
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
            (CoverageStat::Line(ref l), CoverageStat::Line(ref r)) => CoverageStat::Line(l + r),
            (CoverageStat::Branch(ref l), CoverageStat::Branch(ref r)) => {
                CoverageStat::Branch(l + r)
            }
            t => t.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct Trace {
    /// Line the trace is on in the file
    pub line: u64,
    /// Optional address showing location in the test artefact
    pub address: HashSet<u64>,
    /// Length of the instruction (useful to get entire condition/branch)
    pub length: usize,
    /// Coverage stats
    pub stats: CoverageStat,
}

#[derive(Clone, Debug, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Location {
    /// Source file
    pub file: PathBuf,
    /// Source line
    pub line: u64,
}

impl Trace {
    pub fn new(line: u64, address: HashSet<u64>, length: usize) -> Self {
        Self {
            line,
            address,
            length,
            stats: CoverageStat::Line(0),
        }
    }

    pub fn new_stub(line: u64) -> Self {
        Self {
            line,
            address: HashSet::new(),
            length: 0,
            stats: CoverageStat::Line(0),
        }
    }
}

impl PartialOrd for Trace {
    fn partial_cmp(&self, other: &Trace) -> Option<Ordering> {
        // Not sure if I care about the others
        self.line.partial_cmp(&other.line)
    }
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

/// Amount of data coverable in the provided slice traces
pub fn amount_coverable<'a>(traces: impl Iterator<Item = &'a Trace>) -> usize {
    let mut result = 0usize;
    for t in traces {
        result += match t.stats {
            CoverageStat::Branch(_) => 2usize,
            CoverageStat::Condition(ref x) => x.len() * 2usize,
            _ => 1usize,
        };
    }
    result
}

/// Amount of data covered in the provided trace slice
pub fn amount_covered<'a>(traces: impl Iterator<Item = &'a Trace>) -> usize {
    let mut result = 0usize;
    for t in traces {
        result += match t.stats {
            CoverageStat::Branch(ref x) => usize::from(x.been_true) + usize::from(x.been_false),
            CoverageStat::Condition(ref x) => x.iter().fold(0, |acc, x| {
                acc + usize::from(x.been_true) + usize::from(x.been_false)
            }),
            CoverageStat::Line(ref x) => (*x > 0).into(),
        };
    }
    result
}

pub fn coverage_percentage<'a>(traces: impl Iterator<Item = &'a Trace>) -> f64 {
    let t: Vec<_> = traces.collect();
    (amount_covered(t.iter().copied()) as f64) / (amount_coverable(t.iter().copied()) as f64)
}

/// Stores all the program traces mapped to files and provides an interface to
/// add, query and change traces.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct TraceMap {
    ///rTraces in the program mapped to the given file
    traces: BTreeMap<PathBuf, Vec<Trace>>,
    functions: HashMap<PathBuf, Vec<Function>>,
}

impl TraceMap {
    /// Create a new TraceMap
    pub fn new() -> TraceMap {
        Self::default()
    }

    pub fn set_functions(&mut self, functions: HashMap<PathBuf, Vec<Function>>) {
        self.functions = functions;
    }

    /// Returns true if there are no traces
    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    /// Provides an iterator to the underlying map of PathBufs to Vec<Trace>
    pub fn iter(&self) -> Iter<PathBuf, Vec<Trace>> {
        self.traces.iter()
    }

    /// Merges the results of one tracemap into the current one.
    /// This adds records which are missing and adds the statistics gathered to
    /// existing records
    pub fn merge(&mut self, other: &TraceMap) {
        self.functions
            .extend(other.functions.iter().map(|(k, v)| (k.clone(), v.clone())));
        for (k, values) in other.iter() {
            if !self.traces.contains_key(k) {
                self.traces.insert(k.clone(), values.clone());
            } else {
                let existing = self.traces.get_mut(k).unwrap();
                for v in values.iter() {
                    let mut added = false;
                    if let Some(ref mut t) = existing
                        .iter_mut()
                        .find(|x| x.line == v.line && x.address == v.address)
                    {
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

    /// This will collapse duplicate Traces into a single trace. Warning this
    /// will lose the addresses of the duplicate traces but increment the results
    /// should be called only if you don't need those addresses from then on
    /// TODO possibly not the cleanest solution
    pub fn dedup(&mut self) {
        for values in self.traces.values_mut() {
            // Map of lines and stats, merge duplicated stats here
            let mut lines: HashMap<u64, CoverageStat> = HashMap::new();
            // Duplicated traces need cleaning up. Maintain a list of them!
            let mut dirty: Vec<u64> = Vec::new();
            for v in values.iter() {
                lines
                    .entry(v.line)
                    .and_modify(|e| {
                        dirty.push(v.line);
                        *e = e.clone() + v.stats.clone();
                    })
                    .or_insert_with(|| v.stats.clone());
            }
            for d in &dirty {
                let mut first = true;
                values.retain(|x| {
                    let res = x.line != *d;
                    if !res {
                        if first {
                            first = false;
                            true
                        } else {
                            false
                        }
                    } else {
                        res
                    }
                });
                if let Some(new_stat) = lines.remove(d) {
                    if let Some(ref mut t) = values.iter_mut().find(|x| x.line == *d) {
                        t.stats = new_stat;
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

    pub fn add_file(&mut self, file: &Path) {
        if !self.traces.contains_key(file) {
            self.traces.insert(file.to_path_buf(), vec![]);
        }
    }

    /// Gets an immutable reference to a trace from an address. Returns None if
    /// there is no trace at that address
    pub fn get_trace(&self, address: u64) -> Option<&Trace> {
        self.all_traces().find(|x| x.address.contains(&address))
    }

    pub fn increment_hit(&mut self, address: u64) {
        for trace in self
            .all_traces_mut()
            .filter(|x| x.address.contains(&address))
        {
            if let CoverageStat::Line(ref mut x) = trace.stats {
                trace!("Incrementing hit count for trace");
                *x += 1;
            }
        }
    }

    pub fn get_location(&self, address: u64) -> Option<Location> {
        for (k, v) in &self.traces {
            if let Some(s) = v
                .iter()
                .find(|x| x.address.iter().any(|x| (*x & !0x7u64) == address))
            {
                return Some(Location {
                    file: k.clone(),
                    line: s.line,
                });
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

    /// Returns true if the file is among the traces
    pub fn contains_file(&self, file: &Path) -> bool {
        self.traces.contains_key(file)
    }

    /// Gets all traces below a certain path
    pub fn get_child_traces<'a>(&'a self, root: &'a Path) -> impl Iterator<Item = &'a Trace> + 'a {
        self.traces
            .iter()
            .filter(move |&(k, _)| k.starts_with(root))
            .flat_map(|(_, v)| v.iter())
    }

    pub fn get_functions(&self, file: &Path) -> impl Iterator<Item = &Function> {
        let i: Box<dyn Iterator<Item = &Function>> = match self.functions.get(file) {
            Some(f) => Box::new(f.iter()),
            None => Box::new(std::iter::empty()),
        };
        i
    }

    pub fn file_traces_mut(&mut self, file: &Path) -> Option<&mut Vec<Trace>> {
        self.traces.get_mut(file)
    }

    /// Gets all traces
    pub fn all_traces(&self) -> impl Iterator<Item = &Trace> {
        self.traces.values().flat_map(|x| x.iter())
    }

    /// Gets a vector of all the traces to mutate
    fn all_traces_mut(&mut self) -> impl Iterator<Item = &mut Trace> {
        self.traces.values_mut().flat_map(|x| x.iter_mut())
    }

    pub fn files(&self) -> Vec<&PathBuf> {
        self.traces.keys().collect()
    }

    pub fn coverable_in_path(&self, path: &Path) -> usize {
        amount_coverable(self.get_child_traces(path))
    }

    pub fn covered_in_path(&self, path: &Path) -> usize {
        amount_covered(self.get_child_traces(path))
    }

    /// Give the total amount of coverable points in the code. This will vary
    /// based on the statistics available for line coverage it will be total
    /// lines whereas for condition or decision it will count the number of
    /// conditions available
    pub fn total_coverable(&self) -> usize {
        amount_coverable(self.all_traces())
    }

    /// From all the coverable data return the amount covered
    pub fn total_covered(&self) -> usize {
        amount_covered(self.all_traces())
    }

    /// Returns coverage percentage ranging from 0.0-1.0
    pub fn coverage_percentage(&self) -> f64 {
        coverage_percentage(self.all_traces())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    #[allow(clippy::many_single_char_names)]
    fn stat_addition() {
        let x = CoverageStat::Line(0);
        let y = CoverageStat::Line(5);
        let z = CoverageStat::Line(7);
        let xy = x.clone() + y.clone();
        let yx = y.clone() + x;
        let yy = y.clone() + y.clone();
        let zy = z + y;
        assert_eq!(&xy, &CoverageStat::Line(5));
        assert_eq!(&yx, &xy);
        assert_eq!(&yy, &CoverageStat::Line(10));
        assert_eq!(&zy, &CoverageStat::Line(12));

        let tf = LogicState {
            been_true: true,
            been_false: true,
        };
        let t = LogicState {
            been_true: true,
            been_false: false,
        };
        let f = LogicState {
            been_true: false,
            been_false: true,
        };
        let n = LogicState {
            been_true: false,
            been_false: false,
        };

        assert_eq!(&t + &f, tf);
        assert_eq!(&t + &t, t);
        assert_eq!(&tf + &f, tf);
        assert_eq!(&tf + &t, tf);
        assert_eq!(&t + &n, t);
        assert_eq!(&n + &f, f);
        assert_eq!(&n + &n, n);
    }

    #[test]
    fn multiple_traces_per_line() {
        let mut t1 = TraceMap::new();
        let mut address = HashSet::new();
        address.insert(0);
        address.insert(128);
        let trace_1 = Trace {
            line: 1,
            address,
            length: 0,
            stats: CoverageStat::Line(1),
        };
        t1.add_trace(Path::new("file.rs"), trace_1);

        let coverable = t1.total_coverable();
        assert_eq!(coverable, 1);
        let total_covered = t1.total_covered();
        assert_eq!(total_covered, 1);
    }

    #[test]
    fn merge_address_mismatch_and_dedup() {
        let mut t1 = TraceMap::new();
        let mut t2 = TraceMap::new();

        let mut address = HashSet::new();
        address.insert(5);
        let a_trace = Trace {
            line: 1,
            address,
            length: 0,
            stats: CoverageStat::Line(1),
        };
        t1.add_trace(Path::new("file.rs"), a_trace.clone());
        t2.add_trace(
            Path::new("file.rs"),
            Trace {
                line: 1,
                address: HashSet::new(),
                length: 0,
                stats: CoverageStat::Line(2),
            },
        );

        t1.merge(&t2);
        assert_eq!(t1.all_traces().count(), 2);
        assert_eq!(t1.get_trace(5), Some(&a_trace));
        t1.dedup();
        let all = t1.all_traces().collect::<Vec<_>>();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].stats, CoverageStat::Line(3));
    }

    #[test]
    fn no_merge_dedup_needed() {
        let mut t1 = TraceMap::new();
        let mut t2 = TraceMap::new();

        let mut address = HashSet::new();
        address.insert(5);
        let a_trace = Trace {
            line: 1,
            address,
            length: 0,
            stats: CoverageStat::Line(1),
        };
        t1.add_trace(Path::new("file.rs"), a_trace.clone());
        t2.add_trace(
            Path::new("file.rs"),
            Trace {
                line: 2,
                address: HashSet::new(),
                length: 0,
                stats: CoverageStat::Line(2),
            },
        );

        t1.merge(&t2);
        assert_eq!(t1.all_traces().count(), 2);
        assert_eq!(t1.get_trace(5), Some(&a_trace));
        t1.dedup();
        let all = t1.all_traces();
        assert_eq!(all.count(), 2);
    }

    #[test]
    fn merge_needed() {
        let mut t1 = TraceMap::new();
        let mut t2 = TraceMap::new();

        let mut address = HashSet::new();
        address.insert(1);
        t1.add_trace(
            Path::new("file.rs"),
            Trace {
                line: 2,
                address: address.clone(),
                length: 0,
                stats: CoverageStat::Line(5),
            },
        );
        t2.add_trace(
            Path::new("file.rs"),
            Trace {
                line: 2,
                address: address.clone(),
                length: 0,
                stats: CoverageStat::Line(2),
            },
        );
        t1.merge(&t2);
        assert_eq!(t1.all_traces().count(), 1);
        assert_eq!(
            t1.get_trace(1),
            Some(&Trace {
                line: 2,
                address: address.clone(),
                length: 0,
                stats: CoverageStat::Line(7),
            })
        );
        // Deduplicating should have no effect.
        t1.dedup();
        assert_eq!(t1.all_traces().count(), 1);
        assert_eq!(
            t1.get_trace(1),
            Some(&Trace {
                line: 2,
                address,
                length: 0,
                stats: CoverageStat::Line(7),
            })
        );
    }
}
