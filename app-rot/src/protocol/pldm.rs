//! PLDM Type 5 firmware update agent.
//!
//! Manages firmware update lifecycle for BMC, host, and component firmware.
//! Coordinates with the PFR state machine for staged updates with A/B bank
//! management and anti-rollback enforcement.
