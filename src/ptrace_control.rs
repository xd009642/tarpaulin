use nix::errno::Errno;
use nix::libc::{c_long, c_void};
use nix::sys::ptrace::*;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
use nix::{Error, Result};
use std::ptr;

const RIP: u8 = 128;

pub fn trace_children(pid: Pid) -> Result<()> {
    //TODO need to check support.
    let options: Options = Options::PTRACE_O_TRACESYSGOOD
        | Options::PTRACE_O_TRACEEXEC
        | Options::PTRACE_O_TRACEEXIT
        | Options::PTRACE_O_TRACECLONE
        | Options::PTRACE_O_TRACEFORK
        | Options::PTRACE_O_TRACEVFORK;
    setoptions(pid, options)
}

pub fn detach_child(pid: Pid) -> Result<()> {
    detach(pid, None)
}

pub fn continue_exec(pid: Pid, sig: Option<Signal>) -> Result<()> {
    cont(pid, sig)
}

#[allow(deprecated)]
pub fn single_step(pid: Pid) -> Result<()> {
    step(pid, None)
}

pub fn read_address(pid: Pid, address: u64) -> Result<c_long> {
    read(pid, address as AddressType)
}

pub fn write_to_address(pid: Pid, address: u64, data: i64) -> Result<()> {
    write(pid, address as AddressType, data as *mut c_void)
}

#[allow(deprecated)]
pub fn current_instruction_pointer(pid: Pid) -> Result<c_long> {
    let ret = unsafe {
        Errno::clear();
        libc::ptrace(
            Request::PTRACE_PEEKUSER as RequestType,
            libc::pid_t::from(pid),
            RIP as *mut c_void,
            ptr::null_mut() as *mut c_void,
        )
    };
    match Errno::result(ret) {
        Ok(..) | Err(Error::Sys(Errno::UnknownErrno)) => Ok(ret),
        err @ Err(..) => err,
    }
}

#[allow(deprecated)]
pub fn set_instruction_pointer(pid: Pid, pc: u64) -> Result<c_long> {
    unsafe {
        ptrace(
            Request::PTRACE_POKEUSER,
            pid,
            RIP as *mut c_void,
            pc as *mut c_void,
        )
    }
}

pub fn request_trace() -> Result<()> {
    traceme()
}

pub fn get_event_data(pid: Pid) -> Result<c_long> {
    getevent(pid)
}
