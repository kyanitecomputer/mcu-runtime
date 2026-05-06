/*
 * Default exception handler and CVIC dispatch.
 *
 * All 256 vector table entries in startup.S default to default_isr, except
 * vector 70 (0x46) which routes to cvic_dispatch.
 *
 * CVIC dispatch model:
 *   All CVIC interrupt sources share a single CPU exception vector (70).
 *   cvic_dispatch reads CVIC00 (masked IRQ status) and calls the registered
 *   sub-ISR for each active bit.  Sub-ISRs must:
 *     - Be quick (set a flag and return)
 *     - Clear their own CVIC edge-triggered status if edge-sensitive
 *
 * Adding a new CVIC-sourced ISR:
 *   1. Declare the handler in your HAL header: void my_isr(void);
 *   2. Implement it in your HAL .c file.
 *   3. Register it here by adding an entry to the cvic_handlers table.
 *   4. Enable its CVIC INT bit in your HAL init function.
 *   No changes to startup.S or linker scripts needed.
 */

#include "pac/cvic.h"
#include "sdk_config.h"
#include <stdint.h>

/*
 * default_isr — unhandled exception handler.
 * Masks all interrupts (IPL=7) and halts the processor.
 */
__attribute__((interrupt)) void default_isr(void)
{
    __asm__ volatile("stop #0x2700");
    for (;;) {}
}

/* -------------------------------------------------------------------------
 * CVIC sub-ISR registrations.
 * Add one entry per CVIC interrupt source that needs handling.
 * Unhandled CVIC bits are silently ignored.
 * ------------------------------------------------------------------------- */

/* Forward declarations from HAL modules */
void timer_tick_isr(void);   /* hal/timer.c — tick timer overflow */
void mbox_rx_isr(void);      /* hal/mbox.c  — ARM→CF doorbell */

typedef struct {
    uint32_t mask;          /* CVIC bit mask */
    void   (*handler)(void);
} cvic_entry_t;

static const cvic_entry_t cvic_handlers[] = {
    { CVIC_INT_TIMER7_Msk,  timer_tick_isr },
    { CVIC_INT_ARM_SW_Msk,  mbox_rx_isr   },
};

#define CVIC_HANDLER_COUNT \
    ((uint32_t)(sizeof(cvic_handlers) / sizeof(cvic_handlers[0])))

/*
 * cvic_dispatch — CPU vector 70 (0x46), fired on any enabled CVIC interrupt.
 *
 * Reads CVIC00 (masked status) and calls each registered sub-ISR whose bit
 * is set.  Multiple sources can fire simultaneously; all are serviced.
 */
__attribute__((interrupt)) void cvic_dispatch(void)
{
    uint32_t status = CVIC->irq_status;

    for (uint32_t i = 0; i < CVIC_HANDLER_COUNT; i++) {
        if (status & cvic_handlers[i].mask) {
            cvic_handlers[i].handler();
        }
    }
}
