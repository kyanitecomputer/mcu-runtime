//! AST2700 BootMCU: DRAM training and verification.
//!
//! Runs the full SDRAMMC initialization sequence, then verifies DRAM with
//! a walking-bit pattern test across multiple address ranges.
//!
//! Hardware: AST2750-A1 DCSCM with 2 GB DDR4.

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_aspeed::pac::sdrammc_ast2700_v1::{DdrType, DramSize};
use embassy_aspeed::scu;
use embassy_aspeed::sdrammc::DramError;
use embassy_executor::Spawner;
use hal::uart::{Config, Uart};

use panic_halt as _;

const DRAM_BASE: usize = 0x8000_0000;

fn print_hex(uart: &mut Uart, val: u32) {
    let mut buf = [b'0'; 8];
    for i in 0..8 {
        let nibble = ((val >> ((7 - i) * 4)) & 0xF) as u8;
        buf[i] = if nibble < 10 {
            b'0' + nibble
        } else {
            b'a' + nibble - 10
        };
    }
    uart.blocking_write(&buf);
}

/// Write/read-back pattern test at a given base address.
/// Returns true on success.
fn pattern_test(base: usize, patterns: &[u32]) -> bool {
    let ptr = base as *mut u32;
    unsafe {
        for (i, &v) in patterns.iter().enumerate() {
            core::ptr::write_volatile(ptr.add(i), v);
        }
        for (i, &expected) in patterns.iter().enumerate() {
            let got = core::ptr::read_volatile(ptr.add(i));
            if got != expected {
                return false;
            }
        }
    }
    true
}

/// Walking-bit test: write 1<<N at address N, read all back.
fn walking_bit_test(base: usize, count: usize) -> bool {
    let ptr = base as *mut u32;
    unsafe {
        for i in 0..count {
            let pattern = 1u32 << (i % 32);
            core::ptr::write_volatile(ptr.add(i), pattern);
        }
        for i in 0..count {
            let expected = 1u32 << (i % 32);
            let got = core::ptr::read_volatile(ptr.add(i));
            if got != expected {
                return false;
            }
        }
    }
    true
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = Uart::new_uart12(Config::default());

    uart.blocking_write(b"\r\n=== DRAM Training ===\r\n");

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

    let ddr_type = scu::ddr_type_strap();
    uart.blocking_write(b"DDR: ");
    uart.blocking_write(match ddr_type {
        DdrType::DDR4 => b"DDR4",
        DdrType::DDR5 => b"DDR5",
    });
    uart.blocking_write(b"\r\nTraining... ");

    match embassy_aspeed::sdrammc::init(DramSize::GB2) {
        Ok(()) => {
            uart.blocking_write(b"OK\r\n");
        }
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

    // Pattern test at DRAM base.
    uart.blocking_write(b"Pattern test @ 0x80000000... ");
    let patterns = [
        0xDEAD_BEEFu32,
        0xCAFE_BABE,
        0x1234_5678,
        0xA5A5_A5A5,
        0x5A5A_5A5A,
        0x0000_FFFF,
        0xFFFF_0000,
        0x0102_0304,
    ];
    if pattern_test(DRAM_BASE, &patterns) {
        uart.blocking_write(b"PASS\r\n");
    } else {
        uart.blocking_write(b"FAIL\r\n");
        loop {}
    }

    // Walking-bit test (256 words = 1 KB).
    uart.blocking_write(b"Walking-bit test (1 KB)... ");
    if walking_bit_test(DRAM_BASE + 0x1000, 256) {
        uart.blocking_write(b"PASS\r\n");
    } else {
        uart.blocking_write(b"FAIL\r\n");
        loop {}
    }

    // Test at 1 GB offset (address bit coverage).
    uart.blocking_write(b"Pattern test @ 0xC0000000... ");
    if pattern_test(DRAM_BASE + 0x4000_0000, &patterns) {
        uart.blocking_write(b"PASS\r\n");
    } else {
        uart.blocking_write(b"FAIL\r\n");
        loop {}
    }

    // Read back first 4 words and print them.
    uart.blocking_write(b"DRAM[0..3]: ");
    for i in 0..4u32 {
        let val = unsafe { core::ptr::read_volatile((DRAM_BASE as *const u32).add(i as usize)) };
        print_hex(&mut uart, val);
        uart.blocking_write(b" ");
    }
    uart.blocking_write(b"\r\n");

    uart.blocking_write(b"\r\n=== DRAM OK ===\r\n");

    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(60)).await;
    }
}
