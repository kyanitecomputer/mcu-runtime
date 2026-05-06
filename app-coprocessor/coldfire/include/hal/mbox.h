#pragma once
#ifndef HAL_MBOX_H
#define HAL_MBOX_H

#include "async/async.h"
#include "sdk_config.h"
#include <stdint.h>

/*
 * Mailbox HAL — async ARM↔CF message passing over shared SRAM.
 *
 * Protocol:
 *   - Two SPSC ring buffers in SRAM: one per direction.
 *   - ARM → CF: ARM writes to arm2cf_ring, triggers CF via CVIC18 bit 1
 *               (CVIC INT#1, "ARM interrupt").
 *   - CF → ARM: CF writes to cf2arm_ring, triggers ARM via VIC VICB4 bit 13
 *               (ARM IRQ#45, "Coprocessor interrupt").
 *   - Rings are lock-free single-producer/single-consumer (SPSC).
 *     Head written by producer only; tail written by consumer only.
 *
 * Message format (16 bytes):
 *   type=0x00  reserved for printf_mbox debug strings
 *   type=0x01+ application-defined
 *
 * SRAM layout (starts at SDK_SRAM_MBOX_OFFSET from SRAM base):
 *   [header 64B] [arm2cf ring] [cf2arm ring]
 *
 * Endianness:
 *   The msg_t struct is accessed by both CF (BE) and ARM (LE).  The protocol
 *   deliberately stores all multi-byte fields in little-endian byte order so
 *   that the LE ARM sees them natively.  The CF writes/reads them via the LE
 *   SRAM segment (0x720000) using uint8_t accesses for multi-byte fields, or
 *   via explicit byte-swap helpers below.
 *
 * Usage:
 *   typedef struct {
 *       async_state;
 *       mbox_send_t tx;
 *   } my_task_t;
 *
 *   async my_task(my_task_t *ctx) {
 *       async_begin(ctx);
 *       msg_t m = { .type = 1, .len = 0 };
 *       await(async_call(mbox_send, &ctx->tx, &m));
 *       async_end;
 *   }
 */

/* -------------------------------------------------------------------------
 * Message
 * ------------------------------------------------------------------------- */
#define MBOX_MAX_PAYLOAD  SDK_MBOX_MAX_PAYLOAD

typedef struct {
    uint8_t  type;                      /* 0x00 = debug printf; others = user */
    uint8_t  seq;                       /* rolling sequence number (set by sender) */
    uint16_t len;                       /* payload length in bytes */
    uint8_t  payload[MBOX_MAX_PAYLOAD]; /* message body */
} msg_t;

_Static_assert(sizeof(msg_t) == 16, "msg_t must be 16 bytes");

/* -------------------------------------------------------------------------
 * SPSC ring buffer (placed in shared SRAM)
 * ------------------------------------------------------------------------- */
#define MBOX_RING_SIZE  SDK_MBOX_RING_SIZE

typedef struct {
    volatile uint32_t head;             /* producer writes (index of next slot to fill) */
    volatile uint32_t tail;             /* consumer writes (index of next slot to drain) */
    uint32_t          _pad[2];          /* pad to 16 bytes before slots */
    msg_t             slots[MBOX_RING_SIZE];
} mbox_ring_t;

_Static_assert((MBOX_RING_SIZE & (MBOX_RING_SIZE - 1)) == 0,
               "MBOX_RING_SIZE must be a power of 2");

/* -------------------------------------------------------------------------
 * Async context types
 * ------------------------------------------------------------------------- */
typedef struct {
    async_state;
    const msg_t *msg;
    uint8_t      seq;
} mbox_send_t;

typedef struct {
    async_state;
    msg_t  buf;
} mbox_recv_t;

/* -------------------------------------------------------------------------
 * Public API
 * ------------------------------------------------------------------------- */

/** Initialise mailbox hardware (CVIC INT#1 enable). Called from sdk_init(). */
void mbox_init(void);

/** Send msg to ARM.  Yields while the CF→ARM ring is full. */
async mbox_send(mbox_send_t *ctx, const msg_t *msg);

/** Receive a message from ARM.  Yields while the ARM→CF ring is empty. */
async mbox_recv(mbox_recv_t *ctx);

/** Write a printf-style debug string to the ARM host (type=0x00). */
void mbox_printf(const char *fmt, ...);

/** ISR: called by cvic_dispatch when CVIC INT#1 fires (ARM→CF doorbell). */
void mbox_rx_isr(void);

/* -------------------------------------------------------------------------
 * SRAM layout constants
 * Shared SRAM at CF BE address 0x320000 (AST2500) / 0x320000 (AST2400).
 * Layout (offsets from SRAM base):
 *   0x0000  SDK header (64 bytes)
 *   0x0040  ARM→CF ring (mbox_ring_t)
 *   0x0040 + sizeof(mbox_ring_t)  CF→ARM ring (mbox_ring_t)
 * ------------------------------------------------------------------------- */
#define SDK_SRAM_BASE           0x320000U       /* CF BE segment address */
#define SDK_SRAM_LE_BASE        0x720000U       /* CF LE segment address (registers) */
#define SDK_SRAM_HDR_OFFSET     0x0000U
#define SDK_SRAM_HDR_SIZE       0x0040U         /* 64 bytes */
#define SDK_SRAM_ARM2CF_OFFSET  (SDK_SRAM_HDR_SIZE)
#define SDK_SRAM_CF2ARM_OFFSET  (SDK_SRAM_ARM2CF_OFFSET + sizeof(mbox_ring_t))

#define MBOX_ARM2CF  ((mbox_ring_t *)(SDK_SRAM_BASE + SDK_SRAM_ARM2CF_OFFSET))
#define MBOX_CF2ARM  ((mbox_ring_t *)(SDK_SRAM_BASE + SDK_SRAM_CF2ARM_OFFSET))

#endif /* HAL_MBOX_H */
