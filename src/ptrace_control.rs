use std::ptr;
use nix::sys::ptrace::{ptrace, ptrace_setoptions};
use nix::sys::ptrace::ptrace::*;
use nix::libc::{pid_t, c_void, c_long, c_ulonglong, siginfo_t};
use nix::Result;
use std::mem;
use std::fmt;


#[derive(Default, Debug)]
pub struct Regs {
    pub r15: c_ulonglong,
    pub r14: c_ulonglong,
    pub r13: c_ulonglong,
    pub r12: c_ulonglong,
    pub rbp: c_ulonglong,
    pub rbx: c_ulonglong,
    pub r11: c_ulonglong,
    pub r10: c_ulonglong,
    pub r9: c_ulonglong,
    pub r8: c_ulonglong,
    pub rax: c_ulonglong,
    pub rcx: c_ulonglong,
    pub rdx: c_ulonglong,
    pub rsi: c_ulonglong,
    pub rdi: c_ulonglong,
    pub orig_rax: c_ulonglong,
    pub rip: c_ulonglong,
    pub cs: c_ulonglong,
    pub eflags: c_ulonglong,
    pub rsp: c_ulonglong,
    pub ss: c_ulonglong,
    pub fs_base: c_ulonglong,
    pub gs_base: c_ulonglong,
    pub ds: c_ulonglong,
    pub es: c_ulonglong,
    pub fs: c_ulonglong,
    pub gs: c_ulonglong,
}

impl fmt::Display for Regs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "Registers are:\n================================")?;
        writeln!(f, "r15\t\tx{:x}", self.r15 as u64)?;
        writeln!(f, "r14\t\tx{:x}", self.r14)?;
        writeln!(f, "r13\t\tx{:x}", self.r13)?;
        writeln!(f, "r12\t\tx{:x}", self.r12)?;
        writeln!(f, "rbp\t\tx{:x}", self.rbp)?;
        writeln!(f, "rbx\t\tx{:x}", self.rbx)?;
        writeln!(f, "r11\t\tx{:x}", self.r11)?;
        writeln!(f, "r10\t\tx{:x}", self.r10)?;
        writeln!(f, "r9\t\tx{:x}", self.r9)?;
        writeln!(f, "r8\t\tx{:x}", self.r8)?;
        writeln!(f, "rax\t\tx{:x}", self.rax)?;
        writeln!(f, "rcx\t\tx{:x}", self.rcx)?;
        writeln!(f, "rdx\t\tx{:x}", self.rdx)?;
        writeln!(f, "rsi\t\tx{:x}", self.rsi)?;
        writeln!(f, "rdi\t\tx{:x}", self.rdi)?;
        writeln!(f, "orig_rax\t\t0x{:x}", self.orig_rax)?;
        writeln!(f, "rip\t\t0x{:x}", self.rip)?;
        writeln!(f, "cs\t\tx{:x}", self.cs)?;
        writeln!(f, "eflags\t\tx{:x}", self.eflags)?;
        writeln!(f, "rsp\t\tx{:x}", self.rsp)?;
        writeln!(f, "ss\t\tx{:x}", self.ss)?;
        writeln!(f, "fs_base\t\tx{:x}", self.fs_base)?;
        writeln!(f, "gs_base\t\tx{:x}", self.gs_base)?;
        writeln!(f, "ds\t\tx{:x}", self.ds)?;
        writeln!(f, "es\t\tx{:x}", self.es)?;
        writeln!(f, "fs\t\tx{:x}", self.fs)?;
        writeln!(f, "gs\t\tx{:x}", self.gs)?;
        writeln!(f, "================================")
    }
}


const RIP: u8 = 128;


pub fn trace_children(pid: pid_t) -> Result<()> {
    //TODO need to check support.
    let options: PtraceOptions = PTRACE_O_TRACECLONE | PTRACE_O_TRACEFORK | PTRACE_O_TRACEVFORK;
    ptrace_setoptions(pid, options)
}


pub fn report_regs(pid: pid_t) -> Box<Regs> {
    let data = Box::new( Regs{..Default::default()});
    let temp: *mut c_void = unsafe { mem::transmute(data) };
    let _ = ptrace(PTRACE_GETREGS, pid, ptr::null_mut(), temp);
    let data: Box<Regs> = unsafe { mem::transmute(temp) };

    data
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


pub fn get_signal_info(pid: pid_t) -> Result<siginfo_t> {
    let data: Box<siginfo_t> = Box::new( unsafe { mem::uninitialized() });
    let temp: *mut c_void = unsafe { mem::transmute(data) };
    ptrace(PTRACE_GETSIGINFO, pid, ptr::null_mut(), temp)?;
    let data: Box<siginfo_t> = unsafe { mem::transmute(temp) };
    Ok(*data)
}
