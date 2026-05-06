/*
 * Timer HAL — SDK tick timer (Timer 7 by default) and sdk_sleep_ms.
 *
 * Hardware setup (Timer N = SDK_TICK_TIMER):
 *   1. Write reload register: (1 MHz / SDK_TICK_HZ) - 1
 *      e.g. SDK_TICK_HZ=1000 → reload = 999  (1 kHz tick)
 *   2. Set TMC30 bits for timer N:
 *        enable=1, clock=1 MHz (platform-independent), overflow IRQ=1
 *   3. Clear CVIC edge status for timer N's CVIC INT bit.
 *   4. Enable timer N's CVIC INT bit.
 *
 * Interrupt routing:
 *   Timer N overflow → CVIC INT# (8 + N - 1) = INT#14 for timer 7
 *   CVIC → CPU vector 70 (0x46) → cvic_dispatch → timer_tick_isr
 *
 * AST2400 dummy-read errata:
 *   On AST2400, after every 32-bit register write from the CF, an extra read
 *   of any register in the same segment is required to flush the bus.
 *   Gated by SDK_PLATFORM_AST2400.
 */

#include "hal/timer.h"
#include "pac/timer.h"
#include "pac/cvic.h"
#include "sdk_config.h"
#include <stdint.h>

volatile uint32_t sdk_tick_count;

/*
 * CVIC INT# for timer N: timer 1 → INT#8, timer 7 → INT#14.
 */
#define TICK_CVIC_INT_NUM   (7U + SDK_TICK_TIMER)
#define TICK_CVIC_Msk       (1U << TICK_CVIC_INT_NUM)

/*
 * TMC30 bit positions for SDK_TICK_TIMER (1-based).
 * Each timer occupies 4 bits starting at (N-1)*4.
 */
#define TICK_CTRL_BASE      (((uint32_t)(SDK_TICK_TIMER) - 1U) * 4U)
#define TICK_CTRL_EN_Msk    (1U << (TICK_CTRL_BASE + 0U))
#define TICK_CTRL_CLK_Msk   (1U << (TICK_CTRL_BASE + 1U))  /* 1 = 1 MHz clock */
#define TICK_CTRL_IRQ_Msk   (1U << (TICK_CTRL_BASE + 2U))

/* Reload value for SDK_TICK_HZ ticks/second using 1 MHz clock source. */
#define TICK_RELOAD_VAL     ((1000000U / (uint32_t)(SDK_TICK_HZ)) - 1U)

/* AST2400 dummy-read errata helper: read SCU reg to flush write. */
#ifdef SDK_PLATFORM_AST2400
#  define DUMMY_READ()  do { (void)(*(volatile uint32_t *)0x6E2000U); } while (0)
#else
#  define DUMMY_READ()  ((void)0)
#endif

void timer_hal_init(void)
{
    volatile TIMER_CH_TypeDef *ch = timer_ch((uint32_t)SDK_TICK_TIMER);

    /* Stop timer, clear state */
    TIMER->ctrl &= ~(TICK_CTRL_EN_Msk | TICK_CTRL_IRQ_Msk);
    DUMMY_READ();

    /* Set reload value */
    ch->reload = TICK_RELOAD_VAL;
    DUMMY_READ();

    /* Pre-clear the CVIC edge-triggered status for this timer's INT line.
     * This must be done BEFORE enabling the interrupt in CVIC10 to avoid
     * triggering immediately on a stale edge. */
    CVIC->edge_clear = TICK_CVIC_Msk;
    DUMMY_READ();

    /* Enable the CVIC interrupt line for this timer */
    CVIC->irq_enable = TICK_CVIC_Msk;
    DUMMY_READ();

    /* Start timer: enable + 1 MHz clock + overflow IRQ */
    TIMER->ctrl |= TICK_CTRL_EN_Msk | TICK_CTRL_CLK_Msk | TICK_CTRL_IRQ_Msk;
    DUMMY_READ();
}

/*
 * timer_tick_isr — called from cvic_dispatch when the tick timer overflows.
 *
 * Runs in interrupt context.  Must be as short as possible.
 * Clears the edge-triggered status for the timer's CVIC INT line.
 */
void timer_tick_isr(void)
{
    sdk_tick_count++;
    CVIC->edge_clear = TICK_CVIC_Msk;
}

/*
 * sdk_sleep_ms — async sleep for at least `ms` milliseconds.
 *
 * Records a deadline tick count on first entry and yields until the global
 * sdk_tick_count reaches or exceeds the deadline.
 *
 * With SDK_TICK_HZ=1000 and SDK_TICK_TIMER=7, accuracy is ±1 ms + ISR
 * latency.  Large values of `ms` handle uint32_t wrap-around correctly
 * because the comparison uses unsigned subtraction semantics.
 */
async sdk_sleep_ms(sdk_sleep_t *ctx, uint32_t ms)
{
    async_begin(ctx);
    ctx->deadline = sdk_tick_count + ms;
    await((uint32_t)(sdk_tick_count - ctx->deadline) < 0x80000000U);
    async_end;
}
