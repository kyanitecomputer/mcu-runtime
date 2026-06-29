//! Provisioning service — pure command/state model.
//!
//! All provisioning state transitions are expressed as pure functions.
//! The OTP, flash, and I/O backends are injected by the caller so this
//! module stays `no_std`/no-alloc and hardware-independent.
//!
//! # Commands
//!
//! | Command | Reversibility |
//! |---------|---------------|
//! | `WriteRootKey`          | **Irreversible** (OTP) |
//! | `DeployManifest`        | Repeatable until locked |
//! | `SetPlatformDescriptor` | Repeatable until locked |
//! | `BurnAntiRollback`      | **Irreversible** (forward-only) |
//! | `LockProvisioning`      | **Irreversible** (OTP lock bit) |
//! | `ReadDeviceId`          | Read-only |
//! | `QueryStatus`           | Read-only |
//!
//! # Usage pattern
//!
//! ```text
//! let next = state.apply(ProvisionCommand::DeployManifest)?;
//! flash.write(MANIFEST_SLOT, &manifest_bytes)?;
//! // only commit new state after storage succeeds
//! state = next;
//! ```

/// Provisioning command received from the authenticated management channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionCommand {
    /// Write the root public-key hash into OTP (irreversible).
    WriteRootKey,
    /// Deploy a signed platform firmware manifest into flash.
    DeployManifest,
    /// Set the platform configuration descriptor.
    SetPlatformDescriptor,
    /// Burn one anti-rollback counter increment (irreversible).
    BurnAntiRollback,
    /// Lock all provisioning fields (irreversible once root key is written).
    LockProvisioning,
    /// Read-only: return the chip device identifier.
    ReadDeviceId,
    /// Read-only: return the current provisioning status.
    QueryStatus,
}

/// Provisioning operation status returned to the management channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionStatus {
    Idle,
    InProgress,
    Success,
    Failure,
    Locked,
}

/// Provisioning state-transition error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionError {
    /// The provisioning fields are locked; the command is not allowed.
    AlreadyLocked,
    /// A prerequisite step has not been completed.
    MissingPrerequisite,
    /// The command is not valid in the current state.
    InvalidState,
}

/// Pure provisioning state independent of any storage backend.
///
/// `Copy` so the orchestrator can cheaply hold a before/after snapshot.
///
/// # Commit pattern
///
/// Call [`apply`](Self::apply) to get the *next* state, then write it to
/// durable storage.  Only replace the live state if storage succeeds —
/// this keeps provisioning atomic from the caller's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProvisionState {
    pub root_key_written: bool,
    pub manifest_deployed: bool,
    pub descriptor_set: bool,
    pub locked: bool,
    pub svn_burned: u32,
}

impl ProvisionState {
    /// Create the initial (unprovisioned) state.
    pub const fn new() -> Self {
        Self {
            root_key_written: false,
            manifest_deployed: false,
            descriptor_set: false,
            locked: false,
            svn_burned: 0,
        }
    }

    /// Return `true` if the minimum provisioning set is complete.
    pub const fn is_provisioned(&self) -> bool {
        self.root_key_written && self.manifest_deployed && self.descriptor_set
    }

    /// Return the state after applying `command`, without side effects.
    ///
    /// The caller is responsible for persisting the returned state and
    /// performing the associated hardware operation (OTP write, flash write).
    pub fn apply(&self, command: ProvisionCommand) -> Result<Self, ProvisionError> {
        // Read-only commands are always allowed.
        if matches!(
            command,
            ProvisionCommand::ReadDeviceId | ProvisionCommand::QueryStatus
        ) {
            return Ok(*self);
        }

        if self.locked {
            return Err(ProvisionError::AlreadyLocked);
        }

        let mut next = *self;

        match command {
            ProvisionCommand::WriteRootKey => {
                next.root_key_written = true;
            }
            ProvisionCommand::DeployManifest => {
                if !next.root_key_written {
                    return Err(ProvisionError::MissingPrerequisite);
                }
                next.manifest_deployed = true;
            }
            ProvisionCommand::SetPlatformDescriptor => {
                next.descriptor_set = true;
            }
            ProvisionCommand::BurnAntiRollback => {
                next.svn_burned = next.svn_burned.saturating_add(1);
            }
            ProvisionCommand::LockProvisioning => {
                if !next.is_provisioned() {
                    return Err(ProvisionError::MissingPrerequisite);
                }
                next.locked = true;
            }
            // Already handled above.
            ProvisionCommand::ReadDeviceId | ProvisionCommand::QueryStatus => {}
        }

        Ok(next)
    }
}

impl Default for ProvisionState {
    fn default() -> Self {
        Self::new()
    }
}
