use crate::config::Color;
use crate::generate_tracemap;
use crate::statemachine::{create_state_machine, TestState};
use crate::traces::*;
use crate::{Config, EventLog, LineAnalysis, RunError, TestBinary, TraceEngine};
use std::collections::HashMap;
use std::env;
use std::ffi::CString;
use std::path::{Path, PathBuf};
use tracing::{info, trace_span};

cfg_if::cfg_if! {
    if #[cfg(target_os= "linux")] {
        pub mod linux;
        pub use linux::*;

        pub mod breakpoint;
        pub mod ptrace_control;

        pub type ProcessHandle = nix::unistd::Pid;
    } else {
        pub type ProcessHandle = u64;

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
            tracing::error!("Tarpaulin does not support executing tests on this platform");
            Err(RunError::TestCoverage("Unsupported OS".to_string()))
        }
    }
}

/// Collects the coverage data from the launched test
pub(crate) fn collect_coverage(
    test_path: &Path,
    test: ProcessHandle,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
    logger: &Option<EventLog>,
) -> Result<(TraceMap, i32), RunError> {
    let mut ret_code = 0;
    let mut traces = generate_tracemap(test_path, analysis, config)?;
    {
        let span = trace_span!("Collect coverage", pid=%test);
        let _enter = span.enter();
        let (mut state, mut data) =
            create_state_machine(test, &mut traces, analysis, config, logger);
        loop {
            state = state.step(&mut data, config)?;
            if state.is_finished() {
                if let TestState::End(i) = state {
                    ret_code = i;
                }
                break;
            }
        }
    }
    Ok((traces, ret_code))
}

/// Launches the test executable
fn execute_test(test: &TestBinary, ignored: bool, config: &Config) -> Result<(), RunError> {
    let exec_path = CString::new(test.path().to_str().unwrap()).unwrap();
    info!("running {}", test.path().display());
    let _ = match test.manifest_dir() {
        Some(md) => env::set_current_dir(&md),
        None => env::set_current_dir(&config.root()),
    };

    let mut envars: Vec<CString> = Vec::new();

    for (key, value) in env::vars() {
        let mut temp = String::new();
        temp.push_str(key.as_str());
        temp.push('=');
        temp.push_str(value.as_str());
        envars.push(CString::new(temp).unwrap());
    }
    let mut argv = if ignored {
        vec![exec_path.clone(), CString::new("--ignored").unwrap()]
    } else {
        vec![exec_path.clone()]
    };
    if config.verbose {
        envars.push(CString::new("RUST_BACKTRACE=1").unwrap());
    }
    for s in &config.varargs {
        argv.push(CString::new(s.as_bytes()).unwrap_or_default());
    }
    if config.color != Color::Auto {
        argv.push(CString::new("--color").unwrap_or_default());
        argv.push(CString::new(config.color.to_string().to_ascii_lowercase()).unwrap_or_default());
    }

    if let Some(s) = test.pkg_name() {
        envars.push(CString::new(format!("CARGO_PKG_NAME={}", s)).unwrap_or_default());
    }
    if let Some(s) = test.pkg_version() {
        envars.push(CString::new(format!("CARGO_PKG_VERSION={}", s)).unwrap_or_default());
    }
    if let Some(s) = test.pkg_authors() {
        envars.push(CString::new(format!("CARGO_PKG_AUTHORS={}", s.join(":"))).unwrap_or_default());
    }
    if let Some(s) = test.manifest_dir() {
        envars
            .push(CString::new(format!("CARGO_MANIFEST_DIR={}", s.display())).unwrap_or_default());
    }
    if config.engine == TraceEngine::Llvm || config.engine == TraceEngine::Auto {
        // Used for llvm coverage to avoid report naming clashes
        envars.push(CString::new("LLVM_PROFILE_FILE=default_%p.profraw").unwrap_or_default());
    }

    execute(exec_path, &argv, envars.as_slice())
}
