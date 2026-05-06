/*
 * Minimal C runtime memory functions.
 *
 * GCC may synthesise calls to memcpy, memset, memcmp, and strlen even with
 * -fno-builtin (which suppresses inline expansion but not call emission for
 * aggregate copies and string literals).  We provide our own implementations
 * so the binary has no dependency on newlib or any hosted libc.
 *
 * These byte-by-byte implementations are correct for all alignment cases on
 * the CFV1 AHB bus.  Word-at-a-time optimisation is a future option if
 * profiling shows these as bottlenecks.
 */

#include <stddef.h>
#include <stdint.h>

void *memcpy(void *dst, const void *src, size_t n)
{
    uint8_t       *d = dst;
    const uint8_t *s = src;
    while (n--) {
        *d++ = *s++;
    }
    return dst;
}

void *memset(void *dst, int c, size_t n)
{
    uint8_t *d = dst;
    while (n--) {
        *d++ = (uint8_t)c;
    }
    return dst;
}

int memcmp(const void *a, const void *b, size_t n)
{
    const uint8_t *p = a;
    const uint8_t *q = b;
    while (n--) {
        if (*p != *q) {
            return (int)*p - (int)*q;
        }
        p++;
        q++;
    }
    return 0;
}

size_t strlen(const char *s)
{
    size_t n = 0;
    while (*s++) {
        n++;
    }
    return n;
}
