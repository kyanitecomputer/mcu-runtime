#pragma once
#ifndef HAL_TIMER_H
#define HAL_TIMER_H

#include "async/async.h"
#include <stdint.h>

/*
 * Timer HAL — sdk_sleep_ms and tick counter.
 *
 * SDK_TICK_TIMER (default: 7) is initialised by timer_hal_init(), called
 * from sdk_init().  Do not call timer_hal_init() directly.
 *
 * sdk_sleep_ms is an async function; it must be called with await():
 *
 *   typedef struct {
 *       async_state;
 *       sdk_sleep_t sleep;   ← must be in context struct (survives await)
 *   } my_task_t;
 *
 *   async my_task(my_task_t *ctx) {
 *       async_begin(ctx);
 *       await(async_call(sdk_sleep_ms, &ctx->sleep, 500));
 *       async_end;
 *   }
 */

/* Context struct for sdk_sleep_ms — place in your task's context struct. */
typedef struct {
    async_state;
    uint32_t deadline;
} sdk_sleep_t;

/* Monotonic tick counter incremented by the tick ISR (read-only for users). */
extern volatile uint32_t sdk_tick_count;

/* Async sleep.  Yields until sdk_tick_count >= current + ms. */
async sdk_sleep_ms(sdk_sleep_t *ctx, uint32_t ms);

/* Internal: called from sdk_init().  Do not call directly. */
void timer_hal_init(void);

/* Internal: CVIC dispatch calls this when CVIC INT#(TIMER_BIT) fires. */
void timer_tick_isr(void);

#endif /* HAL_TIMER_H */
