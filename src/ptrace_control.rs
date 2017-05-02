use std::ptr;
use nix::sys::ptrace::ptrace;
use nix::sys::ptrace::ptrace::*;
use nix::libc::{pid_t, c_void, c_long};
use nix::Result;



const RIP: u8 = 128;


pub fn continue_exec(pid: pid_t) -> Result<c_long> {
    ptrace(PTRACE_CONT, pid, ptr::null_mut(), ptr::null_mut()) 
}

pub fn step(pid: pid_t) -> Result<c_long> {
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
