use std::ptr;
use nix::sys::ptrace::ptrace;
use nix::sys::ptrace::ptrace::*;
use nix::libc::{pid_t, c_void};


#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const INT: u8 = 0xCC;

/// Breakpoint construct used to monitor program execution. As tarpaulin is an
/// automated process, this will likely have less functionality than most 
/// breakpoint implementations.
#[derive(Debug)]
pub struct Breakpoint { 
    /// Current process id
    pub pid: pid_t,
    /// Program counter
    pub pc: u64,
    /// Address of the breakpoint, used to attach interrupt to memory
    pub address: isize,
    /// Bottom byte of address data. 
    /// This is replaced to enable the interrupt. Rest of data is never changed.
    pub data: u8,
}

impl Breakpoint {
    
    /// Attaches the current breakpoint.
    pub fn enable(&mut self) {
        let raw_addr = self.address as * mut c_void;
        if let Ok(data) = ptrace(PTRACE_PEEKDATA, self.pid, raw_addr, ptr::null_mut()) {
            self.data = (data & 0xFF) as u8;
            let intdata = (data & !(0xFF as i64)) | (INT as i64);
            let intdata = intdata as * mut c_void;
            if let Err(e) = ptrace(PTRACE_POKEDATA, self.pid, raw_addr, intdata) {
                println!("WARNING: Couldn't instrument code {}", e);   
            }
        }
    }
    
    fn disable(&self) {
        let raw_addr = self.address as * mut c_void;
        if let Ok(data) = ptrace(PTRACE_PEEKDATA, self.pid, raw_addr, ptr::null_mut()) {
            let orgdata = data & !(0xFF as i64) | (self.data as i64);
            let orgdata = orgdata as * mut c_void;
            if let Err(e) = ptrace(PTRACE_POKEDATA, self.pid, raw_addr, orgdata) {
                println!("WARNING: Couldn't restore data {}", e);   
            }
        }
    }

    /// Steps past the current breakpoint.
    /// For more advanced coverage may interrogate the variables of a branch.
    pub fn step(&mut self) {
        self.disable();
        // Need to set the program counter back one. 
        self.enable();
        panic!("Unimplemented!");
    }
}
