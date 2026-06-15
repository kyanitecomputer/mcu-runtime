//! AST2700 BootMCU: Caliptra boot handshake and status query.
//!
//! Boot sequence (ROM → FMC):
//!   ROM:  asserts PWRGOOD, writes fuses over APB, signals FUSE_WR_DONE,
//!         loads Caliptra FW from SPI flash via FWLD mailbox.
//!   FMC:  feeds TRNG entropy until Caliptra signals RDY_FOR_RT,
//!         then queries FW_INFO, CAPABILITIES, FIPS_VERSION.
//!
//! Hardware-verified on AST2750-A1 via SPI flash boot.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin caliptra_hello_bootmcu --features ast2700-bootmcu \
//!     --target riscv32imc-unknown-none-elf --release
//! ```

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_executor::Spawner;
use hal::cptra::Caliptra;
use hal::uart::{Config, Uart};

use defmt_rtt as _;
use panic_halt as _;

// Timer counter low word for entropy.
const TIMER_COUNT_L: *const u32 = 0x14C3_6000 as *const u32;

/// Cheap entropy from the free-running 1 MHz timer counter.
/// Adequate for pre-DRAM boot; good enough for Caliptra TRNG seeding
/// since Caliptra also mixes in its own internal entropy sources.
fn timer_entropy() -> [u32; 12] {
    let mut e = [0u32; 12];
    for (i, word) in e.iter_mut().enumerate() {
        // XOR counter with rotation to introduce pattern variation.
        let raw = unsafe { core::ptr::read_volatile(TIMER_COUNT_L) };
        *word = raw.rotate_left(i as u32 * 5) ^ (i as u32 * 0x9e3779b9);
    }
    e
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());

    let mut uart = Uart::new_uart12(Config::default());
    uart.blocking_write(b"\r\n=== AST2700 BootMCU Caliptra ===\r\n");

    // ── Print initial IFC state ───────────────────────────────────────────
    {
        let flow = Caliptra::flow_status();
        let boot = Caliptra::boot_status();
        let (hw_fatal, hw_nf, fw_fatal, fw_nf) = Caliptra::errors();
        let scu1_rdy = Caliptra::is_rdy_for_rt();

        uart.blocking_write(b"FLOW_STS=0x");
        write_hex(&mut uart, flow.0);
        uart.blocking_write(b" [");
        if flow.rdy_for_fuses() {
            uart.blocking_write(b"RDY_FUSES ");
        }
        if flow.rdy_for_fw() {
            uart.blocking_write(b"RDY_FW ");
        }
        if flow.rdy_for_rt() {
            uart.blocking_write(b"RDY_RT ");
        }
        uart.blocking_write(b"]\r\n");

        uart.blocking_write(b"BOOT_STS=0x");
        write_hex(&mut uart, boot.0);
        uart.blocking_write(b" SCU1_RDY_RT=");
        uart.blocking_write(if scu1_rdy { b"1" } else { b"0" });
        uart.blocking_write(b"\r\n");

        if hw_fatal != 0 || hw_nf != 0 || fw_fatal != 0 || fw_nf != 0 {
            uart.blocking_write(b"ERRORS: hw_fatal=0x");
            write_hex(&mut uart, hw_fatal);
            uart.blocking_write(b" hw_nf=0x");
            write_hex(&mut uart, hw_nf);
            uart.blocking_write(b" fw_fatal=0x");
            write_hex(&mut uart, fw_fatal);
            uart.blocking_write(b" fw_nf=0x");
            write_hex(&mut uart, fw_nf);
            uart.blocking_write(b"\r\n");
        }
    }

    // ── Feed TRNG until RDY_FOR_RT ────────────────────────────────────────
    // Caliptra needs external entropy during its internal FMC/RT boot.
    // Without continuous feeding the boot stalls.  The ROM loaded Caliptra
    // FW from SPI flash before jumping here, so Caliptra is already booting.
    uart.blocking_write(b"Waiting for RDY_FOR_RT (feeding TRNG)...\r\n");

    // 50M loops ≈ several minutes at 50 MHz — generous margin.
    match Caliptra::wait_rdy_for_rt_with_trng(timer_entropy, 50_000_000) {
        Ok(()) => {
            uart.blocking_write(b"Caliptra RDY_FOR_RT\r\n");
        }
        Err(hal::cptra::CptraError::HwFatal(code)) => {
            uart.blocking_write(b"HW_FATAL=0x");
            write_hex(&mut uart, code);
            uart.blocking_write(b" - Caliptra fatal error\r\n");
            loop {}
        }
        Err(hal::cptra::CptraError::Timeout) => {
            uart.blocking_write(b"Timeout waiting for RDY_FOR_RT\r\n");
            // Print final state for diagnosis.
            let flow = Caliptra::flow_status();
            uart.blocking_write(b"FLOW_STS=0x");
            write_hex(&mut uart, flow.0);
            uart.blocking_write(b"\r\n");
            loop {}
        }
        Err(e) => {
            uart.blocking_write(b"Error: ");
            uart.blocking_write(err_str(e));
            uart.blocking_write(b"\r\n");
            loop {}
        }
    }

    // ── FW_INFO ───────────────────────────────────────────────────────────
    uart.blocking_write(b"\r\n--- FW_INFO ---\r\n");
    match Caliptra::fw_info() {
        Ok(info) => {
            uart.blocking_write(b"  fips_status:      0x");
            write_hex(&mut uart, info.fips_status);
            uart.blocking_write(b"\r\n  pl0_pauser:       0x");
            write_hex(&mut uart, info.pl0_pauser);
            uart.blocking_write(b"\r\n  runtime_svn:      0x");
            write_hex(&mut uart, info.runtime_svn);
            uart.blocking_write(b"\r\n  min_runtime_svn:  0x");
            write_hex(&mut uart, info.min_runtime_svn);
            uart.blocking_write(b"\r\n  fmc_manifest_svn: 0x");
            write_hex(&mut uart, info.fmc_manifest_svn);
            uart.blocking_write(b"\r\n  rom_rev: ");
            for b in &info.rom_revision {
                write_hex_byte(&mut uart, *b);
            }
            uart.blocking_write(b"\r\n  fmc_rev: ");
            for b in &info.fmc_revision {
                write_hex_byte(&mut uart, *b);
            }
            uart.blocking_write(b"\r\n  rt_rev:  ");
            for b in &info.runtime_revision {
                write_hex_byte(&mut uart, *b);
            }
            uart.blocking_write(b"\r\n");
        }
        Err(e) => {
            uart.blocking_write(b"  ERROR: ");
            uart.blocking_write(err_str(e));
            uart.blocking_write(b"\r\n");
        }
    }

    // ── CAPABILITIES ──────────────────────────────────────────────────────
    uart.blocking_write(b"\r\n--- CAPABILITIES ---\r\n");
    match Caliptra::capabilities() {
        Ok(cap) => {
            uart.blocking_write(b"  fips_status: 0x");
            write_hex(&mut uart, cap.fips_status);
            uart.blocking_write(b"\r\n  caps: 0x");
            for b in &cap.capabilities {
                write_hex_byte(&mut uart, *b);
            }
            uart.blocking_write(b"\r\n");
        }
        Err(e) => {
            uart.blocking_write(b"  ERROR: ");
            uart.blocking_write(err_str(e));
            uart.blocking_write(b"\r\n");
        }
    }

    // ── FIPS_VERSION ──────────────────────────────────────────────────────
    uart.blocking_write(b"\r\n--- FIPS_VERSION ---\r\n");
    match Caliptra::fips_version() {
        Ok(ver) => {
            uart.blocking_write(b"  fips_status: 0x");
            write_hex(&mut uart, ver.fips_status);
            uart.blocking_write(b"\r\n  mode:  0x");
            write_hex(&mut uart, ver.mode);
            uart.blocking_write(b"\r\n  name:  \"");
            let end = ver.name.iter().position(|&b| b == 0).unwrap_or(12);
            uart.blocking_write(&ver.name[..end]);
            uart.blocking_write(b"\"\r\n  rev:   ");
            for &r in &ver.fips_rev {
                write_hex(&mut uart, r);
                uart.blocking_write(b" ");
            }
            uart.blocking_write(b"\r\n");
        }
        Err(e) => {
            uart.blocking_write(b"  ERROR: ");
            uart.blocking_write(err_str(e));
            uart.blocking_write(b"\r\n");
        }
    }

    uart.blocking_write(b"\r\n=== Done ===\r\n");

    // Keep TRNG fed — Caliptra may continue requesting entropy at runtime.
    loop {
        Caliptra::feed_trng(&timer_entropy());
        embassy_time::Timer::after(embassy_time::Duration::from_millis(10)).await;
    }
}

fn err_str(e: hal::cptra::CptraError) -> &'static [u8] {
    match e {
        hal::cptra::CptraError::MboxBusy => b"MboxBusy",
        hal::cptra::CptraError::NotReady => b"NotReady",
        hal::cptra::CptraError::CmdFailed => b"CmdFailed",
        hal::cptra::CptraError::Timeout => b"Timeout",
        hal::cptra::CptraError::HwFatal(_) => b"HwFatal",
    }
}

fn write_hex(uart: &mut Uart, val: u32) {
    const H: &[u8; 16] = b"0123456789abcdef";
    for i in (0..8).rev() {
        uart.write_byte(H[((val >> (i * 4)) & 0xF) as usize]);
    }
}

fn write_hex_byte(uart: &mut Uart, val: u8) {
    const H: &[u8; 16] = b"0123456789abcdef";
    uart.write_byte(H[(val >> 4) as usize]);
    uart.write_byte(H[(val & 0xF) as usize]);
}
