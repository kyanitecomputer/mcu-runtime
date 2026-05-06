//! AST2700 BootMCU: "Hello World" via UART12.
//!
//! Prints a greeting and a 1 Hz counter to UART12 (115200 8N1).
//! UART12 clock = 24 MHz / 13 ≈ 1,846,153 Hz; divisor ≈ 1 → 115200 baud.
//!
//! # Build
//!
//! ```sh
//! just build-example-bootmcu hello_uart_bootmcu
//! ```

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use hal::uart::{Config, Uart};
use embedded_io::Write as _;

use defmt_rtt as _;
use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = Uart::new_uart12(Config::default());

    uart.blocking_write(b"AST2700 BootMCU - embassy-aspeed hello\r\n");

    let mut count: u32 = 0;
    loop {
        let mut buf = [0u8; 32];
        let msg = format_u32(count, &mut buf);
        uart.blocking_write(b"tick: ");
        uart.blocking_write(msg);
        uart.blocking_write(b"\r\n");
        count = count.wrapping_add(1);
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Format a u32 as decimal into `buf`, returning the used slice.
fn format_u32(mut n: u32, buf: &mut [u8; 32]) -> &[u8] {
    if n == 0 {
        buf[0] = b'0';
        return &buf[..1];
    }
    let mut i = buf.len();
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    &buf[i..]
}
