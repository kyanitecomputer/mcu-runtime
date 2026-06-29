//! AST1080 Root of Trust firmware entry point.
//!
//! Hardened RoT processor: Cortex-M4F 400MHz, 128KB SRAM + 16MB HyperRAM,
//! Caliptra v2.1, anti-tamper (voltage glitch/sensor, temperature, clock),
//! PUF, TRNG, 3× QSPI monitor, 2× SMBus filter.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin rot_ast1080 --features ast1080 \
//!     --target thumbv7em-none-eabihf --release
//! ```

#![no_std]
#![no_main]

// AST1080 HAL support not yet in embassy-aspeed. This binary will compile
// once the ast1080 feature is added to embassy-aspeed with appropriate
// linker script and boot code.

use embassy_executor::Spawner;

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // TODO: embassy_aspeed::init() with ast1080 support
    // TODO: Caliptra subsystem init
    // TODO: Anti-tamper monitoring tasks
    // TODO: PFR state machine + protocol tasks

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
