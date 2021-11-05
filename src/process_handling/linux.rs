use crate::config::types::Mode;
use crate::errors::*;
use crate::process_handling::execute_test;
use crate::ptrace_control::*;
use crate::Config;
use crate::TestBinary;
use crate::TestHandle;
use nix::errno::Errno;
use nix::libc::{c_int, c_long};
use nix::sched::*;
use nix::unistd::*;
use std::ffi::{CStr, CString};
use std::path::Path;
use tracing::{info, warn};

#[cfg(any(
    target_arch = "x86",
    target_arch = "x86_64",
    target_arch = "arm",
    target_arch = "aarch64"
))]
type Persona = c_long;

const ADDR_NO_RANDOMIZE: Persona = 0x004_0000;
const GET_PERSONA: Persona = 0xFFFF_FFFF;

mod ffi {
    use nix::libc::{c_int, c_long};

    extern "C" {
        pub fn personality(persona: c_long) -> c_int;
    }
}

/// Returns the coverage statistics for a test executable in the given workspace
pub fn get_test_coverage(
    test: &TestBinary,
    config: &Config,
    ignored: bool,
) -> Result<Option<TestHandle>, RunError> {
    if !test.path().exists() {
        warn!("Test at {} doesn't exist", test.path().display());
        return Ok(None);
    }
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
                execute_test(test, ignored, config)?;
                Ok(None)
            }
            Err(err) => Err(RunError::TestCoverage(format!(
                "Failed to run test {}, Error: {}",
                test.path().display(),
                err.to_string()
            ))),
        }
    }
}

fn personality(persona: Persona) -> nix::Result<c_int> {
    let ret = unsafe {
        Errno::clear();
        ffi::personality(persona)
    };
    match Errno::result(ret) {
        Ok(..) | Err(Errno::UnknownErrno) => Ok(ret),
        err @ Err(..) => err,
    }
}

fn disable_aslr() -> nix::Result<i32> {
    match personality(GET_PERSONA) {
        Ok(p) => match personality(i64::from(p) | ADDR_NO_RANDOMIZE) {
            ok @ Ok(_) => ok,
            err @ Err(..) => err,
        },
        err @ Err(..) => err,
    }
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
    disable_aslr().map_err(|e| RunError::TestRuntime(format!("ASLR disable failed: {}", e)))?;

    request_trace().map_err(|e| RunError::Trace(e.to_string()))?;

    let envar = envar
        .iter()
        .map(|(k, v)| CString::new(format!("{}={}", k, v).as_str()).unwrap_or_default())
        .collect::<Vec<CString>>();

    let argv = argv
        .iter()
        .map(|x| CString::new(x.as_str()).unwrap_or_default())
        .collect::<Vec<CString>>();

    let arg_ref = argv.iter().map(|x| x.as_ref()).collect::<Vec<&CStr>>();
    let env_ref = envar.iter().map(|x| x.as_ref()).collect::<Vec<&CStr>>();
    execve(&program, &arg_ref, &env_ref).map_err(|_| RunError::Internal)?;

    unreachable!();
}
