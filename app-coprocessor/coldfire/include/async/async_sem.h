/*
 * Counting semaphores built on async.h.
 * Vendored from naasking/async.h (no modifications required).
 *
 * Author: Sandro Magi <naasking@gmail.com>
 */

#pragma once
#ifndef ASYNC_SEM_H
#define ASYNC_SEM_H

#include "async.h"

struct async_sem {
    unsigned int count;
};

/**
 * Initialize a semaphore with an initial count.
 * @param s  Pointer to struct async_sem.
 * @param c  Initial count value.
 */
#define init_sem(s, c)  (s)->count = (c)

/**
 * Wait until the semaphore count is > 0, then decrement it.
 * Must be used inside an async function.
 */
#define await_sem(s) \
    do { \
        await((s)->count > 0); \
        --(s)->count; \
    } while (0)

/**
 * Signal the semaphore (increment count).
 * Safe to call from ISR context (atomic on CFV1 for single-byte increments
 * but use caution with multi-byte counts — disable interrupts if needed).
 */
#define signal_sem(s)   ++(s)->count

#endif /* ASYNC_SEM_H */
