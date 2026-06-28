use crate::process_handling::event_source::EventSource;
use crate::statemachine::*;
use nix::libc::c_long;
use nix::unistd::Pid;
use nix::{Error, Result};
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

/// INT refers to the software interrupt instruction. For x64/x86 we use INT3 which is a
/// one byte instruction defined for use by debuggers.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) const BREAKPOINT_INSTRUCTION_SIZE: u64 = 1;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) const BREAKPOINT_PC_OFFSET: u64 = BREAKPOINT_INSTRUCTION_SIZE;
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) const BREAKPOINT_INSTRUCTION: u64 = 0xCC;

/// AArch64 BRK #0, encoded as the little-endian instruction word 0xd4200000.
#[cfg(target_arch = "aarch64")]
pub(crate) const BREAKPOINT_INSTRUCTION_SIZE: u64 = 4;
#[cfg(target_arch = "aarch64")]
pub(crate) const BREAKPOINT_PC_OFFSET: u64 = 0;
#[cfg(target_arch = "aarch64")]
pub(crate) const BREAKPOINT_INSTRUCTION: u64 = 0xd420_0000;

pub(crate) const BREAKPOINT_MASK: u64 = (1u64 << (BREAKPOINT_INSTRUCTION_SIZE * 8)) - 1;

/// Breakpoint construct used to monitor program execution. As tarpaulin is an
/// automated process, this will likely have less functionality than most
/// breakpoint implementations.
pub struct Breakpoint {
    /// Program counter
    pub pc: u64,
    /// Original instruction bytes replaced to enable the breakpoint.
    data: u64,
    /// Reading from memory with ptrace gives addresses aligned to bytes.
    /// We therefore need to know the shift to place the breakpoint in the right place
    shift: u64,
    /// Map of the state of the breakpoint on each thread/process
    is_running: HashMap<Pid, bool>,
    /// Handle to process events.
    event_source: Rc<dyn EventSource>,
}

impl fmt::Debug for Breakpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Breakpoint")
            .field("pc", &self.pc)
            .field("data", &self.data)
            .field("shift", &self.shift)
            .field("is_running", &self.is_running)
            .finish()
    }
}

impl Breakpoint {
    /// Creates a new breakpoint for the given process and program counter.
    pub fn new(pid: Pid, pc: u64, event_source: Rc<dyn EventSource>) -> Result<Self> {
        let aligned = align_address(pc);
        let data = event_source.read_address(pid, aligned)?;
        let shift = 8 * (pc - aligned);
        let data = (data as u64 >> shift) & BREAKPOINT_MASK;

        let mut b = Breakpoint {
            pc,
            data,
            shift,
            is_running: HashMap::new(),
            event_source,
        };
        match b.enable(pid) {
            Ok(_) => Ok(b),
            Err(e) => Err(e),
        }
    }

    pub fn jump_to(&mut self, pid: Pid) -> Result<()> {
        self.event_source
            .set_current_instruction_pointer(pid, self.pc)
            .map(|_| ())
    }

    /// Attaches the current breakpoint.
    pub fn enable(&mut self, pid: Pid) -> Result<()> {
        let data = self
            .event_source
            .read_address(pid, self.aligned_address())?;
        self.is_running.insert(pid, true);
        let mut intdata = data as u64 & !(BREAKPOINT_MASK << self.shift);
        intdata |= BREAKPOINT_INSTRUCTION << self.shift;
        let intdata = intdata as c_long;
        if data == intdata {
            Err(Error::UnknownErrno)
        } else {
            self.event_source
                .write_address(pid, self.aligned_address(), intdata)
        }
    }

    pub fn disable(&mut self, pid: Pid) -> Result<()> {
        // I require the bit fiddlin this end.
        let data = self
            .event_source
            .read_address(pid, self.aligned_address())?;
        let mut orgdata = data as u64 & !(BREAKPOINT_MASK << self.shift);
        orgdata |= self.data << self.shift;
        self.event_source
            .write_address(pid, self.aligned_address(), orgdata as c_long)
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

    /// Call this when a ptrace thread is killed. Won't re-enable the breakpoint
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
