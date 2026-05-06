/*
 * ColdFire V1 Linker Script — AST2500
 *
 * Code region:  CF segment 0 (0x000000–0x0FFFFF), 1 MB
 *               ARM loader programs SCU104 to point to reserved DRAM
 *               (typically at ARM physical 0x9EF00000) with swap bit 0 = 1.
 *
 * Data region:  CF segment 2 (0x200000–0x2FFFFF) → ARM 0x1E600000 with BE swap.
 *               SRAM at ARM 0x1E720000 appears at CF 0x320000.
 *               36 KB total on AST2500.
 *
 * ARM loader requirements:
 *   SCU104 = <dram_base> | 0x00000001   (code, BE swap via bit 0)
 *   SCU10C = 0x1E600000  | 0x00000001   (seg 2 → sysregs, BE swap, default)
 *   SCU11C = 0x1E600000                 (seg 6 → sysregs, LE, no swap, default)
 *   SCU110 = 0x1E700000  | 0x00000001   (seg 3 → periph, BE swap — for data)
 *   SCU120 = 0x1E700000                 (seg 7 → periph, LE — for registers)
 *
 * Stack: grows downward from top of DATA region (_stack_top).
 */

ENTRY(_start)

MEMORY
{
    CODE (rx)  : ORIGIN = 0x000000, LENGTH = 0x100000   /* 1 MB DRAM (code) */
    DATA (rwx) : ORIGIN = 0x320000, LENGTH = 0x9000     /* 36 KB SRAM (BE segment) */
}

SECTIONS
{
    /*
     * Exception vector table — must start at absolute address 0x000000.
     * 256 entries × 4 bytes = 1024 bytes = 0x400 bytes.
     */
    . = 0x000000;
    .vectors :
    {
        KEEP(*(.vectors))
    } > CODE

    /*
     * Firmware header — must start at absolute address 0x000400.
     * 256 bytes.  Layout is compatible with cf-fsi HDR_* offsets so that
     * existing ARM-side loaders can read the signature field.
     */
    . = 0x000400;
    .fw_header :
    {
        KEEP(*(.fw_header))
    } > CODE

    /*
     * Executable code — starts at 0x000500.
     */
    . = 0x000500;
    .text :
    {
        *(.text.startup .text.startup.*)
        *(.text .text.*)
        *(.gnu.linkonce.t.*)
    } > CODE

    .rodata :
    {
        *(.rodata .rodata.*)
        *(.gnu.linkonce.r.*)
        . = ALIGN(4);
    } > CODE

    /*
     * Initialised data: LMA in CODE (loaded from DRAM by ARM),
     * VMA in DATA (SRAM, accessed by CF at runtime).
     * startup.S copies from _data_load → _data_start .. _data_end.
     */
    _data_load = LOADADDR(.data);
    .data :
    {
        _data_start = .;
        *(.data .data.*)
        *(.gnu.linkonce.d.*)
        . = ALIGN(4);
        _data_end = .;
    } > DATA AT > CODE

    /*
     * Zero-initialised data.  startup.S zeroes _bss_start .. _bss_end.
     */
    .bss (NOLOAD) :
    {
        _bss_start = .;
        *(.bss .bss.*)
        *(.gnu.linkonce.b.*)
        *(COMMON)
        . = ALIGN(4);
        _bss_end = .;
    } > DATA

    /*
     * Stack grows downward from the top of the DATA (SRAM) region.
     * This symbol is placed in vector 0 by startup.S as the initial SSP.
     */
    _stack_top = ORIGIN(DATA) + LENGTH(DATA);

    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}
