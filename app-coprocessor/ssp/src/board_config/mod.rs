//! Board configuration: compiled-in sensor tables, fan zones, I2C topology.
//!
//! Each board variant provides a module defining the concrete sensor
//! descriptor table, fan zone mappings, and bus topology. This replaces
//! runtime-parsed JSON/INI files with compiled-in configuration.
//!
//! The AP side (TamaGo Go firmware) is the provisioning authority — the
//! coprocessor is intentionally policy-free beyond compiled-in defaults.
//! Runtime tuning flows through the mailbox or PLDM effecter commands.
