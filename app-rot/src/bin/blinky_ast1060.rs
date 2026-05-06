//! AST1060 GPIO blink example.
//!
//! Toggles GPIO port A pin 0 every 500 ms via `Timer::after`.
//! Verifiable with an oscilloscope (1 Hz square wave) or an LED.
//!
//! # Build
//!
//! ```sh
//! cargo build --example blinky_ast1060 \
//!     --features ast1060 \
//!     --target thumbv7em-none-eabihf \
//!     --release
//! ```
//!
//! # Hardware
//!
//! Change `BLINK_PIN` to match the GPIO pin connected to the LED/oscilloscope
//! on your AST1060 board.

#![no_std]
#![no_main]

use embassy_aspeed::gpio::{Level, Output, pin};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

use defmt_rtt as _;
use panic_halt as _;

/// GPIO pin to toggle. Port A pin 0 by default.
const BLINK_PIN: (char, u8) = ('A', 0);

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    embassy_aspeed::init(embassy_aspeed::Config::default());

    let mut led = Output::new(pin(BLINK_PIN.0, BLINK_PIN.1), Level::Low);

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}
