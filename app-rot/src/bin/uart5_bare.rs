//! Bare-minimum UART5 test for AST1060 — no Embassy, no SysTick, no allocator.
//!
//! Directly writes to UART5 registers in an infinite loop. If this prints
//! on the serial console, the UART hardware works and the issue is in the
//! Embassy/HAL init path. If this doesn't print, the UART clock or pin mux
//! is wrong after UART boot.
//!
//! Build:
//!   cargo build --bin uart5_bare --features ast1060 --target thumbv7em-none-eabihf --release
//!
//! Then gen-uart-image.sh + ast1060-load as usual.

#![no_std]
#![no_main]

use core::ptr;

// UART5 registers (AST1060 CM4 bus view)
const UART5_BASE: usize = 0x7E78_4000;
const REG_THR: *mut u32 = UART5_BASE as *mut u32; // TX Holding Register (offset 0x00)
const REG_IER: *mut u32 = (UART5_BASE + 0x04) as *mut u32;
const REG_FCR: *mut u32 = (UART5_BASE + 0x08) as *mut u32;
const REG_LCR: *mut u32 = (UART5_BASE + 0x0C) as *mut u32;
const REG_LSR: *mut u32 = (UART5_BASE + 0x14) as *mut u32;

// SCU registers
const SCU_KEY: *mut u32 = 0x7E6E_2000 as *mut u32;
const SCU_UNLOCK: u32 = 0x1688_A8A8;
const SCU310: *mut u32 = 0x7E6E_2310 as *mut u32; // Clock Selection 4

// CPACR for FPU enable
const CPACR: *mut u32 = 0xE000_ED88 as *mut u32;

const LSR_THRE: u32 = 1 << 5; // TX Holding Register Empty
const LCR_DLAB: u32 = 1 << 7;
const LCR_8N1: u32 = 0x03;

#[cortex_m_rt::pre_init]
unsafe fn pre_init() {
    // Enable FPU (same as embassy boot.rs)
    let cpacr = ptr::read_volatile(CPACR);
    ptr::write_volatile(CPACR, cpacr | (0b11 << 20) | (0b11 << 22));
}

#[cortex_m_rt::entry]
fn main() -> ! {
    unsafe {
        // Unlock SCU
        ptr::write_volatile(SCU_KEY, SCU_UNLOCK);

        // Read SCU310 to check UART5 clock source (bit 4)
        let clk_sel4 = ptr::read_volatile(SCU310 as *const u32);
        let uart5_fast = (clk_sel4 >> 4) & 1; // 0 = 24MHz/13, 1 = 192MHz/13

        // Compute divisor based on actual clock source
        let uart_clk: u32 = if uart5_fast == 1 {
            192_000_000 / 13 // 14,769,230 Hz
        } else {
            24_000_000 / 13 // 1,846,153 Hz
        };
        let divisor = uart_clk / (16 * 115200);

        // Reinit UART5: disable interrupts, set baud, 8N1, enable FIFO
        ptr::write_volatile(REG_IER, 0);
        ptr::write_volatile(REG_LCR, LCR_8N1 | LCR_DLAB);
        ptr::write_volatile(REG_THR, divisor & 0xFF); // DLL
        ptr::write_volatile(REG_IER, (divisor >> 8) & 0xFF); // DLH
        ptr::write_volatile(REG_LCR, LCR_8N1);
        ptr::write_volatile(REG_FCR, 0x07); // enable + reset FIFOs

        // Print banner
        uart5_puts(b"\r\n=== uart5_bare AST1060 ===\r\n");

        // Print clock info
        uart5_puts(b"SCU310=0x");
        uart5_hex32(clk_sel4);
        uart5_puts(b" UART5_CLK=");
        if uart5_fast == 1 {
            uart5_puts(b"192M/13");
        } else {
            uart5_puts(b"24M/13");
        }
        uart5_puts(b" DIV=");
        uart5_hex32(divisor);
        uart5_puts(b"\r\n");

        // Infinite loop printing dots
        let mut count: u32 = 0;
        loop {
            uart5_puts(b"tick ");
            uart5_hex32(count);
            uart5_puts(b"\r\n");
            count = count.wrapping_add(1);
            // Busy-wait delay (~1 second at 25 MHz)
            for _ in 0..2_500_000u32 {
                core::hint::black_box(0u32);
            }
        }
    }
}

unsafe fn uart5_putc(c: u8) {
    // Wait for TX holding register empty
    while ptr::read_volatile(REG_LSR) & LSR_THRE == 0 {}
    ptr::write_volatile(REG_THR, c as u32);
}

unsafe fn uart5_puts(s: &[u8]) {
    for &b in s {
        uart5_putc(b);
    }
}

unsafe fn uart5_hex32(val: u32) {
    let hex = b"0123456789abcdef";
    for i in (0..8).rev() {
        let nibble = ((val >> (i * 4)) & 0xF) as usize;
        uart5_putc(hex[nibble]);
    }
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    // If we panic, print a marker and loop
    unsafe {
        uart5_puts(b"\r\n!!! PANIC !!!\r\n");
    }
    loop {}
}
