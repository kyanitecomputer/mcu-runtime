/* Linker script for AST2700 CA35 bare-metal.
 *
 * Linked at the CA35 physical address for ATF_LOAD_ADDR:
 *   BootMCU view: 0xB0000000
 *   CA35 view:    (0xB0000000 - 0x80000000) | 0x400000000 = 0x430000000
 *
 * The BootMCU copies the binary to DRAM at 0xB0000000 (its bus view),
 * then sets RVBAR = 0x430000000 >> 4.  CA35 begins execution at 0x430000000.
 */

ENTRY(_start)

MEMORY {
    DRAM (rwx) : ORIGIN = 0x430000000, LENGTH = 4M
}

SECTIONS {
    . = 0x430000000;

    .text : {
        KEEP(*(.text.start))   /* Reset vector must be first */
        *(.text*)
        *(.rodata*)
    } > DRAM

    .data : {
        *(.data*)
    } > DRAM

    .bss : {
        __bss_start = .;
        *(.bss*)
        *(COMMON)
        __bss_end = .;
    } > DRAM

    /DISCARD/ : {
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}
