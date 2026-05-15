//! AST2700 SSP coprocessor firmware entry point.
//!
//! Cortex-M4 400MHz. Same sensor acquisition role as AST2600 SSP but with
//! upgraded core (DSP instructions for signal processing) and MCTP over I3C.
//! AP cores are quad Cortex-A35 running TamaGo Go.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin ssp_ast2700 --features ast2700-ssp \
//!     --target thumbv7em-none-eabihf --release
//! ```

#![no_std]
#![no_main]

// AST2700 SSP HAL support not yet in embassy-aspeed.

use embassy_executor::Spawner;

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // TODO: embassy_aspeed::init() with ast2700-ssp support
    // TODO: Sensor pollers (PECI, I2C, ADC)
    // TODO: Fan control
    // TODO: MCTP/I3C transport + PLDM Type 2
    // TODO: Mailbox IPC

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
