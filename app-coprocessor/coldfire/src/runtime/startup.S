/*
 * ColdFire V1 startup — vector table, firmware header, _start
 *
 * Memory layout (matches linker scripts):
 *   0x000000  .vectors    — 256 × 4-byte exception vectors (1024 bytes)
 *   0x000400  .fw_header  — 256-byte firmware header (cf-fsi HDR_* compatible)
 *   0x000500  _start      — entry point
 *
 * The CF hardware loads vector[0] into SSP and jumps to vector[1] (_start)
 * on reset.  All other vectors default to default_isr.  Vector 70 (0x46)
 * routes to cvic_dispatch, which reads CVIC00 and calls the sub-ISRs.
 *
 * Firmware header signature 0x4153 ("AS") distinguishes SDK firmware from
 * cf-fsi firmware (0x5348/"SH" or 0x5350/"SP").
 */

    .cpu    cfv1

/* -------------------------------------------------------------------------
 * Exception vector table — section .vectors, placed at 0x000000 by linker
 * ------------------------------------------------------------------------- */
    .section .vectors,"a"
    .long   _stack_top          /* 0:    Initial SSP (top of SRAM) */
    .long   _start              /* 1:    Initial PC */
    .rept   68                  /* 2–69: default handler */
    .long   default_isr
    .endr
    .long   cvic_dispatch       /* 70:   CVIC interrupt vector (0x46) */
    .rept   185                 /* 71–255: default handler */
    .long   default_isr
    .endr


/* -------------------------------------------------------------------------
 * Firmware header — section .fw_header, placed at 0x000400 by linker.
 *
 * Offsets are compatible with cf-fsi HDR_* definitions so that ARM-side
 * loaders can read the signature without special-casing SDK firmware.
 * Total size: exactly 256 bytes (padded with zeros).
 * ------------------------------------------------------------------------- */
    .section .fw_header,"a"
    .word   0x4153              /* +0x00  FW_SIG:        "AS" (Aspeed SDK) */
    .word   0x0100              /* +0x02  FW_VERSION:    1.0 */
    .word   0x0100              /* +0x04  API_VERSION:   1.0 */
    .word   0x0000              /* +0x06  reserved */
    .long   0x00000000          /* +0x08  FW_OPTIONS */
    .long   0x00000000          /* +0x0C  reserved */
    .long   0x00000000          /* +0x10  FW_SIZE (filled in post-build if needed) */
    .space  0xEC                /* +0x14  pad to 256 bytes */


/* -------------------------------------------------------------------------
 * _start — entry point, section .text.startup
 *
 * The hardware has already loaded SSP from vector[0] (_stack_top) before
 * jumping here.  We therefore begin with the C environment setup:
 *   1. Zero .bss
 *   2. Copy .data from LMA (DRAM) to VMA (SRAM)
 *   3. Call sdk_init()
 *   4. Call main()
 *   5. Hang (main must never return)
 *
 * Addressing notes:
 *   - _bss_start, _bss_end are VMA addresses in the SRAM (DATA) region.
 *   - _data_load is the LMA of .data, located in the DRAM (CODE) region.
 *   - _data_start, _data_end are VMA addresses in SRAM.
 *   All are absolute, resolved by the linker script.
 * ------------------------------------------------------------------------- */
    .section .text.startup,"ax"
    .global _start
_start:
    /* ---- Zero .bss ---- */
    movea.l #_bss_start, %a0       /* a0 = first byte to zero */
    movea.l #_bss_end,   %a1       /* a1 = one-past-last byte */
    moveq   #0, %d0
.Lbss_loop:
    cmpa.l  %a1, %a0               /* a0 >= a1 → done */
    bge     .Lbss_done
    move.l  %d0, (%a0)+            /* zero 4 bytes, advance a0 */
    bra     .Lbss_loop
.Lbss_done:

    /* ---- Copy .data from LMA to VMA ---- */
    movea.l #_data_load,  %a0      /* a0 = source (LMA in DRAM) */
    movea.l #_data_start, %a1      /* a1 = destination (VMA in SRAM) */
    movea.l #_data_end,   %a2      /* a2 = end of destination */
.Ldata_loop:
    cmpa.l  %a2, %a1               /* a1 >= a2 → done */
    bge     .Ldata_done
    move.l  (%a0)+, (%a1)+         /* copy 4 bytes */
    bra     .Ldata_loop
.Ldata_done:

    /* ---- SDK and application entry ---- */
    jsr     sdk_init               /* initialise hardware (CVIC, timer, ...) */
    jsr     main                   /* run user application */

    /* main() must never return; hang if it does */
.Lhang:
    stop    #0x2700                /* IPL=7, all interrupts masked, halt */
    bra     .Lhang
