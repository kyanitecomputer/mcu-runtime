//! Root of Trust firmware for ASPEED security processors.
//!
//! Implements Platform Firmware Resilience (PFR) per NIST SP 800-193
//! with platform-agnostic design across multiple ASPEED RoT SoCs.
//!
//! # Supported SoCs
//!
//! | SoC | CPU | Role | Feature Flag |
//! |-----|-----|------|-------------|
//! | AST1060 | Cortex-M4F 200MHz | PFR processor | `ast1060` |
//! | AST1080 | Cortex-M4F 400MHz | Hardened RoT | `ast1080` |
//! | AST1040 | Cortex-M4F 400MHz | BIC / BMC | `ast1040` |
//! | AST2700 BootMCU | Cortex-M4F 400MHz | Secure boot MCU | `ast2700-bootmcu` |
//!
//! # Architecture
//!
//! ```text
//! ┌─ Protocol Tasks ──────────────────────────────────────┐
//! │  MCTP Transport → SPDM / Cerberus / PLDM / Provision │
//! ├───────────────────────────────────────────────────────┤
//! │  Services: PFR StateMachine │ DICE │ EventLog │ Crypto│
//! │            ManifestMgr │ ImageMgr │ FlashDriver       │
//! ├───────────────────────────────────────────────────────┤
//! │  Platform HAL (embassy-aspeed)                        │
//! └───────────────────────────────────────────────────────┘
//! ```

#![no_std]

pub mod pfr;
pub mod protocol;
pub mod crypto;
pub mod manifest;
pub mod flash;
pub mod provision;
pub mod event_log;
pub mod platform;
