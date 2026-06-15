ENTRY(reset)

SECTIONS {
    . = 0;
    .vectors : {
        KEEP(*(.vectors))
    }
    .text : {
        *(.text*)
        *(.rodata*)
    }
    .data : {
        *(.data*)
    }
    .bss : {
        *(.bss*)
        *(COMMON)
    }
}
