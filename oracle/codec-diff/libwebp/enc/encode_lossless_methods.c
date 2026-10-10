/*
 * Differential reference for the VP8L encoder at the methods SkWebpEncoder reaches: method 0
 * (lossless pictures) and method 3 with exact = 1 (the alpha plane of lossy pictures), plus
 * methods 1, 2 and 4 for coverage of the same code paths.
 *
 * Encodes the corpus of encode_lossless.c with WebPConfigPreset(DEFAULT, quality), lossless = 1,
 * config->method = m, config->exact = e, and a picture imported as RGBA (use_argb = 1). Writes
 * one line per (image, method, exact, quality):
 *   name width height method exact quality bytes fnv1a64
 * or `name width height method exact quality error <code>`.
 * The image generators and the LCG are the ones of encode_lossless.c, bit for bit.
 *
 * Build: see build.sh in this directory.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "src/webp/encode.h"

static uint32_t lcg_next(uint32_t* s) {
  *s = *s * 1103515245u + 12345u;
  return (*s >> 16) & 0x7fffu;
}

/* Fills rgba (w*h*4 bytes) for corpus entry `kind`. */
static void make_image(int kind, int w, int h, uint8_t* rgba) {
  int x, y;
  uint32_t seed = 12345u + (uint32_t)kind;
  for (y = 0; y < h; ++y) {
    for (x = 0; x < w; ++x) {
      uint8_t* p = rgba + 4 * (y * w + x);
      uint8_t r = 0, g = 0, b = 0, a = 255;
      switch (kind) {
        case 0:  /* gradient */
          r = (uint8_t)(x * 7); g = (uint8_t)(y * 11); b = (uint8_t)((x + y) * 3);
          break;
        case 1:  /* solid blue */
          b = 255;
          break;
        case 2: { /* five colours in blocks */
          static const uint8_t pal[5][3] = {{255, 0, 0}, {0, 255, 0}, {0, 0, 255},
                                            {10, 20, 30}, {200, 200, 0}};
          int idx = ((x / 5) + (y / 3)) % 5;
          r = pal[idx][0]; g = pal[idx][1]; b = pal[idx][2];
          break;
        }
        case 3:  /* noise, opaque */
          r = (uint8_t)lcg_next(&seed); g = (uint8_t)lcg_next(&seed);
          b = (uint8_t)lcg_next(&seed);
          break;
        case 4:  /* noise with alpha; transparent pixels get arbitrary colours */
          r = (uint8_t)lcg_next(&seed); g = (uint8_t)lcg_next(&seed);
          b = (uint8_t)lcg_next(&seed); a = (uint8_t)lcg_next(&seed);
          break;
        case 5: { /* 200 colours */
          int idx = (x * x + y * 3) % 200;
          r = (uint8_t)(idx * 37); g = (uint8_t)(idx * 91 + 7); b = (uint8_t)(idx * 13 + 100);
          break;
        }
        case 6:  /* smooth gradient with a little noise */
          r = (uint8_t)((x * 255) / (w > 1 ? w - 1 : 1));
          g = (uint8_t)((y * 255) / (h > 1 ? h - 1 : 1));
          b = (uint8_t)(((x ^ y) + (lcg_next(&seed) & 3)) & 255);
          break;
        case 7:  /* alpha ramp, opaque colour */
          r = 40; g = 90; b = 200; a = (uint8_t)((x * 255) / (w > 1 ? w - 1 : 1));
          break;
        case 8:  /* two colours, a few transitions: palette of 2 */
          if (((x / 4) + (y / 2)) % 3 == 0) { r = 255; g = 255; b = 255; a = 0; }
          else { r = 16; g = 32; b = 48; }
          break;
        case 9:  /* photo-like: smooth colour with noise, opaque */
          r = (uint8_t)((x * 3 + y) + (lcg_next(&seed) & 15));
          g = (uint8_t)((x + y * 2) + (lcg_next(&seed) & 7));
          b = (uint8_t)((x * y) + (lcg_next(&seed) & 31));
          break;
        default:
          break;
      }
      p[0] = r; p[1] = g; p[2] = b; p[3] = a;
    }
  }
}

static uint64_t fnv1a64(const uint8_t* data, size_t n) {
  uint64_t hash = 1469598103934665603ull;
  size_t i;
  for (i = 0; i < n; ++i) {
    hash ^= data[i];
    hash *= 1099511628211ull;
  }
  return hash;
}

static void run_case(const char* name, int kind, int w, int h, int method,
                     int exact, int quality) {
  uint8_t* rgba = (uint8_t*)malloc((size_t)w * h * 4);
  WebPConfig config;
  WebPPicture pic;
  WebPMemoryWriter writer;
  int ok;
  if (rgba == NULL) return;
  make_image(kind, w, h, rgba);
  if (!WebPConfigPreset(&config, WEBP_PRESET_DEFAULT, (float)quality)) {
    fprintf(stderr, "config preset failed\n");
    exit(1);
  }
  config.lossless = 1;
  config.method = method;
  config.exact = exact;
  if (!WebPPictureInit(&pic)) exit(1);
  pic.use_argb = 1;
  pic.width = w;
  pic.height = h;
  if (!WebPPictureImportRGBA(&pic, rgba, w * 4)) {
    fprintf(stderr, "import failed\n");
    exit(1);
  }
  WebPMemoryWriterInit(&writer);
  pic.writer = WebPMemoryWrite;
  pic.custom_ptr = &writer;
  ok = WebPEncode(&config, &pic);
  if (!ok) {
    printf("%s %d %d %d %d %d error %d\n", name, w, h, method, exact, quality,
           pic.error_code);
  } else {
    printf("%s %d %d %d %d %d %zu %016llx\n", name, w, h, method, exact, quality,
           writer.size, (unsigned long long)fnv1a64(writer.mem, writer.size));
  }
  WebPPictureFree(&pic);
  WebPMemoryWriterClear(&writer);
  free(rgba);
}

int main(void) {
  static const struct { const char* name; int kind; int w; int h; } images[] = {
    {"gradient_37x23", 0, 37, 23},
    {"solid_64x64", 1, 64, 64},
    {"five_colours_50x40", 2, 50, 40},
    {"noise_33x17", 3, 33, 17},
    {"noise_alpha_40x30", 4, 40, 30},
    {"colours200_120x120", 5, 120, 120},
    {"smooth_96x80", 6, 96, 80},
    {"alpha_ramp_64x8", 7, 64, 8},
    {"two_colours_41x23", 8, 41, 23},
    {"photo_64x48", 9, 64, 48},
    {"pixel_1x1", 0, 1, 1},
    {"row_2x1", 0, 2, 1},
    {"column_1x5", 2, 1, 5},
    {"gradient_300x200", 6, 300, 200},
  };
  static const int methods[] = {0, 1, 2, 3, 4};
  static const int exacts[] = {0, 1};
  static const int qualities[] = {0, 50, 100};
  size_t i, m, e, q;
  for (i = 0; i < sizeof(images) / sizeof(images[0]); ++i) {
    for (m = 0; m < sizeof(methods) / sizeof(methods[0]); ++m) {
      for (e = 0; e < sizeof(exacts) / sizeof(exacts[0]); ++e) {
        for (q = 0; q < sizeof(qualities) / sizeof(qualities[0]); ++q) {
          run_case(images[i].name, images[i].kind, images[i].w, images[i].h,
                   methods[m], exacts[e], qualities[q]);
        }
      }
    }
  }
  return 0;
}
