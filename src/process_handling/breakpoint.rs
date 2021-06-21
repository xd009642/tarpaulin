use crate::ptrace_control::*;
use crate::statemachine::*;
use nix::unistd::Pid;
use nix::{Error, Result};
use std::collections::HashMap;

#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
const INT: u64 = 0xCC;

/// Breakpoint construct used to monitor program execution. As tarpaulin is an
/// automated process, this will likely have less functionality than most
/// breakpoint implementations.
#[derive(Debug)]
pub struct Breakpoint {
    /// Program counter
    pub pc: u64,
    /// Bottom byte of address data.
    /// This is replaced to enable the interrupt. Rest of data is never changed.
    data: u8,
    /// Reading from memory with ptrace gives addresses aligned to bytes.
    /// We therefore need to know the shift to place the breakpoint in the right place
    shift: u64,
    /// Map of the state of the breakpoint on each thread/process
    is_running: HashMap<Pid, bool>,
}

impl Breakpoint {
    /// Creates a new breakpoint for the given process and program counter.
    pub fn new(pid: Pid, pc: u64) -> Result<Self> {
        let aligned = align_address(pc);
        let data = read_address(pid, aligned)?;
        let shift = 8 * (pc - aligned);
        let data = ((data >> shift) & 0xFF) as u8;

        let mut b = Breakpoint {
            pc,
            data,
            shift,
            is_running: HashMap::new(),
        };
        match b.enable(pid) {
            Ok(_) => Ok(b),
            Err(e) => Err(e),
        }
    }

    pub fn jump_to(&mut self, pid: Pid) -> Result<()> {
        set_instruction_pointer(pid, self.pc).map(|_| ())
    }

    /// Attaches the current breakpoint.
    pub fn enable(&mut self, pid: Pid) -> Result<()> {
        let data = read_address(pid, self.aligned_address())?;
        self.is_running.insert(pid, true);
        let mut intdata = data & (!(0xFFu64 << self.shift) as i64);
        intdata |= (INT << self.shift) as i64;
        if data == intdata {
            Err(Error::UnsupportedOperation)
        } else {
            write_to_address(pid, self.aligned_address(), intdata)
        }
    }

    pub fn disable(&self, pid: Pid) -> Result<()> {
        // I require the bit fiddlin this end.
        let data = read_address(pid, self.aligned_address())?;
        let mut orgdata = data & (!(0xFFu64 << self.shift) as i64);
        orgdata |= i64::from(self.data) << self.shift;
        write_to_address(pid, self.aligned_address(), orgdata)
    }

    /// Processes the breakpoint. This steps over the breakpoint
    pub fn process(
        &mut self,
        pid: Pid,
        reenable: bool,
    ) -> Result<(bool, TracerAction<ProcessInfo>)> {
        let is_running = match self.is_running.get(&pid) {
            Some(r) => *r,
            None => true,
        };
        if is_running {
            let _ = self.enable(pid);
            self.step(pid)?;
            self.is_running.insert(pid, false);
            Ok((true, TracerAction::Step(pid.into())))
        } else {
            self.disable(pid)?;
            if reenable {
                self.enable(pid)?;
            }
            self.is_running.insert(pid, true);
            Ok((false, TracerAction::Continue(pid.into())))
        }
    }

    /// Call this when a ptrace thread is killed. Won't reenable the breakpoint
    /// so may lose the ability to instrument this line.
    pub fn thread_killed(&mut self, pid: Pid) {
        self.is_running.remove(&pid);
    }

    /// Steps past the current breakpoint.
    /// For more advanced coverage may interrogate the variables of a branch.
    fn step(&mut self, pid: Pid) -> Result<()> {
        // Remove the breakpoint, reset the program counter to step before it
        // hit the breakpoint then step to execute the original instruction.
        self.disable(pid)?;
        self.jump_to(pid)?;
        Ok(())
    }

    pub fn aligned_address(&self) -> u64 {
        align_address(self.pc)
    }
}

#[inline(always)]
pub(crate) fn align_address(addr: u64) -> u64 {
    addr & !0x7u64
}
