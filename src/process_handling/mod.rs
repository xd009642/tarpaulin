cfg_if::cfg_if! {
    if #[cfg(target_os = "macos")] {
        pub mod mac;
        pub use mac::*;

        pub type ProcessHandle = nix::unistd::Pid;
    } else if #[cfg(target_os= "linux")] {
        pub mod linux;
        pub use linux::*;

        pub mod breakpoint;
        pub mod ptrace_control;

        pub type ProcessHandle = nix::unistd::Pid;
    } else {
        use crate::{TestBinary, Config, RunError, EventLog, TraceMap, LineAnalysis};
        use std::ffi::CString;
        use std::collections::HashMap;
        use std::path::PathBuf;

        pub type ProcessHandle = u64;

        pub fn execute_test(test: &TestBinary, ignored: bool, config: &Config) -> Result<(), RunError> {
            tracing::error!("Tarpaulin does not support executing tests on this platform");
            Err(RunError::TestCoverage("Unsupported OS".to_string()))
        }

        /// Returns the coverage statistics for a test executable in the given workspace
        pub fn get_test_coverage(
            test: &TestBinary,
            analysis: &HashMap<PathBuf, LineAnalysis>,
            config: &Config,
            ignored: bool,
            logger: &Option<EventLog>,
        ) -> Result<Option<(TraceMap, i32)>, RunError> {
            tracing::error!("Tarpaulin does not support executing tests on this platform");
            Err(RunError::TestCoverage("Unsupported OS".to_string()))
        }

        pub fn execute(program: CString, argv: &[CString], envar: &[CString]) -> Result<(), RunError> {
            todo!()
        }
    }
}
