//! Flash storage and image management.
//!
//! Uses `embedded-storage` traits for backend portability (internal
//! SRAM-backed flash vs external SPI).
//!
//! # Image management
//!
//! - **A/B bank scheme**: Active, staging, and recovery image slots
//! - **Anti-rollback**: Monotonic counters in OTP prevent downgrade attacks
//! - **Recovery**: Autonomous restore from golden image on failure
//!
//! # SPI filter engine
//!
//! Address-range write protection derived from PFM policies.
//! - AST1060: 4× QSPI monitor interfaces
//! - AST1080: 3× QSPI monitor interfaces
