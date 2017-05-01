
use std::ptr;
use nix::sys::ptrace::ptrace;
use nix::sys::ptrace::ptrace::*;
use nix::libc::{pid_t, c_void, c_long};
use nix::Result;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const INT: u64 = 0xCC;

const RIP: u8 = 128;

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
    data: i64,
    /// Reading from memory with ptrace gives addresses aligned to bytes. 
    /// We therefore need to know the shift to place the breakpoint in the right place
    shift: u64,
}

impl Breakpoint {
    
    pub fn new(pid:pid_t, pc:u64) ->Result<Breakpoint> {
        let aligned = pc & !0x7u64;
        let data = ptrace(PTRACE_PEEKDATA, pid, aligned as * mut c_void, ptr::null_mut())?;
        let mut b = Breakpoint{ 
            pid: pid,
            pc: pc,
            data: data,
            shift: 8 * (pc - aligned)
        };
        match b.enable() {
            Ok(_) => Ok(b),
            Err(e) => Err(e)
        }
    }

    /// Attaches the current breakpoint.
    fn enable(&mut self) -> Result<c_long> {
        let mut intdata = self.data & (!(0xFFu64 << self.shift) as i64);
        intdata |= (INT << self.shift) as i64;
        let intdata = intdata as * mut c_void;
        
        ptrace(PTRACE_POKEDATA, self.pid, self.pc as * mut c_void, intdata) 
    }
    
    fn disable(&self) -> Result<c_long> {
        let raw_addr = self.aligned_address() as * mut c_void;
        ptrace(PTRACE_POKEDATA, self.pid, raw_addr, self.data as * mut c_void) 
    }

    /// Steps past the current breakpoint.
    /// For more advanced coverage may interrogate the variables of a branch.
    pub fn step(&mut self) -> Result<c_long> {
        self.disable()?;
        // Need to set the program counter back one. 
        ptrace(PTRACE_POKEUSER, self.pid, RIP as * mut c_void, self.pc as * mut c_void)?;
        ptrace(PTRACE_SINGLESTEP, self.pid, ptr::null_mut(), ptr::null_mut())?;
        self.enable()
    }

    fn aligned_address(&self) -> u64 {
        self.pc & !0x7u64
    }
}
