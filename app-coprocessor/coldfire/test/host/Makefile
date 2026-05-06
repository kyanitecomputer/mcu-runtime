# Host-native unit tests — compiled with system gcc (not cross-compiler).
# Run from SDK root: make check
# Or directly:       make -C test/host check

SDK_ROOT := ../..
CC       := gcc
CFLAGS   := -std=c11 -Wall -Wextra -Werror -g -DSDK_MAX_TASKS=8
COVFLAGS := -fprofile-arcs -ftest-coverage
INCLUDES := -I$(SDK_ROOT)/include

# Sources under test (compiled for host — SDK_IDLE() is a no-op on non-m68k)
SDK_SRCS := $(SDK_ROOT)/src/runtime/executor.c

TESTS    := test_executor test_async_sm test_ring

.PHONY: all check clean

all: $(TESTS)

test_executor: test_executor.c $(SDK_SRCS)
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $^

test_async_sm: test_async_sm.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $^

test_ring: test_ring.c
	$(CC) $(CFLAGS) $(INCLUDES) -o $@ $^

check: all
	@for t in $(TESTS); do \
	    echo "Running $$t..."; \
	    ./$$t || exit 1; \
	done
	@echo "All host tests passed."

# Generate coverage report (requires gcov).
# Run separately from 'check' because -fprofile-arcs conflicts with
# the Duff's device switch pattern used in async.h.
coverage: $(TESTS:%=%-cov)
	gcov -b $(SDK_SRCS)

%-cov: %.c $(SDK_SRCS)
	$(CC) $(CFLAGS) $(COVFLAGS) $(INCLUDES) -o $@ $^
	./$@

clean:
	rm -f $(TESTS) *.gcda *.gcno *.gcov
