//! blinky — GPIO toggle every 500 ms, verifiable with oscilloscope or LED.
//!
//! Toggle GPIO port E, pin 0 (SSP user LED on AST2600 EVB reference design).
//! Change `BLINK_PIN` to match your board's LED.
//!
//! Build:
//! ```sh
//! just build-example blinky
//! ```
//!
//! On hardware: the GPIO should toggle at ~1 Hz (500 ms high / 500 ms low).

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_aspeed::gpio::{Level, Output, pin};
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use defmt::info;
use defmt_rtt as _;
use panic_halt as _;

/// GPIO pin to toggle. Adjust to match your board.
const BLINK_PORT: char = 'E';
const BLINK_BIT: u8 = 0;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let blink_pin = pin(BLINK_PORT, BLINK_BIT);
    let mut led = Output::new(blink_pin, Level::Low);

    info!("blinky: toggling GPIO{}{}...", BLINK_PORT as u8, BLINK_BIT);

    loop {
        led.set_high();
        Timer::after(Duration::from_millis(500)).await;
        led.set_low();
        Timer::after(Duration::from_millis(500)).await;
    }
}
