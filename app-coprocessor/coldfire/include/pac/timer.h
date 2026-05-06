/*
 * Aspeed AST2400 / AST2500 Timer Controller — Stub PAC Header
 * GENERATED FILE — do not edit by hand.
 *
 * Physical base: 0x1E782000
 * CF LE virtual: 0x782000  (within periph segment: seg 7 on AST2500, seg 3 on AST2400)
 *
 * 8 independent 32-bit decrement counters.  Each decrements from its reload
 * value to zero, then auto-reloads and fires an overflow interrupt (if enabled).
 *
 * All 8 timers are visible from the CF.  Timers 1–7 route to CVIC INT#8–14.
 * Timer 8 routes to CVIC INT#15 on AST2400 only (not present on AST2500).
 *
 * Clock sources (per timer, controlled by TMC_CTRL bits):
 *   0 = PCLK (APB clock, platform-dependent)
 *   1 = 1 MHz fixed (platform-independent; use this for known tick rates)
 *
 * SDK RESERVATION: Timer 7 (CVIC INT#14) is reserved for sdk_sleep_ms.
 *   Do not use Timer 7 in application code.  Configure SDK_TICK_TIMER in
 *   sdk_config.h to select a different timer if Timer 7 is unavailable.
 */

#pragma once
#ifndef PAC_TIMER_H
#define PAC_TIMER_H

#include <stdint.h>

#define TIMER_BASE  0x782000U

typedef struct {
    volatile uint32_t status;       /* 0x00: Current counter value (RW) */
    volatile uint32_t reload;       /* 0x04: Reload value */
    volatile uint32_t match1;       /* 0x08: First match → edge IRQ */
    volatile uint32_t match2;       /* 0x0C: Second match → edge IRQ */
} TIMER_CH_TypeDef;

typedef struct {
    TIMER_CH_TypeDef ch[3];         /* 0x00–0x2F: Timers 1–3 (channels 0–2) */
    volatile uint32_t ctrl;         /* 0x30: Master control register */
    volatile uint32_t ctrl2;        /* 0x34: Control register 2 */
    volatile uint32_t ctrl3;        /* 0x38: Control register 3 (separate clear mode) */
    volatile uint32_t ctrl_clr;     /* 0x3C: Control clear register */
    TIMER_CH_TypeDef ch45[5];       /* 0x40–0x8F: Timers 4–8 (channels 3–7 in array) */
} TIMER_TypeDef;

#define TIMER  ((TIMER_TypeDef *)TIMER_BASE)

/*
 * TMC30 Master Control Register bit fields.
 * Each timer N (1-based) occupies 4 bits at offset [(N-1)*4].
 * Bit layout within each 4-bit group:
 *   base+0 = enable
 *   base+1 = clock select (0=PCLK, 1=1MHz)
 *   base+2 = overflow IRQ enable
 *   base+3 = pulse generation enable (timers 5–8 only)
 */
#define TIMER_CTRL_T1_EN_Pos      0U
#define TIMER_CTRL_T1_CLK_Pos     1U
#define TIMER_CTRL_T1_IRQ_Pos     2U

#define TIMER_CTRL_T2_EN_Pos      4U
#define TIMER_CTRL_T2_CLK_Pos     5U
#define TIMER_CTRL_T2_IRQ_Pos     6U

#define TIMER_CTRL_T3_EN_Pos      8U
#define TIMER_CTRL_T3_CLK_Pos     9U
#define TIMER_CTRL_T3_IRQ_Pos     10U

#define TIMER_CTRL_T4_EN_Pos      12U
#define TIMER_CTRL_T4_CLK_Pos     13U
#define TIMER_CTRL_T4_IRQ_Pos     14U

#define TIMER_CTRL_T5_EN_Pos      16U
#define TIMER_CTRL_T5_CLK_Pos     17U
#define TIMER_CTRL_T5_IRQ_Pos     18U
#define TIMER_CTRL_T5_PULSE_Pos   19U

#define TIMER_CTRL_T6_EN_Pos      20U
#define TIMER_CTRL_T6_CLK_Pos     21U
#define TIMER_CTRL_T6_IRQ_Pos     22U
#define TIMER_CTRL_T6_PULSE_Pos   23U

#define TIMER_CTRL_T7_EN_Pos      24U
#define TIMER_CTRL_T7_CLK_Pos     25U
#define TIMER_CTRL_T7_IRQ_Pos     26U
#define TIMER_CTRL_T7_PULSE_Pos   27U

#define TIMER_CTRL_T8_EN_Pos      28U
#define TIMER_CTRL_T8_CLK_Pos     29U
#define TIMER_CTRL_T8_IRQ_Pos     30U
#define TIMER_CTRL_T8_PULSE_Pos   31U

/* Convenience masks */
#define TIMER_CTRL_TN_Msk(n, pos)  (1U << (((n)-1)*4 + (pos)))

/*
 * Channel accessor — timer N (1-based) → ch array index.
 * Timers 1–3 are in ch[0..2]; timers 4–8 are in ch45[0..4].
 * Use TIMER_CH(N) for the TIMER_CH_TypeDef* of timer N.
 */
static inline volatile TIMER_CH_TypeDef *timer_ch(uint32_t n)
{
    if (n <= 3U) {
        return &TIMER->ch[n - 1U];
    }
    return &TIMER->ch45[n - 4U];
}

#endif /* PAC_TIMER_H */
