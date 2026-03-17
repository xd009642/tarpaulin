use crate::statemachine::TestState;
use crate::{errors::RunError, process_handling::ptrace_control::*};
use libc::c_long;
use nix::{
    sys::{
        ptrace::detach,
        signal::Signal,
        wait::{waitpid, WaitPidFlag, WaitStatus},
    },
    unistd::Pid,
};

pub trait EventSource {
    fn request_trace(&mut self) -> nix::Result<()>;
    fn waitpid(&mut self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus>;
    fn next_events(
        &mut self,
        wait_queue: &mut Vec<WaitStatus>,
    ) -> Result<Option<TestState>, RunError>;
    fn continue_pid(&mut self, pid: Pid, signal: Option<Signal>) -> nix::Result<()>;
    fn single_step(&mut self, pid: Pid) -> nix::Result<()>;
    fn detach_child(&mut self, pid: Pid) -> nix::Result<()>;
    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long>;
    fn set_current_instruction_pointer(&mut self, pid: Pid, addr: u64) -> nix::Result<c_long>;
    fn poke_text(&mut self, pid: Pid, addr: u64, data: c_long) -> nix::Result<()>;
    fn peek_text(&mut self, pid: Pid, addr: u64) -> nix::Result<c_long>;
}

pub struct PtraceEventSource;

impl EventSource for PtraceEventSource {
    fn request_trace(&mut self) -> nix::Result<()> {
        request_trace()
    }

    fn waitpid(&mut self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        waitpid(pid, options)
    }

    fn next_events(
        &mut self,
        wait_queue: &mut Vec<WaitStatus>,
    ) -> Result<Option<TestState>, RunError> {
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

    fn continue_pid(&mut self, pid: Pid, signal: Option<Signal>) -> nix::Result<()> {
        continue_exec(pid, signal)
    }

    fn single_step(&mut self, pid: Pid) -> nix::Result<()> {
        single_step(pid)
    }

    fn detach_child(&mut self, pid: Pid) -> nix::Result<()> {
        detach_child(pid)
    }

    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long> {
        current_instruction_pointer(pid)
    }

    fn set_current_instruction_pointer(&mut self, pid: Pid, addr: u64) -> nix::Result<c_long> {
        set_instruction_pointer(pid, addr)
    }

    fn poke_text(&mut self, pid: Pid, address: u64, data: c_long) -> nix::Result<()> {
        write_to_address(pid, address, data)
    }

    fn peek_text(&mut self, pid: Pid, address: u64) -> nix::Result<c_long> {
        read_address(pid, address)
    }
}

pub struct ReplayEventSource {}

impl EventSource for ReplayEventSource {
    fn request_trace(&mut self) -> nix::Result<()> {
        Ok(())
    }

    fn waitpid(&mut self, pid: Pid, options: Option<WaitPidFlag>) -> nix::Result<WaitStatus> {
        todo!()
    }

    fn next_events(
        &mut self,
        wait_queue: &mut Vec<WaitStatus>,
    ) -> Result<Option<TestState>, RunError> {
        todo!();
    }

    fn continue_pid(&mut self, pid: Pid, signal: Option<Signal>) -> nix::Result<()> {
        todo!();
    }

    fn single_step(&mut self, pid: Pid) -> nix::Result<()> {
        todo!();
    }

    fn detach_child(&mut self, pid: Pid) -> nix::Result<()> {
        todo!()
    }

    fn current_instruction_pointer(&self, pid: Pid) -> nix::Result<c_long> {
        todo!();
    }

    fn set_current_instruction_pointer(&mut self, pid: Pid, addr: u64) -> nix::Result<c_long> {
        todo!();
    }

    fn poke_text(&mut self, pid: Pid, address: u64, data: c_long) -> nix::Result<()> {
        todo!();
    }

    fn peek_text(&mut self, pid: Pid, address: u64) -> nix::Result<c_long> {
        todo!();
    }
}
