use nix::Result;
use nix::errno::Errno;
#[cfg(target_arch = "aarch64")]
use nix::libc::{NT_PRSTATUS, iovec};
use nix::libc::{c_long, c_void};
use nix::sys::ptrace::*;
use nix::sys::signal::Signal;
use nix::unistd::Pid;
#[cfg(target_arch = "aarch64")]
use std::mem::MaybeUninit;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
use std::ptr;

#[cfg(target_arch = "x86_64")]
const PC_INDEX: usize = libc::RIP as usize;
#[cfg(target_arch = "x86")]
const PC_INDEX: usize = libc::EIP as usize;

#[cfg(target_arch = "aarch64")]
#[repr(C)]
struct UserPtRegs {
    regs: [u64; 31],
    sp: u64,
    pc: u64,
    pstate: u64,
}

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

pub fn write_to_address(pid: Pid, address: u64, data: c_long) -> Result<()> {
    write(pid, address as AddressType, data)
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(deprecated)]
pub fn current_instruction_pointer(pid: Pid) -> Result<c_long> {
    let ret = unsafe {
        Errno::clear();
        libc::ptrace(
            Request::PTRACE_PEEKUSER as RequestType,
            libc::pid_t::from(pid),
            (PC_INDEX * std::mem::size_of::<usize>()) as *mut c_void,
            ptr::null_mut::<c_void>(),
        )
    };
    match Errno::result(ret) {
        Ok(..) | Err(Errno::UnknownErrno) => Ok(ret),
        err @ Err(..) => err,
    }
}

#[cfg(target_arch = "aarch64")]
pub fn current_instruction_pointer(pid: Pid) -> Result<c_long> {
    let mut regs = MaybeUninit::<UserPtRegs>::uninit();
    let mut iov = iovec {
        iov_base: regs.as_mut_ptr().cast(),
        iov_len: std::mem::size_of::<UserPtRegs>(),
    };
    let ret = unsafe {
        libc::ptrace(
            Request::PTRACE_GETREGSET as RequestType,
            libc::pid_t::from(pid),
            NT_PRSTATUS as *mut c_void,
            (&raw mut iov).cast::<c_void>(),
        )
    };
    Errno::result(ret).map(|_| unsafe { regs.assume_init().pc as c_long })
}

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
#[allow(deprecated)]
pub fn set_instruction_pointer(pid: Pid, pc: u64) -> Result<c_long> {
    let ret = unsafe {
        libc::ptrace(
            Request::PTRACE_POKEUSER as _,
            libc::pid_t::from(pid),
            (PC_INDEX * std::mem::size_of::<usize>()) as *mut c_void,
            pc as *mut c_void,
        )
    };
    Errno::result(ret).map(|_| 0)
}

#[cfg(target_arch = "aarch64")]
pub fn set_instruction_pointer(pid: Pid, pc: u64) -> Result<c_long> {
    let mut regs = MaybeUninit::<UserPtRegs>::uninit();
    let mut iov = iovec {
        iov_base: regs.as_mut_ptr().cast(),
        iov_len: std::mem::size_of::<UserPtRegs>(),
    };
    let ret = unsafe {
        libc::ptrace(
            Request::PTRACE_GETREGSET as RequestType,
            libc::pid_t::from(pid),
            NT_PRSTATUS as *mut c_void,
            (&raw mut iov).cast::<c_void>(),
        )
    };
    Errno::result(ret)?;

    let mut regs = unsafe { regs.assume_init() };
    let old_pc = regs.pc;
    regs.pc = pc;
    let mut iov = iovec {
        iov_base: (&raw mut regs).cast::<c_void>(),
        iov_len: std::mem::size_of::<UserPtRegs>(),
    };
    let ret = unsafe {
        libc::ptrace(
            Request::PTRACE_SETREGSET as RequestType,
            libc::pid_t::from(pid),
            NT_PRSTATUS as *mut c_void,
            (&raw mut iov).cast::<c_void>(),
        )
    };
    Errno::result(ret).map(|_| old_pc as c_long)
}

pub fn request_trace() -> Result<()> {
    traceme()
}

pub fn get_event_data(pid: Pid) -> Result<c_long> {
    getevent(pid)
}
