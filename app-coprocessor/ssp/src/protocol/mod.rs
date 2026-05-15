//! Protocol stack for coprocessor MCTP/PLDM services.
//!
//! AST2600 SSP: MCTP over I2C/SMBus.
//! AST2700 SSP: MCTP over I3C.
//! AST2700 TSP: MCTP over UART (serial binding) or I3C.

pub mod mctp;
pub mod pldm;
pub mod spdm;
