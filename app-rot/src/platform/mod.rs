//! Platform abstraction traits and implementations.
//!
//! The PFR core is platform-agnostic. Platform-specific behavior is
//! injected via trait implementations selected by compile-time feature
//! flags or runtime dispatch on the platform descriptor type field.
//!
//! # Traits
//!
//! - [`ManifestParser`] — Parse PFM/CFM/PCD and verify capsules
//! - [`PlatformSequencer`] — Boot sequence, component release, watchdog
//! - [`FlashLayout`] — Active/recovery/staging region mapping
//! - [`PlatformSignals`] — GPIO reset/ready signal control
//!
//! # Implementations
//!
//! | Crate | Manifest format | Platforms |
//! |-------|----------------|-----------|
//! | `intel-pfr` | Intel PFM | Intel server platforms |
//! | `amd-pfr` | AMD manifest | AMD server platforms |
//! | `generic-pfr` | Custom format | OEM / evaluation |

pub mod intel;
pub mod amd;
pub mod generic;
