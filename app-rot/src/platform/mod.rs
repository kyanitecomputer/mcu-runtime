//! Platform abstraction traits and hardware-independent command types.
//!
//! All traits here are `no_std`/no-alloc friendly and deliberately free of
//! Embassy, logging, or HAL imports.  Hardware-specific implementations live in
//! the sub-modules below or in `embassy-aspeed`.
//!
//! # Usage pattern
//!
//! ```text
//! PFR orchestrator (pfr::Machine)
//!   emits Action
//!   ──► platform executor reads trait impls
//!         reads flash via FlashReader
//!         verifies images via Hasher + SignatureVerifier
//!         parses manifests via ManifestParser
//!         applies policy via PlatformPolicy
//!         returns Transition to orchestrator
//! ```
//!
//! # Implementations
//!
//! | Module | Platform |
//! |--------|----------|
//! | `generic` | AST1060 DCSCM board sequencer |
//! | `intel`   | Intel PFR manifest / sequencer |
//! | `amd`     | AMD PFR manifest / sequencer |

pub mod amd;
#[cfg(feature = "ast1060")]
pub mod ast1060;
pub mod generic;
pub mod intel;

use crate::flash::{RecoveryLevel, Slot};
use crate::manifest::{HashAlgorithm, PlatformFirmwareManifest, SignatureAlgorithm};
use crate::protocol::SecureState;

// ── Flash I/O traits ──────────────────────────────────────────────────────────

/// Read raw bytes from a named flash region.
///
/// Implementations may back this with SPI NOR flash, SRAM, or any other
/// byte-addressable storage.
pub trait FlashReader {
    /// Error type returned on I/O failure.
    type Error: core::fmt::Debug;

    /// Read `buf.len()` bytes from `slot` starting at `offset`.
    fn read(&self, slot: Slot, offset: u32, buf: &mut [u8]) -> Result<(), Self::Error>;

    /// Return the byte size of `slot`, or `None` if the slot is not available.
    fn region_size(&self, slot: Slot) -> Option<u32>;
}

/// Write and erase a named flash region.
pub trait FlashWriter: FlashReader {
    /// Write `data` to `slot` starting at `offset`.
    ///
    /// The region must already be erased if the underlying medium requires it.
    #[allow(async_fn_in_trait)]
    async fn write(&mut self, slot: Slot, offset: u32, data: &[u8]) -> Result<(), Self::Error>;

    /// Erase the sector that contains `sector_offset` within `slot`.
    #[allow(async_fn_in_trait)]
    async fn erase_sector(&mut self, slot: Slot, sector_offset: u32) -> Result<(), Self::Error>;
}

// ── Manifest parsing ──────────────────────────────────────────────────────────

/// Parse a raw byte buffer into a [`PlatformFirmwareManifest`].
///
/// The lifetime `'a` is tied to the input buffer so no allocation is needed.
pub trait ManifestParser {
    /// Error returned when the buffer is malformed or unsigned.
    type Error: core::fmt::Debug;

    /// Parse `data` and return a manifest that borrows from it.
    fn parse<'a>(&self, data: &'a [u8]) -> Result<PlatformFirmwareManifest<'a>, Self::Error>;
}

// ── Crypto traits ─────────────────────────────────────────────────────────────

/// Compute a cryptographic hash over a contiguous byte slice.
///
/// The AST1060 implementation backs this with HACE hardware.
pub trait Hasher {
    /// Error returned on hardware or configuration failure.
    type Error: core::fmt::Debug;

    /// Fill `out` with the digest and return the number of bytes written.
    ///
    /// `out` must be at least `algorithm.digest_len()` bytes long.
    #[allow(async_fn_in_trait)]
    async fn digest(
        &self,
        algorithm: HashAlgorithm,
        data: &[u8],
        out: &mut [u8],
    ) -> Result<usize, Self::Error>;
}

/// Verify a digital signature against a digest and a public key.
pub trait SignatureVerifier {
    /// Error returned on hardware or verification failure.
    type Error: core::fmt::Debug;

    /// Return `Ok(())` if the signature is valid.
    #[allow(async_fn_in_trait)]
    async fn verify(
        &self,
        algorithm: SignatureAlgorithm,
        digest: &[u8],
        signature: &[u8],
        public_key: &[u8],
    ) -> Result<(), Self::Error>;
}

// ── Platform policy ───────────────────────────────────────────────────────────

/// Platform-level policy decisions, independent of storage and hardware.
///
/// This trait separates policy from mechanism: the same trait can be
/// implemented by a ROM-resident default, a flash-resident config, or a
/// test double.
pub trait PlatformPolicy {
    /// Return `true` if `svn >= minimum_svn` (standard anti-rollback rule).
    fn accepts_svn(&self, svn: u32, minimum_svn: u32) -> bool;

    /// Return `true` if another recovery attempt at `level` is allowed.
    fn accepts_recovery_level(&self, level: RecoveryLevel) -> bool;

    /// Return the current platform secure state for mailbox reporting.
    fn current_secure_state(&self) -> SecureState;
}

// ── Default policy ────────────────────────────────────────────────────────────

/// Permissive default policy used in development / unprovisioned mode.
///
/// Accepts all SVNs and all recovery levels up to [`RecoveryLevel::MAX`].
pub struct DefaultPolicy;

impl PlatformPolicy for DefaultPolicy {
    fn accepts_svn(&self, svn: u32, minimum_svn: u32) -> bool {
        svn >= minimum_svn
    }

    fn accepts_recovery_level(&self, level: RecoveryLevel) -> bool {
        level <= RecoveryLevel::MAX
    }

    fn current_secure_state(&self) -> SecureState {
        SecureState::Unprovisioned
    }
}

// ── PlatformCommand ───────────────────────────────────────────────────────────

/// Hardware-independent platform command emitted by the PFR orchestrator.
///
/// The executor maps each variant to the concrete HAL calls required by the
/// target SoC and board.
///
/// Marked `#[non_exhaustive]` so adding variants in the future is not a
/// breaking change for downstream executors.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlatformCommand {
    /// Assert target reset lines and switch SPI monitors to RoT ownership.
    HoldTargetsInReset,
    /// Release SPI monitor passthrough to allow target flash access.
    ReleaseSpiPassthrough,
    /// Program the SPI write-protect filter from the active PFM policy.
    ProgramSpiFilter,
    /// Deassert target reset and sideband signals in board order.
    ReleaseTargetsFromReset,
    /// Arm the boot-progress watchdog for the named component.
    StartWatchdog,
    /// Disarm the boot-progress watchdog.
    StopWatchdog,
    /// Begin image recovery from the golden/recovery slot.
    TriggerRecovery,
    /// Enter terminal lockdown; no further release is possible.
    EnterLockdown,
}
