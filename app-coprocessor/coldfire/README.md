# ast-cf-runtime

Stackless cooperative async SDK for the ColdFire V1 coprocessor embedded in
the Aspeed AST2400 and AST2500 BMC SoCs.

See **ROADMAP.md** for the full specification, memory architecture, and
implementation status.

## 10-minute quickstart

### Prerequisites

You need `m68k-elf-gcc` built with CFV1 multilib support.  Distro packages
frequently ship ColdFire V2/V4e only; build your own or use a known-good
crosstool-NG config.  Verify with:

```sh
m68k-elf-gcc -mcpu=cfv1 -print-multi-lib | grep cfv1
```

The output must contain a `cfv1` entry.  If it shows only `.;` the compiler
lacks CFV1 multilib and will produce incorrect code.

### Clone and build

```sh
git clone https://github.com/kyanitecomputer/ast-cf-runtime
cd ast-cf-runtime/template
make          # produces firmware.elf + firmware.bin
make size     # confirm binary fits in DRAM window
make disasm   # annotated disassembly in firmware.lst
```

### Load on hardware

The ARM side must:
1. Assert `SCU_COPRO_RESET` (SCU100 bit 1), wait ≥ 1 µs.
2. Program the segment registers (see ROADMAP §5.1 for exact SCU values per platform).
3. Copy `firmware.bin` byte-for-byte into the reserved DRAM region.
4. Deassert reset, set `SCU_COPRO_CLK_EN` (SCU100 bit 0).

The CF core boots, runs `_start`, and jumps to `main`.

### Run the host unit tests

No cross-compiler or hardware needed:

```sh
make check          # 24 tests across executor, async.h, and mailbox ring
```

## Async discipline — the one rule that trips everyone

```c
/* WRONG: counter lives on the C stack and is destroyed across await() */
async bad_task(bad_t *ctx) {
    async_begin(ctx);
    int counter = 0;              /* ← stack local */
    await(async_call(sdk_sleep_ms, &ctx->sleep, 100));
    counter++;                    /* ← reads garbage — stack was reused */
    async_end;
}

/* RIGHT: counter lives in the context struct */
async good_task(good_t *ctx) {
    async_begin(ctx);
    ctx->counter = 0;             /* ← in struct, survives yield */
    await(async_call(sdk_sleep_ms, &ctx->sleep, 100));
    ctx->counter++;               /* ← still valid */
    async_end;
}
```

**All five rules** (in `include/async/async.h`):
1. Locals that survive `await()` go in the context struct, not the stack.
2. Never `return` from inside an async function — use `async_end` or `async_exit`.
3. Never `switch` inside an async function (conflicts with Duff's device).
4. ISRs set flags only — no logic, no calls to async functions.
5. No blocking calls — every wait is `await(condition)`.

## Writing a task

```c
#include <sdk.h>
#include <hal/timer.h>
#include <hal/mbox.h>

typedef struct {
    async_state;
    sdk_sleep_t  sleep;     /* sdk_sleep_ms context — MUST be in struct */
    mbox_send_t  tx;        /* mbox_send context */
    uint32_t     counter;
} heartbeat_t;

static heartbeat_t s_hb;

async heartbeat_task(heartbeat_t *ctx)
{
    async_begin(ctx);

    for (;;) {
        ctx->counter++;
        await(async_call(mbox_send, &ctx->tx,
              &(msg_t){ .type = 1, .len = 4,
                        .payload = { ctx->counter & 0xFF } }));
        await(async_call(sdk_sleep_ms, &ctx->sleep, 1000));
    }

    async_end;
}

int main(void)
{
    sdk_init();
    SDK_TASK(heartbeat_task, &s_hb);
    sdk_run();   /* never returns */
}
```

User Makefile (three lines):

```make
SDK_DIR := ../ast-cf-runtime
include $(SDK_DIR)/sdk.mk
SRCS    += main.c
```

## Configuration

All knobs have safe defaults.  Set before `include $(SDK_DIR)/sdk.mk`:

| Variable | Default | Description |
|----------|---------|-------------|
| `SDK_PLATFORM` | `ast2500` | `ast2500` or `ast2400` |
| `SDK_MAX_TASKS` | `8` | Max concurrent tasks |
| `SDK_TICK_HZ` | `1000` | Tick rate (Timer 7, 1 MHz clock) |
| `SDK_TICK_TIMER` | `7` | SoC timer reserved for SDK tick (1–7) |
| `SDK_SRAM_STACK_SIZE` | `2048` | Stack bytes at top of SRAM |
| `SDK_USE_NEWLIB_NANO` | `0` | Set to `1` to link newlib-nano |

**Timer reservation:** `SDK_TICK_TIMER` (default: 7) must not be assigned to
the ARM Linux kernel in the device tree.  Add to your DTS:

```
/* Reserve Timer 7 for ColdFire SDK tick */
&timer { aspeed,timer-forbidden = <7>; };
```

## SRAM layout

Both processors share the 36 KB (AST2500) or 32 KB (AST2400) internal SRAM at
ARM physical `0x1E720000`.  The SDK claims a fixed region:

```
CF BE 0x320000  ┌──────────────────────────┐
                │  SDK header (64 B)        │  startup flag, version
        +0x0040 ├──────────────────────────┤
                │  ARM→CF mailbox ring      │  mbox_ring_t
        +...    ├──────────────────────────┤
                │  CF→ARM mailbox ring      │  mbox_ring_t
        +...    ├──────────────────────────┤
                │  Application shared area  │
                ├──────────────────────────┤
                │  CF stack (grows ↓)       │  SDK_SRAM_STACK_SIZE bytes
CF BE 0x329000  └──────────────────────────┘  (AST2500 top)
```

The ARM firmware must not write above the SDK header boundary after CF reset.

## Adding a HAL driver (SC-7 checklist)

Adding a new peripheral requires exactly these files — no executor, startup,
or build system changes:

1. `include/pac/<periph>.h` — register map (or use chiptool-generated output)
2. `include/hal/<periph>.h` — context structs + function declarations
3. `src/hal/<periph>.c` — `init`, async functions, ISR hook
4. One line in `src/runtime/vectors.c` — forward-declare the ISR and add it
   to the `cvic_handlers[]` table

The UART driver (`src/hal/uart.c`) was added in M5 without touching any other
file — verifying SC-7.

## Target smoke tests (future — M7)

Hardware verification tests are planned for `test/target/`.  Until then, the
ARM-side echo harness for SC-3 verification is:

1. ARM writes 1000 messages to the ARM→CF ring.
2. CF `mbox_recv` + `mbox_send` echo each one back.
3. ARM verifies sequence numbers match and no messages are dropped.
