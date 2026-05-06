/*
 * Aspeed AST2400 / AST2500 UART Controller — Stub PAC Header
 * GENERATED FILE — do not edit by hand.
 *
 * The AST SoC has five UARTs (UART1–UART5).  All are 16550-compatible.
 *
 * Base addresses (ARM physical → CF LE virtual):
 *   UART1  0x1E783000 → 0x783000
 *   UART2  0x1E78D000 → 0x78D000
 *   UART3  0x1E78E000 → 0x78E000
 *   UART4  0x1E78F000 → 0x78F000
 *   UART5  0x1E784000 → 0x784000
 *
 * All UARTs route interrupts to the CF via CVIC INT#3–INT#7 (UART1–UART5).
 *
 * DLAB (Divisor Latch Access Bit) in LCR must be set to access DLL/DLM;
 * must be cleared for normal TX/RX operation.
 *
 * Register layout (16550 standard, 8-bit wide):
 *   With DLAB=0: offset 0 = RBR (read) / THR (write), offset 1 = IER
 *   With DLAB=1: offset 0 = DLL, offset 1 = DLM
 *
 * This PAC header models each UART as a 4-byte-wide struct so CF 32-bit
 * bus cycles work correctly.  Individual register bytes are accessed via
 * uint8_t fields with explicit compiler packing.
 */

#pragma once
#ifndef PAC_UART_H
#define PAC_UART_H

#include <stdint.h>

/* Base addresses — LE segment (CF virtual) */
#define UART1_BASE  0x783000U
#define UART2_BASE  0x78D000U
#define UART3_BASE  0x78E000U
#define UART4_BASE  0x78F000U
#define UART5_BASE  0x784000U

/*
 * 16550 register map.  Each register is 8 bits, mapped at 4-byte stride
 * (32-bit bus, byte-lane access via the LE segment).
 */
typedef struct {
    volatile uint8_t rbr_thr_dll;   /* 0x00: RBR (r), THR (w), DLL (DLAB) */
    volatile uint8_t _pad0[3];
    volatile uint8_t ier_dlm;       /* 0x04: IER / DLM (DLAB) */
    volatile uint8_t _pad1[3];
    volatile uint8_t iir_fcr;       /* 0x08: IIR (r) / FCR (w) */
    volatile uint8_t _pad2[3];
    volatile uint8_t lcr;           /* 0x0C: Line Control Register */
    volatile uint8_t _pad3[3];
    volatile uint8_t mcr;           /* 0x10: Modem Control Register */
    volatile uint8_t _pad4[3];
    volatile uint8_t lsr;           /* 0x14: Line Status Register */
    volatile uint8_t _pad5[3];
    volatile uint8_t msr;           /* 0x18: Modem Status Register */
    volatile uint8_t _pad6[3];
    volatile uint8_t scr;           /* 0x1C: Scratch Register */
    volatile uint8_t _pad7[3];
} UART_TypeDef;

#define UART1  ((UART_TypeDef *)UART1_BASE)
#define UART2  ((UART_TypeDef *)UART2_BASE)
#define UART3  ((UART_TypeDef *)UART3_BASE)
#define UART4  ((UART_TypeDef *)UART4_BASE)
#define UART5  ((UART_TypeDef *)UART5_BASE)

/* LSR (Line Status Register) bit fields */
#define UART_LSR_DR_Pos    0U   /* Data Ready */
#define UART_LSR_DR_Msk    (1U << UART_LSR_DR_Pos)
#define UART_LSR_OE_Pos    1U   /* Overrun Error */
#define UART_LSR_OE_Msk    (1U << UART_LSR_OE_Pos)
#define UART_LSR_PE_Pos    2U   /* Parity Error */
#define UART_LSR_PE_Msk    (1U << UART_LSR_PE_Pos)
#define UART_LSR_FE_Pos    3U   /* Framing Error */
#define UART_LSR_FE_Msk    (1U << UART_LSR_FE_Pos)
#define UART_LSR_BI_Pos    4U   /* Break Interrupt */
#define UART_LSR_BI_Msk    (1U << UART_LSR_BI_Pos)
#define UART_LSR_THRE_Pos  5U   /* TX Holding Register Empty */
#define UART_LSR_THRE_Msk  (1U << UART_LSR_THRE_Pos)
#define UART_LSR_TEMT_Pos  6U   /* Transmitter Empty */
#define UART_LSR_TEMT_Msk  (1U << UART_LSR_TEMT_Pos)

/* IER (Interrupt Enable Register) bit fields */
#define UART_IER_RDI_Pos   0U   /* Received Data Interrupt */
#define UART_IER_RDI_Msk   (1U << UART_IER_RDI_Pos)
#define UART_IER_THRI_Pos  1U   /* TX Holding Register Empty Interrupt */
#define UART_IER_THRI_Msk  (1U << UART_IER_THRI_Pos)

/* FCR (FIFO Control Register) write-only bits */
#define UART_FCR_FIFO_EN_Msk   (1U << 0)
#define UART_FCR_RX_RST_Msk    (1U << 1)
#define UART_FCR_TX_RST_Msk    (1U << 2)

/* LCR (Line Control Register) */
#define UART_LCR_WLS_8BIT      0x03U   /* 8-bit word length */
#define UART_LCR_DLAB_Msk      (1U << 7)

/* CVIC interrupt numbers for UARTs (from CVIC INT source table) */
#define CVIC_INT_UART1_NUM  3U
#define CVIC_INT_UART2_NUM  4U
#define CVIC_INT_UART3_NUM  5U
#define CVIC_INT_UART4_NUM  6U
#define CVIC_INT_UART5_NUM  7U

#endif /* PAC_UART_H */
