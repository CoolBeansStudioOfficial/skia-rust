/*
 * Differential dump of the libpng 1.6.56 read path, driven the way SkPngCodec drives it
 * (third_party/skia/src/codec/SkPngCodec.cpp). The Rust port in
 * crates/skia-rust-libpng/examples/pngdump.rs prints the same text; any difference is a bug in
 * the port.
 *
 * Per file and per feed size N (1, 7 and 4096 bytes per png_process_data call):
 *   - the header (every byte before the first IDAT) is fed, as decodeBounds does;
 *   - SkPngCodec::infoCallback's transform choices are set (strip_16, packing, tRNS_to_alpha,
 *     expand_gray_1_2_4_to_8, interlace_handling);
 *   - png_read_update_info, then the info and rowbytes are printed;
 *   - the rest is fed with png_process_data; each row goes to the row callback, which prints a
 *     hash (FNV-1a 64) of the row, or, for an interlaced image, combines it into a zeroed buffer
 *     with png_progressive_combine_row, as SkPngInterlacedDecoder does.
 *
 * Build (Linux, clang, the pinned libpng sources in third_party/codec-externals/libpng):
 *   clang -O2 -ffp-contract=off -DPNG_SET_OPTION_SUPPORTED=1 -I<pnglibconf dir> -I<libpng src>
 *         -o pngdump pngdump.c <libpng src>/png.c ...(the read files: pngerror pngget pngmem
 *         pngpread pngread pngrio pngrtran pngrutil pngset pngtrans png.c) -lz
 *   where <pnglibconf dir> holds scripts/pnglibconf.h.prebuilt copied to pnglibconf.h with
 *   `#undef PNG_READ_OPT_PLTE_SUPPORTED` removed (as Skia's third_party/libpng/pnglibconf.h does).
 *
 * Usage: pngdump FILE...   (prints the dump for each file)
 */

#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "png.h"

typedef struct {
  jmp_buf jb;
  png_structp png;
  png_infop info;
  int interlaced;
  int height;
  size_t rowbytes;
  unsigned char* buf;
  int rows;
  uint64_t hash;
} Ctx;

static uint64_t fnv_update(uint64_t h, const unsigned char* p, size_t n) {
  for (size_t i = 0; i < n; i++) {
    h ^= p[i];
    h *= 1099511628211ULL;
  }
  return h;
}

static uint64_t fnv_init(void) { return 1469598103934665603ULL; }

static void error_fn(png_structp png, png_const_charp msg) {
  /* Messages go to stderr, which the diff does not compare. */
  fprintf(stderr, "error: %s\n", msg ? msg : "(null)");
  Ctx* c = (Ctx*)png_get_error_ptr(png);
  longjmp(c->jb, 1);
}

static void warning_fn(png_structp png, png_const_charp msg) {
  (void)png;
  (void)msg;
}

static void row_fn(png_structp png, png_bytep row, png_uint_32 row_num, int pass) {
  Ctx* c = (Ctx*)png_get_progressive_ptr(png);
  if (c->interlaced) {
    if (row_num < 0 || (int)row_num >= c->height) {
      return;
    }
    png_bytep old_row = c->buf + row_num * c->rowbytes;
    png_progressive_combine_row(png, old_row, row);
    (void)pass;
    return;
  }
  printf("row %u %d %016llx\n", (unsigned)row_num, pass,
         (unsigned long long)fnv_update(fnv_init(), row, c->rowbytes));
  c->rows++;
}

/* Port of SkPngCodec's AutoCleanPng::infoCallback transform choices. Returns the pass count. */
static int setup_transforms(png_structp png, png_infop info) {
  png_uint_32 w = 0, h = 0;
  int bd = 0, ct = 0, il = 0, cm = 0, fm = 0;
  png_get_IHDR(png, info, &w, &h, &bd, &ct, &il, &cm, &fm);
  if (bd == 16 && (ct == PNG_COLOR_TYPE_GRAY || ct == PNG_COLOR_TYPE_GRAY_ALPHA)) {
    bd = 8;
    png_set_strip_16(png);
  }
  switch (ct) {
    case PNG_COLOR_TYPE_PALETTE:
      if (bd < 8) {
        bd = 8;
        png_set_packing(png);
      }
      break;
    case PNG_COLOR_TYPE_RGB:
      if (png_get_valid(png, info, PNG_INFO_tRNS)) {
        png_set_tRNS_to_alpha(png);
      }
      break;
    case PNG_COLOR_TYPE_GRAY:
      if (bd < 8) {
        bd = 8;
        png_set_expand_gray_1_2_4_to_8(png);
      }
      if (png_get_valid(png, info, PNG_INFO_tRNS)) {
        png_set_tRNS_to_alpha(png);
      }
      break;
    default:
      break;
  }
  return png_set_interlace_handling(png);
}

/* Returns the byte offset of the first IDAT chunk, or -1 if there is none. */
static long first_idat(const unsigned char* data, size_t len) {
  size_t pos = 8;
  while (pos + 8 <= len) {
    uint32_t l = ((uint32_t)data[pos] << 24) | ((uint32_t)data[pos + 1] << 16) |
                 ((uint32_t)data[pos + 2] << 8) | (uint32_t)data[pos + 3];
    if (memcmp(data + pos + 4, "IDAT", 4) == 0) {
      return (long)pos;
    }
    if ((uint64_t)pos + 12 + l > len) {
      break;
    }
    pos += 12 + l;
  }
  return -1;
}

/* Feeds data[from..to) to png_process_data in pieces of `piece` bytes. Returns 0 on success. */
static int feed(Ctx* c, const unsigned char* data, size_t from, size_t to, size_t piece) {
  if (setjmp(c->jb)) {
    return 1;
  }
  size_t pos = from;
  while (pos < to) {
    size_t n = to - pos < piece ? to - pos : piece;
    png_process_data(c->png, c->info, (png_bytep)(data + pos), n);
    pos += n;
  }
  return 0;
}

static void run(const char* path, const unsigned char* data, size_t len, size_t piece) {
  printf("== %s piece=%zu\n", path, piece);
  long idat = first_idat(data, len);
  if (idat < 0) {
    printf("header: incomplete\nresult: error\n");
    return;
  }
  Ctx c;
  memset(&c, 0, sizeof c);
  c.png = png_create_read_struct(PNG_LIBPNG_VER_STRING, &c, error_fn, warning_fn);
  if (!c.png) {
    printf("result: error\n");
    return;
  }
  png_set_option(c.png, PNG_MAXIMUM_INFLATE_WINDOW, PNG_OPTION_ON);
  c.info = png_create_info_struct(c.png);
  png_set_keep_unknown_chunks(c.png, PNG_HANDLE_CHUNK_ALWAYS, (png_const_bytep) "", 0);
  if (setjmp(c.jb)) {
    printf("header: error\nresult: error\n");
    png_destroy_read_struct(&c.png, &c.info, NULL);
    return;
  }
  png_set_progressive_read_fn(c.png, &c, NULL, row_fn, NULL);
  if (feed(&c, data, 0, (size_t)idat, piece)) {
    printf("header: error\nresult: error\n");
    png_destroy_read_struct(&c.png, &c.info, NULL);
    return;
  }
  printf("header: ok\n");
  int passes = 1;
  if (setjmp(c.jb)) {
    printf("update: error\nresult: error\n");
    png_destroy_read_struct(&c.png, &c.info, NULL);
    return;
  }
  passes = setup_transforms(c.png, c.info);
  png_read_update_info(c.png, c.info);
  c.rowbytes = png_get_rowbytes(c.png, c.info);
  c.height = (int)png_get_image_height(c.png, c.info);
  c.interlaced = passes > 1;
  printf("info: %u %u %d %d %zu %d\n", (unsigned)png_get_image_width(c.png, c.info),
         (unsigned)png_get_image_height(c.png, c.info), png_get_bit_depth(c.png, c.info),
         png_get_color_type(c.png, c.info), c.rowbytes, png_get_channels(c.png, c.info));
  if (c.interlaced) {
    c.buf = (unsigned char*)calloc((size_t)c.height * c.rowbytes, 1);
  }
  int failed = feed(&c, data, (size_t)idat, len, piece);
  if (c.interlaced) {
    printf("interlace-hash: %016llx\n",
           (unsigned long long)fnv_update(fnv_init(), c.buf, (size_t)c.height * c.rowbytes));
    free(c.buf);
  }
  printf("rows: %d\nresult: %s\n", c.rows, failed ? "error" : "ok");
  png_destroy_read_struct(&c.png, &c.info, NULL);
}

int main(int argc, char** argv) {
  static const size_t kPieces[] = {1, 7, 4096};
  for (int a = 1; a < argc; a++) {
    FILE* f = fopen(argv[a], "rb");
    if (!f) {
      printf("== %s: cannot open\n", argv[a]);
      continue;
    }
    fseek(f, 0, SEEK_END);
    long len = ftell(f);
    fseek(f, 0, SEEK_SET);
    unsigned char* data = (unsigned char*)malloc((size_t)len);
    if (fread(data, 1, (size_t)len, f) != (size_t)len) {
      printf("== %s: short read\n", argv[a]);
    } else {
      for (size_t k = 0; k < sizeof kPieces / sizeof kPieces[0]; k++) {
        run(argv[a], data, (size_t)len, kPieces[k]);
      }
    }
    free(data);
    fclose(f);
  }
  return 0;
}
