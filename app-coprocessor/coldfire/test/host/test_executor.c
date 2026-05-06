/*
 * Host-native unit tests for the cooperative executor.
 *
 * Compiled with system gcc (not m68k-elf-gcc).
 * SDK_IDLE() expands to a no-op on non-m68k hosts (see sdk_config.h).
 *
 * Coverage targets:
 *   - sdk_register_task: stores task, enforces SDK_MAX_TASKS limit
 *   - sdk_step: round-robin poll, ASYNC_DONE removal, array compaction
 *   - sdk_run: not tested directly (infinite loop); sdk_step covers the logic
 */

#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#include "sdk.h"

/* -------------------------------------------------------------------------
 * Test infrastructure
 * ------------------------------------------------------------------------- */
static int s_failures;

#define TEST(name) \
    do { printf("  %-40s", #name); fflush(stdout); } while (0)

#define PASS() \
    do { puts("PASS"); } while (0)

#define FAIL(msg) \
    do { printf("FAIL: %s\n", msg); s_failures++; } while (0)

#define EXPECT(cond) \
    do { if (!(cond)) { FAIL(#cond); return; } } while (0)



/* -------------------------------------------------------------------------
 * Mock poll functions
 * ------------------------------------------------------------------------- */

/* Always returns ASYNC_CONT */
static async mock_cont(void *ctx)
{
    int *count = (int *)ctx;
    (*count)++;
    return ASYNC_CONT;
}

/* Returns ASYNC_DONE on the Nth call */
typedef struct {
    int calls_remaining;
    int total_calls;
} countdown_ctx_t;

static async mock_countdown(void *ctx)
{
    countdown_ctx_t *c = (countdown_ctx_t *)ctx;
    c->total_calls++;
    if (--c->calls_remaining <= 0) {
        return ASYNC_DONE;
    }
    return ASYNC_CONT;
}

/* Immediately done */
static async mock_done(void *ctx)
{
    (void)ctx;
    return ASYNC_DONE;
}

/* -------------------------------------------------------------------------
 * Tests
 * ------------------------------------------------------------------------- */

static void test_empty_step(void)
{
    TEST(test_empty_step);
    sdk_init();
    uint8_t n = sdk_step();
    EXPECT(n == 0);
    EXPECT(sdk_task_count() == 0);
    PASS();
}

static void test_single_cont_task(void)
{
    TEST(test_single_cont_task);
    sdk_init();
    int call_count = 0;
    sdk_register_task(mock_cont, &call_count);
    EXPECT(sdk_task_count() == 1);
    uint8_t n = sdk_step();
    EXPECT(n == 1);                     /* still active */
    EXPECT(call_count == 1);            /* was polled once */
    EXPECT(sdk_task_count() == 1);
    PASS();
}

static void test_single_done_task(void)
{
    TEST(test_single_done_task);
    sdk_init();
    sdk_register_task(mock_done, NULL);
    EXPECT(sdk_task_count() == 1);
    uint8_t n = sdk_step();
    EXPECT(n == 0);                     /* removed after ASYNC_DONE */
    EXPECT(sdk_task_count() == 0);
    PASS();
}

static void test_countdown_task(void)
{
    TEST(test_countdown_task);
    sdk_init();
    countdown_ctx_t c = { .calls_remaining = 3, .total_calls = 0 };
    sdk_register_task(mock_countdown, &c);

    /* Step 1 and 2: ASYNC_CONT */
    uint8_t n = sdk_step();
    EXPECT(n == 1);
    EXPECT(c.total_calls == 1);

    n = sdk_step();
    EXPECT(n == 1);
    EXPECT(c.total_calls == 2);

    /* Step 3: ASYNC_DONE — task removed */
    n = sdk_step();
    EXPECT(n == 0);
    EXPECT(c.total_calls == 3);
    EXPECT(sdk_task_count() == 0);
    PASS();
}

static void test_two_tasks_both_cont(void)
{
    TEST(test_two_tasks_both_cont);
    sdk_init();
    int ca = 0, cb = 0;
    sdk_register_task(mock_cont, &ca);
    sdk_register_task(mock_cont, &cb);
    EXPECT(sdk_task_count() == 2);

    sdk_step();

    EXPECT(ca == 1);                    /* both polled once */
    EXPECT(cb == 1);
    EXPECT(sdk_task_count() == 2);
    PASS();
}

static void test_done_task_removal_compaction(void)
{
    TEST(test_done_task_removal_compaction);
    sdk_init();
    int ca = 0;
    countdown_ctx_t cb = { .calls_remaining = 1, .total_calls = 0 };
    int cc = 0;

    /* Register three tasks: A (cont), B (done after 1), C (cont) */
    sdk_register_task(mock_cont,      &ca);
    sdk_register_task(mock_countdown, &cb);
    sdk_register_task(mock_cont,      &cc);
    EXPECT(sdk_task_count() == 3);

    uint8_t n = sdk_step();
    EXPECT(n == 2);                     /* B removed */
    EXPECT(ca == 1);                    /* A was polled */
    EXPECT(cb.total_calls == 1);        /* B was polled (then removed) */
    EXPECT(cc == 1);                    /* C was polled (compacted into B's slot) */
    PASS();
}

static void test_max_tasks_limit(void)
{
    TEST(test_max_tasks_limit);
    sdk_init();
    int counts[SDK_MAX_TASKS + 1] = {0};

    /* Register exactly SDK_MAX_TASKS tasks */
    for (int i = 0; i < SDK_MAX_TASKS; i++) {
        sdk_register_task(mock_cont, &counts[i]);
    }
    EXPECT(sdk_task_count() == SDK_MAX_TASKS);

    /* Registering one more must be silently ignored */
    sdk_register_task(mock_cont, &counts[SDK_MAX_TASKS]);
    EXPECT(sdk_task_count() == SDK_MAX_TASKS);
    PASS();
}

static void test_multiple_steps_round_robin(void)
{
    TEST(test_multiple_steps_round_robin);
    sdk_init();
    int ca = 0, cb = 0;
    sdk_register_task(mock_cont, &ca);
    sdk_register_task(mock_cont, &cb);

    for (int i = 0; i < 5; i++) {
        sdk_step();
    }

    /* Both tasks polled equally */
    EXPECT(ca == 5);
    EXPECT(cb == 5);
    PASS();
}

/* -------------------------------------------------------------------------
 * main
 * ------------------------------------------------------------------------- */
int main(void)
{
    printf("executor tests\n");

    test_empty_step();
    test_single_cont_task();
    test_single_done_task();
    test_countdown_task();
    test_two_tasks_both_cont();
    test_done_task_removal_compaction();
    test_max_tasks_limit();
    test_multiple_steps_round_robin();

    if (s_failures == 0) {
        printf("All tests passed.\n");
        return 0;
    }
    printf("%d test(s) FAILED.\n", s_failures);
    return 1;
}
