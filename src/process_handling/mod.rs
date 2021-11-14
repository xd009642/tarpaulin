use crate::config::Color;
use crate::generate_tracemap;
use crate::path_utils::get_profile_walker;
use crate::statemachine::{create_state_machine, TestState};
use crate::traces::*;
use crate::{Config, EventLog, LineAnalysis, RunError, TestBinary, TraceEngine};
use std::collections::HashMap;
use std::env;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use tracing::{debug, error, info, trace_span};

/// Handle to a test currently either PID or a `std::process::Child`
pub enum TestHandle {
    Id(ProcessHandle),
    Process(RunningProcessHandle),
}

pub struct RunningProcessHandle {
    /// Used to map coverage counters to line numbers
    pub(crate) path: PathBuf,
    /// Get the exit status to work out if tests have passed
    pub(crate) child: Child,
    /// maintain a list of existing profraws in the project root to avoid picking up old results
    pub(crate) existing_profraws: Vec<PathBuf>,
}

impl RunningProcessHandle {
    pub fn new(path: PathBuf, cmd: &mut Command, config: &Config) -> Result<Self, RunError> {
        let child = cmd.spawn()?;
        let existing_profraws = get_profile_walker(config)
            .map(|x| x.path().to_path_buf())
            .collect();

        Ok(Self {
            path,
            child,
            existing_profraws,
        })
    }
}

impl fmt::Display for TestHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TestHandle::Id(id) => write!(f, "{}", id),
            TestHandle::Process(c) => write!(f, "{}", c.child.id()),
        }
    }
}

impl From<ProcessHandle> for TestHandle {
    fn from(handle: ProcessHandle) -> Self {
        Self::Id(handle)
    }
}

impl From<RunningProcessHandle> for TestHandle {
    fn from(handle: RunningProcessHandle) -> Self {
        Self::Process(handle)
    }
}

pub fn get_test_coverage(
    test: &TestBinary,
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
    ignored: bool,
    logger: &Option<EventLog>,
) -> Result<Option<(TraceMap, i32)>, RunError> {
    let handle = launch_test(test, config, ignored, logger)?;
    if let Some(handle) = handle {
        match collect_coverage(test.path(), handle, analysis, config, logger) {
            Ok(t) => Ok(Some(t)),
            Err(e) => Err(RunError::TestCoverage(e.to_string())),
        }
    } else {
        Ok(None)
    }
}

fn launch_test(
    test: &TestBinary,
    config: &Config,
    ignored: bool,
    logger: &Option<EventLog>,
) -> Result<Option<TestHandle>, RunError> {
    if let Some(log) = logger.as_ref() {
        log.push_binary(test.clone());
    }
    match config.engine() {
        TraceEngine::Ptrace => {
            cfg_if::cfg_if! {
                if #[cfg(target_os="linux")] {
                    linux::get_test_coverage(test, config, ignored)
                } else {
                    error!("Ptrace is not supported on this platform");
                    Err(RunError::TestCoverage("Unsupported OS".to_string()))
                }
            }
        }
        TraceEngine::Llvm => {
            let res = execute_test(test, ignored, config, None)?;
            Ok(Some(res))
        }
        e => {
            error!(
                "Tarpaulin cannot execute tests with {:?} on this platform",
                e
            );
            Err(RunError::TestCoverage("Unsupported OS".to_string()))
        }
    }
}

cfg_if::cfg_if! {
    if #[cfg(target_os= "linux")] {
        pub mod linux;
        pub use linux::*;

        pub mod breakpoint;
        pub mod ptrace_control;

        pub type ProcessHandle = nix::unistd::Pid;
    } else {
        pub type ProcessHandle = u64;
    }
}

/// Collects the coverage data from the launched test
pub(crate) fn collect_coverage(
    test_path: &Path,
    test: TestHandle,
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
            } else if let Some(event_logger) = logger {
                event_logger.push_marker();
            }
        }
    }
    Ok((traces, ret_code))
}

fn get_env_vars(test: &TestBinary, config: &Config) -> Vec<(String, String)> {
    let mut envars: Vec<(String, String)> = Vec::new();

    for (key, value) in env::vars() {
        // Avoid adding it twice
        if key == "LD_LIBRARY_PATH" && test.has_linker_paths() {
            continue;
        }
        envars.push((key.to_string(), value.to_string()));
    }
    if config.verbose {
        envars.push(("RUST_BACKTRACE".to_string(), "1".to_string()));
    }
    if let Some(s) = test.pkg_name() {
        envars.push(("CARGO_PKG_NAME".to_string(), s.to_string()));
    }
    if let Some(s) = test.pkg_version() {
        envars.push(("CARGO_PKG_VERSION".to_string(), s.to_string()));
    }
    if let Some(s) = test.pkg_authors() {
        envars.push(("CARGO_PKG_AUTHORS".to_string(), s.join(":")));
    }
    if let Some(s) = test.manifest_dir() {
        envars.push(("CARGO_MANIFEST_DIR".to_string(), s.display().to_string()));
    }
    if test.has_linker_paths() {
        envars.push(("LD_LIBRARY_PATH".to_string(), test.ld_library_path()));
    }

    envars
}

/// Launches the test executable
fn execute_test(
    test: &TestBinary,
    ignored: bool,
    config: &Config,
    num_threads: Option<usize>,
) -> Result<TestHandle, RunError> {
    info!("running {}", test.path().display());
    let _ = match test.manifest_dir() {
        Some(md) => env::set_current_dir(&md),
        None => env::set_current_dir(&config.root()),
    };

    debug!("Current working dir: {:?}", env::current_dir());

    let mut envars = get_env_vars(test, config);

    let mut argv = vec![];
    if ignored {
        argv.push("--ignored".to_string());
    }
    argv.extend_from_slice(&config.varargs);
    if config.color != Color::Auto {
        argv.push("--color".to_string());
        argv.push(config.color.to_string().to_ascii_lowercase());
    }
    let no_test_env = if let Ok(threads) = env::var("RUST_TEST_THREADS") {
        envars.push(("RUST_TEST_THREADS".to_string(), threads));
        false
    } else {
        true
    };

    if no_test_env
        && test.is_test_type()
        && !config.implicit_test_threads
        && !config.varargs.iter().any(|x| x.contains("--test-threads"))
    {
        if let Some(threads) = num_threads {
            argv.push("--test-threads".to_string());
            argv.push(threads.to_string());
        }
    }

    match config.engine() {
        TraceEngine::Llvm => {
            info!("Setting LLVM_PROFILE_FILE");
            // Used for llvm coverage to avoid report naming clashes TODO could have clashes
            // between runs
            envars.push((
                "LLVM_PROFILE_FILE".to_string(),
                format!("{}_%p.profraw", test.file_name()),
            ));
            debug!("Env vars: {:?}", envars);
            debug!("Args: {:?}", argv);
            let mut child = Command::new(test.path());
            child.envs(envars).args(&argv);

            let hnd = RunningProcessHandle::new(test.path().to_path_buf(), &mut child, config)?;
            Ok(hnd.into())
        }
        #[cfg(target_os = "linux")]
        TraceEngine::Ptrace => {
            argv.insert(0, test.path().display().to_string());
            debug!("Env vars: {:?}", envars);
            debug!("Args: {:?}", argv);
            execute(test.path(), &argv, envars.as_slice())
        }
        e => Err(RunError::Engine(format!(
            "invalid execution engine {:?}",
            e
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn check_ld_library_path_correct() {
        let mut binary = TestBinary::new(PathBuf::from("dummy"), None);
        let default_config = Config::default();

        let vars = get_env_vars(&binary, &default_config);

        if let Some(ld) = vars
            .iter()
            .find(|(key, _)| key == "LD_LIBRARY_PATH")
            .map(|(_, val)| val)
        {
            let sys = env::var("LD_LIBRARY_PATH").unwrap();
            assert_eq!(ld, &sys);
        }

        binary
            .linker_paths
            .push(PathBuf::from("/usr/local/lib/foo"));

        let vars = get_env_vars(&binary, &default_config);
        let res = vars
            .iter()
            .find(|(key, _)| key == "LD_LIBRARY_PATH")
            .map(|(_, val)| val);

        assert!(res.is_some());
        let res = res.unwrap();
        assert!(res.contains("/usr/local/lib/foo"));
    }
}
