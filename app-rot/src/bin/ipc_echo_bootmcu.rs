//! AST2700 BootMCU: IPC1 echo example.
//!
//! Waits for a message from the non-secure CA35 (sub-channel 1) on any
//! message ID, prints a summary to UART12, and echoes the payload back.
//!
//! # Build
//!
//! ```sh
//! just build-example-bootmcu ipc_echo_bootmcu
//! ```

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;
use embedded_io::Write as _;
use hal::ipc1::Ipc1;
use hal::uart::{Config, Uart};

use defmt_rtt as _;
use panic_halt as _;

/// Non-secure CA35 sub-channel index.
const IPC_NS_CA35: u8 = 1;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = Uart::new_uart12(Config::default());
    let ipc = Ipc1::new();

    uart.blocking_write(b"AST2700 BootMCU - IPC1 echo ready\r\n");

    loop {
        // Busy-wait for a message from the non-secure CA35.
        let (id, payload) = ipc.recv(IPC_NS_CA35);

        let mut buf = [0u8; 16];
        let len = format_nibble(id, &mut buf);
        uart.blocking_write(b"IPC recv id=");
        uart.blocking_write(&buf[..len]);
        uart.blocking_write(b"\r\n");

        // Echo the payload back on the same channel / same ID.
        ipc.send(IPC_NS_CA35, id, &payload);
    }
}

fn format_nibble(n: u8, buf: &mut [u8; 16]) -> usize {
    buf[0] = b'0' + (n & 0xF);
    1
}
