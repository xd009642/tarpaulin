use crate::statemachine::TestState;
use crate::{errors::RunError, process_handling::ptrace_control::*};
use libc::c_long;
use nix::{
    sys::{
        signal::Signal,
        wait::{waitpid, WaitPidFlag, WaitStatus},
    },
    unistd::Pid,
};

pub trait EventSource {
    fn request_trace(&self) -> nix::Result<()>;
    fn trace_children(&self, pid: Pid) -> nix::Result<()>;
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
}

#[derive(Clone)]
pub struct PtraceEventSource;

impl EventSource for PtraceEventSource {
    fn request_trace(&self) -> nix::Result<()> {
        request_trace()
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
}

#[derive(Clone)]
pub struct ReplayEventSource {}

impl EventSource for ReplayEventSource {
    fn request_trace(&self) -> nix::Result<()> {
        Ok(())
    }

    fn trace_children(&self, pid: Pid) -> nix::Result<()> {
        todo!()
    }

    fn waitpid(&self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        todo!()
    }

    fn next_events(&self, wait_queue: &mut Vec<WaitStatus>) -> Result<Option<TestState>, RunError> {
        todo!();
    }

    fn get_event_data(&self, pid: Pid) -> nix::Result<c_long> {
        todo!()
    }

    fn continue_pid(&self, pid: Pid, signal: Option<Signal>) -> nix::Result<()> {
        todo!();
    }

    fn single_step(&self, pid: Pid) -> nix::Result<()> {
        todo!();
    }

    fn detach_child(&self, pid: Pid) -> nix::Result<()> {
        todo!()
    }

    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long> {
        todo!();
    }

    fn set_current_instruction_pointer(&self, pid: Pid, addr: u64) -> nix::Result<c_long> {
        todo!();
    }

    fn write_address(&self, pid: Pid, address: u64, data: c_long) -> nix::Result<()> {
        todo!();
    }

    fn read_address(&self, pid: Pid, address: u64) -> nix::Result<c_long> {
        todo!();
    }
}
