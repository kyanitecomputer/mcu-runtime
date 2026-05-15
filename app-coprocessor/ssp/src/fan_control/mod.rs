//! Fan control subsystem.
//!
//! Closed-loop thermal management: maps N temperature sensors to M fan
//! outputs per thermal zone. Control mode (PID or step-table) is
//! configurable per zone via mailbox commands from the AP side.
//!
//! The coprocessor owns the real-time loop; the AP side only sets policy.
//! Manual override is supported for testing and manufacturing.
