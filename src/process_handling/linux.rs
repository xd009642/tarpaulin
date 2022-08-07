use crate::config::types::Mode;
use crate::errors::RunError;
use crate::process_handling::execute_test;
use crate::ptrace_control::*;
use crate::{Config, TestBinary, TestHandle};
use nix::sys::personality;
use nix::{sched, unistd};
use std::ffi::{CStr, CString};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(
    test: &TestBinary,
    config: &Config,
    ignored: bool,
) -> Result<Option<TestHandle>, RunError> {
    if !test.path().exists() {
        tracing::warn!("Test at {} doesn't exist", test.path().display());
        return Ok(None);
    }

    // Solves CI issue when fixing #953 and #966 in PR #962
    let threads = if config.follow_exec { 1 } else { sched::CpuSet::count() };

    if let Err(e) = limit_affinity() {
        tracing::warn!("Failed to set processor affinity {}", e);
    }

    unsafe {
        match unistd::fork() {
            Ok(unistd::ForkResult::Parent { child }) => Ok(Some(TestHandle::Id(child))),
            Ok(unistd::ForkResult::Child) => {
                let bin_type = match config.command {
                    Mode::Test => "test",
                    Mode::Build => "binary",
                };
                tracing::info!("Launching {}", bin_type);
                execute_test(test, ignored, config, Some(threads))?;
                Ok(None)
            }
            Err(err) => Err(RunError::TestCoverage(format!(
                "Failed to run test {}, Error: {}",
                test.path().display(),
                err
            ))),
        }
    }
}

fn disable_aslr() -> nix::Result<personality::Persona> {
    let this = personality::get()?;
    nix::sys::personality::set(this | personality::Persona::ADDR_NO_RANDOMIZE)
}

pub fn limit_affinity() -> nix::Result<()> {
    let this = unistd::Pid::this();
    // Get current affinity to be able to limit the cores to one of
    // those already in the affinity mask.
    let affinity = sched::sched_getaffinity(this)?;
    let mut selected_cpu = 0;
    for i in 0..sched::CpuSet::count() {
        if affinity.is_set(i)? {
            selected_cpu = i;
            break;
        }
    }
    let mut cpu_set = sched::CpuSet::new();
    cpu_set.set(selected_cpu)?;
    sched::sched_setaffinity(this, &cpu_set)
}

pub fn execute(
    test: &Path,
    argv: &[String],
    envar: &[(String, String)],
) -> Result<TestHandle, RunError> {
    let program = CString::new(&*test.as_os_str().as_bytes()).unwrap_or_default();
    disable_aslr().map_err(|e| RunError::TestRuntime(format!("ASLR disable failed: {}", e)))?;

    request_trace().map_err(|e| RunError::Trace(e.to_string()))?;

    let envar = envar
        .iter()
        .map(|(k, v)| CString::new(format!("{}={}", k, v).as_str()).unwrap_or_default())
        .collect::<Vec<_>>();

    let argv = argv
        .iter()
        .map(|x| CString::new(x.as_str()).unwrap_or_default())
        .collect::<Vec<_>>();

    let arg_ref = argv.iter().map(AsRef::as_ref).collect::<Vec<&CStr>>();
    let env_ref = envar.iter().map(AsRef::as_ref).collect::<Vec<&CStr>>();
    unistd::execve(&program, &arg_ref, &env_ref).map_err(|_| RunError::Internal)?;

    unreachable!();
}
