//! PFR (Platform Firmware Resilience) state machine.
//!
//! Implements the NIST SP 800-193 lifecycle: Verify → Release → Monitor → Recover → Lockdown.
//!
//! The state machine is the top-level orchestrator. Protocol tasks are
//! subordinate — they can trigger re-verification or forced recovery but
//! cannot override the state machine.
//!
//! # States
//!
//! - **Init**: Entry point; hardware is not yet held in reset.
//! - **TMinus1**: Platform held in reset; awaiting verification.
//! - **Verify**: Validate firmware images against signed manifests (PFM).
//! - **Release**: Program SPI filter engine, deassert component reset.
//! - **Monitor**: Runtime SPI filtering, watchdog supervision.
//! - **Recover**: Restore from golden/recovery image on failure or timeout.
//! - **Lockdown**: Terminal state when all recovery attempts are exhausted.

/// PFR state machine states.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Initial platform setup before targets are held in reset.
    Init,
    /// Platform is held in reset before firmware verification.
    TMinus1,
    /// Verifying firmware images against signed manifests.
    Verify,
    /// Releasing component (SPI filter active, reset deasserted).
    Release,
    /// Runtime monitoring (SPI filter + watchdog).
    Monitor,
    /// Recovering from golden/recovery image.
    Recover,
    /// Terminal: all recovery attempts exhausted; platform stays locked.
    Lockdown,
}

/// PFR state machine events.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    /// Initial boot event.
    Boot,
    /// Platform hold/setup completed.
    PlatformHeld,
    /// Firmware verification completed successfully.
    VerificationPassed,
    /// Firmware verification failed.
    VerificationFailed,
    /// Platform release completed.
    ReleaseComplete,
    /// A monitored reset was detected at runtime.
    ResetDetected,
    /// A watchdog or checkpoint timeout occurred.
    WatchdogTimeout,
    /// Recovery completed successfully; re-enter verification.
    RecoveryComplete,
    /// Recovery failed; no further recovery is possible.
    RecoveryFailed,
}

/// Platform component associated with an event or action.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Component {
    Platform,
    Bmc,
    Pch,
    Cpld,
    Afm,
}

/// Event plus component metadata for protocol and monitor tasks.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRecord {
    pub event: Event,
    pub component: Component,
    pub code: u32,
}

impl EventRecord {
    /// Create an event record with no implementation-specific status code.
    pub const fn new(event: Event, component: Component) -> Self {
        Self {
            event,
            component,
            code: 0,
        }
    }

    /// Create an event record with an implementation-specific status code.
    pub const fn with_code(event: Event, component: Component, code: u32) -> Self {
        Self {
            event,
            component,
            code,
        }
    }
}

/// Construct an `EventRecord` for a component — shorthand for [`EventRecord::new`].
pub const fn event_for(event: Event, component: Component) -> EventRecord {
    EventRecord::new(event, component)
}

/// Reason the state machine entered `Recover` or `Lockdown`.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryReason {
    VerificationFailed,
    ResetDetected,
    WatchdogTimeout,
    RecoveryFailed,
}

/// Hardware-independent action requested by a state transition.
///
/// The embedding executor maps each variant to concrete platform calls.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    None,
    HoldPlatform,
    VerifyImages,
    ReleasePlatform,
    StartMonitoring,
    RecoverImages,
    Lockdown,
}

/// Whether an event produced a state change.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventDisposition {
    Transitioned,
    Ignored,
}

/// Result of processing one event.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transition {
    pub state: State,
    pub action: Action,
    pub disposition: EventDisposition,
}

impl Transition {
    /// Return `true` if the event was a no-op and no action is requested.
    pub const fn is_ignored(&self) -> bool {
        match self.disposition {
            EventDisposition::Ignored => true,
            EventDisposition::Transitioned => false,
        }
    }

    /// Return `true` if the transition requests concrete platform work.
    pub const fn needs_action(&self) -> bool {
        match self.action {
            Action::None => false,
            _ => true,
        }
    }
}

/// Minimal pure-logic PFR state machine.
///
/// `Copy` so Embassy tasks can hold a value-type snapshot for logging.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    state: State,
    recovery_reason: Option<RecoveryReason>,
}

impl Machine {
    /// Create a new machine in the [`State::Init`] state.
    pub const fn new() -> Self {
        Self {
            state: State::Init,
            recovery_reason: None,
        }
    }

    /// Return the current state.
    pub const fn state(&self) -> State {
        self.state
    }

    /// Return the reason for the last recovery or lockdown, if any.
    pub const fn recovery_reason(&self) -> Option<RecoveryReason> {
        self.recovery_reason
    }

    /// Return `true` if the machine has reached the terminal [`State::Lockdown`].
    pub const fn is_locked_down(&self) -> bool {
        match self.state {
            State::Lockdown => true,
            _ => false,
        }
    }

    /// Process an event using `Component::Platform` as the source.
    pub fn step(&mut self, event: Event) -> State {
        self.step_record(EventRecord::new(event, Component::Platform))
            .state
    }

    /// Process an event and return the full transition including the requested action.
    pub fn step_with_action(&mut self, event: Event) -> Transition {
        self.step_record(EventRecord::new(event, Component::Platform))
    }

    /// Process a component-tagged event record.
    ///
    /// The returned [`Action`] is hardware-independent; the caller is
    /// responsible for executing platform-specific consequences.
    pub fn step_record(&mut self, record: EventRecord) -> Transition {
        let previous = self.state;
        let event = record.event;

        let action = match (self.state, event) {
            (State::Init, Event::Boot) => {
                self.state = State::TMinus1;
                Action::HoldPlatform
            }
            (State::TMinus1, Event::PlatformHeld) => {
                self.state = State::Verify;
                Action::VerifyImages
            }
            (State::Verify, Event::VerificationPassed) => {
                self.state = State::Release;
                Action::ReleasePlatform
            }
            (State::Release, Event::ReleaseComplete) => {
                self.state = State::Monitor;
                Action::StartMonitoring
            }
            (State::Recover, Event::RecoveryComplete) => {
                self.recovery_reason = None;
                self.state = State::Verify;
                Action::VerifyImages
            }
            // RecoveryFailed is terminal — enter Lockdown, not Recover.
            (_, Event::RecoveryFailed) => {
                self.recovery_reason = Some(RecoveryReason::RecoveryFailed);
                self.state = State::Lockdown;
                Action::Lockdown
            }
            (_, Event::VerificationFailed) => {
                self.enter_recovery(RecoveryReason::VerificationFailed)
            }
            (_, Event::ResetDetected) => self.enter_recovery(RecoveryReason::ResetDetected),
            (_, Event::WatchdogTimeout) => self.enter_recovery(RecoveryReason::WatchdogTimeout),
            // All other combinations (including Lockdown + any event) are no-ops.
            _ => Action::None,
        };

        Transition {
            state: self.state,
            action,
            disposition: if self.state == previous && matches!(action, Action::None) {
                EventDisposition::Ignored
            } else {
                EventDisposition::Transitioned
            },
        }
    }

    fn enter_recovery(&mut self, reason: RecoveryReason) -> Action {
        self.recovery_reason = Some(reason);
        self.state = State::Recover;
        Action::RecoverImages
    }
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}
