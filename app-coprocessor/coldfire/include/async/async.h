/*
 * Copyright 2021 Sandro Magi
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions are met:
 *
 * 1. Redistributions of source code must retain the above copyright notice,
 * this list of conditions and the following disclaimer.
 *
 * 2. Redistributions in binary form must reproduce the above copyright notice,
 * this list of conditions and the following disclaimer in the documentation
 * and/or other materials provided with the distribution.
 *
 * 3. Neither the name of the copyright holder nor the names of its contributors
 * may be used to endorse or promote products derived from this software without
 * specific prior written permission.
 *
 * THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS "AS IS"
 * AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE
 * IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE
 * ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT HOLDER OR CONTRIBUTORS BE
 * LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR
 * CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF
 * SUBSTITUTE GOODS OR SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS
 * INTERRUPTION) HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN
 * CONTRACT, STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE)
 * ARISING IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF
 * THE POSSIBILITY OF SUCH DAMAGE.
 *
 * Upstream: https://github.com/naasking/async.h
 * Author:   Sandro Magi <naasking@gmail.com>
 *
 * SDK patches applied (see ROADMAP §6):
 *   - Issue #18 : removed unused #include <limits.h>
 *   - PR #14    : use __COUNTER__ instead of __LINE__ (with __LINE__ fallback)
 *   - PR #10    : add block_until(cond) (renamed from async_run per maintainer)
 *   - Issue #15 : renamed struct async → struct async_s (avoids typedef/tag clash)
 *   - SDK-local : async_state narrowed to uint16_t (2 bytes; 65534 yield points max)
 */

#pragma once
#ifndef ASYNC_H
#define ASYNC_H

#include <stdint.h>

/*
 * __COUNTER__ produces a unique integer per expansion within a translation
 * unit, avoiding duplicate case labels when two await/yield macros share a
 * source line.  Fall back to __LINE__ on compilers that lack __COUNTER__.
 */
#ifndef __COUNTER__
#  define __COUNTER__ __LINE__
#endif

/*
 * _AWAIT_IMPL / _YIELD_IMPL accept the unique ID as a macro parameter so
 * that __COUNTER__ is evaluated exactly once per await_while / async_yield
 * use.  If __COUNTER__ were referenced twice inside the same macro body,
 * each reference would increment the counter independently, producing
 * different values for the assignment and the case label — breaking the
 * Duff's device state machine.
 *
 * The ID is offset by 2 so generated values never collide with
 * ASYNC_INIT (0) or ASYNC_DONE (1).
 */
#define _AWAIT_IMPL(uid, cond) \
    *_async_k = (uid); __attribute__((fallthrough)); case (uid): \
    if (cond) return ASYNC_CONT

#define _YIELD_IMPL(uid) \
    *_async_k = (uid); return ASYNC_CONT; case (uid):

/**
 * The async computation status.
 */
typedef enum ASYNC_EVT {
    ASYNC_INIT = 0,
    ASYNC_CONT = ASYNC_INIT,
    ASYNC_DONE = 1
} async;

/**
 * Declare the async state field inside a context struct.
 * Narrowed to uint16_t: saves 2 bytes per context struct on CFV1.
 * Max 65534 distinct yield points per async function.
 */
#define async_state uint16_t _async_k

/**
 * Core async structure — optional.  Renamed to async_s to avoid
 * the typedef-name / struct-tag collision (Issue #15).
 */
struct async_s { async_state; };

/**
 * Begin an async subroutine.
 * @param k  Pointer to the context struct (must contain async_state first field).
 *
 * Creates a local pointer _async_k so all subsequent macros can update state
 * without knowing the parameter name.
 */
#define async_begin(k) \
    uint16_t *_async_k = &(k)->_async_k; \
    switch (*_async_k) { default:

/**
 * End an async subroutine.
 */
#define async_end \
    *_async_k = ASYNC_DONE; __attribute__((fallthrough)); \
    case ASYNC_DONE: \
    return ASYNC_DONE; }

/**
 * Wait until cond is true (yields while cond is false).
 */
#define await(cond)        await_while(!(cond))

/**
 * Wait while cond is true (yields while cond holds).
 */
#define await_while(cond)  _AWAIT_IMPL(__COUNTER__ + 2, cond)

/**
 * Unconditionally yield execution for one scheduler pass.
 */
#define async_yield        _YIELD_IMPL(__COUNTER__ + 2)

/**
 * Exit the async subroutine immediately.
 */
#define async_exit \
    *_async_k = ASYNC_DONE; return ASYNC_DONE

/**
 * Reset an async context to its initial state.
 */
#define async_init(state)  (state)->_async_k = ASYNC_INIT

/**
 * Return true when the async subroutine has completed.
 */
#define async_done(state)  ((state)->_async_k == ASYNC_DONE)

/**
 * Drive a nested async call.  Evaluates true (non-zero) when f(state) is done.
 * Usage: await(async_call(my_fn, &ctx->nested));
 */
#define async_call(f, state) (async_done(state) || (f)(state))

/**
 * Blocking loop — for use in synchronous context (e.g. main() or executor
 * top-level) only.  Do NOT use inside an async function; it will block the
 * entire scheduler.
 */
#define block_until(cond) while (!(cond))

#endif /* ASYNC_H */
