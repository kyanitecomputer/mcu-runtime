//! Manifest management: PFM, CFM, PCD.
//!
//! All manifests are signed. The PFR core never operates without a verified
//! manifest. Platform implementations provide parsers via the
//! [`ManifestParser`] trait.
//!
//! | Manifest | Purpose |
//! |----------|---------|
//! | **PFM** | Expected SPI flash layout, address ranges, permissions |
//! | **CFM** | Component topology and expected measurements |
//! | **PCD** | Platform-specific configuration (GPIO, I2C, timeouts) |
