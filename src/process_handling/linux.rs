use crate::errors::*;
use crate::ptrace_control::*;
use nix::errno::Errno;
use nix::libc::{c_int, c_long};
use nix::sched::*;
use nix::unistd::*;
use nix::Error;
use std::ffi::CString;

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

    execve(&program, argv, envar)
        .map_err(|_| RunError::Internal)
        .map(|_| ())
}
