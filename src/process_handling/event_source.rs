use crate::cargo::rust_flags;
use crate::config::Config;
use crate::statemachine::*;
use crate::{errors::RunError, process_handling::ptrace_control::*};
use libc::c_long;
use nix::{
    sys::{
        signal::Signal,
        wait::{waitpid, WaitPidFlag, WaitStatus},
    },
    unistd::Pid,
};
use procfs::process::{MMapPath, Process};
use tracing::info;

pub trait EventSource {
    fn request_trace(&self) -> nix::Result<()>;
    fn get_offset(&self, pid: Pid, config: &Config) -> u64;
    fn trace_children(&self, pid: Pid) -> nix::Result<()>;
    /// This will be at the start of every wait/handle/continue loop so can be used to pop out and
    /// apply state changes which occur when tests are running for any mock event sources. In real
    /// life the actual processes under test will be modifying things!
    fn waitpid(&self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus>;
    fn next_events(&self, wait_queue: &mut Vec<WaitStatus>) -> Result<Option<TestState>, RunError>;
    fn get_event_data(&self, pid: Pid) -> nix::Result<c_long>;
    fn continue_pid(&self, pid: Pid, signal: Option<Signal>) -> nix::Result<()>;
    fn single_step(&self, pid: Pid) -> nix::Result<()>;
    fn detach_child(&self, pid: Pid) -> nix::Result<()>;
    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long>;
    fn set_current_instruction_pointer(&self, pid: Pid, addr: u64) -> nix::Result<c_long>;
    fn write_address(&self, pid: Pid, addr: u64, data: c_long) -> nix::Result<()>;
    fn read_address(&self, pid: Pid, addr: u64) -> nix::Result<c_long>;

    fn get_tids(&self, pid: Pid) -> Box<dyn Iterator<Item = Pid> + '_>;
    fn get_ppid(&self, pid: Pid) -> Option<Pid>;
}

#[derive(Clone)]
pub struct PtraceEventSource;

impl EventSource for PtraceEventSource {
    fn request_trace(&self) -> nix::Result<()> {
        request_trace()
    }

    fn get_offset(&self, pid: Pid, config: &Config) -> u64 {
        // TODO I don't think this would be an issue... but I'll test it
        if rust_flags(config, &Default::default()).contains("dynamic-no-pic") {
            0
        } else if let Ok(proc) = Process::new(pid.as_raw()) {
            let exe = proc.exe().ok();
            if let Ok(maps) = proc.maps() {
                let offset_info = maps.iter().find(|x| match (&x.pathname, exe.as_ref()) {
                    (MMapPath::Path(p), Some(e)) => p == e,
                    (MMapPath::Path(_), None) => true,
                    _ => false,
                });
                if let Some(first) = offset_info {
                    first.address.0
                } else {
                    0
                }
            } else {
                0
            }
        } else {
            0
        }
    }

    fn trace_children(&self, pid: Pid) -> nix::Result<()> {
        trace_children(pid)
    }

    fn waitpid(&self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        waitpid(pid, options)
    }

    fn next_events(&self, wait_queue: &mut Vec<WaitStatus>) -> Result<Option<TestState>, RunError> {
        let mut result = Ok(None);
        let mut running = true;
        while running {
            let wait = self.waitpid(
                Pid::from_raw(-1),
                Some(WaitPidFlag::WNOHANG | WaitPidFlag::__WALL),
            );
            match wait {
                Ok(WaitStatus::StillAlive) => {
                    running = false;
                }
                Ok(WaitStatus::Exited(_, _)) => {
                    wait_queue.push(wait.unwrap());
                    result = Ok(Some(TestState::Stopped));
                    running = false;
                }
                Ok(WaitStatus::PtraceEvent(_, _, _)) => {
                    wait_queue.push(wait.unwrap());
                    result = Ok(Some(TestState::Stopped));
                    running = false;
                }
                Ok(s) => {
                    wait_queue.push(s);
                    result = Ok(Some(TestState::Stopped));
                }
                Err(e) => {
                    running = false;
                    result = Err(RunError::TestRuntime(format!(
                        "An error occurred while waiting for response from test: {e}"
                    )));
                }
            }
        }
        result
    }

    fn get_event_data(&self, pid: Pid) -> nix::Result<c_long> {
        get_event_data(pid)
    }

    fn continue_pid(&self, pid: Pid, signal: Option<Signal>) -> nix::Result<()> {
        continue_exec(pid, signal)
    }

    fn single_step(&self, pid: Pid) -> nix::Result<()> {
        single_step(pid)
    }

    fn detach_child(&self, pid: Pid) -> nix::Result<()> {
        detach_child(pid)
    }

    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long> {
        current_instruction_pointer(pid)
    }

    fn set_current_instruction_pointer(&self, pid: Pid, addr: u64) -> nix::Result<c_long> {
        set_instruction_pointer(pid, addr)
    }

    fn write_address(&self, pid: Pid, address: u64, data: c_long) -> nix::Result<()> {
        write_to_address(pid, address, data)
    }

    fn read_address(&self, pid: Pid, address: u64) -> nix::Result<c_long> {
        read_address(pid, address)
    }

    fn get_tids(&self, pid: Pid) -> Box<dyn Iterator<Item = Pid> + '_> {
        if let Some(proc) = Process::new(pid.as_raw()).ok().and_then(|x| x.tasks().ok()) {
            Box::new(proc.filter_map(Result::ok).map(|x| Pid::from_raw(x.tid)))
        } else {
            Box::new(std::iter::empty())
        }
    }

    fn get_ppid(&self, pid: Pid) -> Option<Pid> {
        let proc = Process::new(pid.as_raw()).ok()?;
        if let Ok(status) = proc.status() {
            info!("Found potential parent");
            let pid = Pid::from_raw(status.ppid);
            Some(pid)
        } else {
            None
        }
    }
}
