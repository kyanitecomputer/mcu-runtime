//! AST1060 Root of Trust firmware entry point.
//!
//! PFR processor: Cortex-M4F 200MHz, 768KB SRAM, HACE crypto,
//! 4× QSPI monitor, 4× SMBus filter.
//!
//! # Boot sequence
//!
//! 1. ROM → HW init (FPU, SCU unlock)
//! 2. DICE measurement → certificate chain
//! 3. Parse platform descriptor from signed flash
//! 4. Program SPI filter engine from PFM
//! 5. Start PFR state machine (verify → release → monitor)
//! 6. Start Embassy executor with protocol tasks
//!
//! # Build
//!
//! ```sh
//! cargo build --bin rot_ast1060 --features ast1060 \
//!     --target thumbv7em-none-eabihf --release
//! ```

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;

use defmt_rtt as _;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = hal::uart::Uart::new_uart5(hal::uart::Config::default());
    uart.blocking_write(b"\r\nAST1060 RoT firmware starting\r\n");

    // TODO: DICE measurement
    // TODO: Parse platform descriptor
    // TODO: Program SPI filter engine
    // TODO: Start PFR state machine
    // TODO: Spawn protocol tasks (MCTP, SPDM, Cerberus, PLDM, Provisioning)

    uart.blocking_write(b"PFR state machine not yet implemented\r\n");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
