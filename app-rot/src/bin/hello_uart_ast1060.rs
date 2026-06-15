//! AST1060 UART5 hello-world example.
//!
//! Boots embassy, prints "Hello from embassy on AST1060!" to UART5
//! (115200 baud), then counts up every second.
//!
//! # Build
//!
//! ```sh
//! cargo build --example hello_uart_ast1060 \
//!     --features ast1060 \
//!     --target thumbv7em-none-eabihf \
//!     --release
//! ```
//!
//! # Hardware
//!
//! UART5 debug header at 115200 baud 8N1. Connect a USB-UART adapter to the

#![no_std]
#![no_main]

use embassy_aspeed::uart::{Config as UartConfig, Uart};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use defmt_rtt as _;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // Initialise HAL: unlock SCU, start SysTick time driver (25 MHz default).
    embassy_aspeed::init(embassy_aspeed::Config::default());

    // Initialise UART5 at 115200 baud 8N1.
    // UART5 clock is always-on on AST1060 — no gate call needed.
    let mut uart = Uart::new_uart5(UartConfig::default());

    uart.blocking_write(b"\r\nHello from embassy on AST1060!\r\n");

    let mut counter: u32 = 0;
    loop {
        let mut buf = [0u8; 32];
        let msg = format_counter(&mut buf, counter);
        uart.blocking_write(msg);
        counter = counter.wrapping_add(1);
        Timer::after(Duration::from_secs(1)).await;
    }
}

fn format_counter<'a>(buf: &'a mut [u8; 32], n: u32) -> &'a [u8] {
    // Minimal integer formatter — no alloc needed.
    let s = b"tick: ";
    buf[..s.len()].copy_from_slice(s);
    let mut pos = s.len();
    let mut tmp = [0u8; 10];
    let mut len = 0;
    let mut v = n;
    loop {
        tmp[len] = b'0' + (v % 10) as u8;
        len += 1;
        v /= 10;
        if v == 0 {
            break;
        }
    }
    for i in (0..len).rev() {
        if pos < buf.len() - 2 {
            buf[pos] = tmp[i];
            pos += 1;
        }
    }
    buf[pos] = b'\r';
    buf[pos + 1] = b'\n';
    pos += 2;
    &buf[..pos]
}
