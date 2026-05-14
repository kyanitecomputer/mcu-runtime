//! Provisioning service.
//!
//! All provisioning uses authenticated MCTP vendor-defined messages.
//! No UART debug shell — minimized attack surface.
//!
//! # Commands
//!
//! | Command | Reversibility |
//! |---------|---------------|
//! | `WriteRootKey` | **Irreversible** (OTP) |
//! | `DeployManifest` | Repeatable until locked |
//! | `SetPlatformDescriptor` | Repeatable until locked |
//! | `BurnAntiRollback` | **Irreversible** (forward-only) |
//! | `LockProvisioning` | **Irreversible** (OTP lock bit) |
//! | `ReadDeviceId` | Read-only |
//! | `QueryStatus` | Read-only |
