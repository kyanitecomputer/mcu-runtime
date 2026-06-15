//! AST2700 BootMCU SPI boot test — bare-metal UART12 tick counter.
//!
//! No Embassy, no timer interrupt, no async — just register-banging UART12
//! in a busy-wait loop. This is the absolute minimum firmware to verify that
//! the BootMCU ROM loads and executes our code from SPI flash.
//!
//! UART12 is already configured by the ROM at 115200 8N1. We reinit it anyway
//! for safety, then print a banner and 1 Hz ticks.
//!
//! # Build
//!
//! ```sh
//! cargo build --bin spi_test_bootmcu --features ast2700-bootmcu \
//!     --target riscv32imc-unknown-none-elf --release
//! ```

#![no_std]
#![no_main]

use core::ptr;

// ── UART12 registers (AST2700 BootMCU, reg-shift=2) ─────────────────────────

const UART12_BASE: usize = 0x14C3_3B00;
const REG_THR: *mut u32 = (UART12_BASE + 0x00) as *mut u32;
const REG_IER: *mut u32 = (UART12_BASE + 0x04) as *mut u32;
const REG_FCR: *mut u32 = (UART12_BASE + 0x08) as *mut u32;
const REG_LCR: *mut u32 = (UART12_BASE + 0x0C) as *mut u32;
const REG_LSR: *const u32 = (UART12_BASE + 0x14) as *const u32;

const LSR_THRE: u32 = 1 << 5;
const LCR_8N1: u32 = 0x03;
const LCR_DLAB: u32 = 1 << 7;

// ── BootMCU timer (read-only, for rough calibration) ─────────────────────────

const TIMER_COUNT_L: *const u32 = 0x14C3_6000 as *const u32;

// ── Entry point ──────────────────────────────────────────────────────────────

#[riscv_rt::entry]
fn main() -> ! {
    unsafe {
        // Reinit UART12: 115200 8N1. Clock = 24 MHz / 13 ≈ 1,846,153 Hz.
        // Divisor = 1846153 / (16 × 115200) ≈ 1.
        ptr::write_volatile(REG_IER, 0);
        ptr::write_volatile(REG_LCR, LCR_8N1 | LCR_DLAB);
        ptr::write_volatile(REG_THR, 1); // DLL = 1
        ptr::write_volatile(REG_IER, 0); // DLH = 0
        ptr::write_volatile(REG_LCR, LCR_8N1);
        ptr::write_volatile(REG_FCR, 0x07); // enable + reset FIFOs

        uart_puts(b"\r\n=== spi_test_bootmcu AST2700 ===\r\n");
        uart_puts(b"Embassy-aspeed BootMCU SPI boot OK!\r\n");

        // Print timer counter to show the timer peripheral is alive.
        uart_puts(b"TIMER_L=0x");
        uart_hex32(ptr::read_volatile(TIMER_COUNT_L));
        uart_puts(b"\r\n");

        let mut count: u32 = 0;
        loop {
            uart_puts(b"tick ");
            uart_dec32(count);
            uart_puts(b"\r\n");
            count = count.wrapping_add(1);
            // Busy-wait ~1 second. BootMCU ibex runs at ~200 MHz.
            // ~4 cycles per loop iteration → 50M iterations ≈ 1 second.
            for _ in 0..50_000_000u32 {
                core::hint::black_box(0u32);
            }
        }
    }
}

// ── UART helpers ─────────────────────────────────────────────────────────────

unsafe fn uart_putc(c: u8) {
    while ptr::read_volatile(REG_LSR) & LSR_THRE == 0 {}
    ptr::write_volatile(REG_THR, c as u32);
}

unsafe fn uart_puts(s: &[u8]) {
    for &b in s {
        uart_putc(b);
    }
}

unsafe fn uart_hex32(val: u32) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for i in (0..8).rev() {
        uart_putc(HEX[((val >> (i * 4)) & 0xF) as usize]);
    }
}

unsafe fn uart_dec32(mut n: u32) {
    if n == 0 {
        uart_putc(b'0');
        return;
    }
    let mut buf = [0u8; 10];
    let mut i = buf.len();
    while n > 0 {
        i -= 1;
        buf[i] = b'0' + (n % 10) as u8;
        n /= 10;
    }
    for &b in &buf[i..] {
        uart_putc(b);
    }
}

// ── Panic handler ────────────────────────────────────────────────────────────

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    unsafe {
        uart_puts(b"\r\n!!! PANIC !!!\r\n");
    }
    loop {}
}
