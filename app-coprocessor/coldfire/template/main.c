/*
 * Template — async blinky using sdk_sleep_ms (M3+).
 *
 * Demonstrates the canonical task pattern:
 *   - All locals that survive await() live in the context struct.
 *   - sdk_sleep_ms is itself async; its context (sdk_sleep_t) must be in
 *     the outer struct.
 *   - SDK_TASK registers the task; sdk_run() never returns.
 *
 * Async discipline reminder:
 *   WRONG — stack-local survives across await (silently broken):
 *     int counter = 0;
 *     await(async_call(sdk_sleep_ms, ...));
 *     counter++;   ← counter is gone; this reads garbage
 *
 *   RIGHT — counter lives in the context struct:
 *     ctx->counter++;
 *     await(async_call(sdk_sleep_ms, ...));
 *     ctx->counter++;  ← ctx->counter is still valid
 *
 *   The struct below shows the correct pattern.
 */

#include <sdk.h>
#include <pac/gpio.h>
#include <hal/timer.h>

typedef struct {
    async_state;
    sdk_sleep_t sleep;      /* sdk_sleep_ms context — MUST be in struct */
    uint32_t    counter;    /* blink counter — survives across await */
} blink_t;

static blink_t s_blink;

async blink_task(blink_t *ctx)
{
    async_begin(ctx);

    /* Configure GPIO port A pin 0 as output (only on first entry) */
    GPIO->abcd_dir |= GPIO_PA_PIN0_Msk;

    for (;;) {
        GPIO->abcd_data ^= GPIO_PA_PIN0_Msk;
        ctx->counter++;

        /* Sleep 500 ms (driver-level await — yields entire scheduler pass) */
        await(async_call(sdk_sleep_ms, &ctx->sleep, 500));
    }

    async_end;
}

int main(void)
{
    sdk_init();
    SDK_TASK(blink_task, &s_blink);
    sdk_run();   /* never returns */
}
