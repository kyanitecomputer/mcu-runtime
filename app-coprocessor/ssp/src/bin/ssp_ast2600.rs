//! AST2600 SSP coprocessor firmware entry point.
//!
//! Cortex-M3 200MHz. Offloads sensor acquisition, fan control, and
//! peripheral management from the AP cores (Cortex-A7 running TamaGo Go).
//! Communicates via hardware mailbox IPC and PLDM/MCTP over I2C/SMBus.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin ssp_ast2600 --features ast2600-ssp \
//!     --target thumbv7m-none-eabi --release
//! ```

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    hal::clock::clock_enable(hal::clock::ClockGate::UART11CLK);
    let mut uart = hal::uart::Uart::new_uart11(hal::uart::Config::default());

    uart.blocking_write(b"\r\nAST2600 SSP coprocessor starting\r\n");

    // TODO: Initialize sensor pollers (PECI, I2C/PMBus, ADC)
    // TODO: Initialize fan control (PID/table per thermal zone)
    // TODO: Initialize mailbox IPC responder
    // TODO: Initialize MCTP transport (I2C/SMBus)
    // TODO: Initialize PLDM Type 2 terminus

    uart.blocking_write(b"Sensor subsystem not yet implemented\r\n");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
