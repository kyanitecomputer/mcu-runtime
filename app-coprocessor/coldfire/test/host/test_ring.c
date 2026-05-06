/*
 * Host-native unit tests for the SPSC mailbox ring buffer.
 *
 * Tests the ring arithmetic, full/empty detection, wrap-around, and
 * power-of-2 sizing invariants — all without touching hardware.
 *
 * The ring is a pure data structure; we test it by directly manipulating
 * head/tail and exercising the helper logic extracted here.
 */

#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>

#include "hal/mbox.h"

static int s_failures;

#define TEST(name) \
    do { printf("  %-50s", #name); fflush(stdout); } while (0)
#define PASS()  puts("PASS")
#define FAIL(msg) \
    do { printf("FAIL: %s\n", msg); s_failures++; return; } while (0)
#define EXPECT(cond) \
    do { if (!(cond)) { FAIL(#cond); } } while (0)

/* -------------------------------------------------------------------------
 * Ring helpers (mirrors mbox.c — kept here so test has no link dep on mbox.c)
 * ------------------------------------------------------------------------- */
static uint32_t ring_count(const mbox_ring_t *r)
{
    return r->head - r->tail;
}

static int ring_full(const mbox_ring_t *r)
{
    return ring_count(r) >= MBOX_RING_SIZE;
}

static int ring_empty(const mbox_ring_t *r)
{
    return ring_count(r) == 0U;
}

static void ring_push(mbox_ring_t *r, uint8_t type)
{
    uint32_t idx = r->head & (MBOX_RING_SIZE - 1U);
    r->slots[idx].type = type;
    r->head++;
}

static uint8_t ring_pop(mbox_ring_t *r)
{
    uint32_t idx = r->tail & (MBOX_RING_SIZE - 1U);
    uint8_t t = r->slots[idx].type;
    r->tail++;
    return t;
}

/* -------------------------------------------------------------------------
 * Tests
 * ------------------------------------------------------------------------- */

static void test_ring_starts_empty(void)
{
    TEST(test_ring_starts_empty);
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));
    EXPECT(ring_empty(&r));
    EXPECT(!ring_full(&r));
    EXPECT(ring_count(&r) == 0);
    PASS();
}

static void test_ring_single_element(void)
{
    TEST(test_ring_single_element);
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));
    ring_push(&r, 0x42);
    EXPECT(!ring_empty(&r));
    EXPECT(ring_count(&r) == 1);
    uint8_t v = ring_pop(&r);
    EXPECT(v == 0x42);
    EXPECT(ring_empty(&r));
    PASS();
}

static void test_ring_full(void)
{
    TEST(test_ring_full);
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));
    for (uint32_t i = 0; i < MBOX_RING_SIZE; i++) {
        EXPECT(!ring_full(&r));
        ring_push(&r, (uint8_t)i);
    }
    EXPECT(ring_full(&r));
    EXPECT(ring_count(&r) == MBOX_RING_SIZE);
    PASS();
}

static void test_ring_fifo_order(void)
{
    TEST(test_ring_fifo_order);
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));
    for (uint8_t i = 0; i < MBOX_RING_SIZE; i++) {
        ring_push(&r, i);
    }
    for (uint8_t i = 0; i < MBOX_RING_SIZE; i++) {
        EXPECT(ring_pop(&r) == i);
    }
    EXPECT(ring_empty(&r));
    PASS();
}

static void test_ring_wrap_around(void)
{
    TEST(test_ring_wrap_around);
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));

    /* Fill completely, then drain completely, then fill again */
    for (uint32_t i = 0; i < MBOX_RING_SIZE; i++) {
        ring_push(&r, (uint8_t)i);
    }
    for (uint32_t i = 0; i < MBOX_RING_SIZE; i++) {
        ring_pop(&r);
    }
    /* head and tail are both MBOX_RING_SIZE; second fill wraps indices */
    for (uint8_t i = 0; i < MBOX_RING_SIZE; i++) {
        ring_push(&r, (uint8_t)(i + 100U));
    }
    for (uint8_t i = 0; i < MBOX_RING_SIZE; i++) {
        EXPECT(ring_pop(&r) == (uint8_t)(i + 100U));
    }
    EXPECT(ring_empty(&r));
    PASS();
}

static void test_ring_head_tail_overflow(void)
{
    TEST(test_ring_head_tail_overflow);
    /*
     * Push head/tail to near UINT32_MAX and verify that unsigned wrap-around
     * doesn't break the full/empty logic.
     */
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));

    /* Start near overflow */
    r.head = UINT32_MAX - (MBOX_RING_SIZE / 2U);
    r.tail = UINT32_MAX - (MBOX_RING_SIZE / 2U);

    for (uint8_t i = 0; i < MBOX_RING_SIZE; i++) {
        EXPECT(!ring_full(&r));
        ring_push(&r, i);
    }
    EXPECT(ring_full(&r));

    for (uint8_t i = 0; i < MBOX_RING_SIZE; i++) {
        EXPECT(ring_pop(&r) == i);
    }
    EXPECT(ring_empty(&r));
    PASS();
}

static void test_ring_interleaved(void)
{
    TEST(test_ring_interleaved);
    mbox_ring_t r;
    memset(&r, 0, sizeof(r));

    /*
     * Produce 2, consume 1 each round.  FIFO order means the consumed value
     * in round N is not round*2 but the oldest item still in the ring:
     *   round 0: push [0,1],   pop 0  → ring: [1]
     *   round 1: push [2,3],   pop 1  → ring: [2,3]
     *   round 2: push [4,5],   pop 2  → ring: [3,4,5]
     *   round 3: push [6,7],   pop 3  → ring: [4,5,6,7]
     */
    for (uint32_t round = 0; round < 4U; round++) {
        ring_push(&r, (uint8_t)(round * 2U));
        ring_push(&r, (uint8_t)(round * 2U + 1U));
        uint8_t v = ring_pop(&r);
        EXPECT(v == (uint8_t)round);
    }
    /* Drain remaining */
    for (uint32_t i = 0; i < 4U; i++) {
        ring_pop(&r);
    }
    EXPECT(ring_empty(&r));
    PASS();
}

static void test_msg_size(void)
{
    TEST(test_msg_size);
    EXPECT(sizeof(msg_t) == 16);
    PASS();
}

/* -------------------------------------------------------------------------
 * main
 * ------------------------------------------------------------------------- */
int main(void)
{
    printf("mailbox ring buffer tests\n");

    test_ring_starts_empty();
    test_ring_single_element();
    test_ring_full();
    test_ring_fifo_order();
    test_ring_wrap_around();
    test_ring_head_tail_overflow();
    test_ring_interleaved();
    test_msg_size();

    if (s_failures == 0) {
        printf("All tests passed.\n");
        return 0;
    }
    printf("%d test(s) FAILED.\n", s_failures);
    return 1;
}
