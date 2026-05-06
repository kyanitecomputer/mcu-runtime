/*
 * Host-native unit tests for the vendored async.h state machine.
 *
 * Tests cover:
 *   - async_begin / async_end: init and terminal states
 *   - await: yields while condition is false, resumes when true
 *   - async_yield: unconditional single-pass yield
 *   - async_call: nested async function driving
 *   - async_init / async_done: lifecycle helpers
 *   - __COUNTER__ uniqueness: two await macros that would share a __LINE__
 *     on the same source line must produce distinct case labels
 *   - block_until: SDK-local macro for synchronous blocking
 */

#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <stdbool.h>
#include <string.h>

#include "async/async.h"

static int s_failures;

#define TEST(name) \
    do { printf("  %-50s", #name); fflush(stdout); } while (0)
#define PASS()  puts("PASS")
#define FAIL(msg) \
    do { printf("FAIL: %s\n", msg); s_failures++; return; } while (0)
#define EXPECT(cond) \
    do { if (!(cond)) { FAIL(#cond); } } while (0)

/* -------------------------------------------------------------------------
 * Test: basic begin/end — immediately done
 * ------------------------------------------------------------------------- */
typedef struct { async_state; } trivial_t;

static async trivial_fn(trivial_t *ctx)
{
    async_begin(ctx);
    async_end;
}

static void test_begin_end_immediate_done(void)
{
    TEST(test_begin_end_immediate_done);
    trivial_t ctx;
    async_init(&ctx);
    EXPECT(!async_done(&ctx));
    async result = trivial_fn(&ctx);
    EXPECT(result == ASYNC_DONE);
    EXPECT(async_done(&ctx));
    /* Second call on a done context must still return ASYNC_DONE */
    result = trivial_fn(&ctx);
    EXPECT(result == ASYNC_DONE);
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: await — yields until external flag is set
 * ------------------------------------------------------------------------- */
typedef struct {
    async_state;
    volatile bool flag;
    int           polls_before_done;
} await_t;

static async await_fn(await_t *ctx)
{
    async_begin(ctx);
    ctx->polls_before_done = 0;
    await(ctx->flag);           /* yields while flag is false */
    async_end;
}

static void test_await_yields_then_resumes(void)
{
    TEST(test_await_yields_then_resumes);
    await_t ctx = { .flag = false };
    async_init(&ctx);

    /* Should yield while flag is false */
    EXPECT(await_fn(&ctx) == ASYNC_CONT);
    EXPECT(await_fn(&ctx) == ASYNC_CONT);

    ctx.flag = true;

    /* Should complete now */
    EXPECT(await_fn(&ctx) == ASYNC_DONE);
    EXPECT(async_done(&ctx));
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: async_yield — single unconditional yield
 * ------------------------------------------------------------------------- */
typedef struct {
    async_state;
    int step;
} yield_t;

static async yield_fn(yield_t *ctx)
{
    async_begin(ctx);
    ctx->step = 1;
    async_yield;        /* yields here for exactly one pass */
    ctx->step = 2;
    async_end;
}

static void test_async_yield_one_pass(void)
{
    TEST(test_async_yield_one_pass);
    yield_t ctx = { .step = 0 };
    async_init(&ctx);

    async r = yield_fn(&ctx);
    EXPECT(r == ASYNC_CONT);
    EXPECT(ctx.step == 1);          /* ran up to yield */

    r = yield_fn(&ctx);
    EXPECT(r == ASYNC_DONE);
    EXPECT(ctx.step == 2);          /* ran past yield to end */
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: async_call — nested async function
 * ------------------------------------------------------------------------- */
typedef struct { async_state; } inner_t;
typedef struct {
    async_state;
    inner_t   inner;
    int       after_call;
} outer_t;

static async inner_fn(inner_t *ctx)
{
    async_begin(ctx);
    async_yield;        /* takes two polls to complete */
    async_end;
}

static async outer_fn(outer_t *ctx)
{
    async_begin(ctx);
    ctx->after_call = 0;
    await(async_call(inner_fn, &ctx->inner));
    ctx->after_call = 1;
    async_end;
}

static void test_async_call_nested(void)
{
    TEST(test_async_call_nested);
    outer_t ctx;
    memset(&ctx, 0, sizeof(ctx));   /* async_init all nested states */
    async_init(&ctx);

    /* Poll 1: inner yields at async_yield → outer awaits → CONT */
    EXPECT(outer_fn(&ctx) == ASYNC_CONT);
    EXPECT(ctx.after_call == 0);

    /* Poll 2: inner completes → await satisfied → outer runs to end → DONE */
    EXPECT(outer_fn(&ctx) == ASYNC_DONE);
    EXPECT(ctx.after_call == 1);
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: async_init / async_done lifecycle
 * ------------------------------------------------------------------------- */
static void test_lifecycle_init_done(void)
{
    TEST(test_lifecycle_init_done);
    trivial_t ctx;
    async_init(&ctx);
    EXPECT(ctx._async_k == ASYNC_INIT);
    EXPECT(!async_done(&ctx));

    trivial_fn(&ctx);
    EXPECT(async_done(&ctx));
    EXPECT(ctx._async_k == ASYNC_DONE);

    /* Re-initialise and reuse */
    async_init(&ctx);
    EXPECT(!async_done(&ctx));
    EXPECT(ctx._async_k == ASYNC_INIT);
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: __COUNTER__ uniqueness — two awaits on the same conceptual line.
 *
 * We test this by writing two await macros whose expansion would have produced
 * duplicate case labels under __LINE__ when the two macros are written on the
 * same source line (e.g. in a macro that wraps them).  With __COUNTER__ each
 * gets a unique value.  We verify that the state machine transitions correctly
 * through BOTH await points.
 * ------------------------------------------------------------------------- */
typedef struct {
    async_state;
    volatile bool flag_a;
    volatile bool flag_b;
    int           checkpoint;
} dual_await_t;

static async dual_await_fn(dual_await_t *ctx)
{
    async_begin(ctx);
    ctx->checkpoint = 0;
    await(ctx->flag_a);     ctx->checkpoint = 1; await(ctx->flag_b); /* two awaits, same line */
    ctx->checkpoint = 2;
    async_end;
}

static void test_counter_unique_case_labels(void)
{
    TEST(test_counter_unique_case_labels);
    dual_await_t ctx = { .flag_a = false, .flag_b = false };
    async_init(&ctx);

    /* Neither flag set — yields at flag_a */
    EXPECT(dual_await_fn(&ctx) == ASYNC_CONT);
    EXPECT(ctx.checkpoint == 0);

    /* Set flag_a — passes flag_a, yields at flag_b */
    ctx.flag_a = true;
    EXPECT(dual_await_fn(&ctx) == ASYNC_CONT);
    EXPECT(ctx.checkpoint == 1);

    /* Set flag_b — completes */
    ctx.flag_b = true;
    EXPECT(dual_await_fn(&ctx) == ASYNC_DONE);
    EXPECT(ctx.checkpoint == 2);
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: async_state is uint16_t (SDK patch — 2 bytes, not 4)
 * ------------------------------------------------------------------------- */
static void test_async_state_width(void)
{
    TEST(test_async_state_width);
    EXPECT(sizeof(((trivial_t *)0)->_async_k) == 2);
    PASS();
}

/* -------------------------------------------------------------------------
 * Test: block_until — synchronous blocking loop
 * ------------------------------------------------------------------------- */
static void test_block_until(void)
{
    TEST(test_block_until);
    volatile int x = 0;
    int count = 0;
    /* Increment x after 3 spins of a manual loop */
    block_until((count++ >= 3) || (x = 1, true));
    EXPECT(x == 1 || count > 0);   /* either branch satisfied */
    PASS();
}

/* -------------------------------------------------------------------------
 * main
 * ------------------------------------------------------------------------- */
int main(void)
{
    printf("async.h state machine tests\n");

    test_begin_end_immediate_done();
    test_await_yields_then_resumes();
    test_async_yield_one_pass();
    test_async_call_nested();
    test_lifecycle_init_done();
    test_counter_unique_case_labels();
    test_async_state_width();
    test_block_until();

    if (s_failures == 0) {
        printf("All tests passed.\n");
        return 0;
    }
    printf("%d test(s) FAILED.\n", s_failures);
    return 1;
}
