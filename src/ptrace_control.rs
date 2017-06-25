use std::ptr;
use nix::sys::ptrace::{ptrace, ptrace_setoptions};
use nix::sys::ptrace::ptrace::*;
use nix::libc::{pid_t, c_void, c_long};
use nix::Result;
use std::mem;


const RIP: u8 = 128;


pub fn trace_children(pid: pid_t) -> Result<()> {
    //TODO need to check support.
    let options: PtraceOptions = PTRACE_O_TRACECLONE | PTRACE_O_TRACEFORK | PTRACE_O_TRACEVFORK;
    ptrace_setoptions(pid, options)
}


pub fn continue_exec(pid: pid_t) -> Result<c_long> {
    ptrace(PTRACE_CONT, pid, ptr::null_mut(), ptr::null_mut()) 
}

pub fn single_step(pid: pid_t) -> Result<c_long> {
    ptrace(PTRACE_SINGLESTEP, pid, ptr::null_mut(), ptr::null_mut())
}

pub fn read_address(pid: pid_t, address:u64) -> Result<c_long> {
    ptrace(PTRACE_PEEKDATA, pid, address as * mut c_void, ptr::null_mut())
}

pub fn write_to_address(pid: pid_t, 
                        address: u64, 
                        data: i64) -> Result<c_long> {
    ptrace(PTRACE_POKEDATA, pid, address as * mut c_void, data as * mut c_void)
}

pub fn current_instruction_pointer(pid: pid_t) -> Result<c_long> {
    ptrace(PTRACE_PEEKUSER, pid, RIP as * mut c_void, ptr::null_mut())
}

pub fn set_instruction_pointer(pid: pid_t, pc: u64) -> Result<c_long> {
    ptrace(PTRACE_POKEUSER, pid, RIP as * mut c_void, pc as * mut c_void)
}

pub fn request_trace() -> Result<c_long> {
    ptrace(PTRACE_TRACEME, 0, ptr::null_mut(), ptr::null_mut())
}

pub fn get_event_data(pid: pid_t) -> Result<c_long> {
    let data = Box::new(0u64 as c_long);
    let temp: *mut c_void = unsafe { mem::transmute(data) };
    let res = ptrace(PTRACE_GETEVENTMSG, pid, ptr::null_mut(), temp);
    match res {
        Ok(_) => { 
            let data: Box<c_long> = unsafe { mem::transmute(temp) };
            Ok(*data)
        },
        err @ Err(..) => err,
    }
}


