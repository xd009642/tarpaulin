use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use config::Config;

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, PartialOrd)]
pub struct Trace {
    pub line: u64,
    pub address: Option<u64>,
    pub hits: u64,
}

/// Stores all the program traces mapped to files and provides an interface to
/// add, query and change traces.
pub struct TraceMap<'a> {
    traces: BTreeMap<PathBuf, Vec<Trace>>,
    config: &'a Config
}

impl<'a> TraceMap<'a> {
    
    pub fn new(config: &'a Config) -> TraceMap<'a> {
        TraceMap {
            config: config,
            traces: BTreeMap::new(),
        }
    }

    pub fn add_trace(&mut self, file: &Path, trace: Trace) {
        if self.traces.contains_key(file) {
            if let Some(trace_vec) = self.traces.get_mut(file) {
                trace_vec.push(trace);
            }
        } else {
            self.traces.insert(file.to_path_buf(), vec![trace]);
        }
    }

    pub fn get_trace(&self, address: u64) -> Option<&Trace> {
        self.all_traces()
            .iter()
            .find(|x| x.address == Some(address))
            .map(|x| *x)
    }

    pub fn get_trace_mut(&mut self, address: u64) -> Option<&mut Trace> {
        None
    }
    
    pub fn get_child_traces(&self, root: &Path) -> Vec<&Trace> {
        self.traces.iter()
                   .filter(|&(ref k, _)| k.starts_with(root))
                   .flat_map(|(_, ref v)| v.iter())
                   .collect()
    }

    fn all_traces(&self) -> Vec<&Trace> {
        self.traces.values().flat_map(|ref x| x.iter()).collect()
    }

}
