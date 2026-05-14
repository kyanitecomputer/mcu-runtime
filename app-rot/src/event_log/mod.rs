//! Hash-chained event log.
//!
//! TCG-style tamper-evident log for forensics and remote attestation evidence.
//! Each entry is hash-chained to the previous, making retroactive modification
//! detectable.
