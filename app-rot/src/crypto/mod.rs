//! Cryptographic service.
//!
//! Singleton async task serializing access to hardware crypto accelerators.
//! Consumers submit requests via channel; the service dispatches to the
//! appropriate hardware path based on algorithm selection.
//!
//! # Hardware backends
//!
//! | SoC | Engine | Algorithms |
//! |-----|--------|-----------|
//! | AST1060 | HACE | AES, RSA 256–4096, ECDSA-384, SHA-1/2, HMAC |
//! | AST1080 | Caliptra v2.1 | (Caliptra-managed crypto) + TRNG + PUF |
//! | AST1040 | Caliptra v2.1 | (Caliptra-managed crypto) |
//! | AST2700 BootMCU | HACE | AES, RSA, ECDSA, SHA-1/2/3, SM3, SM4, HMAC |
