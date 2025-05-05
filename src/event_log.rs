use crate::cargo::TestBinary;
use crate::config::Config;
#[cfg(ptrace_supported)]
use crate::ptrace_control::*;
#[cfg(ptrace_supported)]
use crate::statemachine::ProcessInfo;
#[cfg(ptrace_supported)]
use crate::statemachine::TracerAction;
use crate::traces::Location;
#[cfg(ptrace_supported)]
use crate::traces::TraceMap;
use chrono::offset::Local;
#[cfg(ptrace_supported)]
use nix::libc::*;
#[cfg(ptrace_supported)]
use nix::sys::{signal::Signal, wait::WaitStatus};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::File;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{info, warn};

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Event {
    ConfigLaunch(String),
    BinaryLaunch(TestBinary),
    Trace(TraceEvent),
    Marker(Option<()>),
}

#[derive(Clone, PartialEq, PartialOrd, Serialize, Deserialize)]
pub struct EventWrapper {
    #[serde(flatten)]
    event: Event,
    // The time this was created in seconds
    created: f64,
}

impl EventWrapper {
    fn new(event: Event, since: Instant) -> Self {
        let created = Instant::now().duration_since(since).as_secs_f64();
        Self { event, created }
    }
}

#[derive(Clone, Default, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TraceEvent {
    pid: Option<i64>,
    child: Option<i64>,
    signal: Option<String>,
    addr: Option<u64>,
    return_val: Option<i64>,
    location: Option<Location>,
    description: String,
}

impl TraceEvent {
    #[cfg(ptrace_supported)]
    pub(crate) fn new_from_action(action: &TracerAction<ProcessInfo>) -> Self {
        match action {
            TracerAction::TryContinue(t) => TraceEvent {
                pid: Some(t.pid.as_raw().into()),
                signal: t.signal.map(|x| x.to_string()),
                description: "Try continue child".to_string(),
                ..Default::default()
            },
            TracerAction::Continue(t) => TraceEvent {
                pid: Some(t.pid.as_raw().into()),
                signal: t.signal.map(|x| x.to_string()),
                description: "Continue child".to_string(),
                ..Default::default()
            },
            TracerAction::Step(t) => TraceEvent {
                pid: Some(t.pid.as_raw().into()),
                description: "Step child".to_string(),
                ..Default::default()
            },
            TracerAction::Detach(t) => TraceEvent {
                pid: Some(t.pid.as_raw().into()),
                description: "Detach child".to_string(),
                ..Default::default()
            },
            TracerAction::Nothing => TraceEvent {
                description: "Do nothing".to_string(),
                ..Default::default()
            },
        }
    }

    #[cfg(ptrace_supported)]
    pub(crate) fn new_from_wait(wait: &WaitStatus, offset: u64, traces: &TraceMap) -> Self {
        let pid = wait.pid().map(|p| p.as_raw().into());
        let mut event = TraceEvent {
            pid,
            ..Default::default()
        };
        match wait {
            WaitStatus::Exited(_, i) => {
                event.description = "Exited".to_string();
                event.return_val = Some((*i).into());
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
                            event.child = get_event_data(*pid).ok().map(|x| x as i64);
                        }
                    }
                    PTRACE_EVENT_FORK => {
                        event.description = "Ptrace fork".to_string();
                        if *sig == Signal::SIGTRAP {
                            event.child = get_event_data(*pid).ok().map(|x| x as i64);
                        }
                    }
                    PTRACE_EVENT_VFORK => {
                        event.description = "Ptrace vfork".to_string();
                        if *sig == Signal::SIGTRAP {
                            event.child = get_event_data(*pid).ok().map(|x| x as i64);
                        }
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

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EventLog {
    events: RefCell<Vec<EventWrapper>>,
    #[serde(skip)]
    start: Option<Instant>,
    manifest_paths: HashSet<PathBuf>,
    #[serde(skip)]
    output_folder: PathBuf,
}

impl EventLog {
    pub fn new(manifest_paths: HashSet<PathBuf>, config: &Config) -> Self {
        Self {
            events: RefCell::new(vec![]),
            start: Some(Instant::now()),
            manifest_paths,
            output_folder: config.output_dir(),
        }
    }

    pub fn push_binary(&self, binary: TestBinary) {
        self.events.borrow_mut().push(EventWrapper::new(
            Event::BinaryLaunch(binary),
            self.start.unwrap(),
        ));
    }

    pub fn push_trace(&self, event: TraceEvent) {
        self.events
            .borrow_mut()
            .push(EventWrapper::new(Event::Trace(event), self.start.unwrap()));
    }

    pub fn push_config(&self, name: String) {
        self.events.borrow_mut().push(EventWrapper::new(
            Event::ConfigLaunch(name),
            self.start.unwrap(),
        ));
    }

    pub fn push_marker(&self) {
        // Prevent back to back markers when we spend a lot of time waiting on events
        if self
            .events
            .borrow()
            .last()
            .filter(|x| matches!(x.event, Event::Marker(_)))
            .is_none()
        {
            self.events
                .borrow_mut()
                .push(EventWrapper::new(Event::Marker(None), self.start.unwrap()));
        }
    }
}

impl Drop for EventLog {
    fn drop(&mut self) {
        let fname = format!("tarpaulin_{}.json", Local::now().format("%Y%m%d%H%M%S"));
        let path = self.output_folder.join(fname);
        info!("Serializing tarpaulin debug log to {}", path.display());
        if let Ok(output) = File::create(path) {
            if let Err(e) = serde_json::to_writer(output, self) {
                warn!("Failed to serialise or write result: {e}");
            }
        } else {
            warn!("Failed to create log file");
        }
    }
}
