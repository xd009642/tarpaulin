use test_loader::TracerData;
use config::Config;
use serde::Serialize;

pub mod cobertura;
pub mod coveralls;
/// Trait for report formats to implement. 
/// Currently reports must be serializable using serde
pub trait Report<Out: Serialize> {
    /// Export coverage report
    fn export(coverage_data: &[TracerData], config: &Config);
    
}
