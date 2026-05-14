//! AST1040 BIC firmware entry point.
//!
//! Bridge IC / BMC processor: Cortex-M4F 400MHz, 128KB SRAM + 16MB HyperRAM,
//! Caliptra v2.1, 13 UARTs, eSPI, USB, Super I/O, PWM/PECI.
//!
//! Unlike the AST1060/1080 (dedicated PFR/RoT), the AST1040 is a full BMC
//! processor that can also perform RoT functions via Caliptra.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin rot_ast1040 --features ast1040 \
//!     --target thumbv7em-none-eabihf --release
//! ```

#![no_std]
#![no_main]

use embassy_executor::Spawner;

use defmt_rtt as _;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // TODO: embassy_aspeed::init() with ast1040 support
    // TODO: Caliptra subsystem init
    // TODO: eSPI controller init
    // TODO: PFR state machine + protocol tasks

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
