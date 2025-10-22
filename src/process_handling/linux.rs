use crate::cargo::{CargoConfigFields, TestBinary};
use crate::config::types::Mode;
use crate::errors::*;
use crate::process_handling::execute_test;
use crate::ptrace_control::*;
use crate::Config;
use crate::TestHandle;
use std::sync::LazyLock;
use nix::sched::*;
use nix::sys::personality;
use nix::unistd::*;
use std::ffi::{CStr, CString};
use std::path::Path;
use std::rc::Rc;
use tracing::{info, warn};

static NUM_CPUS: LazyLock<usize> = LazyLock::new(|| num_cpus::get());

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(
    test: &TestBinary,
    config: &Config,
    cargo_config: Rc<CargoConfigFields>,
    ignored: bool,
) -> Result<Option<TestHandle>, RunError> {
    if !test.path().exists() {
        warn!("Test at {} doesn't exist", test.path().display());
        return Ok(None);
    }

    // Solves CI issue when fixing #953 and #966 in PR #962
    let threads = if config.follow_exec { 1 } else { *NUM_CPUS };

    if let Err(e) = limit_affinity() {
        warn!("Failed to set processor affinity {}", e);
    }

    unsafe {
        match fork() {
            Ok(ForkResult::Parent { child }) => Ok(Some(TestHandle::Id(child))),
            Ok(ForkResult::Child) => {
                let bin_type = match config.command {
                    Mode::Test => "test",
                    Mode::Build => "binary",
                };
                info!("Launching {}", bin_type);
                execute_test(test, &[], ignored, config, cargo_config, Some(threads))?;
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

fn disable_aslr() -> nix::Result<()> {
    let this = personality::get()?;
    personality::set(this | personality::Persona::ADDR_NO_RANDOMIZE).map(|_| ())
}

fn is_aslr_enabled() -> bool {
    !personality::get()
        .map(|x| x.contains(personality::Persona::ADDR_NO_RANDOMIZE))
        .unwrap_or(true)
}

pub fn limit_affinity() -> nix::Result<()> {
    let this = Pid::this();
    // Get current affinity to be able to limit the cores to one of
    // those already in the affinity mask.
    let affinity = sched_getaffinity(this)?;
    let mut selected_cpu = 0;
    for i in 0..CpuSet::count() {
        if affinity.is_set(i)? {
            selected_cpu = i;
            break;
        }
    }
    let mut cpu_set = CpuSet::new();
    cpu_set.set(selected_cpu)?;
    sched_setaffinity(this, &cpu_set)
}

pub fn execute(
    test: &Path,
    argv: &[String],
    envar: &[(String, String)],
) -> Result<TestHandle, RunError> {
    let program = CString::new(test.display().to_string()).unwrap_or_default();
    if is_aslr_enabled() {
        disable_aslr().map_err(|e| RunError::TestRuntime(format!("ASLR disable failed: {e}")))?;
    }
    request_trace().map_err(|e| RunError::Trace(e.to_string()))?;

    let envar = envar
        .iter()
        .map(|(k, v)| CString::new(format!("{k}={v}").as_str()).unwrap_or_default())
        .collect::<Vec<CString>>();

    let argv = argv
        .iter()
        .map(|x| CString::new(x.as_str()).unwrap_or_default())
        .collect::<Vec<CString>>();

    let arg_ref = argv.iter().map(AsRef::as_ref).collect::<Vec<&CStr>>();
    let env_ref = envar.iter().map(AsRef::as_ref).collect::<Vec<&CStr>>();
    execve(&program, &arg_ref, &env_ref).map_err(|_| RunError::Internal)?;

    unreachable!();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_disable_aslr() {
        assert!(is_aslr_enabled());
        disable_aslr().unwrap();
        assert!(!is_aslr_enabled());
    }
}
