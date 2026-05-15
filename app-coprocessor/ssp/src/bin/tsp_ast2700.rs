//! AST2700 TSP (Ternary Service Processor) firmware entry point.
//!
//! Cortex-M4 400MHz. Handles interfaces not present on earlier SoCs:
//! - CAN bus for PSU communication (PMBus-over-CAN for AI server racks)
//! - LTPI link management for satellite BMC topologies
//! - Downstream MCTP bridging over UART
//!
//! AP cores are quad Cortex-A35 running TamaGo Go.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin tsp_ast2700 --features ast2700-tsp \
//!     --target thumbv7em-none-eabihf --release
//! ```

#![no_std]
#![no_main]

// AST2700 TSP HAL support not yet in embassy-aspeed.

use embassy_executor::Spawner;

use panic_halt as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    // TODO: embassy_aspeed::init() with ast2700-tsp support
    // TODO: CAN bus PSU telemetry poller
    // TODO: LTPI link management
    // TODO: Sensor store + PLDM Type 2 terminus
    // TODO: MCTP/UART transport for downstream bridging

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
