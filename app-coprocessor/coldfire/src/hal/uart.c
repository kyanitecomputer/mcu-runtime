/*
 * UART HAL — async 16550 driver.
 *
 * Follows the canonical three-part shape: context struct (in .h), async
 * functions, ISR hooks.  No changes to executor, startup.S, linker scripts,
 * or sdk.mk were required to add this driver (SC-7 validated).
 *
 * ISR model:
 *   The CVIC fires when either THRE (TX holding register empty) or DR (data
 *   ready / RX) asserts.  A single per-UART flag tracks each event.
 *   uart_isr() only sets the flags; all framing logic runs in task context.
 *
 * Multi-UART support:
 *   Up to 5 UART instances (UART1–UART5) each have their own flag pair.
 *   uart_isr() identifies the instance by pointer comparison.
 *
 * AST2400 dummy-read errata:
 *   After every 32-bit write from the CF on AST2400, a read of any register
 *   in the same segment is needed to flush the bus.  On UARTs the simplest
 *   target is LSR (which does not have write side-effects).
 */

#include "hal/uart.h"
#include "pac/cvic.h"
#include "sdk_config.h"
#include <stddef.h>
#include <stdint.h>

#ifdef SDK_PLATFORM_AST2400
#  define DUMMY_READ(uart)  ((void)(uart)->lsr)
#else
#  define DUMMY_READ(uart)  ((void)0)
#endif

/* -------------------------------------------------------------------------
 * Per-UART ISR wake flags
 * ISR sets; HAL clears.
 * ------------------------------------------------------------------------- */
#define MAX_UART_INSTANCES  5U

static const UART_TypeDef *s_uart_instances[MAX_UART_INSTANCES];
static volatile uint8_t    s_thre_flag[MAX_UART_INSTANCES]; /* TX ready */
static volatile uint8_t    s_dr_flag[MAX_UART_INSTANCES];   /* RX ready */
static uint8_t             s_uart_count;

static int uart_index(const UART_TypeDef *uart)
{
    for (uint8_t i = 0; i < s_uart_count; i++) {
        if (s_uart_instances[i] == uart) {
            return (int)i;
        }
    }
    return -1;
}

/* -------------------------------------------------------------------------
 * uart_init — configure baud rate, 8N1, enable FIFO
 * ------------------------------------------------------------------------- */
void uart_init(UART_TypeDef *uart, uint32_t baud, uint32_t clk_hz)
{
    uint16_t divisor = (uint16_t)(clk_hz / (16U * baud));

    /* Set DLAB to access divisor latch */
    uart->lcr = UART_LCR_DLAB_Msk;
    DUMMY_READ(uart);

    uart->rbr_thr_dll = (uint8_t)(divisor & 0xFFU);       /* DLL */
    DUMMY_READ(uart);
    uart->ier_dlm     = (uint8_t)((divisor >> 8) & 0xFFU); /* DLM */
    DUMMY_READ(uart);

    /* Clear DLAB, set 8N1 */
    uart->lcr = UART_LCR_WLS_8BIT;
    DUMMY_READ(uart);

    /* Enable and reset FIFOs (FCR) */
    uart->iir_fcr = (uint8_t)(UART_FCR_FIFO_EN_Msk |
                               UART_FCR_RX_RST_Msk  |
                               UART_FCR_TX_RST_Msk);
    DUMMY_READ(uart);

    /* Disable all interrupts initially; enabled selectively by uart_enable_irq */
    uart->ier_dlm = 0;
    DUMMY_READ(uart);
}

/* -------------------------------------------------------------------------
 * uart_enable_irq — register this UART with CVIC and enable its interrupt
 * ------------------------------------------------------------------------- */
void uart_enable_irq(UART_TypeDef *uart, uint32_t cvic_int_num)
{
    if (s_uart_count >= MAX_UART_INSTANCES) {
        return;
    }
    s_uart_instances[s_uart_count] = uart;
    s_thre_flag[s_uart_count] = 0;
    s_dr_flag[s_uart_count]   = 0;
    s_uart_count++;

    /* Enable THRE and RX data interrupts in the UART itself */
    uart->ier_dlm = (uint8_t)(UART_IER_RDI_Msk | UART_IER_THRI_Msk);
    DUMMY_READ(uart);

    /* Clear any stale CVIC edge status, then enable */
    CVIC->edge_clear = (1U << cvic_int_num);
    CVIC->irq_enable = (1U << cvic_int_num);
}

/* -------------------------------------------------------------------------
 * uart_isr — ISR hook; called from cvic_dispatch
 * Must be fast: read LSR, set flags, return.
 * ------------------------------------------------------------------------- */
void uart_isr(UART_TypeDef *uart)
{
    int idx = uart_index(uart);
    if (idx < 0) {
        return;
    }

    uint8_t lsr = uart->lsr;
    if (lsr & UART_LSR_THRE_Msk) {
        s_thre_flag[(uint8_t)idx] = 1;
    }
    if (lsr & UART_LSR_DR_Msk) {
        s_dr_flag[(uint8_t)idx] = 1;
    }
}

/* -------------------------------------------------------------------------
 * uart_write — async TX: send buf[0..len-1]
 * ------------------------------------------------------------------------- */
async uart_write(uart_write_t *ctx, UART_TypeDef *uart,
                 const uint8_t *buf, size_t len)
{
    async_begin(ctx);

    ctx->uart = uart;
    ctx->buf  = buf;
    ctx->len  = len;
    ctx->pos  = 0;

    while (ctx->pos < ctx->len) {
        int idx = uart_index(ctx->uart);

        /* Wait until THRE: either polled LSR or ISR flag */
        await((idx >= 0 && s_thre_flag[(uint8_t)idx]) ||
              (ctx->uart->lsr & UART_LSR_THRE_Msk));

        if (idx >= 0) {
            s_thre_flag[(uint8_t)idx] = 0;
        }

        ctx->uart->rbr_thr_dll = ctx->buf[ctx->pos++];
        DUMMY_READ(ctx->uart);
    }

    /* Wait until shift register drains (TEMT) */
    await(ctx->uart->lsr & UART_LSR_TEMT_Msk);

    async_end;
}

/* -------------------------------------------------------------------------
 * uart_read — async RX: receive exactly `cap` bytes into `buf`
 * ------------------------------------------------------------------------- */
async uart_read(uart_read_t *ctx, UART_TypeDef *uart,
                uint8_t *buf, size_t cap)
{
    async_begin(ctx);

    ctx->uart     = uart;
    ctx->buf      = buf;
    ctx->cap      = cap;
    ctx->received = 0;

    while (ctx->received < ctx->cap) {
        int idx = uart_index(ctx->uart);

        /* Wait until a byte is available */
        await((idx >= 0 && s_dr_flag[(uint8_t)idx]) ||
              (ctx->uart->lsr & UART_LSR_DR_Msk));

        if (idx >= 0) {
            s_dr_flag[(uint8_t)idx] = 0;
        }

        ctx->buf[ctx->received++] = ctx->uart->rbr_thr_dll;
    }

    async_end;
}
