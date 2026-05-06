/*
 * Cooperative task executor — sdk_init, sdk_register_task, sdk_step, sdk_run.
 *
 * Design:
 *   - Static array of (poll_fn, ctx) pairs; no heap.
 *   - sdk_step() polls every task once in round-robin order.
 *     ASYNC_DONE tasks are removed by swapping with the last entry
 *     (O(1) removal, order not preserved — acceptable for a cooperative
 *     scheduler where task identity is tracked by the task itself).
 *   - sdk_run() calls sdk_step() in a tight loop; after each pass in which
 *     no task completed, it issues SDK_IDLE() (CFV1 "stop #0x2000") so the
 *     core sleeps until the next IRQ.  The tick timer (M3) or mailbox doorbell
 *     (M4) will wake it within one tick period.
 *
 * Expanding sdk_init():
 *   M3 — call timer_init() to configure Timer 7 and enable CVIC INT#14.
 *   M4 — call mbox_init() to initialise the SPSC rings and CVIC INT#1.
 */

#include "sdk.h"
#include <stddef.h>
#ifdef __m68k__
#  include "hal/timer.h"
#  include "hal/mbox.h"
#endif

typedef struct {
    poll_fn_t  poll;
    void      *ctx;
} task_entry_t;

static task_entry_t s_tasks[SDK_MAX_TASKS];
static uint8_t      s_task_count;

/* -------------------------------------------------------------------------
 * sdk_init — hardware initialisation (grows in M3 and M4)
 * ------------------------------------------------------------------------- */
void sdk_init(void)
{
    s_task_count = 0;
#ifdef __m68k__
    timer_hal_init();
    mbox_init();
#endif
}

/* -------------------------------------------------------------------------
 * sdk_register_task — add a task to the executor's static array
 * ------------------------------------------------------------------------- */
void sdk_register_task(poll_fn_t poll, void *ctx)
{
    if (s_task_count >= SDK_MAX_TASKS) {
        return;
    }
    s_tasks[s_task_count].poll = poll;
    s_tasks[s_task_count].ctx  = ctx;
    s_task_count++;
}

/* -------------------------------------------------------------------------
 * sdk_task_count — return number of active tasks (diagnostics / tests)
 * ------------------------------------------------------------------------- */
uint8_t sdk_task_count(void)
{
    return s_task_count;
}

/* -------------------------------------------------------------------------
 * sdk_step — poll every task once; return remaining active task count
 * ------------------------------------------------------------------------- */
uint8_t sdk_step(void)
{
    uint8_t i = 0;

    while (i < s_task_count) {
        if (s_tasks[i].poll(s_tasks[i].ctx) == ASYNC_DONE) {
            /*
             * Compact: overwrite completed task with the last entry and
             * shrink the array.  Re-poll position i (now a different task)
             * on the next iteration — do NOT increment i.
             */
            s_tasks[i] = s_tasks[s_task_count - 1u];
            s_task_count--;
        } else {
            i++;
        }
    }

    return s_task_count;
}

/* -------------------------------------------------------------------------
 * sdk_run — cooperative scheduler loop; never returns
 * ------------------------------------------------------------------------- */
void sdk_run(void)
{
    for (;;) {
        uint8_t before = s_task_count;
        uint8_t after  = sdk_step();

        if (after == before) {
            /*
             * No task completed this pass — all tasks are blocked on
             * external conditions.  Sleep until the next IRQ (timer tick,
             * mailbox doorbell, etc.) wakes the core.
             */
            SDK_IDLE();
        }
    }
}
