use std::path::Path;
use tracer::TracerData;
use serde::Serialize;


pub mod coveralls;

/// Trait for report formats to implement. 
/// Currently reports must be serializable using serde
pub trait Report<Out: Serialize> {
    /// Export coverage report
    fn export(coverage_data: &Vec<TracerData>, path: &Path);
    
}
