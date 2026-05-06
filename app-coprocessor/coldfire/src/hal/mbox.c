/*
 * Mailbox HAL — SPSC ring buffers over shared SRAM with software interrupt
 * doorbell signalling.
 *
 * Ring invariants (power-of-2 size N):
 *   full  when (head - tail) == N      (unsigned arithmetic)
 *   empty when head == tail
 *   slot  index via head & (N-1) or tail & (N-1)
 *
 * ARM→CF signal: CVIC INT#1 ("ARM interrupt"), raised by ARM writing
 *   bit 1 to CVIC18 (CF address 0x6C2018).
 *   ISR clears via CVIC1C; sets s_mbox_rx_wake so mbox_recv's await resumes.
 *
 * CF→ARM signal: write bit 13 of VIC VICB4 (CF LE address 0x6C00B4).
 *   This triggers ARM IRQ#45 ("Coprocessor interrupt").
 *   The ARM Linux driver (separate project) clears it via VICBC.
 */

#include "hal/mbox.h"
#include "pac/cvic.h"
#include "sdk_config.h"
#include <stdint.h>
#include <stddef.h>

/* AST2400 dummy-read errata: after every CVIC/VIC register write from the CF,
 * read any SCU register to flush the write to the AHB bus. */
#ifdef SDK_PLATFORM_AST2400
#  define DUMMY_READ()  do { (void)(*(volatile uint32_t *)0x6E2000U); } while (0)
#else
#  define DUMMY_READ()  ((void)0)
#endif

/* Wake flag: set by mbox_rx_isr, cleared by mbox_recv */
static volatile uint8_t s_mbox_rx_wake;

/* Rolling sequence counter for outbound messages */
static uint8_t s_tx_seq;

/* -------------------------------------------------------------------------
 * Ring helpers
 * ------------------------------------------------------------------------- */

static inline uint32_t ring_count(const mbox_ring_t *r)
{
    return r->head - r->tail;               /* unsigned subtraction wraps correctly */
}

static inline int ring_full(const mbox_ring_t *r)
{
    return ring_count(r) >= MBOX_RING_SIZE;
}

static inline int ring_empty(const mbox_ring_t *r)
{
    return ring_count(r) == 0U;
}

/* -------------------------------------------------------------------------
 * mbox_init — enable CVIC INT#1 (ARM→CF doorbell)
 * ------------------------------------------------------------------------- */
void mbox_init(void)
{
    /* Pre-clear any stale ARM interrupt before enabling */
    CVIC->sw_clear   = CVIC_INT_ARM_SW_Msk;
    DUMMY_READ();
    /* Enable CVIC INT#1 */
    CVIC->irq_enable = CVIC_INT_ARM_SW_Msk;
    DUMMY_READ();
}

/* -------------------------------------------------------------------------
 * mbox_rx_isr — ARM→CF doorbell; runs in interrupt context
 * ------------------------------------------------------------------------- */
void mbox_rx_isr(void)
{
    /* Clear the CVIC software interrupt so it can fire again */
    CVIC->sw_clear = CVIC_INT_ARM_SW_Msk;
    DUMMY_READ();
    s_mbox_rx_wake = 1;
}

/* -------------------------------------------------------------------------
 * mbox_send — async send; yields while CF→ARM ring is full
 * ------------------------------------------------------------------------- */
async mbox_send(mbox_send_t *ctx, const msg_t *msg)
{
    async_begin(ctx);

    ctx->msg = msg;
    ctx->seq = s_tx_seq++;

    /* Yield while ring is full */
    await(!ring_full(MBOX_CF2ARM));

    /* Write message into the next slot */
    {
        uint32_t idx = MBOX_CF2ARM->head & (MBOX_RING_SIZE - 1U);
        MBOX_CF2ARM->slots[idx] = *ctx->msg;
        MBOX_CF2ARM->slots[idx].seq = ctx->seq;
        /* Publish: advance head after writing slot (barrier via volatile) */
        MBOX_CF2ARM->head = MBOX_CF2ARM->head + 1U;
    }

    /* Doorbell: trigger ARM IRQ#45 via VIC VICB4 bit 13 */
    VIC_VICB4 = VIC_COPRO_INT_Msk;
    DUMMY_READ();

    async_end;
}

/* -------------------------------------------------------------------------
 * mbox_recv — async receive; yields while ARM→CF ring is empty
 * ------------------------------------------------------------------------- */
async mbox_recv(mbox_recv_t *ctx)
{
    async_begin(ctx);

    /* Yield while ring is empty (woken by mbox_rx_isr) */
    s_mbox_rx_wake = 0;
    await(!ring_empty(MBOX_ARM2CF) || s_mbox_rx_wake);
    s_mbox_rx_wake = 0;

    /* Re-check; ISR may have fired spuriously */
    if (ring_empty(MBOX_ARM2CF)) {
        async_exit;
    }

    /* Copy message out of the ring slot */
    {
        uint32_t idx = MBOX_ARM2CF->tail & (MBOX_RING_SIZE - 1U);
        ctx->buf = MBOX_ARM2CF->slots[idx];
        /* Consume: advance tail after copying */
        MBOX_ARM2CF->tail = MBOX_ARM2CF->tail + 1U;
    }

    async_end;
}

/* -------------------------------------------------------------------------
 * mbox_printf — minimal debug string to ARM (type=0x00)
 *
 * Formats into a fixed stack buffer and sends synchronously (busy-wait fill,
 * then signals without async context).  Intended for early debug only.
 * Production code should use mbox_send with a proper context struct.
 * ------------------------------------------------------------------------- */
void mbox_printf(const char *fmt, ...)
{
    /*
     * Minimal implementation without stdio: copy fmt string directly.
     * Full printf-style formatting requires newlib-nano opt-in (sdk_config.h).
     * For now: send the format string as-is, truncated to MBOX_MAX_PAYLOAD-1.
     */
    msg_t m;
    uint8_t i = 0;
    m.type = 0x00;
    m.seq  = s_tx_seq++;
    while (fmt[i] && i < (MBOX_MAX_PAYLOAD - 1U)) {
        m.payload[i] = (uint8_t)fmt[i];
        i++;
    }
    m.payload[i] = 0;
    m.len = i;

    /* Busy-wait until CF→ARM ring has space */
    while (ring_full(MBOX_CF2ARM)) {}

    uint32_t idx = MBOX_CF2ARM->head & (MBOX_RING_SIZE - 1U);
    MBOX_CF2ARM->slots[idx] = m;
    MBOX_CF2ARM->head = MBOX_CF2ARM->head + 1U;

    VIC_VICB4 = VIC_COPRO_INT_Msk;
    DUMMY_READ();

    (void)fmt;  /* suppress unused warning if va_args not compiled */
}
