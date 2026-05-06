#pragma once
#ifndef SDK_H
#define SDK_H

#include "sdk_config.h"
#include "async/async.h"
#include <stdint.h>
#include <stddef.h>

/* -------------------------------------------------------------------------
 * Async task poll function type.
 * Every cooperative task must have signature: async task_fn(ctx_type *ctx)
 * The void* cast in SDK_TASK is safe on CFV1 (single pointer calling convention).
 * ------------------------------------------------------------------------- */
typedef async (*poll_fn_t)(void *ctx);

/* -------------------------------------------------------------------------
 * SDK lifecycle
 * ------------------------------------------------------------------------- */

/** Initialise SDK hardware (CVIC, tick timer, GPIO defaults).
 *  Must be called once from main() before SDK_TASK or sdk_run(). */
void sdk_init(void);

/** Poll all registered tasks once.
 *  Returns the number of tasks still active after this pass.
 *  Useful for unit tests; production code calls sdk_run() instead. */
uint8_t sdk_step(void);

/** Run the cooperative scheduler. Never returns. */
void sdk_run(void);

/* -------------------------------------------------------------------------
 * Task registration
 * ------------------------------------------------------------------------- */

/** Register a (poll_fn, ctx) pair with the executor.
 *  Use SDK_TASK instead of calling this directly. */
void sdk_register_task(poll_fn_t poll, void *ctx);

/** Return the current number of registered tasks (for diagnostics/testing). */
uint8_t sdk_task_count(void);

/** Register fn as a cooperative task.
 *  fn must have signature:  async fn(SomeContextType *ctx)
 *  ctx must remain valid for the lifetime of the task.
 *
 *  Example:
 *    static heartbeat_t s_hb;
 *    SDK_TASK(heartbeat_task, &s_hb);
 */
#define SDK_TASK(fn, ctx) \
    sdk_register_task((poll_fn_t)(fn), (void *)(ctx))

#endif /* SDK_H */
