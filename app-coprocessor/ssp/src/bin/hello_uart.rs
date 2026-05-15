//! hello_uart — first vertical slice: boot, print "Hello!" on UART11, count seconds.
//!
//! Build:
//! ```sh
//! just build-example hello_uart
//! ```
//!
//! On AST2600 EVB: connect to UART11 at 115200 8N1 to see output.

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use embedded_io::Write as _;

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    // Enable UART11 clock gate.
    hal::clock::clock_enable(hal::clock::ClockGate::UART11CLK);

    // Initialise UART11 at 115200 8N1.
    let mut uart = hal::uart::Uart::new_uart11(hal::uart::Config::default());

    uart.write_all(b"Hello from embassy on AST2600 SSP!\r\n").ok();

    let mut counter: u32 = 0;
    loop {
        let mut buf = [0u8; 32];
        let msg = format_counter(&mut buf, counter);
        uart.write_all(msg).ok();
        counter += 1;
        Timer::after(Duration::from_secs(1)).await;
    }
}

/// Format "count = N\r\n" into `buf`, returning the filled slice.
fn format_counter(buf: &mut [u8; 32], n: u32) -> &[u8] {
    let prefix = b"count = ";
    let mut pos = 0;
    buf[pos..pos + prefix.len()].copy_from_slice(prefix);
    pos += prefix.len();

    // Decimal encoding of `n`.
    let mut tmp = [0u8; 10];
    let mut tlen = 0;
    let mut v = n;
    if v == 0 {
        tmp[tlen] = b'0';
        tlen += 1;
    } else {
        while v > 0 {
            tmp[tlen] = b'0' + (v % 10) as u8;
            tlen += 1;
            v /= 10;
        }
        tmp[..tlen].reverse();
    }
    buf[pos..pos + tlen].copy_from_slice(&tmp[..tlen]);
    pos += tlen;
    buf[pos] = b'\r';
    buf[pos + 1] = b'\n';
    pos += 2;
    &buf[..pos]
}
