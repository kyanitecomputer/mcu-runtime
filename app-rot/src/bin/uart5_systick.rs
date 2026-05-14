//! Bare UART5 + SysTick test for AST1060 — isolates SysTick as the problem.
//!
//! Same as uart5_bare but enables SysTick at the same 1 MHz rate Embassy uses.
//! If this binary hangs, SysTick is the issue. If it prints, the problem is
//! elsewhere in Embassy init.
//!
//! Build:
//!   cargo build --bin uart5_systick --features ast1060 --target thumbv7em-none-eabihf --release

#![no_std]
#![no_main]

use core::ptr;
use core::sync::atomic::{AtomicU32, Ordering};

// UART5 registers
const UART5_BASE: usize = 0x7E78_4000;
const REG_THR: *mut u32 = UART5_BASE as *mut u32;
const REG_IER: *mut u32 = (UART5_BASE + 0x04) as *mut u32;
const REG_FCR: *mut u32 = (UART5_BASE + 0x08) as *mut u32;
const REG_LCR: *mut u32 = (UART5_BASE + 0x0C) as *mut u32;
const REG_LSR: *mut u32 = (UART5_BASE + 0x14) as *mut u32;

// SCU registers
const SCU_KEY: *mut u32 = 0x7E6E_2000 as *mut u32;
const SCU_UNLOCK: u32 = 0x1688_A8A8;
const SCU310: *const u32 = 0x7E6E_2310 as *const u32;

// SysTick registers (Cortex-M)
const SYST_CSR: *mut u32 = 0xE000_E010 as *mut u32; // Control and Status
const SYST_RVR: *mut u32 = 0xE000_E014 as *mut u32; // Reload Value
const SYST_CVR: *mut u32 = 0xE000_E018 as *mut u32; // Current Value
const CPACR: *mut u32 = 0xE000_ED88 as *mut u32;

const LSR_THRE: u32 = 1 << 5;
const LCR_DLAB: u32 = 1 << 7;
const LCR_8N1: u32 = 0x03;

static TICK_COUNT: AtomicU32 = AtomicU32::new(0);

#[cortex_m_rt::pre_init]
unsafe fn pre_init() {
    let cpacr = ptr::read_volatile(CPACR);
    ptr::write_volatile(CPACR, cpacr | (0b11 << 20) | (0b11 << 22));
}

#[cortex_m_rt::entry]
fn main() -> ! {
    unsafe {
        // Unlock SCU
        ptr::write_volatile(SCU_KEY, SCU_UNLOCK);

        // Read clock source
        let clk_sel4 = ptr::read_volatile(SCU310);
        let uart5_fast = (clk_sel4 >> 4) & 1;
        let uart_clk: u32 = if uart5_fast == 1 {
            192_000_000 / 13
        } else {
            24_000_000 / 13
        };
        let divisor = uart_clk / (16 * 115200);

        // Init UART5
        ptr::write_volatile(REG_IER, 0);
        ptr::write_volatile(REG_LCR, LCR_8N1 | LCR_DLAB);
        ptr::write_volatile(REG_THR, divisor & 0xFF);
        ptr::write_volatile(REG_IER, (divisor >> 8) & 0xFF);
        ptr::write_volatile(REG_LCR, LCR_8N1);
        ptr::write_volatile(REG_FCR, 0x07);

        uart5_puts(b"\r\n=== uart5_systick AST1060 ===\r\n");

        // Test 1: SysTick at Embassy's rate (1 MHz = every 25 cycles at 25 MHz)
        uart5_puts(b"Enabling SysTick (reload=24, 1MHz)...\r\n");
        ptr::write_volatile(SYST_RVR, 24); // reload = HCLK_HZ / 1_000_000 - 1 = 24
        ptr::write_volatile(SYST_CVR, 0);  // clear current
        ptr::write_volatile(SYST_CSR, 0x07); // enable + interrupt + core clock

        // If we get here, SysTick didn't immediately fault
        uart5_puts(b"SysTick enabled. Printing ticks...\r\n");

        let mut last_tick: u32 = 0;
        let mut print_count: u32 = 0;
        loop {
            let t = TICK_COUNT.load(Ordering::Relaxed);
            // Print every ~1M ticks (roughly every second)
            if t.wrapping_sub(last_tick) >= 1_000_000 {
                last_tick = t;
                uart5_puts(b"ticks=");
                uart5_hex32(t);
                uart5_puts(b" prints=");
                uart5_hex32(print_count);
                uart5_puts(b"\r\n");
                print_count += 1;
                if print_count >= 10 {
                    break;
                }
            }
        }

        // Test 2: Disable SysTick, continue printing
        ptr::write_volatile(SYST_CSR, 0); // disable
        uart5_puts(b"SysTick disabled. Still alive.\r\n");

        // Test 3: Try at a slower rate (1 kHz = every 25000 cycles)
        uart5_puts(b"Enabling SysTick (reload=24999, 1kHz)...\r\n");
        TICK_COUNT.store(0, Ordering::Relaxed);
        ptr::write_volatile(SYST_RVR, 24999);
        ptr::write_volatile(SYST_CVR, 0);
        ptr::write_volatile(SYST_CSR, 0x07);

        last_tick = 0;
        print_count = 0;
        loop {
            let t = TICK_COUNT.load(Ordering::Relaxed);
            if t.wrapping_sub(last_tick) >= 1000 {
                last_tick = t;
                uart5_puts(b"ticks=");
                uart5_hex32(t);
                uart5_puts(b"\r\n");
                print_count += 1;
                if print_count >= 5 {
                    break;
                }
            }
        }

        uart5_puts(b"=== ALL TESTS PASSED ===\r\n");
        loop {}
    }
}

#[cortex_m_rt::exception]
fn SysTick() {
    TICK_COUNT.fetch_add(1, Ordering::Relaxed);
}

unsafe fn uart5_putc(c: u8) {
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
    unsafe { uart5_puts(b"\r\n!!! PANIC !!!\r\n"); }
    loop {}
}
