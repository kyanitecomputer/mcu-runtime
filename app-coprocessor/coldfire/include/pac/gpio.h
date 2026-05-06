/*
 * Aspeed AST2400 / AST2500 GPIO Controller — Stub PAC Header
 *
 * GENERATED FILE — do not edit by hand.
 * Replace with chiptool-generated output once the C backend is complete.
 *
 * Physical base: 0x1E780000
 * CF LE virtual: 0x780000  (segment 7 on AST2500, segment 3 on AST2400)
 *
 * Only the registers needed for the M1 blinky template are defined here.
 * Expand as additional peripherals are needed.
 */

#pragma once
#ifndef PAC_GPIO_H
#define PAC_GPIO_H

#include <stdint.h>

/* Base address (LE segment — use for register read/write) */
#define GPIO_BASE  0x780000U

typedef struct {
    volatile uint32_t abcd_data;    /* 0x000: Port A/B/C/D data */
    volatile uint32_t abcd_dir;     /* 0x004: Port A/B/C/D direction (1=output) */
    volatile uint32_t abcd_irqen;   /* 0x008: IRQ enable */
    volatile uint32_t abcd_irqsen;  /* 0x00C: IRQ sensitivity */
    volatile uint32_t abcd_irqboth; /* 0x010: Both-edge trigger */
    volatile uint32_t abcd_irqevt;  /* 0x014: IRQ event (hi/lo) */
    volatile uint32_t abcd_irqstat; /* 0x018: IRQ status (w1c) */
    volatile uint32_t abcd_rst;     /* 0x01C: Reset tolerance */
    volatile uint32_t efgh_data;    /* 0x020: Port E/F/G/H data */
    volatile uint32_t efgh_dir;     /* 0x024 */
    volatile uint32_t efgh_irqen;   /* 0x028 */
    volatile uint32_t efgh_irqsen;  /* 0x02C */
    volatile uint32_t efgh_irqboth; /* 0x030 */
    volatile uint32_t efgh_irqevt;  /* 0x034 */
    volatile uint32_t efgh_irqstat; /* 0x038 */
    volatile uint32_t efgh_rst;     /* 0x03C */
} GPIO_TypeDef;

#define GPIO  ((GPIO_TypeDef *)GPIO_BASE)

/* Port A — bits [7:0] of abcd_* registers */
#define GPIO_PA_PIN0_Pos   0U
#define GPIO_PA_PIN0_Msk   (1U << GPIO_PA_PIN0_Pos)
#define GPIO_PA_PIN1_Pos   1U
#define GPIO_PA_PIN1_Msk   (1U << GPIO_PA_PIN1_Pos)
#define GPIO_PA_PIN2_Pos   2U
#define GPIO_PA_PIN2_Msk   (1U << GPIO_PA_PIN2_Pos)
#define GPIO_PA_PIN3_Pos   3U
#define GPIO_PA_PIN3_Msk   (1U << GPIO_PA_PIN3_Pos)
#define GPIO_PA_PIN4_Pos   4U
#define GPIO_PA_PIN4_Msk   (1U << GPIO_PA_PIN4_Pos)
#define GPIO_PA_PIN5_Pos   5U
#define GPIO_PA_PIN5_Msk   (1U << GPIO_PA_PIN5_Pos)
#define GPIO_PA_PIN6_Pos   6U
#define GPIO_PA_PIN6_Msk   (1U << GPIO_PA_PIN6_Pos)
#define GPIO_PA_PIN7_Pos   7U
#define GPIO_PA_PIN7_Msk   (1U << GPIO_PA_PIN7_Pos)

/* Port B — bits [15:8] of abcd_* registers */
#define GPIO_PB_PIN0_Pos   8U
#define GPIO_PB_PIN0_Msk   (1U << GPIO_PB_PIN0_Pos)

/* GPIO command source register (I2CG08-style, in GPIO global) — TBD */

#endif /* PAC_GPIO_H */
