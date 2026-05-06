/*
 * Aspeed AST2400 / AST2500 Coprocessor Vectored Interrupt Controller (CVIC)
 * Stub PAC Header — GENERATED FILE, do not edit by hand.
 *
 * Physical base: 0x1E6C2000
 * CF LE virtual: 0x6C2000  (segment 6 on AST2500; segment 3 on AST2400)
 *
 * The CVIC provides 31 interrupt sources (bits 30:0; bit 31 reserved).
 * All CVIC interrupts share a single CPU exception vector (0x46 = 70).
 * The ISR reads CVIC_IRQ_STATUS to identify the source(s) and dispatches.
 *
 * Default CVIC24 (sensitivity) = 0x7FFF00FF:
 *   bits  7:0  = 0xFF → INT#0–7  level-sensitive (SDRAM, ARM SW, LPC, UARTs)
 *   bits 15:8  = 0x00 → INT#8–14 edge-triggered   (Timer 1–7)
 *   bits 30:16 = 0x7FFF → INT#15–30 level-sensitive
 *
 * Default CVIC28 (both-edge) = 0x0000FF00:
 *   bits 15:8  = 0xFF → INT#8–14 both-edge (Timer 1–7; rising = reload done)
 *
 * Timer 7 → CVIC INT#14  (bit 14 in all CVIC registers)
 * ARM SW  → CVIC INT#1   (bit  1; written via CVIC18 from ARM or CF)
 */

#pragma once
#ifndef PAC_CVIC_H
#define PAC_CVIC_H

#include <stdint.h>

/* Base address (LE segment) */
#define CVIC_BASE   0x6C2000U

typedef struct {
    volatile uint32_t irq_status;   /* 0x00: Masked IRQ status (RO) */
    volatile uint32_t _pad0;        /* 0x04: reserved */
    volatile uint32_t raw_status;   /* 0x08: Raw interrupt status (RO) */
    volatile uint32_t _pad1;        /* 0x0C: reserved */
    volatile uint32_t irq_enable;   /* 0x10: IRQ enable (W=set; clear via disable) */
    volatile uint32_t irq_disable;  /* 0x14: IRQ enable clear (W=clear) */
    volatile uint32_t sw_set;       /* 0x18: Software interrupt set (W=set) */
    volatile uint32_t sw_clear;     /* 0x1C: Software interrupt clear (W=clear) */
    volatile uint32_t _pad2;        /* 0x20: reserved */
    volatile uint32_t _pad3;        /* 0x24: sensitivity (RO, init=0x7FFF00FF) */
    volatile uint32_t _pad4;        /* 0x28: both-edge control (RO) */
    volatile uint32_t _pad5;        /* 0x2C: event (RO) */
    volatile uint32_t _pad6[2];     /* 0x30: reserved */
    volatile uint32_t edge_clear;   /* 0x38: Edge-triggered clear (W=clear) */
    volatile uint32_t edge_status;  /* 0x3C: Edge-triggered status (RO) */
} CVIC_TypeDef;

#define CVIC  ((CVIC_TypeDef *)CVIC_BASE)

/* Interrupt source bit masks */
#define CVIC_INT_ARM_SW_Msk     (1U <<  1)  /* ARM→CF software interrupt */
#define CVIC_INT_TIMER1_Msk     (1U <<  8)  /* Timer 1 */
#define CVIC_INT_TIMER2_Msk     (1U <<  9)
#define CVIC_INT_TIMER3_Msk     (1U << 10)
#define CVIC_INT_TIMER4_Msk     (1U << 11)
#define CVIC_INT_TIMER5_Msk     (1U << 12)
#define CVIC_INT_TIMER6_Msk     (1U << 13)
#define CVIC_INT_TIMER7_Msk     (1U << 14)  /* SDK tick timer (reserved) */
#define CVIC_INT_I2C_Msk        (1U << 16)
#define CVIC_INT_GPIO_Msk       (1U << 19)

/* CF → ARM software interrupt: write bit 13 of ARM VIC VICB4 register.
 * ARM physical 0x1E6C00B4; CF LE virtual 0x6C00B4. */
#define VIC_BASE         0x6C0000U
#define VIC_VICB4_OFFSET 0xB4U      /* software interrupt high set */
#define VIC_VICBC_OFFSET 0xBCU      /* software interrupt clear */
#define VIC_COPRO_INT_Msk  (1U << 13)   /* ARM IRQ#45 = bit 13 of VICB4 */

#define VIC_VICB4  (*(volatile uint32_t *)(VIC_BASE + VIC_VICB4_OFFSET))
#define VIC_VICBC  (*(volatile uint32_t *)(VIC_BASE + VIC_VICBC_OFFSET))

#endif /* PAC_CVIC_H */
