
use std::ptr;
use nix::sys::ptrace::ptrace;
use nix::sys::ptrace::ptrace::*;
use nix::libc::{pid_t, c_void, c_long};
use nix::{Result, Error, Errno};

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
    /// Bottom byte of address data. 
    /// This is replaced to enable the interrupt. Rest of data is never changed.
    data: u8,
}

impl Breakpoint {
    
    pub fn new(pid:pid_t, pc:u64) ->Result<Breakpoint> {
        let mut b = Breakpoint{ 
            pid:pid,
            pc:pc,
            data:0x00,
        };
        match b.enable() {
            Ok(_) => Ok(b),
            Err(e) => Err(e)
        }
    }

    /// Attaches the current breakpoint.
    fn enable(&mut self) -> Result<c_long> {
        let raw_addr = self.pc as * mut c_void;
        let data = ptrace(PTRACE_PEEKDATA, self.pid, 
                          raw_addr, ptr::null_mut())?;

        self.data = (data & 0xFF) as u8;
        let intdata = (data & !(0xFF as i64)) | (INT as i64);
        let intdata = intdata as * mut c_void;
        ptrace(PTRACE_POKEDATA, self.pid, raw_addr, intdata) 
    }
    
    fn disable(&self) -> Result<c_long> {
        let raw_addr = self.pc as * mut c_void;
        let data = ptrace(PTRACE_PEEKDATA, self.pid, 
                          raw_addr, ptr::null_mut())?;
        
        let orgdata = data & !(0xFF as i64) | (self.data as i64);
        let orgdata = orgdata as * mut c_void;
        ptrace(PTRACE_POKEDATA, self.pid, raw_addr, orgdata) 
    }

    /// Steps past the current breakpoint.
    /// For more advanced coverage may interrogate the variables of a branch.
    pub fn step(&mut self) -> Result<c_long> {
        self.disable()?;
        // Need to set the program counter back one. 
        self.enable()?;
        Err(Error::Sys(Errno::UnknownErrno))
    }
}
