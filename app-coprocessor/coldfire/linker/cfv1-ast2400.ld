/*
 * ColdFire V1 Linker Script — AST2400
 *
 * Code region:  CF segment 0 (0x000000–0x1FFFFF), 2 MB
 *               ARM loader programs SCU104 with BE swap (bit 31 = 1).
 *
 * Data region:  CF segment 1 (0x200000–0x3FFFFF) programmed to ARM 0x1E600000
 *               with BE swap (bit 31 = 1).  SRAM at ARM 0x1E720000 then
 *               appears at CF 0x320000  (0x200000 + 0x120000).
 *               32 KB total on AST2400.
 *
 * ARM loader requirements (differs from AST2500 — swap bit is bit 31):
 *   SCU104 = <dram_base> | 0x80000000   (code, BE swap)
 *   SCU108 = 0x1E600000  | 0x80000000   (seg 1 → sysregs+SRAM, BE swap)
 *   SCU110 = 0x1E600000                 (seg 3 → sysregs, LE, no swap)
 *
 * Dummy-read errata: after every 32-bit register write from the CF on
 * AST2400, an additional read of any register in the same segment is required
 * to flush the write to the bus.  HAL drivers gate this via
 * #ifdef SDK_PLATFORM_AST2400.
 *
 * Stack: grows downward from top of DATA region (_stack_top = 0x328000).
 */

ENTRY(_start)

MEMORY
{
    CODE (rx)  : ORIGIN = 0x000000, LENGTH = 0x200000   /* 2 MB DRAM (code) */
    DATA (rwx) : ORIGIN = 0x320000, LENGTH = 0x8000     /* 32 KB SRAM (BE segment) */
}

SECTIONS
{
    . = 0x000000;
    .vectors :
    {
        KEEP(*(.vectors))
    } > CODE

    . = 0x000400;
    .fw_header :
    {
        KEEP(*(.fw_header))
    } > CODE

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

    _data_load = LOADADDR(.data);
    .data :
    {
        _data_start = .;
        *(.data .data.*)
        *(.gnu.linkonce.d.*)
        . = ALIGN(4);
        _data_end = .;
    } > DATA AT > CODE

    .bss (NOLOAD) :
    {
        _bss_start = .;
        *(.bss .bss.*)
        *(.gnu.linkonce.b.*)
        *(COMMON)
        . = ALIGN(4);
        _bss_end = .;
    } > DATA

    _stack_top = ORIGIN(DATA) + LENGTH(DATA);

    /DISCARD/ :
    {
        *(.ARM.exidx*)
        *(.comment)
        *(.note*)
        *(.eh_frame*)
    }
}
