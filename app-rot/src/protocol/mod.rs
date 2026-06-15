//! Protocol-independent policy types.
//!
//! All types here are pure data — no I2C, I3C, or MCTP transport.  Embassy
//! tasks receive hardware bytes, deserialise them into these types, and pass
//! them to the PFR state machine via [`ProtocolEvent`] or [`CheckpointEvent`].
//!
//! Transport bindings (MCTP over I2C, MCTP over I3C) are implemented in
//! the sub-modules.

pub mod cerberus;
pub mod mctp;
pub mod pldm;
pub mod spdm;

use crate::pfr;

// ── Mailbox types ─────────────────────────────────────────────────────────────

/// Logical mailbox peer independent of I2C/I3C transport.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxPeer {
    Bmc,
    Pch,
}

/// Mailbox command independent of the storage/transport backend.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailboxCommand {
    None,
    Provision,
    UpdateIntent,
    Checkpoint,
    ResetCommunication,
    RequestRecovery,
}

/// Protocol event delivered to the PFR state machine adapter.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProtocolEvent {
    pub peer: MailboxPeer,
    pub command: MailboxCommand,
    pub arg: u32,
}

impl ProtocolEvent {
    /// Create a protocol event with no argument.
    pub const fn new(peer: MailboxPeer, command: MailboxCommand) -> Self {
        Self {
            peer,
            command,
            arg: 0,
        }
    }

    /// Create a protocol event with an implementation-defined argument.
    pub const fn with_arg(peer: MailboxPeer, command: MailboxCommand, arg: u32) -> Self {
        Self { peer, command, arg }
    }

    /// Map this event to a PFR state machine event, if any action is required.
    pub const fn to_pfr_event(&self) -> Option<pfr::Event> {
        match self.command {
            MailboxCommand::RequestRecovery => Some(pfr::Event::ResetDetected),
            MailboxCommand::ResetCommunication => Some(pfr::Event::ResetDetected),
            _ => None,
        }
    }

    /// Return the PFR component corresponding to this mailbox peer.
    pub const fn pfr_component(&self) -> pfr::Component {
        match self.peer {
            MailboxPeer::Bmc => pfr::Component::Bmc,
            MailboxPeer::Pch => pfr::Component::Pch,
        }
    }

    /// Convert directly to an [`pfr::EventRecord`] for `step_record`.
    pub const fn to_event_record(&self) -> Option<pfr::EventRecord> {
        match self.to_pfr_event() {
            Some(event) => Some(pfr::EventRecord::new(event, self.pfr_component())),
            None => None,
        }
    }
}

// ── Checkpoint types ──────────────────────────────────────────────────────────

/// Firmware checkpoint class independent of watchdog hardware.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointKind {
    Bmc,
    Acm,
    Bios,
}

/// Firmware boot progress independent of mailbox register layout.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckpointProgress {
    Started,
    Paused,
    Resumed,
    Completed,
    AuthenticationFailed,
}

/// A checkpoint event from the platform mailbox.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CheckpointEvent {
    pub kind: CheckpointKind,
    pub progress: CheckpointProgress,
}

impl CheckpointEvent {
    /// Create a checkpoint event.
    pub const fn new(kind: CheckpointKind, progress: CheckpointProgress) -> Self {
        Self { kind, progress }
    }

    /// Map to a PFR event when boot-progress failure requires state machine action.
    pub const fn to_pfr_event(&self) -> Option<pfr::Event> {
        match self.progress {
            CheckpointProgress::AuthenticationFailed => Some(pfr::Event::WatchdogTimeout),
            _ => None,
        }
    }

    /// Return the PFR component for this checkpoint kind.
    pub const fn pfr_component(&self) -> pfr::Component {
        match self.kind {
            CheckpointKind::Bmc => pfr::Component::Bmc,
            CheckpointKind::Acm | CheckpointKind::Bios => pfr::Component::Pch,
        }
    }

    /// Convert to a [`pfr::EventRecord`] for `step_record`, if action is needed.
    pub const fn to_event_record(&self) -> Option<pfr::EventRecord> {
        match self.to_pfr_event() {
            Some(event) => Some(pfr::EventRecord::new(event, self.pfr_component())),
            None => None,
        }
    }
}

// ── UpdateIntent ──────────────────────────────────────────────────────────────

/// Update intent carried by a BMC or PCH mailbox message.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UpdateIntent {
    pub peer: MailboxPeer,
    pub update_active: bool,
    pub update_recovery: bool,
    pub update_staging: bool,
    pub dynamic_at_reset: bool,
}

impl UpdateIntent {
    /// Create an update intent.
    pub const fn new(peer: MailboxPeer) -> Self {
        Self {
            peer,
            update_active: false,
            update_recovery: false,
            update_staging: false,
            dynamic_at_reset: false,
        }
    }

    /// Return `true` if any update flag is set.
    pub const fn any_update(&self) -> bool {
        self.update_active || self.update_recovery || self.update_staging
    }

    /// Map to a PFR event; an update request always triggers re-verification.
    pub const fn to_pfr_event(&self) -> Option<pfr::Event> {
        if self.any_update() {
            Some(pfr::Event::VerificationFailed)
        } else {
            None
        }
    }

    /// Return the PFR component for this intent.
    pub const fn pfr_component(&self) -> pfr::Component {
        match self.peer {
            MailboxPeer::Bmc => pfr::Component::Bmc,
            MailboxPeer::Pch => pfr::Component::Pch,
        }
    }
}

// ── SecureState ───────────────────────────────────────────────────────────────

/// Platform secure-state value exposed via mailbox and protocol.
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecureState {
    Unprovisioned,
    Provisioned,
    Recovery,
    Lockdown,
}
