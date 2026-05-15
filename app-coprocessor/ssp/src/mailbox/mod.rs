//! Hardware mailbox IPC between coprocessor and AP cores.
//!
//! Lightweight binary protocol over the raw mailbox channel.
//! Commands from AP: read sensors, set fan config, set LED state.
//! Notifications to AP: threshold alerts, watchdog heartbeat, fault events.
//!
//! # Hardware mechanism
//!
//! | SoC | Mailbox HW | Transport |
//! |-----|-----------|-----------|
//! | AST2600 | SSP ↔ AP doorbell + shared SRAM | Register-based |
//! | AST2700 | SSP/TSP ↔ PSP doorbell + shared SRAM | Register or MCTP/I3C |
//!
//! On AST2600/2700 this protocol coexists with (or is eventually replaced
//! by) MCTP/PLDM. The raw mailbox remains as a fast path for fan control
//! and boards without a full MCTP stack on the AP side.
