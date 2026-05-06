#pragma once
#ifndef SDK_CONFIG_H
#define SDK_CONFIG_H

/* -------------------------------------------------------------------------
 * Platform selection — set SDK_PLATFORM_AST2500 or SDK_PLATFORM_AST2400
 * in CFLAGS (sdk.mk does this automatically from SDK_PLATFORM).
 * Default: AST2500.
 * ------------------------------------------------------------------------- */
#if !defined(SDK_PLATFORM_AST2500) && !defined(SDK_PLATFORM_AST2400)
#  define SDK_PLATFORM_AST2500
#endif

/* -------------------------------------------------------------------------
 * Scheduler
 * ------------------------------------------------------------------------- */
#ifndef SDK_MAX_TASKS
#  define SDK_MAX_TASKS     8
#endif

/* -------------------------------------------------------------------------
 * Tick timer (Timer 7 reserved by default; CVIC INT#14)
 * ------------------------------------------------------------------------- */
#ifndef SDK_TICK_TIMER
#  define SDK_TICK_TIMER    7
#endif

#ifndef SDK_TICK_HZ
#  define SDK_TICK_HZ       1000
#endif

/* -------------------------------------------------------------------------
 * SRAM stack reservation (bytes, grows downward from top of SRAM)
 * ------------------------------------------------------------------------- */
#ifndef SDK_SRAM_STACK_SIZE
#  define SDK_SRAM_STACK_SIZE   2048
#endif

/* -------------------------------------------------------------------------
 * Mailbox ring buffer (must be power of 2; max payload bytes per message)
 * ------------------------------------------------------------------------- */
#define SDK_MBOX_RING_SIZE      16
#define SDK_MBOX_MAX_PAYLOAD    12

/* -------------------------------------------------------------------------
 * Compile-time assertions
 * ------------------------------------------------------------------------- */
_Static_assert(SDK_MAX_TASKS > 0,
               "SDK_MAX_TASKS must be >= 1");
_Static_assert(SDK_MAX_TASKS <= 255,
               "SDK_MAX_TASKS must fit in uint8_t");
_Static_assert((SDK_MBOX_RING_SIZE & (SDK_MBOX_RING_SIZE - 1)) == 0,
               "SDK_MBOX_RING_SIZE must be a power of 2");
_Static_assert(SDK_TICK_TIMER >= 1 && SDK_TICK_TIMER <= 7,
               "SDK_TICK_TIMER must be in range [1, 7]");

/* -------------------------------------------------------------------------
 * Low-power idle: CFV1 STOP instruction, IPL=0 (all IRQs unmasked).
 * Expands to no-op when compiled on the host for unit tests.
 * ------------------------------------------------------------------------- */
#ifdef __m68k__
#  define SDK_IDLE()  __asm__ volatile("stop #0x2000")
#else
#  define SDK_IDLE()  ((void)0)
#endif

#endif /* SDK_CONFIG_H */
