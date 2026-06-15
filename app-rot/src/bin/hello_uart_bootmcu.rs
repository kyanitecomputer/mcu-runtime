//! AST2700 BootMCU: Embassy async hello — 1 Hz UART12 tick counter.
//!
//! Hardware-verified on AST2750-A1 silicon via SPI flash boot.
//!
//! # Memory model
//!
//! ROM copies the FMC binary from SPI flash into GSRAM at 0x14B80A00
//! and jumps to it.  All code and data live in SRAM — no SDRAM dependency.
//! SDRAM (0x80000000) requires DRAM training before use.

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use hal::uart::{Config, Uart};

use defmt_rtt as _;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = Uart::new_uart12(Config::default());

    uart.blocking_write(b"BootMCU OK!\r\n");

    let mut count: u32 = 0;
    loop {
        uart.blocking_write(b"tick ");
        write_u32(&mut uart, count);
        uart.blocking_write(b"\r\n");
        count = count.wrapping_add(1);

        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Write a u32 as decimal.
fn write_u32(uart: &mut Uart, mut n: u32) {
    if n == 0 {
        uart.write_byte(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for &b in &buf[i..] {
        uart.write_byte(b);
    }
}
