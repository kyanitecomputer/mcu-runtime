#pragma once
#ifndef HAL_UART_H
#define HAL_UART_H

#include "async/async.h"
#include "pac/uart.h"
#include <stdint.h>
#include <stddef.h>

/*
 * UART HAL — async byte-stream TX and RX on a 16550-compatible UART.
 *
 * Usage pattern (canonical three-part shape):
 *
 *   // Context struct — all locals that survive await() live here
 *   typedef struct {
 *       async_state;
 *       uart_write_t tx;        // nested async context
 *       const uint8_t *data;
 *       size_t         len;
 *   } my_task_t;
 *
 *   async my_task(my_task_t *ctx) {
 *       async_begin(ctx);
 *       static const uint8_t msg[] = "hello\r\n";
 *       await(async_call(uart_write, &ctx->tx, UART1, msg, sizeof(msg)));
 *       async_end;
 *   }
 *
 * Initialisation:
 *   uart_init(UART5, 115200, SDK_UART_CLK_HZ);
 *
 * ISR hook (called from cvic_dispatch when the UART's CVIC INT fires):
 *   uart_tx_isr(UART5);   // set THRE flag — no logic in ISR
 *
 * Async discipline:
 *   ISRs set volatile flags only.  All framing logic runs in task context.
 */

/* Clock driving all UARTs.  Override in sdk_config.h if non-standard. */
#ifndef SDK_UART_CLK_HZ
#  define SDK_UART_CLK_HZ  24000000UL
#endif

/* -------------------------------------------------------------------------
 * Async context types
 * ------------------------------------------------------------------------- */

typedef struct {
    async_state;
    const uint8_t *buf;
    size_t         len;
    size_t         pos;
    UART_TypeDef  *uart;
} uart_write_t;

typedef struct {
    async_state;
    uint8_t       *buf;
    size_t         cap;
    size_t         received;
    UART_TypeDef  *uart;
} uart_read_t;

/* -------------------------------------------------------------------------
 * Public API
 * ------------------------------------------------------------------------- */

/**
 * Initialise a UART: set baud rate, 8N1, enable FIFO.
 * @param uart      UART instance (UART1 … UART5)
 * @param baud      Desired baud rate (e.g. 115200)
 * @param clk_hz    UART reference clock in Hz (SDK_UART_CLK_HZ by default)
 */
void uart_init(UART_TypeDef *uart, uint32_t baud, uint32_t clk_hz);

/**
 * Async TX: send `len` bytes from `buf`.
 * Yields after each byte until the TX holding register is empty.
 */
async uart_write(uart_write_t *ctx, UART_TypeDef *uart,
                 const uint8_t *buf, size_t len);

/**
 * Async RX: receive up to `cap` bytes into `buf`.
 * Yields until each byte is available; returns when `cap` bytes received.
 */
async uart_read(uart_read_t *ctx, UART_TypeDef *uart,
                uint8_t *buf, size_t cap);

/**
 * ISR hook: called by cvic_dispatch when this UART's CVIC INT fires.
 * Sets the THRE/DR wake flags; no logic runs here.
 * @param uart  Which UART fired (pass the same instance used in uart_init)
 */
void uart_isr(UART_TypeDef *uart);

/**
 * Register this UART's ISR with the CVIC dispatch table and enable its
 * CVIC interrupt bit.  Call once from your HAL init sequence.
 */
void uart_enable_irq(UART_TypeDef *uart, uint32_t cvic_int_num);

#endif /* HAL_UART_H */
