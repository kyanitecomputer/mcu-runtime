//! Coprocessor firmware for ASPEED SSP/TSP cores.
//!
//! Real-time sensor acquisition, fan control, and peripheral management
//! offloaded from the main AP cores. Communicates results back via hardware
//! mailbox IPC and optionally PLDM Type 2 sensors over MCTP.
//!
//! # Supported SoCs
//!
//! | SoC | Core | ISA | Role | Feature Flag |
//! |-----|------|-----|------|-------------|
//! | AST2600 | SSP (Cortex-M3) | ARMv7-M | Sensor/fan/IPC | `ast2600-ssp` |
//! | AST2700 | SSP (Cortex-M4) | ARMv7E-M | Sensor/fan/IPC | `ast2700-ssp` |
//! | AST2700 | TSP (Cortex-M4) | ARMv7E-M | CAN/LTPI/PSU | `ast2700-tsp` |
//!
//! The AST2400/AST2500 ColdFire V1 coprocessors share the same mission but
//! run a separate C SDK (`app-coprocessor/coldfire/`) due to the M68K ISA.
//!
//! # Architecture
//!
//! ```text
//! ┌─ Sensor Pollers ──────────────────────────────────────┐
//! │  PECI │ I2C/PMBus │ ADC │ CAN (TSP only)             │
//! ├───────────────────────────────────────────────────────┤
//! │  Sensor Store (atomic slots, lock-free)               │
//! ├──────────┬──────────────┬─────────────────────────────┤
//! │ Fan Ctrl │ PLDM Type 2  │ Mailbox IPC (raw commands) │
//! │ PID/table│ Terminus     │ AP ↔ Coprocessor            │
//! ├──────────┴──────┬───────┴─────────────────────────────┤
//! │  MCTP Transport │  HW Mailbox Driver                  │
//! ├─────────────────┴─────────────────────────────────────┤
//! │  Platform HAL (embassy-aspeed)                        │
//! └───────────────────────────────────────────────────────┘
//! ```
//!
//! The main AP cores (Cortex-A7 on AST2600, Cortex-A35 on AST2700) run
//! bare-metal Go via TamaGo and communicate with these coprocessors over
//! the hardware mailbox or MCTP/PLDM.

#![no_std]

pub mod sensor;
pub mod fan_control;
pub mod mailbox;
pub mod protocol;
pub mod board_config;
