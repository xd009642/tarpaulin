use std::collections::BTreeMap;
use std::path::{PathBuf, Path};
use config::Config;

#[derive(Debug, Clone, Default, Hash, PartialEq, Eq, PartialOrd)]
pub struct Trace {
    pub line: u64,
    pub address: Option<u64>,
    pub hits: u64,
}


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
    
    pub fn get_child_traces(&self, root: &Path) -> Vec<&Trace> {
        self.traces.iter()
                   .filter(|&(ref k, _)| k.starts_with(root))
                   .flat_map(|(_, ref v)| v.iter())
                   .collect()
    }
}
