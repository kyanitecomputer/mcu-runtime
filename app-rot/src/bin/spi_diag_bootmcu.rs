//! AST2700 BootMCU startup diagnostic — no Embassy, no complex init.
//!
//! Prints one ASCII character at each key startup stage using raw UART12
//! register writes.  All characters are immediate constants.
//!
//! Expected output (all immediate, everything runs from SRAM):
//!
//!   'P'  — __pre_init called (before .bss zero)
//!   'R'  — riscv-rt main() reached
//!   'U'×20 then 'K' — UART works after full riscv-rt init
//!
//! Diagnosis table:
//!   Nothing    → early crash (before pre_init)
//!   'P' only   → .bss zero crashes (unlikely)
//!   'PRUUUUUK' → riscv-rt works; add Embassy next
//!
//! Build:
//!   cargo build --bin spi_diag_bootmcu --features ast2700-bootmcu \
//!       --target riscv32imc-unknown-none-elf --release
//!   rust-objcopy -O binary \
//!       target/riscv32imc-unknown-none-elf/release/spi_diag_bootmcu \
//!       spi_diag_bootmcu.bin

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::ptr;

// ── Safe startup stub at 0x80000000 ──────────────────────────────────────────
// riscv-rt's _start opens with `lui ra,0x80000; jr 0x8(ra)` — hardware-
// verified to cause a silent hang on AST2700 A1 (2-instruction window
// before mie is cleared lets a pending ROM interrupt fire).
//
// .section .init is KEEP'd and placed BEFORE .init.rust in riscv-rt's
// link.x, so this code lands at 0x80000000.  No .global _start → no
// symbol conflict with riscv-rt.
global_asm!(
    ".section .init,\"ax\"",
    "_spi_diag_init:", // local label, no .global → no conflict
    "   csrwi mie, 0", // FIRST: disable interrupts
    "   csrwi mip, 0",
    "   .option push",
    "   .option norelax",
    "   la    gp, __global_pointer$",
    "   .option pop",
    "   la    sp, _stack_start",
    "   j     _start_rust", // riscv-rt runtime → __pre_init → main
);

const UART12_THR: *mut u32 = 0x14C3_3B00 as *mut u32;
const UART12_LSR: *const u32 = 0x14C3_3B14 as *const u32;

/// Write one byte to UART12.
/// All arguments are immediate constants — no memory loads from SDRAM.
#[inline(always)]
unsafe fn putc(c: u8) {
    while ptr::read_volatile(UART12_LSR) & (1 << 5) == 0 {}
    ptr::write_volatile(UART12_THR, c as u32);
}

/// __pre_init: called by riscv-rt BEFORE .data copy and .bss zero.
///
/// sp is already set to SRAM (_stack_start = 0x14bc0000) by riscv-rt.
/// No globals or statics are valid yet.  Only register and IO operations.
///
/// Prints 'P' to show we are alive before the SDRAM .data copy begins.
#[no_mangle]
unsafe extern "C" fn __pre_init() {
    putc(b'P');
}

/// riscv-rt entry point.  Reached AFTER .data copy and .bss zero.
///
/// The .data copy (56 bytes, 14 word loads from SDRAM) takes ~18 s on
/// uninitialized SDRAM.  If 'R' never appears after 'P', the copy crashes.
#[riscv_rt::entry]
fn main() -> ! {
    unsafe {
        // Immediate confirmation that riscv-rt startup completed.
        putc(b'R');

        // 20× 'U' (autobaud pattern) then 'K'.
        for _ in 0..20u32 {
            putc(b'U');
        }
        putc(b'K');
        putc(b'\r');
        putc(b'\n');

        loop {}
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    // Print '!' on panic so we can distinguish from a silent hang.
    unsafe {
        putc(b'!');
    }
    loop {}
}
