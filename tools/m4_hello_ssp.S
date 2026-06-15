.syntax unified
.cpu cortex-m4
.thumb

.section .vectors, "a", %progbits
.word 0x00100000
.word reset + 1

.section .text.reset, "ax", %progbits
.thumb_func
.global reset
reset:
    ldr r0, =0x00020000
    ldr r1, =0x53535021
    str r1, [r0]
    ldr r0, =0x00020004
    ldr r1, =0x48454c4f
    str r1, [r0]
    dsb sy
1:
    ldr r2, =0x14c33b14
2:
    ldr r3, [r2]
    tst r3, #0x20
    beq 2b
    ldr r2, =0x14c33b00
    movs r3, #'S'
    str r3, [r2]
    ldr r2, =0x14c33b14
    b 1b
