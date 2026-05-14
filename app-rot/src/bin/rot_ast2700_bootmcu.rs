//! AST2700 BootMCU firmware entry point.
//!
//! Secure boot MCU embedded in AST2700 SoC: Cortex-M4F 400MHz,
//! manages Caliptra lifecycle, OTP keys, and secure boot chain for
//! the full AST2700 (quad A35 + SSP + TSP).
//!
//! The BootMCU is the first processor to execute after power-on.
//! It measures and loads firmware for the other cores.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin rot_ast2700_bootmcu --features ast2700-bootmcu \
//!     --target riscv32imc-unknown-none-elf --release
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

    let mut uart = hal::uart::Uart::new_uart12(hal::uart::Config::default());
    uart.blocking_write(b"\r\nAST2700 BootMCU RoT firmware starting\r\n");

    // TODO: Caliptra lifecycle management
    // TODO: OTP key verification
    // TODO: Measure and load PSP/SSP/TSP firmware
    // TODO: Secure boot chain orchestration

    uart.blocking_write(b"Secure boot chain not yet implemented\r\n");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
