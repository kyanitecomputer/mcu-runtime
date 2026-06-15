//! AST2700 BootMCU: SLI (CPU die ↔ IO die) link bringup + DRAM training.
//!
//! Sequence:
//!   1. SLI init_f — calibrate AHB + MBUS downstream from IO-die side
//!   2. DRAM training (DRAM is on the IO die, requires SLI)
//!   3. SLI init_r — wait for IO-die ready, restore AHB timeouts,
//!                   calibrate Video SLI
//!
//! On success: "SLI OK" printed and DRAM accessible.

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_aspeed::pac::sdrammc_ast2700_v1::DdrType;
use embassy_aspeed::pac::sdrammc_ast2700_v1::DramSize;
use embassy_aspeed::scu;
use embassy_aspeed::sdrammc::DramError;
use embassy_aspeed::sli::SliError;
use embassy_executor::Spawner;
use hal::uart::{Config, Uart};

use defmt_rtt as _;
use panic_halt as _;

const DRAM_BASE: usize = 0x8000_0000;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = Uart::new_uart12(Config::default());
    uart.blocking_write(b"\r\n=== SLI + DRAM Init ===\r\n");

    let (dev, hw) = scu::silicon_rev();
    uart.blocking_write(b"Device: ");
    uart.blocking_write(match dev {
        scu::DeviceId::Ast2750 => b"AST2750",
        scu::DeviceId::Ast2700 => b"AST2700",
        scu::DeviceId::Ast2720 => b"AST2720",
        scu::DeviceId::Unknown(_) => b"Unknown",
    });
    uart.blocking_write(b" Rev: ");
    uart.blocking_write(match hw {
        scu::HwRev::A0 => b"A0",
        scu::HwRev::A1 => b"A1",
        scu::HwRev::Unknown(_) => b"??",
    });
    uart.blocking_write(b"\r\n");

    // Step 1: SLI init_f (IO-die side AHB + MBUS calibration).
    uart.blocking_write(b"SLI init_f... ");
    embassy_aspeed::sli::init_f();
    uart.blocking_write(b"OK\r\n");

    // Step 2: DRAM training (requires SLI to be up — DRAM is on IO die).
    let ddr_type = scu::ddr_type_strap();
    uart.blocking_write(b"DRAM (");
    uart.blocking_write(match ddr_type {
        DdrType::DDR4 => b"DDR4",
        DdrType::DDR5 => b"DDR5",
    });
    uart.blocking_write(b") training... ");

    match embassy_aspeed::sdrammc::init(DramSize::GB2) {
        Ok(()) => uart.blocking_write(b"OK\r\n"),
        Err(e) => {
            uart.blocking_write(b"FAIL: ");
            uart.blocking_write(match e {
                DramError::PhyInitTimeout => b"PHY timeout",
                DramError::SelfRefTimeout => b"Self-ref timeout",
                DramError::BistFail => b"BIST fail",
                DramError::NoPhyFirmware => b"No PHY FW",
            });
            uart.blocking_write(b"\r\n");
            loop {}
        }
    }

    // Step 3: SLI init_r (Video SLI calibration + AHB timeout restore).
    uart.blocking_write(b"SLI init_r... ");
    match embassy_aspeed::sli::init_r() {
        Ok(()) => uart.blocking_write(b"OK\r\n"),
        Err(SliError::RemoteTimeout) => {
            uart.blocking_write(b"TIMEOUT waiting for SLI0_READY\r\n");
            loop {}
        }
    }

    // Quick DRAM smoke test.
    uart.blocking_write(b"DRAM R/W test... ");
    let patterns = [0xDEAD_BEEFu32, 0xCAFE_BABE, 0xA5A5_A5A5, 0x5A5A_5A5A];
    let ptr = DRAM_BASE as *mut u32;
    let ok = unsafe {
        for (i, &v) in patterns.iter().enumerate() {
            core::ptr::write_volatile(ptr.add(i), v);
        }
        patterns
            .iter()
            .enumerate()
            .all(|(i, &exp)| core::ptr::read_volatile(ptr.add(i)) == exp)
    };
    uart.blocking_write(if ok { b"PASS\r\n" } else { b"FAIL\r\n" });
    if !ok {
        loop {}
    }

    uart.blocking_write(b"\r\n=== SLI OK ===\r\n");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
