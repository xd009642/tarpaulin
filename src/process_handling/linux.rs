use crate::collect_coverage;
use crate::errors::*;
use crate::event_log::*;
use crate::process_handling::execute_test;
use crate::ptrace_control::*;
use crate::source_analysis::LineAnalysis;
use crate::traces::*;
use crate::Config;
use crate::TestBinary;
use nix::errno::Errno;
use nix::libc::{c_int, c_long};
use nix::sched::*;
use nix::unistd::*;
use nix::Error;
use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use tracing::{info, warn};

#[cfg(any(target_arch = "x86", target_arch = "x86_64", target_arch = "arm"))]
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
    analysis: &HashMap<PathBuf, LineAnalysis>,
    config: &Config,
    ignored: bool,
    logger: &Option<EventLog>,
) -> Result<Option<(TraceMap, i32)>, RunError> {
    if !test.path().exists() {
        return Ok(None);
    }
    if let Err(e) = limit_affinity() {
        warn!("Failed to set processor affinity {}", e);
    }
    if let Some(log) = logger.as_ref() {
        log.push_binary(test.clone());
    }
    unsafe {
        match fork() {
            Ok(ForkResult::Parent { child }) => {
                match collect_coverage(test.path(), child, analysis, config, logger) {
                    Ok(t) => Ok(Some(t)),
                    Err(e) => Err(RunError::TestCoverage(e.to_string())),
                }
            }
            Ok(ForkResult::Child) => {
                info!("Launching test");
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
        Ok(..) | Err(Error::Sys(Errno::UnknownErrno)) => Ok(ret),
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
    let mut cpu_set = CpuSet::new();
    cpu_set.set(0)?;
    let this = Pid::this();
    sched_setaffinity(this, &cpu_set)
}

pub fn execute(program: CString, argv: &[CString], envar: &[CString]) -> Result<(), RunError> {
    disable_aslr().map_err(|e| RunError::TestRuntime(format!("ASLR disable failed: {}", e)))?;

    request_trace().map_err(|e| RunError::Trace(e.to_string()))?;

    let arg_ref = argv.iter().map(|x| x.as_ref()).collect::<Vec<&CStr>>();
    let env_ref = envar.iter().map(|x| x.as_ref()).collect::<Vec<&CStr>>();
    execve(&program, &arg_ref, &env_ref)
        .map_err(|_| RunError::Internal)
        .map(|_| ())
}
