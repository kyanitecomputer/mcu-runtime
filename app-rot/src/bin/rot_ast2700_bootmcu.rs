//! AST2700 BootMCU — Root of Trust firmware.
//!
//! Boot sequence: WDT/EXTRST masks → SLI calibration → SCU policy →
//! DRAM init → fabric/PCI → load payloads → MPU → release CA35/SSP/TSP.

#![no_std]
#![no_main]

use embassy_aspeed as hal;
use embassy_aspeed::bootmode::{self, BootMode};
use embassy_aspeed::ca35;
use embassy_aspeed::cptra::{Caliptra, MAX_IDEVID_ECC384_CERT_SIZE, MAX_IDEVID_ECC384_TBS_SIZE};
use embassy_aspeed::display;
use embassy_aspeed::ipc1::{Ipc1, Payload};
use embassy_aspeed::manifest::{
    self, Manifest, HDR_ID_SOC_MANIFEST, RAW_A35_HEADER_FLASH_OFFSET, RAW_A35_HEADER_MAGIC,
    RAW_A35_PAYLOAD_FLASH_OFFSET,
};
use embassy_aspeed::otp::Otp;
use embassy_aspeed::scu;
use embassy_aspeed::spi;
use embassy_aspeed::ssp_tsp;
use embassy_executor::Spawner;
use hal::uart::{Config, Uart};
use heapless::spsc::Queue;

use panic_halt as _;

const A35_PAYLOAD_SIZE: usize = 16 * 1024 * 1024;
const A35_LOAD_ADDR: usize = 0x83FF_FF60;
// Load-placement compensation: A35_LOAD_ADDR + A35_LOAD_FUDGE is the BootMCU-space
// address corresponding to the A35 binary's link text base. Adding the header's
// entry_off (offset of _rt0 within the raw payload) yields the entry address.
// Preserves the historically-proven mapping (was baked into A35_ENTRY_RAW_OFFSET
// as +0xA0 over the true file offset).
const A35_LOAD_FUDGE: usize = 0xA0;
const DRAM_SIZE_BYTES: u64 = 1024 * 1024 * 1024;
const RAW_SSP_PAYLOAD_FLASH_OFFSET: usize = 0x180_0000;
const RAW_TSP_PAYLOAD_FLASH_OFFSET: usize = 0x182_0000;
const M4_PAYLOAD_SIZE: usize = 128 * 1024;
const SSP_LOAD_ADDR: usize = 0xAC00_0000;
const TSP_LOAD_ADDR: usize = 0xAE00_0000;
const SPI_BASE: usize = 0x2000_0000;
const PAYLOAD_LOAD_ATTEMPTS: usize = 3;
const IPC_SECURE_CA35: u8 = 0;
const SCU1_HWSTRAP1: usize = 0x14C0_2010;
const HWSTRAP1_EN_SECBOOT: u32 = 1 << 5;
const OTPCAL_IDEVID_TBS_OFFSET: u32 = embassy_aspeed::otp::CALIPTRA_START + 0x62;
const OTPCAL_IDEVID_SIGN_OFFSET: u32 = embassy_aspeed::otp::CALIPTRA_START + 0x262;

// ── AST2700-A2 boot parameters ──────────────────────────────────────────────
// A2 boots from a top-level FLSH container (see embassy_aspeed::flsh) instead
// of the A1 raw-header scheme. The CA35 payload is one of the SoC images.
//
// PROVISIONAL — confirm/decouple before relying on these:
//   * A2_FLSH_ID_CA35 is coupled to the --soc-image order in cairn's
//     tools/a2/gen-a2-image.sh (SoC ids start at 0x1000; cairn is the 8th).
//     A dedicated well-known id would be more robust.
//   * A2_CA35_FW_ID must match the cairn.raw.bin fw_id in cairn-a2-manifest.toml.
//   * A2_CA35_ENTRY_OFF is the offset of _rt0 within the raw payload
//     (e_entry - link text base, currently -T 0x404000000). Same role as the
//     A1 header's entry_off; a small header or manifest field would avoid the
//     hardcode.
const A2_FLSH_ID_CA35: u32 = embassy_aspeed::flsh::ID_SOC_IMAGES_BASE + 7;
const A2_CA35_FW_ID: u32 = 9;
// CA35 DRAM mapping: the CA35 sees DRAM at 0x4_0000_0000, the BootMCU at
// 0x8000_0000 (CA35 = BootMCU + 0x3_8000_0000, confirmed via RVBAR readback).
// The cairn payload is position-dependent, so raw-byte 0 of the payload must
// land at the BootMCU address that the CA35 sees as the payload's link vaddr.
//
// CRITICAL: `objcopy -O binary` starts the flat image at the first *allocatable
// section* VMA, NOT the LOAD *segment* vaddr. For this cairn build:
//   first PT_LOAD vaddr   = 0x4_03ff_f000   (segment, includes 0xf60 of ELF hdr
//                                            padding that objcopy does NOT emit)
//   first section (.note) = 0x4_03ff_ff60   -> raw-bin offset 0  -> load here
//   .text                 = 0x4_0400_0000   (raw-bin offset 0xa0)
//   e_entry               = 0x4_0407_dc00   -> raw-bin offset 0x7_dca0
// So raw[0] must land at CA35 0x4_03ff_ff60 = BootMCU 0x83ff_ff60, which places
// every section at its correct VMA; the entry is then LOAD_ADDR + 0x7_dca0 =
// 0x8407_dc00 (RVBAR 0x4040_7dc0). Verify after every cairn change with:
//   LOAD = firstSectionVMA - 0x3_8000_0000, OFF = e_entry - firstSectionVMA.
//
// PROVISIONAL — offsets move whenever the cairn build changes; a per-image
// header or manifest field should replace these hardcodes (tracked as follow-up).
const A2_CA35_LOAD_ADDR: usize = 0x83FF_FF60;
const A2_CA35_ENTRY_OFF: usize = 0x7_DCA0;
/// Whether the CA35 payload SoC image is LZ4-compressed (size-prepended). The
/// current gen-a2-image.sh stores it uncompressed; flip once the pipeline
/// compresses it.
const A2_CA35_LZ4: bool = false;

static mut IDEVID_CERT: [u8; MAX_IDEVID_ECC384_CERT_SIZE] = [0; MAX_IDEVID_ECC384_CERT_SIZE];
static mut IDEVID_CERT_SIZE: usize = 0;

fn rd32(addr: usize) -> u32 {
    unsafe { core::ptr::read_volatile(addr as *const u32) }
}

const SCU1_CLK_SEL1: usize = 0x14C0_2280;

/// Program the MAC/RGMII/RMII source-clock dividers in SCU1 clk_sel1.
///
/// HPLL is 1000 MHz on this board (SCU1 hpll reg 0x10000027), so:
///   MAC core 200 MHz: div_idx 4 (HPLL/5)  -> bits[31:29]
///   RGMII    125 MHz: div_idx 3 (HPLL/8)  -> bits[27:25]
///   RMII      50 MHz: div_idx 4 (HPLL/20) -> bits[23:21]
///
/// This mirrors u-boot clk_ast2700.c ast2700_init_{mac,rgmii,rmii}_clk and the
/// vendor's dumped clk_sel1 = 0x86910000. Without it the RGMII 125 MHz TX clock
/// is left at divider 0 (unconfigured): MAC0 RX works off the PHY-sourced RXC,
/// but TX runs on a wrong SoC clock and large frames lose their tail.
///
/// MUST run before `apply_ibex_default_register_policy()`, which locks these
/// clk_sel1 fields (SYS_POLICY_CLK1_SEL1_LOCK) — after the lock they are
/// read-only to both the BootMCU and the CA35.
fn init_mac_rgmii_clk() {
    const MASK: u32 = 0xEEE0_0000; // [31:29] | [27:25] | [23:21]
    let val = (4u32 << 29) | (3u32 << 25) | (4u32 << 21);
    unsafe {
        let p = SCU1_CLK_SEL1 as *mut u32;
        let cur = core::ptr::read_volatile(p);
        core::ptr::write_volatile(p, (cur & !MASK) | val);
    }
}

fn print_hex32(uart: &mut Uart, val: u32) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut buf = [0u8; 10];
    buf[0] = b'0';
    buf[1] = b'x';
    for i in 0..8 {
        buf[2 + (7 - i)] = HEX[((val >> (i * 4)) & 0xF) as usize];
    }
    uart.blocking_write(&buf);
}

fn print_dram_error(uart: &mut Uart, err: hal::sdrammc::DramError) {
    use hal::sdrammc::DramError;
    uart.blocking_write(match err {
        DramError::PhyInitTimeout => b"PhyInitTimeout",
        DramError::SelfRefTimeout => b"SelfRefTimeout",
        DramError::BistFail => b"BistFail",
        DramError::NoPhyFirmware => b"NoPhyFirmware",
    });
}

/// Reads the A35 boot header (written by imgtools spi-image) and returns the
/// absolute BootMCU-space entry address to jump to. Halts on an invalid header
/// rather than jumping to a guessed offset (which yields a silent hang).
fn read_a35_entry_addr(uart: &mut Uart) -> usize {
    let base = SPI_BASE + RAW_A35_HEADER_FLASH_OFFSET;
    let magic = rd32(base);
    let entry_off = rd32(base + 4);
    let payload_len = rd32(base + 8);
    let check = rd32(base + 12);
    if magic != RAW_A35_HEADER_MAGIC || check != (magic ^ entry_off ^ payload_len) {
        uart.blocking_write(b"A35 BOOT HEADER INVALID magic=");
        print_hex32(uart, magic);
        uart.blocking_write(b" - rebuild image with --psp-elf. Halting.\r\n");
        loop {}
    }
    A35_LOAD_ADDR + A35_LOAD_FUDGE + entry_off as usize
}

fn verify_payload(flash_offset: usize, dst: usize, len: usize) -> bool {
    let src = SPI_BASE + flash_offset;
    let mut ok = true;
    let mut non_blank = false;
    for i in 0..4usize {
        let s = rd32(src + i * 4);
        let d = rd32(dst + i * 4);
        ok &= s == d;
        non_blank |= s != 0 && s != 0xFFFF_FFFF;
    }
    if len >= 16 {
        let tail = len - 16;
        for i in 0..4usize {
            let s = rd32(src + tail + i * 4);
            let d = rd32(dst + tail + i * 4);
            ok &= s == d;
        }
    }
    ok && non_blank
}

fn load_payload(flash_offset: usize, dst: usize, len: usize) -> bool {
    for _ in 0..PAYLOAD_LOAD_ATTEMPTS {
        spi::ast2700_fmc_enable_ce0_4byte_addr();
        if spi::ast2700_fmc_dma_read_sync(flash_offset, dst, len)
            && verify_payload(flash_offset, dst, len)
        {
            return true;
        }
        // SAFETY: The destination address and size are fixed BootMCU payload
        // windows and are validated by verify_payload before use.
        unsafe { manifest::load_raw(flash_offset, dst, len) };
        if verify_payload(flash_offset, dst, len) {
            return true;
        }
    }
    false
}

fn load_payload_word_copy_first(flash_offset: usize, dst: usize, len: usize) -> bool {
    for _ in 0..PAYLOAD_LOAD_ATTEMPTS {
        spi::ast2700_fmc_enable_ce0_4byte_addr();
        // SAFETY: The destination address and size are fixed BootMCU payload
        // windows and are validated by verify_payload before use.
        unsafe { manifest::load_raw(flash_offset, dst, len) };
        if verify_payload(flash_offset, dst, len) {
            return true;
        }
    }
    false
}

fn load_payload_from_boot_media(
    mode: BootMode,
    flash_offset: usize,
    dst: usize,
    len: usize,
) -> bool {
    match mode {
        BootMode::Emmc => match hal::emmc::Emmc::init() {
            Ok(emmc) => {
                // SAFETY: The destination address and size are fixed BootMCU
                // payload windows selected by the boot flow.
                unsafe { emmc.copy(dst, flash_offset as u32, len) }.is_ok()
            }
            Err(_) => false,
        },
        BootMode::NorFlash => load_payload(flash_offset, dst, len),
        _ => false,
    }
}

fn entropy(seed: &mut u32) -> [u32; 12] {
    let mut out = [0u32; 12];
    for word in &mut out {
        *seed ^= *seed << 13;
        *seed ^= *seed >> 17;
        *seed ^= *seed << 5;
        *word = *seed ^ rd32(0x14C3_6000);
    }
    out
}

fn service_caliptra(seed: &mut u32) {
    let e = entropy(seed);
    Caliptra::feed_trng(&e);
}

fn secure_boot_enabled() -> bool {
    rd32(SCU1_HWSTRAP1) & HWSTRAP1_EN_SECBOOT != 0
}

fn set_auth_manifest(uart: &mut Uart, hw: scu::HwRev) -> bool {
    uart.blocking_write(b"Caliptra auth manifest... ");
    if !Caliptra::is_rdy_for_rt() {
        uart.blocking_write(b"SKIP (RT not ready)\r\n");
        return !secure_boot_enabled();
    }
    let manifest = match Manifest::parse_for(hw).and_then(|m| m.image_slice(HDR_ID_SOC_MANIFEST)) {
        Ok(manifest) => manifest,
        Err(_) => {
            uart.blocking_write(b"SKIP (no CMAN SoC manifest)\r\n");
            return !secure_boot_enabled();
        }
    };
    match Caliptra::set_auth_manifest(manifest) {
        Ok(()) => {
            uart.blocking_write(b"OK\r\n");
            true
        }
        Err(_) => {
            uart.blocking_write(b"FAIL\r\n");
            !secure_boot_enabled()
        }
    }
}

fn populate_idevid(uart: &mut Uart, otp: &Otp) -> bool {
    uart.blocking_write(b"Caliptra IDEVID... ");
    if !Caliptra::is_rdy_for_rt() {
        uart.blocking_write(b"SKIP (RT not ready)\r\n");
        return !secure_boot_enabled();
    }

    let tag = match otp.read_word(OTPCAL_IDEVID_TBS_OFFSET) {
        Ok(tag) => tag,
        Err(_) => {
            uart.blocking_write(b"FAIL (OTP tag read)\r\n");
            return !secure_boot_enabled();
        }
    };
    if tag == 0 {
        uart.blocking_write(b"SKIP (OTP empty)\r\n");
        return !secure_boot_enabled();
    }
    if tag != 0x8230 {
        uart.blocking_write(b"FAIL (bad TBS tag)\r\n");
        return !secure_boot_enabled();
    }

    let len_word = match otp.read_word(OTPCAL_IDEVID_TBS_OFFSET + 1) {
        Ok(len) => len,
        Err(_) => {
            uart.blocking_write(b"FAIL (OTP len read)\r\n");
            return !secure_boot_enabled();
        }
    };
    let tbs_size = len_word.swap_bytes() as usize + 4;
    if tbs_size > MAX_IDEVID_ECC384_TBS_SIZE {
        uart.blocking_write(b"FAIL (TBS too large)\r\n");
        return !secure_boot_enabled();
    }

    let mut tbs = [0u8; MAX_IDEVID_ECC384_TBS_SIZE];
    if otp
        .read_bytes(OTPCAL_IDEVID_TBS_OFFSET, &mut tbs[..tbs_size])
        .is_err()
    {
        uart.blocking_write(b"FAIL (OTP TBS read)\r\n");
        return !secure_boot_enabled();
    }

    let mut sig_r = [0u8; 48];
    let mut sig_s = [0u8; 48];
    if otp
        .read_bytes(OTPCAL_IDEVID_SIGN_OFFSET, &mut sig_r)
        .is_err()
        || otp
            .read_bytes(OTPCAL_IDEVID_SIGN_OFFSET + 0x18, &mut sig_s)
            .is_err()
    {
        uart.blocking_write(b"FAIL (OTP sig read)\r\n");
        return !secure_boot_enabled();
    }

    let mut cert_buf = [0u8; MAX_IDEVID_ECC384_CERT_SIZE];
    let cert_size =
        match Caliptra::get_idev_ecc384_cert(&tbs[..tbs_size], &sig_r, &sig_s, &mut cert_buf) {
            Ok(size) => size,
            Err(_) => {
                uart.blocking_write(b"FAIL (GET)\r\n");
                return !secure_boot_enabled();
            }
        };

    match Caliptra::populate_idev_ecc384_cert(&cert_buf[..cert_size]) {
        Ok(()) => unsafe {
            core::ptr::copy_nonoverlapping(
                cert_buf.as_ptr(),
                core::ptr::addr_of_mut!(IDEVID_CERT) as *mut u8,
                cert_size,
            );
            IDEVID_CERT_SIZE = cert_size;
            uart.blocking_write(b"OK\r\n");
            true
        },
        Err(_) => {
            uart.blocking_write(b"FAIL (POPULATE)\r\n");
            !secure_boot_enabled()
        }
    }
}

/// A2: authorize the CA35 payload against the installed SoC auth-manifest.
///
/// Best-effort on an unprovisioned dev board (mirrors `set_auth_manifest`): a
/// failure is tolerated only when secure boot is disabled. Uses `LoadAddress`
/// so Caliptra hashes the image at the load address recorded in the manifest
/// metadata for `A2_CA35_FW_ID` (no in-BootMCU digest needed).
fn authorize_ca35_a2(uart: &mut Uart, image_size: u32) -> bool {
    use embassy_aspeed::cptra::{ImageHashSource, IMAGE_DIGEST_SIZE};
    uart.blocking_write(b"A2 authorize CA35... ");
    if !Caliptra::is_rdy_for_rt() {
        uart.blocking_write(b"SKIP (RT not ready)\r\n");
        return !secure_boot_enabled();
    }
    // digest is ignored for LoadAddress; Caliptra hashes the manifest-recorded
    // image_load_address itself.
    let digest = [0u8; IMAGE_DIGEST_SIZE];
    match Caliptra::authorize_and_stash(
        A2_CA35_FW_ID,
        &digest,
        0,
        true,
        ImageHashSource::LoadAddress,
        image_size,
    ) {
        Ok(r) if r.is_authorized() => {
            uart.blocking_write(b"OK\r\n");
            true
        }
        Ok(_) => {
            uart.blocking_write(b"DENIED\r\n");
            !secure_boot_enabled()
        }
        Err(_) => {
            uart.blocking_write(b"FAIL\r\n");
            !secure_boot_enabled()
        }
    }
}

/// A2: locate the CA35 payload in the FLSH container, authorize it, load it to
/// DRAM at `A35_LOAD_ADDR`, and return the entry address. `None` on failure.
fn load_ca35_payload_a2(uart: &mut Uart, hw: scu::HwRev) -> Option<usize> {
    let manifest = match Manifest::parse_for(hw) {
        Ok(m) => m,
        Err(_) => {
            uart.blocking_write(b"A2 FLSH parse FAIL\r\n");
            return None;
        }
    };
    let img = manifest.find(A2_FLSH_ID_CA35)?;

    if !authorize_ca35_a2(uart, img.size) {
        return None;
    }

    uart.blocking_write(b"Load A35 (A2 FLSH)... ");
    if A2_CA35_LZ4 {
        // Compressed payload: expand straight into the DRAM load window.
        let src = match manifest.image_slice(A2_FLSH_ID_CA35) {
            Ok(s) => s,
            Err(_) => return None,
        };
        // SAFETY: A2_CA35_LOAD_ADDR..+A35_PAYLOAD_SIZE is the fixed CA35 payload
        // window in DRAM and does not overlap any live reference.
        let dst = unsafe {
            core::slice::from_raw_parts_mut(A2_CA35_LOAD_ADDR as *mut u8, A35_PAYLOAD_SIZE)
        };
        if embassy_aspeed::lz4::decompress_size_prepended_into(src, dst).is_err() {
            uart.blocking_write(b"LZ4 FAIL\r\n");
            return None;
        }
    } else {
        // Uncompressed: word-copy from the XIP window to DRAM.
        // SAFETY: fixed CA35 payload window; validated by the manifest bounds.
        if unsafe { manifest.load_image(A2_FLSH_ID_CA35, A2_CA35_LOAD_ADDR) }.is_err() {
            uart.blocking_write(b"COPY FAIL\r\n");
            return None;
        }
    }
    uart.blocking_write(b"OK\r\n");
    Some(A2_CA35_LOAD_ADDR + A2_CA35_ENTRY_OFF)
}

#[derive(Clone, Copy)]
struct IpcEvent {
    id: u8,
    payload: Payload,
}

fn put_word(payload: &mut Payload, index: usize, value: u32) {
    payload[index * 4..index * 4 + 4].copy_from_slice(&value.to_le_bytes());
}

fn respond_ipc_status(ipc: &mut Ipc1, id: u8, request: &Payload) {
    let mut response = [0u8; 32];
    response[..4].copy_from_slice(b"BMCU");
    put_word(&mut response, 1, Caliptra::flow_status().0);
    put_word(&mut response, 2, Caliptra::boot_status().0);
    put_word(&mut response, 3, rd32(0x12C0_2110));
    response[16..20].copy_from_slice(&request[..4]);
    ipc.send(IPC_SECURE_CA35, id, &response);
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    hal::init(hal::Config::default());
    let mut uart = Uart::new_uart12(Config::default());
    uart.blocking_write(b"\r\n=== AST2700 BootMCU RoT ===\r\n");

    let (dev, hw) = scu::silicon_rev();
    uart.blocking_write(match dev {
        scu::DeviceId::Ast2750 => b"AST2750",
        _ => b"AST27xx",
    });
    uart.blocking_write(match hw {
        scu::HwRev::A1 => b" A1\r\n",
        scu::HwRev::A2 => b" A2\r\n",
        _ => b" ??\r\n",
    });

    hal::wdt_ast2700::init();
    hal::extrst::init();

    let boot_mode = bootmode::detect();
    uart.blocking_write(b"Boot mode: ");
    uart.blocking_write(boot_mode.as_str().as_bytes());
    uart.blocking_write(b"\r\n");

    hal::sli::init_f();
    match hal::sli::init_r() {
        Ok(()) => {}
        Err(_) => {
            uart.blocking_write(b"SLI TIMEOUT\r\n");
            loop {}
        }
    }
    display::early_crt_clock_select();
    init_mac_rgmii_clk();
    uart.blocking_write(b"MAC clk_sel1=");
    print_hex32(&mut uart, rd32(SCU1_CLK_SEL1));
    uart.blocking_write(b"\r\n");
    scu::apply_ibex_default_register_policy();

    uart.blocking_write(b"DRAM... ");
    match hal::sdrammc::init() {
        Ok(()) => uart.blocking_write(b"OK\r\n"),
        Err(err) => {
            uart.blocking_write(b"FAIL ");
            print_dram_error(&mut uart, err);
            uart.blocking_write(b"\r\n");
            loop {}
        }
    }

    // Bring up the DisplayPort MCU here, before the CA35 is released, mirroring
    // the vendor `dp_init` (aspeed-zephyr mcu-runtime): load the firmware into
    // IMEM, release the DPMCU core, and set the DP scratch handshake so it trains
    // the link now. Doing this while the CA35 is still held in reset avoids the
    // bus-contention race that intermittently wedges the CA35 when the core is
    // instead released late from the running payload. The DPMCU only drives the
    // DP aux/link here (no framebuffer yet), so it does not contend for DRAM; the
    // CA35 still owns the display side (VLink, CRT timing, framebuffer, scanout).
    uart.blocking_write(b"DP bring-up... ");
    uart.blocking_write(if display::bring_up_dp() {
        b"UP\r\n"
    } else {
        b"SKIP\r\n"
    });

    ca35::init_ufs_axi_path();
    ca35::init_pci_e2m();

    spi::ast2700_fmc_enable_ce0_4byte_addr();

    let otp = match Otp::new() {
        Ok(otp) => otp,
        Err(_) => {
            uart.blocking_write(b"OTP init FAIL\r\n");
            if secure_boot_enabled() {
                loop {}
            }
            Otp::with_ecc_enabled(false)
        }
    };

    if !set_auth_manifest(&mut uart, hw) {
        loop {}
    }
    if !populate_idevid(&mut uart, &otp) {
        loop {}
    }

    // A2 boots from the FLSH container: locate + authorize + load the CA35
    // payload by identifier. A1 (and unknown steppings) use the raw-header path.
    let a35_entry_addr = if hw == scu::HwRev::A2 {
        match load_ca35_payload_a2(&mut uart, hw) {
            Some(entry) => {
                print_hex32(&mut uart, entry as u32);
                uart.blocking_write(b"\r\n");
                entry
            }
            None => {
                uart.blocking_write(b"A2 CA35 load FAIL\r\n");
                loop {}
            }
        }
    } else {
        let a35_entry_addr = read_a35_entry_addr(&mut uart);

        uart.blocking_write(b"A35 load=");
        print_hex32(&mut uart, A35_LOAD_ADDR as u32);
        uart.blocking_write(b" entry=");
        print_hex32(&mut uart, a35_entry_addr as u32);
        uart.blocking_write(b" size=");
        print_hex32(&mut uart, A35_PAYLOAD_SIZE as u32);
        uart.blocking_write(b"\r\n");

        uart.blocking_write(b"Load A35... ");
        let a35_loaded = match boot_mode {
            BootMode::NorFlash => load_payload_word_copy_first(
                RAW_A35_PAYLOAD_FLASH_OFFSET,
                A35_LOAD_ADDR,
                A35_PAYLOAD_SIZE,
            ),
            _ => load_payload_from_boot_media(
                boot_mode,
                RAW_A35_PAYLOAD_FLASH_OFFSET,
                A35_LOAD_ADDR,
                A35_PAYLOAD_SIZE,
            ),
        };
        if !a35_loaded {
            uart.blocking_write(b"FAIL\r\n");
            loop {}
        }
        uart.blocking_write(b"OK\r\n");
        a35_entry_addr
    };

    uart.blocking_write(b"Load SSP... ");
    let ssp_loaded = load_payload_from_boot_media(
        boot_mode,
        RAW_SSP_PAYLOAD_FLASH_OFFSET,
        SSP_LOAD_ADDR,
        M4_PAYLOAD_SIZE,
    );
    uart.blocking_write(if ssp_loaded { b"OK\r\n" } else { b"SKIP\r\n" });

    uart.blocking_write(b"Load TSP... ");
    let tsp_loaded = load_payload_from_boot_media(
        boot_mode,
        RAW_TSP_PAYLOAD_FLASH_OFFSET,
        TSP_LOAD_ADDR,
        M4_PAYLOAD_SIZE,
    );
    uart.blocking_write(if tsp_loaded { b"OK\r\n" } else { b"SKIP\r\n" });

    let mut ipc = Ipc1::new();
    let mut ipc_queue = Queue::<IpcEvent, 8>::new();

    ca35::enable_dram_access(DRAM_SIZE_BYTES);
    ca35::set_rvbar(a35_entry_addr);
    if ssp_loaded {
        ssp_tsp::init_ssp(SSP_LOAD_ADDR, 0x0010_0000, false);
    }
    if tsp_loaded {
        ssp_tsp::init_tsp(TSP_LOAD_ADDR, 0x0010_0000, false);
    }

    uart.blocking_write(b"RVBAR0=");
    print_hex32(&mut uart, rd32(0x12C0_2110));
    uart.blocking_write(b"\r\n");

    // Confirm the CA35 reset-vector fetch target is present in DRAM (BootMCU
    // view) just before release. After release the CA35 owns UART12, so the
    // BootMCU stays silent from here on to avoid interleaving with the CA35
    // console output.
    uart.blocking_write(b"CA35 entry ");
    print_hex32(&mut uart, (A2_CA35_LOAD_ADDR + A2_CA35_ENTRY_OFF) as u32);
    uart.blocking_write(b"=");
    print_hex32(&mut uart, rd32(A2_CA35_LOAD_ADDR + A2_CA35_ENTRY_OFF));
    uart.blocking_write(b"\r\nBootMCU done, releasing CA35 (UART -> CA35).\r\n");
    embedded_io::Write::flush(&mut uart).ok();

    ca35::release();

    if ssp_loaded {
        ssp_tsp::enable_ssp();
    }
    if tsp_loaded {
        ssp_tsp::enable_tsp();
    }

    let mut seed = 0x2700_0001;
    let mut heartbeat = 0u32;
    loop {
        service_caliptra(&mut seed);
        if let Some((id, payload)) = ipc.try_recv(IPC_SECURE_CA35) {
            let _ = ipc_queue.enqueue(IpcEvent { id, payload });
        }
        if let Some(event) = ipc_queue.dequeue() {
            respond_ipc_status(&mut ipc, event.id, &event.payload);
        }
        heartbeat = heartbeat.wrapping_add(1);
        if heartbeat % 10 == 0 && Caliptra::is_rdy_for_rt() {
            let _ = Caliptra::fw_info();
        }
        embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}
