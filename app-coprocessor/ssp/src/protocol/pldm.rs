//! PLDM Type 2 — Sensor / Effecter terminus.
//!
//! Primary standardized interface for sensor data. Maintains a Platform
//! Descriptor Record (PDR) repository generated at compile time from the
//! board's sensor descriptor table.
//!
//! | Command | Function |
//! |---------|----------|
//! | `GetPDR` | Enumerate sensor/effecter descriptors |
//! | `GetSensorReading` | Read from sensor store |
//! | `SetNumericEffecterValue` | Fan speed override, LED control |
//! | `PlatformEventMessage` | Threshold crossing → async event to AP |
