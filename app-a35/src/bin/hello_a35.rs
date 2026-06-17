//! Minimal AArch64 bare-metal "Hello from A35" for AST2700 CA35.
//!
//! Linked at 0x430000000 — the CA35 physical address corresponding to
//! ATF_LOAD_ADDR (0xB0000000 from the BootMCU / IO-die bus view).
//! Conversion: (0xB0000000 - 0x80000000) | 0x400000000 = 0x430000000.
//!
//! UART: UART12 at 0x14C33B00, 16550-compatible with reg-shift=2 (stride 4).
//! Clock: ~1.846 MHz, divisor=1 → 115384 baud (~115200).
//!
//! After printing, spins forever. No MMU, no caches, EL3 or EL2 entry.

#![no_std]
#![no_main]
#![allow(named_asm_codepoint)]

use core::arch::global_asm;

// ── Reset vector ─────────────────────────────────────────────────────────────
//
// AArch64 bare-metal entry: must be at the very start of the binary.
// Disable all interrupts, set up a minimal stack, jump to Rust main.
global_asm!(
    ".section .text.start",
    ".global _start",
    "_start:",
    // ── SCTLR_EL3 init (must be FIRST — reset value is IMPLEMENTATION DEFINED) ──
    // ATF's el3_entrypoint_common does this before any memory access.
    // Value = SCTLR_EL3_RES1 with EE/WXN/SA/A/DSSBS cleared.
    // RES1 bits: 29,28,23,22,18,16,11,5,4 = 0x30C50830
    "movz x0, #0x0830",
    "movk x0, #0x30C5, lsl #16",
    "msr sctlr_el3, x0",
    "isb",
    // Mask all interrupts at current EL.
    "msr DAIFSet, #0xf",
    // ── Early canary: write 0xCA35CA35 to DRAM base (0x400000000) ────────
    // Uses only MOVZ/MOVK (no literal pool) so it works before stack setup.
    // BootMCU reads this from 0x80000000 to confirm CA35 started.
    "movz x2, #0x4, lsl #32",    // x2 = 0x400000000
    "movz w3, #0xCA35",          // w3 = 0x0000CA35
    "movk w3, #0xCA35, lsl #16", // w3 = 0xCA35CA35
    "str w3, [x2]",              // DRAM[0] = 0xCA35CA35
    // Set stack pointer (grows down from CA35 link address - 4 KB).
    // 0x430000000 - 0x1000 = 0x42FFFF000
    "ldr x0, =0x42FFFF000",
    "mov sp, x0",
    // Jump to Rust entry point.
    "bl rust_main",
    // Should never return; spin if it does.
    "1: wfe",
    "b 1b",
);

// ── UART12 registers (base 0x14C33B00, reg-shift=2 → stride 4) ───────────────

const UART_BASE: usize = 0x14C3_3B00;
const THR: usize = UART_BASE + 0x00; // TX holding register (DLAB=0)
const LSR: usize = UART_BASE + 0x14; // Line status register
const LSR_THRE: u32 = 1 << 5; // TX holding register empty

#[inline(always)]
fn uart_putc(c: u8) {
    unsafe {
        // Spin until TX FIFO has room.
        while core::ptr::read_volatile(LSR as *const u32) & LSR_THRE == 0 {}
        core::ptr::write_volatile(THR as *mut u32, c as u32);
    }
}

fn uart_puts(s: &[u8]) {
    for &b in s {
        if b == b'\n' {
            uart_putc(b'\r');
        }
        uart_putc(b);
    }
}

// ── DRAM handshake ────────────────────────────────────────────────────────────
// CA35 view of 0xAFFFFF00 (BootMCU view) = 0x42FFFFF00.
// Write magic before UART to confirm CA35 is executing.
const HANDSHAKE_ADDR: usize = 0x4_2FFF_FF00;
const HANDSHAKE_MAGIC: u32 = 0xA35B_EEF0;

// ── Entry point ───────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn rust_main() -> ! {
    unsafe {
        core::ptr::write_volatile(HANDSHAKE_ADDR as *mut u32, HANDSHAKE_MAGIC);
    }
    uart_puts(b"\n=== Hello from A35 ===\n");
    uart_puts(b"AST2700 CA35 bare-metal Rust\n");
    uart_puts(b"DRAM, SLI, and BootMCU handoff complete.\n");
    uart_puts(b"======================\n");

    loop {
        unsafe { core::arch::asm!("wfe") }
    }
}

// ── Panic handler ─────────────────────────────────────────────────────────────

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    uart_puts(b"\n[PANIC]\n");
    loop {
        unsafe { core::arch::asm!("wfe") }
    }
}
