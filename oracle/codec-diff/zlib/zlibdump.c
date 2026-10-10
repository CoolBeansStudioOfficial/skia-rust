/*
 * Differential dump of Chromium's zlib deflate (zlib@646b7f56, 1.3.0.1-motley), the reference for
 * crates/skia-rust-zlib/src/deflate.rs. The Rust test crates/skia-rust-zlib/tests/deflate_diff.rs
 * builds the same corpus and the same call sequence, and prints the same text; any difference is
 * a bug in the port.
 *
 * The corpus is generated from a 32-bit LCG (see gen_input), so no data files are needed. Each
 * case compresses one input with one parameter set, in one call pattern, and prints the length and
 * the FNV-1a 64 hash of the compressed bytes. The call patterns are the ones SkPngEncoder causes
 * through libpng: input fed in pieces with Z_NO_FLUSH into an 8192-byte output buffer, then
 * Z_FINISH; and one whole-input call.
 *
 * Build: sh oracle/codec-diff/zlib/build.sh <zlib-checkout> <output-binary>
 * Usage: zlibdump            (prints every case)
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "zlib.h"

static uint32_t rng_state;

static uint32_t rnd(void) {
    rng_state = rng_state * 1103515245u + 12345u;
    return (rng_state >> 16) & 0x7fff;
}

static const char *const kWords[] = {"the", "quick", "brown", "fox", "jumps", "over", "lazy",
                                     "dog", "zlib", "deflate", "png", "hash", "chain", "match",
                                     "literal"};

/* Kinds: 0 text, 1 random bytes, 2 runs, 3 gradient rows (image-like), 4 mixed. */
static unsigned char *gen_input(int kind, size_t n, uint32_t seed) {
    unsigned char *b = malloc(n ? n : 1);
    size_t i = 0;
    rng_state = seed;
    if (kind == 0) {
        while (i < n) {
            const char *w = kWords[rnd() % 15];
            for (const char *p = w; *p && i < n; p++) b[i++] = (unsigned char)*p;
            if (i < n) b[i++] = (rnd() % 10 == 0) ? '\n' : ' ';
        }
    } else if (kind == 1) {
        for (i = 0; i < n; i++) b[i] = (unsigned char)(rnd() & 0xff);
    } else if (kind == 2) {
        while (i < n) {
            unsigned char v = (unsigned char)(rnd() % 4);
            size_t run = 1 + rnd() % 300;
            for (size_t k = 0; k < run && i < n; k++) b[i++] = v;
        }
    } else if (kind == 3) {
        for (i = 0; i < n; i++) {
            size_t x = i % 257, y = i / 257;
            unsigned v = (unsigned)(x + y * 3);
            if (rnd() % 4 == 0) v += 1;
            b[i] = (unsigned char)v;
        }
    } else {
        /* Mixed: segments of text, runs and random bytes of differing sizes. */
        size_t seg = 0;
        int k = 0;
        while (i < n) {
            size_t len = 500 + rnd() % 3000;
            if (len > n - i) len = n - i;
            unsigned char *part = gen_input(k % 3 == 0 ? 0 : (k % 3 == 1 ? 2 : 1), len, seed + (uint32_t)seg);
            memcpy(b + i, part, len);
            free(part);
            i += len;
            seg++;
            k++;
        }
    }
    return b;
}

static uint64_t fnv1a(const unsigned char *p, size_t n) {
    uint64_t h = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < n; i++) {
        h ^= p[i];
        h *= 0x100000001b3ull;
    }
    return h;
}

/* Compresses `in` with the given parameters. `feed` bytes of input per deflate call (Z_NO_FLUSH),
 * `obsz` bytes of output space per call; then Z_FINISH. Returns the compressed bytes. */
static unsigned char *compress_case(const unsigned char *in, size_t n, int level, int wbits,
                                    int mem, int strategy, size_t feed, size_t obsz,
                                    size_t *out_len) {
    z_stream strm;
    memset(&strm, 0, sizeof(strm));
    int ret = deflateInit2(&strm, level, Z_DEFLATED, wbits, mem, strategy);
    if (ret != Z_OK) {
        fprintf(stderr, "deflateInit2 failed: %d\n", ret);
        exit(1);
    }
    size_t cap = 1 << 16, len = 0;
    unsigned char *out = malloc(cap);
    unsigned char *obuf = malloc(obsz);
    size_t pos = 0;
    while (pos < n) {
        size_t take = n - pos < feed ? n - pos : feed;
        strm.next_in = (Bytef *)(in + pos);
        strm.avail_in = (uInt)take;
        pos += take;
        do {
            strm.next_out = obuf;
            strm.avail_out = (uInt)obsz;
            ret = deflate(&strm, Z_NO_FLUSH);
            size_t produced = obsz - strm.avail_out;
            if (len + produced > cap) {
                while (len + produced > cap) cap *= 2;
                out = realloc(out, cap);
            }
            memcpy(out + len, obuf, produced);
            len += produced;
        } while (strm.avail_in != 0 || strm.avail_out == 0);
    }
    for (;;) {
        strm.next_out = obuf;
        strm.avail_out = (uInt)obsz;
        ret = deflate(&strm, Z_FINISH);
        size_t produced = obsz - strm.avail_out;
        if (len + produced > cap) {
            while (len + produced > cap) cap *= 2;
            out = realloc(out, cap);
        }
        memcpy(out + len, obuf, produced);
        len += produced;
        if (ret != Z_OK) break;
    }
    if (ret != Z_STREAM_END) {
        fprintf(stderr, "deflate(Z_FINISH) returned %d\n", ret);
        exit(1);
    }
    deflateEnd(&strm);
    free(obuf);
    *out_len = len;
    return out;
}

static void run_case(int kind, size_t n, uint32_t seed, int level, int wbits, int mem,
                     int strategy, size_t feed, size_t obsz) {
    unsigned char *in = gen_input(kind, n, seed);
    size_t len;
    unsigned char *out = compress_case(in, n, level, wbits, mem, strategy, feed, obsz, &len);
    printf("in=%d:%zu lvl=%d wbits=%d mem=%d strat=%d feed=%zu obuf=%zu len=%zu fnv=%016llx\n",
           kind, n, level, wbits, mem, strategy, feed, obsz, len,
           (unsigned long long)fnv1a(out, len));
    free(out);
    free(in);
}

static const size_t kSizes[] = {0, 1, 3, 300, 4096, 40000, 70000};

int main(void) {
    /* A: every level, default strategy and window, chunked calls (as libpng feeds rows). */
    for (int kind = 0; kind < 5; kind++) {
        for (size_t si = 0; si < sizeof(kSizes) / sizeof(kSizes[0]); si++) {
            for (int level = 0; level <= 9; level++) {
                run_case(kind, kSizes[si], 1000u + (uint32_t)(kind * 17 + si), level, 15, 8, 0,
                         4096, 8192);
            }
        }
    }
    /* B: one whole-input call per case, at level 6 (the default) and 9. */
    for (int kind = 0; kind < 5; kind++) {
        for (size_t si = 0; si < sizeof(kSizes) / sizeof(kSizes[0]); si++) {
            run_case(kind, kSizes[si], 2000u + (uint32_t)(kind * 17 + si), 6, 15, 8, 0,
                     kSizes[si] ? kSizes[si] : 1, 1 << 20);
            run_case(kind, kSizes[si], 2000u + (uint32_t)(kind * 17 + si), 9, 15, 8, 0,
                     kSizes[si] ? kSizes[si] : 1, 1 << 20);
        }
    }
    /* C: strategies, memory levels and window sizes. */
    for (int strategy = 1; strategy <= 4; strategy++) {
        run_case(0, 40000, 3000, 6, 15, 8, strategy, 4096, 8192);
        run_case(3, 40000, 3001, 1, 15, 8, strategy, 4096, 8192);
        run_case(2, 300, 3002, 9, 15, 8, strategy, 4096, 8192);
    }
    run_case(0, 40000, 3100, 6, 15, 1, 0, 4096, 8192);
    run_case(0, 40000, 3100, 6, 15, 9, 0, 4096, 8192);
    run_case(3, 40000, 3101, 4, 15, 5, 0, 4096, 8192);
    run_case(0, 40000, 3200, 6, 9, 8, 0, 4096, 8192);
    run_case(3, 40000, 3201, 6, 12, 8, 0, 4096, 8192);
    run_case(0, 70000, 3202, 9, 9, 8, 0, 4096, 8192);
    /* D: one byte per call (the smallest feed) and a small output buffer. */
    run_case(0, 300, 4000, 6, 15, 8, 0, 1, 8192);
    run_case(3, 300, 4001, 9, 15, 8, 0, 1, 8192);
    run_case(1, 4096, 4002, 1, 15, 8, 0, 4096, 7);
    run_case(0, 40000, 4003, 6, 15, 8, 0, 4096, 1000);
    return 0;
}
