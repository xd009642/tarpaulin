use crate::config::Config;
use crate::test_loader::TracerData;
use serde::Serialize;

mod safe_json;
pub mod cobertura;
pub mod coveralls;
pub mod html;
pub mod json;
/// Trait for report formats to implement.
/// Currently reports must be serializable using serde
pub trait Report<Out: Serialize> {
    /// Export coverage report
    fn export(coverage_data: &[TracerData], config: &Config);
}
