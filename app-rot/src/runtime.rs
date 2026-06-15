//! Runtime driver for the PFR state machine.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};

use crate::pfr::{Action, Component, Event, EventRecord, Machine, State, Transition};

pub const EVENT_QUEUE_DEPTH: usize = 8;

pub type EventChannel = Channel<CriticalSectionRawMutex, EventRecord, EVENT_QUEUE_DEPTH>;
pub type EventSender<'a> = Sender<'a, CriticalSectionRawMutex, EventRecord, EVENT_QUEUE_DEPTH>;
pub type EventReceiver<'a> = Receiver<'a, CriticalSectionRawMutex, EventRecord, EVENT_QUEUE_DEPTH>;

#[derive(Debug)]
pub struct Runtime {
    machine: Machine,
}

impl Runtime {
    pub const fn new() -> Self {
        Self {
            machine: Machine::new(),
        }
    }

    pub const fn state(&self) -> State {
        self.machine.state()
    }

    pub fn process(&mut self, record: EventRecord) -> Transition {
        self.machine.step_record(record)
    }

    pub const fn success_event(action: Action, component: Component) -> Option<EventRecord> {
        let event = match action {
            Action::HoldPlatform => Event::PlatformHeld,
            Action::VerifyImages => Event::VerificationPassed,
            Action::ReleasePlatform => Event::ReleaseComplete,
            Action::RecoverImages => Event::RecoveryComplete,
            Action::None | Action::StartMonitoring | Action::Lockdown => return None,
        };

        Some(EventRecord::new(event, component))
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
