/*
 * Differential dump of libpng's write path (libpng 1.6.56, skia.googlesource.com/third_party/
 * libpng@d5515b5b, with scripts/pnglibconf.h.prebuilt), the reference for
 * crates/skia-rust-libpng/src/{write,wutil,wtran}.rs. The Rust test
 * crates/skia-rust-libpng/tests/write_diff.rs builds the same corpus and makes the same calls, and
 * prints the same text; any difference is a bug in the port.
 *
 * The calls are the ones SkPngEncoderImpl makes (setHeader, setColorSpace, setHdrMetadata,
 * writeInfo, onEncodeRow per row, onFinishEncoding), with the zlib level and filter flags of
 * SkPngEncoder::Options. The corpus is generated from a 32-bit LCG (see gen_pixels), so no data
 * files are needed. Each case prints the length and the FNV-1a 64 hash of the PNG bytes.
 *
 * Build: sh oracle/codec-diff/libpng/build.sh <libpng-checkout> <zlib-checkout> <output-binary>
 * Usage: pngwdump            (prints every case)
 */

#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "png.h"

static uint32_t rng_state;

static uint32_t rnd(void) {
    rng_state = rng_state * 1103515245u + 12345u;
    return (rng_state >> 16) & 0x7fff;
}

/* A gradient with noise, the shape of decoded photos. The index i is the byte index. */
static unsigned char *gen_pixels(size_t n, uint32_t seed) {
    unsigned char *b = malloc(n ? n : 1);
    rng_state = seed;
    for (size_t i = 0; i < n; i++) {
        uint32_t v = (uint32_t)(i * 7u);
        if (rnd() % 4 == 0) v += rnd();
        b[i] = (unsigned char)v;
    }
    return b;
}

/* A 132-byte ICC header-sized profile: the size, the version, and a fixed tail. */
static unsigned char *gen_profile(size_t *len) {
    const size_t n = 132;
    unsigned char *p = calloc(n, 1);
    p[3] = 132;
    p[8] = 2;
    p[12] = 'm';
    p[13] = 'n';
    p[14] = 't';
    p[15] = 'r';
    for (size_t i = 128; i < n; i++) {
        p[i] = (unsigned char)(i * 3);
    }
    *len = n;
    return p;
}

/* png.c refers to the read-side teardown (png_destroy_read_struct, in the simplified-API error
 * path). The harness links the write sources only, so these stubs satisfy the references. The
 * write path never reaches them. */
void png_destroy_read_struct(png_structpp png_ptr_ptr, png_infopp info_ptr_ptr,
                             png_infopp end_info_ptr_ptr) {
    (void)png_ptr_ptr;
    (void)info_ptr_ptr;
    (void)end_info_ptr_ptr;
}

/* png.c's read teardown calls inflateReset; the inflate sources are not built (see build.sh). */
int inflateReset(void *strm) {
    (void)strm;
    return 0;
}

typedef struct {
    unsigned char *buf;
    size_t len;
    size_t cap;
} Out;

static void out_write(png_structp png, png_bytep data, png_size_t len) {
    Out *o = (Out *)png_get_io_ptr(png);
    if (o->len + len > o->cap) {
        o->cap = (o->len + len) * 2 + 1024;
        o->buf = realloc(o->buf, o->cap);
    }
    memcpy(o->buf + o->len, data, len);
    o->len += len;
}

static void error_fn(png_structp png, png_const_charp msg) {
    (void)msg;
    longjmp(png_jmpbuf(png), 1);
}

static void warning_fn(png_structp png, png_const_charp msg) {
    (void)png;
    (void)msg;
}

static uint64_t fnv1a(const unsigned char *p, size_t n) {
    uint64_t h = 0xcbf29ce484222325ULL;
    for (size_t i = 0; i < n; i++) {
        h ^= p[i];
        h *= 0x100000001b3ULL;
    }
    return h;
}

/* Source formats: the PNG colour type and depth Skia picks, the channels of the source rows, and
 * whether the source has a filler channel after RGB (Skia strips it on write). */
typedef struct {
    const char *name;
    int color_type;
    int bit_depth;
    int src_channels;
    int filler_after;
} Format;

static const Format kFormats[] = {
    {"rgba8", 6, 8, 4, 0},
    {"rgb8x", 2, 8, 4, 1},
    {"gray8", 0, 8, 1, 0},
    {"graya8", 4, 8, 2, 0},
    {"rgba16", 6, 16, 8, 0},
    {"rgb16x", 2, 16, 8, 1},
};

static const int kSizes[][2] = {{1, 1},   {1, 7},   {9, 1},   {3, 3},
                                {17, 13}, {64, 64}, {200, 150}, {300, 120}};

static const int kFilters[] = {0x00, 0x08, 0x10, 0x20, 0x40, 0x80, 0xf8};

static const int kLevels[] = {-1, 1, 9};

/* Encodes one image. Returns 0 on success, 1 on a libpng error. */
static int encode(const Format *f, int w, int h, int filters, int level, int extras, Out *out) {
    /* sRGB for even widths, iCCP for odd ones, when the extras are on. */
    int use_icc = (w % 2 == 0) ? 0 : 1;
    size_t src_bpp = (size_t)f->src_channels * (size_t)(f->bit_depth / 8);
    size_t rowbytes_src = (size_t)w * src_bpp;
    unsigned char *pixels =
        gen_pixels(rowbytes_src * (size_t)h, (uint32_t)(w * 131 + h * 17 + filters));

    png_structp png = png_create_write_struct(PNG_LIBPNG_VER_STRING, NULL, error_fn, warning_fn);
    png_infop info = png_create_info_struct(png);
    int failed = 0;
    if (setjmp(png_jmpbuf(png))) {
        failed = 1;
        goto done;
    }
    png_set_write_fn(png, out, out_write, NULL);

    /* setHeader */
    {
        png_color_8 sig;
        memset(&sig, 0, sizeof sig);
        int bits = f->bit_depth;
        if (f->color_type == 0 || f->color_type == 4) {
            sig.gray = (png_byte)bits;
            if (f->color_type == 4) sig.alpha = (png_byte)bits;
        } else {
            sig.red = sig.green = sig.blue = (png_byte)bits;
            if (f->color_type == 6) sig.alpha = (png_byte)bits;
        }
        png_set_IHDR(png, info, (png_uint_32)w, (png_uint_32)h, f->bit_depth, f->color_type,
                     PNG_INTERLACE_NONE, PNG_COMPRESSION_TYPE_BASE, PNG_FILTER_TYPE_BASE);
        png_set_sBIT(png, info, &sig);
        png_set_filter(png, PNG_FILTER_TYPE_BASE, filters);
        png_set_compression_level(png, level);
        if (extras) {
            png_text texts[1];
            memset(texts, 0, sizeof texts);
            texts[0].compression = PNG_TEXT_COMPRESSION_NONE;
            texts[0].key = (png_charp)"Comment";
            texts[0].text = (png_charp)"skia rust";
            png_set_text(png, info, texts, 1);
        }
    }

    /* setColorSpace */
    if (extras) {
        if (!use_icc) {
            png_set_sRGB(png, info, PNG_sRGB_INTENT_PERCEPTUAL);
        } else {
            size_t plen = 0;
            unsigned char *profile = gen_profile(&plen);
            png_set_iCCP(png, info, "Skia", 0, profile, plen);
            free(profile);
        }
    }

    /* setHdrMetadata: the keep list, and one unknown chunk at the IHDR location. */
    if (extras) {
        static const char kNames[] = "gmAP\0gdAT\0mDCV\0cLLI\0";
        png_set_keep_unknown_chunks(png, PNG_HANDLE_CHUNK_ALWAYS, (png_const_bytep)kNames, 4);
        png_unknown_chunk chunk;
        memset(&chunk, 0, sizeof chunk);
        memcpy(chunk.name, "gmAP", 4);
        chunk.data = (png_byte *)"\0\0\0\1";
        chunk.size = 4;
        chunk.location = PNG_HAVE_IHDR;
        png_set_unknown_chunks(png, info, &chunk, 1);
    }

    /* writeInfo */
    png_write_info(png, info);
    if (f->filler_after) {
        png_set_filler(png, 0, PNG_FILLER_AFTER);
    }

    /* onEncodeRow */
    for (int y = 0; y < h; y++) {
        png_bytep row = pixels + (size_t)y * rowbytes_src;
        if (png_get_bit_depth(png, info) == 16) {
            png_set_swap(png);
        }
        png_write_rows(png, &row, 1);
    }

    /* onFinishEncoding */
    png_write_end(png, info);

done:
    png_destroy_write_struct(&png, &info);
    free(pixels);
    return failed;
}

int main(void) {
    int index = 0;
    size_t nf = sizeof kFormats / sizeof kFormats[0];
    size_t ns = sizeof kSizes / sizeof kSizes[0];
    size_t nfl = sizeof kFilters / sizeof kFilters[0];
    size_t nl = sizeof kLevels / sizeof kLevels[0];
    for (size_t fi = 0; fi < nf; fi++) {
        for (size_t si = 0; si < ns; si++) {
            for (size_t fl = 0; fl < nfl; fl++) {
                for (size_t li = 0; li < nl; li++) {
                    const Format *f = &kFormats[fi];
                    int w = kSizes[si][0], h = kSizes[si][1];
                    int filters = kFilters[fl], level = kLevels[li];
                    int extras = (level == -1) ? 1 : 0;
                    Out out = {NULL, 0, 0};
                    int failed = encode(f, w, h, filters, level, extras, &out);
                    if (failed) {
                        printf("case=%d fmt=%s w=%d h=%d filter=%02x level=%d extras=%d "
                               "status=err\n",
                               index, f->name, w, h, filters, level, extras);
                    } else {
                        printf("case=%d fmt=%s w=%d h=%d filter=%02x level=%d extras=%d "
                               "len=%zu fnv=%016llx\n",
                               index, f->name, w, h, filters, level, extras, out.len,
                               (unsigned long long)fnv1a(out.buf, out.len));
                    }
                    free(out.buf);
                    index++;
                }
            }
        }
    }
    return 0;
}
