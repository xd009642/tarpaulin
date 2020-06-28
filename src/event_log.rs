use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use libc::pid_t;

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Event {
    pid: pid_t,
    child: Option<pid_t>,
    addr: i64,
    description: String,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventStream {
    binary: PathBuf,
    trace_events: Vec<Event>,
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EventLog {
    streams: Vec<EventStream>,    
}

impl EventLog {
    pub fn push_binary(&mut self, binary: PathBuf) {
        let new_event = EventStream {
            binary,
            trace_events: vec![],
        };
        self.streams.push(new_event);
    }

    pub fn push_event(&mut self, event: Event) {
        if let Some(current) = self.streams.last_mut() {
            current.trace_events.push(event);
        } else {
            self.streams.push(EventStream {
                binary: PathBuf::new(),
                trace_events: vec![event],
            });
        }
    }
}
