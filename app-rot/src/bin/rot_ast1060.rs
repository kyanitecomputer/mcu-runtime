//! AST1060 Root of Trust firmware entry point.
//!
//! PFR processor: Cortex-M4F 200MHz, 768KB SRAM, HACE crypto,
//! 4× QSPI monitor, 4× SMBus filter.
//!
//! # Boot sequence
//!
//! 1. ROM → HW init (FPU, SCU unlock)
//! 2. DICE measurement → certificate chain
//! 3. Parse platform descriptor from signed flash
//! 4. Program SPI filter engine from PFM
//! 5. Start PFR state machine (verify → release → monitor)
//! 6. Start Embassy executor with protocol tasks
//!
//! # Build
//!
//! ```sh
//! cargo build --bin rot_ast1060 --features ast1060 \
//!     --target thumbv7em-none-eabihf --release
//! ```

#![no_std]
#![no_main]

use app_rot::filter::SpiFilterPolicy;
use app_rot::pfr::{Action, Component, Event, EventRecord};
use app_rot::platform::ast1060::{Ast1060Crypto, Ast1060Flash};
use app_rot::platform::generic::Ast1060DcscmPlatform;
use app_rot::runtime::{EventChannel, Runtime};
use app_rot::verify::{ImageVerifier, NoVerificationMaterial};
use app_rot::{
    flash::Slot,
    manifest::{AntiRollbackPolicy, PlatformFirmwareManifest},
};
use cortex_m_rt::entry;
use embassy_aspeed as hal;
use embassy_executor::Executor;
use static_cell::StaticCell;

static EVENTS: EventChannel = EventChannel::new();
static EXECUTOR: StaticCell<Executor> = StaticCell::new();

use defmt_rtt as _;
use panic_halt as _;

#[entry]
fn main() -> ! {
    let executor = EXECUTOR.init(Executor::new());
    executor.run(|spawner| {
        let Ok(task) = main_task() else {
            loop {
                cortex_m::asm::wfi();
            }
        };
        spawner.spawn(task);
    })
}

#[embassy_executor::task]
async fn main_task() {
    hal::init(hal::Config::default());

    let mut uart = hal::uart::Uart::new_uart5(hal::uart::Config::default());
    uart.blocking_write(b"\r\nAST1060 RoT firmware starting\r\n");

    let mut platform = Ast1060DcscmPlatform::new();
    let flash = Ast1060Flash::<0>::new([]);
    let crypto = Ast1060Crypto::new();
    let mut verifier = ImageVerifier::<4096, 64>::new();
    let verification_material = NoVerificationMaterial;
    let manifest = PlatformFirmwareManifest {
        version: 0,
        protected_regions: &[],
        images: &[],
    };
    let mut runtime = Runtime::new();

    // TODO: DICE measurement
    // TODO: Parse platform descriptor
    // TODO: Program SPI filter engine
    // TODO: Spawn protocol tasks (MCTP, SPDM, Cerberus, PLDM, Provisioning)

    uart.blocking_write(b"Starting PFR state machine\r\n");
    EVENTS
        .send(EventRecord::new(Event::Boot, Component::Platform))
        .await;

    loop {
        let record = EVENTS.receive().await;
        let transition = runtime.process(record);

        match transition.action {
            Action::HoldPlatform => {
                uart.blocking_write(b"Holding AST2700 BootMCU in reset\r\n");
                platform.hold_bootmcu_in_reset();
                embassy_time::Timer::after(embassy_time::Duration::from_millis(100)).await;
            }
            Action::VerifyImages => {
                uart.blocking_write(b"Verifying firmware images\r\n");
                let result = verifier
                    .verify_manifest_or_unprovisioned(
                        &flash,
                        &crypto,
                        &verification_material,
                        Slot::Active,
                        &manifest,
                        AntiRollbackPolicy { minimum_svn: 0 },
                    )
                    .await;
                if result.is_err() {
                    uart.blocking_write(b"Firmware verification failed\r\n");
                    EVENTS
                        .send(EventRecord::new(
                            Event::VerificationFailed,
                            record.component,
                        ))
                        .await;
                    continue;
                }
                uart.blocking_write(b"Firmware verification passed\r\n");
            }
            Action::ReleasePlatform => {
                let filter_policy = if manifest.images.is_empty() {
                    SpiFilterPolicy::unprovisioned_bypass()
                } else {
                    SpiFilterPolicy::from_manifest(&manifest)
                };
                uart.blocking_write(b"Applying SPI filter policy\r\n");
                platform.apply_spi_filter_policy(&filter_policy);
                uart.blocking_write(b"Resetting target SPI flashes and releasing AST2700\r\n");
                platform.release_bootmcu_from_reset().await;
                uart.blocking_write(b"AST2700 BootMCU release sequence complete\r\n");
            }
            Action::StartMonitoring => {
                uart.blocking_write(b"PFR monitor started\r\n");
            }
            Action::RecoverImages => {
                uart.blocking_write(b"Recovering firmware images: mocked complete\r\n");
            }
            Action::Lockdown => {
                uart.blocking_write(b"PFR lockdown entered\r\n");
            }
            Action::None => {}
        }

        if let Some(next) = Runtime::success_event(transition.action, record.component) {
            EVENTS.send(next).await;
        }
    }
}
