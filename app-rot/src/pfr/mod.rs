//! PFR (Platform Firmware Resilience) state machine.
//!
//! Implements the NIST SP 800-193 lifecycle: Verify → Release → Monitor → Recover.
//!
//! The state machine is the top-level orchestrator. Protocol tasks are
//! subordinate — they can trigger re-verification or forced recovery but
//! cannot override the state machine.
//!
//! # States
//!
//! - **Verify**: Validate firmware image(s) against signed manifests (PFM).
//! - **Release**: Program SPI filter engine, deassert component reset.
//! - **Monitor**: Runtime SPI filtering, watchdog supervision.
//! - **Recover**: Restore from golden/recovery image on failure or timeout.

/// PFR state machine states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Verifying firmware images against PFM.
    Verify,
    /// Releasing component (SPI filter active, reset deasserted).
    Release,
    /// Runtime monitoring (SPI filter + watchdog).
    Monitor,
    /// Recovering from golden/recovery image.
    Recover,
}
