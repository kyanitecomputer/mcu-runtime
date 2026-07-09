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
use heapless::spsc::Queue;
use log::{error, info, warn};

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
// The CA35 entry offset is no longer hardcoded: the SoC image is prefixed with
// a 16-byte boot header (imgtools a35-header) whose entry_off word is the byte
// offset of _rt0 within the raw payload. The BootMCU reads it at load time, so
// growing the cairn payload never requires bumping a constant here again.
const A2_CA35_LOAD_ADDR: usize = 0x83FF_FF60;
/// Length of the CA35 boot header prefixed to the SoC image (see manifest
/// RAW_A35_HEADER_*: magic, entry_off, payload_len, check — four u32 words).
const A2_HDR_LEN: usize = 16;

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

fn dram_error_str(err: hal::sdrammc::DramError) -> &'static str {
    use hal::sdrammc::DramError;
    match err {
        DramError::PhyInitTimeout => "PhyInitTimeout",
        DramError::SelfRefTimeout => "SelfRefTimeout",
        DramError::BistFail => "BistFail",
        DramError::NoPhyFirmware => "NoPhyFirmware",
    }
}

/// Reads the A35 boot header (written by imgtools spi-image) and returns the
/// absolute BootMCU-space entry address to jump to. Halts on an invalid header
/// rather than jumping to a guessed offset (which yields a silent hang).
fn read_a35_entry_addr() -> usize {
    let base = SPI_BASE + RAW_A35_HEADER_FLASH_OFFSET;
    let magic = rd32(base);
    let entry_off = rd32(base + 4);
    let payload_len = rd32(base + 8);
    let check = rd32(base + 12);
    if magic != RAW_A35_HEADER_MAGIC || check != (magic ^ entry_off ^ payload_len) {
        error!("A35 BOOT HEADER INVALID magic={magic:#010X} - rebuild image with --psp-elf. Halting.");
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

fn set_auth_manifest(hw: scu::HwRev) -> bool {
    if !Caliptra::is_rdy_for_rt() {
        warn!("Caliptra auth manifest SKIP (RT not ready)");
        return !secure_boot_enabled();
    }
    let manifest = match Manifest::parse_for(hw).and_then(|m| m.image_slice(HDR_ID_SOC_MANIFEST)) {
        Ok(manifest) => manifest,
        Err(_) => {
            warn!("Caliptra auth manifest SKIP (no CMAN SoC manifest)");
            return !secure_boot_enabled();
        }
    };
    match Caliptra::set_auth_manifest(manifest) {
        Ok(()) => {
            info!("Caliptra auth manifest OK");
            true
        }
        Err(_) => {
            error!("Caliptra auth manifest FAIL");
            !secure_boot_enabled()
        }
    }
}

fn populate_idevid(otp: &Otp) -> bool {
    if !Caliptra::is_rdy_for_rt() {
        warn!("Caliptra IDEVID SKIP (RT not ready)");
        return !secure_boot_enabled();
    }

    let tag = match otp.read_word(OTPCAL_IDEVID_TBS_OFFSET) {
        Ok(tag) => tag,
        Err(_) => {
            error!("Caliptra IDEVID FAIL (OTP tag read)");
            return !secure_boot_enabled();
        }
    };
    if tag == 0 {
        warn!("Caliptra IDEVID SKIP (OTP empty)");
        return !secure_boot_enabled();
    }
    if tag != 0x8230 {
        error!("Caliptra IDEVID FAIL (bad TBS tag)");
        return !secure_boot_enabled();
    }

    let len_word = match otp.read_word(OTPCAL_IDEVID_TBS_OFFSET + 1) {
        Ok(len) => len,
        Err(_) => {
            error!("Caliptra IDEVID FAIL (OTP len read)");
            return !secure_boot_enabled();
        }
    };
    let tbs_size = len_word.swap_bytes() as usize + 4;
    if tbs_size > MAX_IDEVID_ECC384_TBS_SIZE {
        error!("Caliptra IDEVID FAIL (TBS too large)");
        return !secure_boot_enabled();
    }

    let mut tbs = [0u8; MAX_IDEVID_ECC384_TBS_SIZE];
    if otp
        .read_bytes(OTPCAL_IDEVID_TBS_OFFSET, &mut tbs[..tbs_size])
        .is_err()
    {
        error!("Caliptra IDEVID FAIL (OTP TBS read)");
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
        error!("Caliptra IDEVID FAIL (OTP sig read)");
        return !secure_boot_enabled();
    }

    let mut cert_buf = [0u8; MAX_IDEVID_ECC384_CERT_SIZE];
    let cert_size =
        match Caliptra::get_idev_ecc384_cert(&tbs[..tbs_size], &sig_r, &sig_s, &mut cert_buf) {
            Ok(size) => size,
            Err(_) => {
                error!("Caliptra IDEVID FAIL (GET)");
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
            info!("Caliptra IDEVID OK");
            true
        },
        Err(_) => {
            error!("Caliptra IDEVID FAIL (POPULATE)");
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
fn authorize_ca35_a2(image_size: u32) -> bool {
    use embassy_aspeed::cptra::{ImageHashSource, IMAGE_DIGEST_SIZE};
    if !Caliptra::is_rdy_for_rt() {
        warn!("A2 authorize CA35 SKIP (RT not ready)");
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
            info!("A2 authorize CA35 OK");
            true
        }
        Ok(_) => {
            warn!("A2 authorize CA35 DENIED");
            !secure_boot_enabled()
        }
        Err(_) => {
            error!("A2 authorize CA35 FAIL");
            !secure_boot_enabled()
        }
    }
}

/// A2: locate the CA35 payload in the FLSH container, authorize it, load it to
/// DRAM at `A35_LOAD_ADDR`, and return the entry address. `None` on failure.
fn load_ca35_payload_a2(hw: scu::HwRev) -> Option<usize> {
    let manifest = match Manifest::parse_for(hw) {
        Ok(m) => m,
        Err(_) => {
            error!("A2 FLSH parse FAIL");
            return None;
        }
    };
 	let img = manifest.find(A2_FLSH_ID_CA35)?;

 	if !authorize_ca35_a2(img.size) {
 		return None;
 	}

	// The SoC image is prefixed with the 16-byte CA35 boot header. Read it via
	// XIP and recover the entry offset before copying; this removes the old
	// hardcoded A2_CA35_ENTRY_OFF that had to be bumped on every payload change.
	let hdr_addr = match manifest.image_addr(A2_FLSH_ID_CA35) {
		Ok(a) => a,
		Err(_) => return None,
	};
	let magic = rd32(hdr_addr);
	let entry_off = rd32(hdr_addr + 4) as usize;
	let payload_len = rd32(hdr_addr + 8);
	let check = rd32(hdr_addr + 12);
 	if magic != RAW_A35_HEADER_MAGIC || check != (magic ^ (entry_off as u32) ^ payload_len) {
 		error!("A35 HEADER INVALID magic={magic:#010X} - rebuild image (imgtools a35-header). Halting.");
 		return None;
 	}

	// Uncompressed: word-copy the payload (past the header) from the XIP
	// window to DRAM, so raw payload byte 0 lands at A2_CA35_LOAD_ADDR.
	// SAFETY: fixed CA35 payload window; validated by the manifest bounds.
	unsafe {
		embassy_aspeed::manifest::copy32(
			A2_CA35_LOAD_ADDR,
			hdr_addr + A2_HDR_LEN,
			img.size as usize - A2_HDR_LEN,
		)
	};
 	info!("Load A35 (A2 FLSH) OK");
 	Some(A2_CA35_LOAD_ADDR + entry_off)
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
    // UART12 is ROM-configured; install the log-crate global logger over it.
    hal::log_uart::init(log::LevelFilter::Info);
    info!("=== AST2700 BootMCU RoT ===");

    let (dev, hw) = scu::silicon_rev();
    let dev_str = match dev {
        scu::DeviceId::Ast2750 => "AST2750",
        _ => "AST27xx",
    };
    let hw_str = match hw {
        scu::HwRev::A1 => "A1",
        scu::HwRev::A2 => "A2",
        _ => "??",
    };
    info!("{dev_str} {hw_str}");

    hal::wdt_ast2700::init();
    hal::extrst::init();

    let boot_mode = bootmode::detect();
    info!("Boot mode: {}", boot_mode.as_str());

    hal::sli::init_f();
    match hal::sli::init_r() {
        Ok(()) => {}
        Err(_) => {
            error!("SLI TIMEOUT");
            loop {}
        }
    }
    display::early_crt_clock_select();
    init_mac_rgmii_clk();
    info!("MAC clk_sel1={:#010X}", rd32(SCU1_CLK_SEL1));
    scu::apply_ibex_default_register_policy();

    match hal::sdrammc::init() {
        Ok(()) => info!("DRAM OK"),
        Err(err) => {
            error!("DRAM FAIL {}", dram_error_str(err));
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
    info!("DP bring-up... {}", if display::bring_up_dp() { "UP" } else { "SKIP" });

    ca35::init_ufs_axi_path();
    ca35::init_pci_e2m();

    spi::ast2700_fmc_enable_ce0_4byte_addr();

    let otp = match Otp::new() {
        Ok(otp) => otp,
        Err(_) => {
            error!("OTP init FAIL");
            if secure_boot_enabled() {
                loop {}
            }
            Otp::with_ecc_enabled(false)
        }
    };

    if !set_auth_manifest(hw) {
        loop {}
    }
    if !populate_idevid(&otp) {
        loop {}
    }

    // A2 boots from the FLSH container: locate + authorize + load the CA35
    // payload by identifier. A1 (and unknown steppings) use the raw-header path.
    let a35_entry_addr = if hw == scu::HwRev::A2 {
        match load_ca35_payload_a2(hw) {
            Some(entry) => {
                info!("A2 CA35 entry={:#010X}", entry as u32);
                entry
            }
            None => {
                error!("A2 CA35 load FAIL");
                loop {}
            }
        }
    } else {
        let a35_entry_addr = read_a35_entry_addr();

        info!(
            "A35 load={:#010X} entry={:#010X} size={:#010X}",
            A35_LOAD_ADDR as u32, a35_entry_addr as u32, A35_PAYLOAD_SIZE as u32
        );

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
            error!("Load A35 FAIL");
            loop {}
        }
        info!("Load A35 OK");
        a35_entry_addr
    };

    let ssp_loaded = load_payload_from_boot_media(
        boot_mode,
        RAW_SSP_PAYLOAD_FLASH_OFFSET,
        SSP_LOAD_ADDR,
        M4_PAYLOAD_SIZE,
    );
    info!("Load SSP {}", if ssp_loaded { "OK" } else { "SKIP" });

    let tsp_loaded = load_payload_from_boot_media(
        boot_mode,
        RAW_TSP_PAYLOAD_FLASH_OFFSET,
        TSP_LOAD_ADDR,
        M4_PAYLOAD_SIZE,
    );
    info!("Load TSP {}", if tsp_loaded { "OK" } else { "SKIP" });

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

    info!("RVBAR0={:#010X}", rd32(0x12C0_2110));

    // Confirm the CA35 reset-vector fetch target is present in DRAM (BootMCU
    // view) just before release. After release the CA35 owns UART12, so the
    // BootMCU stays silent from here on to avoid interleaving with the CA35
    // console output.
    info!(
        "CA35 entry {:#010X}={:#010X}",
        a35_entry_addr as u32,
        rd32(a35_entry_addr)
    );
    info!("BootMCU done, releasing CA35 (UART -> CA35).");
    log::logger().flush();

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
