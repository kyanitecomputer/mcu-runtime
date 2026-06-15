//! AST2700 BootMCU SPI boot — minimal UART12 tick counter.
//!
//! Key constraints discovered through hardware testing:
//!   - UART12: ROM-configured at 115200 8N1. Writing LCR/FCR/IER kills it.
//!     Only THR (write) and LSR (read) are safe.
//!   - DRAM (0x80000000): ibex has I-cache but NO D-cache. SDRAMMC isn't
//!     initialized → data loads from 0x80000000 take ~1.3s each.
//!     Instruction fetches work fine (I-cache).
//!   - SRAM (0x14b80000, 192KB): Fast for both read and write.
//!     Stack and all data must be in SRAM.
//!   - mtvec: ROM's trap handler must be overridden immediately.
//!   - mie/mip: Must be cleared to prevent stale interrupts.

#![no_std]
#![no_main]

use core::arch::global_asm;
use core::ptr;

const UART12_BASE: usize = 0x14C3_3B00;
const REG_THR: *mut u32 = (UART12_BASE + 0x00) as *mut u32;
const REG_LSR: *const u32 = (UART12_BASE + 0x14) as *const u32;
const LSR_THRE: u32 = 1 << 5;

global_asm!(
    ".section .text._start",
    ".global _start",
    "_start:",
    "   csrwi mie, 0",      // disable all machine interrupts
    "   csrwi mip, 0",      // clear any pending interrupts
    "   la t0, _trap_halt", // redirect mtvec to our trap handler
    "   csrw mtvec, t0",
    "   lui sp, 0x14BB0", // sp = 0x14BB0000
    "   j _rust_entry",
    "",
    ".align 2",
    "_trap_halt:",
    "   j _trap_halt",
);

#[no_mangle]
unsafe extern "C" fn _rust_entry() -> ! {
    // Banner — all immediate values, no memory loads from 0x80000000.
    uart_putc(b'\r');
    uart_putc(b'\n');
    uart_putc(b'S');
    uart_putc(b'P');
    uart_putc(b'I');
    uart_putc(b' ');
    uart_putc(b'O');
    uart_putc(b'K');
    uart_putc(b'!');
    uart_putc(b'\r');
    uart_putc(b'\n');

    // Tick loop — all characters via immediates.
    let mut count: u32 = 0;
    loop {
        uart_putc(b't');
        uart_putc(b'i');
        uart_putc(b'c');
        uart_putc(b'k');
        uart_putc(b' ');
        uart_dec(count);
        uart_putc(b'\r');
        uart_putc(b'\n');
        count = count.wrapping_add(1);
        delay();
    }
}

#[inline(never)]
unsafe fn uart_putc(c: u8) {
    while ptr::read_volatile(REG_LSR) & LSR_THRE == 0 {}
    ptr::write_volatile(REG_THR, c as u32);
}

#[inline(never)]
unsafe fn uart_dec(mut n: u32) {
    if n == 0 {
        uart_putc(b'0');
        return;
    }
    // Stack buffer — stack is in SRAM (fast).
    let mut buf = [0u8; 10];
    let mut i = 10usize;
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    while i < 10 {
        uart_putc(buf[i]);
        i += 1;
    }
}

#[inline(never)]
unsafe fn delay() {
    // ~1 second at ~50 MHz (ibex default before PLL init, ~5 cycles/iteration).
    // Calibrated from hardware: 50M iters = 5.5s → 9M iters ≈ 1s.
    // Counter on stack (SRAM) — fast.
    for _ in 0..9_000_000u32 {
        core::hint::black_box(0u32);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}
