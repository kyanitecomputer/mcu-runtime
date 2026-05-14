//! Protocol stack modules.
//!
//! All application protocols run over MCTP transport. Each protocol is an
//! independent Embassy async task communicating with the PFR state machine
//! via async channels.

pub mod mctp;
pub mod spdm;
pub mod cerberus;
pub mod pldm;
