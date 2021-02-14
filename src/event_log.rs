use crate::cargo::TestBinary;
use crate::ptrace_control::*;
use crate::statemachine::{ProcessInfo, TracerAction};
use crate::traces::{Location, TraceMap};
use chrono::offset::Local;
use nix::libc::*;
use nix::sys::{signal::Signal, wait::WaitStatus};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fs::File;
use std::path::Path;
use tracing::{info, warn};

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Event {
    ConfigLaunch(String),
    BinaryLaunch(TestBinary),
    Trace(TraceEvent),
}

#[derive(Clone, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TraceEvent {
    pid: Option<pid_t>,
    child: Option<pid_t>,
    signal: Option<String>,
    addr: Option<u64>,
    return_val: Option<i64>,
    location: Option<Location>,
    description: String,
}

impl TraceEvent {
    pub(crate) fn new_from_action(action: &TracerAction<ProcessInfo>) -> Self {
        match *action {
            TracerAction::TryContinue(t) => TraceEvent {
                pid: Some(t.pid.as_raw()),
                signal: t.signal.map(|x| x.to_string()),
                description: "Try continue child".to_string(),
                ..Default::default()
            },
            TracerAction::Continue(t) => TraceEvent {
                pid: Some(t.pid.as_raw()),
                signal: t.signal.map(|x| x.to_string()),
                description: "Continue child".to_string(),
                ..Default::default()
            },
            TracerAction::Step(t) => TraceEvent {
                pid: Some(t.pid.as_raw()),
                description: "Step child".to_string(),
                ..Default::default()
            },
            TracerAction::Detach(t) => TraceEvent {
                pid: Some(t.pid.as_raw()),
                description: "Detach child".to_string(),
                ..Default::default()
            },
            TracerAction::Nothing => TraceEvent {
                description: "Do nothing".to_string(),
                ..Default::default()
            },
        }
    }

    pub(crate) fn new_from_wait(wait: &WaitStatus, offset: u64, traces: &TraceMap) -> Self {
        let pid = wait.pid().map(|p| p.as_raw());
        let mut event = TraceEvent {
            pid,
            ..Default::default()
        };
        match wait {
            WaitStatus::Exited(_, i) => {
                event.description = "Exited".to_string();
                event.return_val = Some(*i as _);
            }
            WaitStatus::Signaled(_, sig, _) => {
                event.signal = Some(sig.to_string());
                event.description = "Signaled".to_string();
            }
            WaitStatus::Stopped(c, sig) => {
                event.signal = Some(sig.to_string());
                if *sig == Signal::SIGTRAP {
                    event.description = "Stopped".to_string();
                    event.addr = current_instruction_pointer(*c).ok().map(|x| (x - 1) as u64);
                    if let Some(addr) = event.addr {
                        event.location = traces.get_location(addr - offset);
                    }
                } else {
                    event.description = "Non-trace stop".to_string();
                }
            }
            WaitStatus::PtraceEvent(pid, sig, val) => {
                event.signal = Some(sig.to_string());
                match *val {
                    PTRACE_EVENT_CLONE => {
                        event.description = "Ptrace Clone".to_string();
                        if *sig == Signal::SIGTRAP {
                            event.child = get_event_data(*pid).ok().map(|x| x as pid_t);
                        }
                    }
                    PTRACE_EVENT_FORK => {
                        event.description = "Ptrace fork".to_string();
                    }
                    PTRACE_EVENT_VFORK => {
                        event.description = "Ptrace vfork".to_string();
                    }
                    PTRACE_EVENT_EXEC => {
                        event.description = "Ptrace exec".to_string();
                    }
                    PTRACE_EVENT_EXIT => {
                        event.description = "Ptrace exit".to_string();
                    }
                    _ => {
                        event.description = "Ptrace unknown event".to_string();
                    }
                }
            }
            WaitStatus::Continued(_) => {
                event.description = "Continued".to_string();
            }
            WaitStatus::StillAlive => {
                event.description = "StillAlive".to_string();
            }
            WaitStatus::PtraceSyscall(_) => {
                event.description = "PtraceSyscall".to_string();
            }
        }
        event
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EventLog {
    events: RefCell<Vec<Event>>,
}

impl EventLog {
    pub fn new() -> Self {
        Self {
            events: RefCell::new(vec![]),
        }
    }

    pub fn push_binary(&self, binary: TestBinary) {
        self.events.borrow_mut().push(Event::BinaryLaunch(binary));
    }

    pub fn push_trace(&self, event: TraceEvent) {
        self.events.borrow_mut().push(Event::Trace(event));
    }

    pub fn push_config(&self, name: String) {
        self.events.borrow_mut().push(Event::ConfigLaunch(name));
    }
}

impl Drop for EventLog {
    fn drop(&mut self) {
        let fname = format!("tarpaulin-run-{}.json", Local::now().to_rfc3339());
        let path = Path::new(&fname);
        info!("Serializing tarpaulin debug log to {}", path.display());
        if let Ok(output) = File::create(path) {
            if let Err(e) = serde_json::to_writer(output, self) {
                warn!("Failed to serialise or write result: {}", e);
            }
        } else {
            warn!("Failed to create log file");
        }
    }
}
