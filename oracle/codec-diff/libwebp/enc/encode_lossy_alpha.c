/*
 * Differential reference for the skia-rust lossy encoder with transparency (the ALPH chunk and
 * the transparent-area cleanup of crates/skia-rust-libwebp/src/enc).
 *
 * Encodes RGBA pictures the way SkWebpEncoderImpl does for lossy: WebPConfigPreset(DEFAULT,
 * quality) with lossless = 0, method = 3, exact = 0, use_sharp_yuv = 0, preprocessing = 0; the
 * picture is imported with WebPPictureImportRGBA (use_argb = 0). One line per image:
 *   name width height quality bytes fnv1a64
 * The generators and the LCG are those of encode_lossy.c, plus the transparent-block kinds below.
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
        case 4:  /* noise with alpha; transparent pixels get arbitrary colours */
          r = (uint8_t)lcg_next(&seed); g = (uint8_t)lcg_next(&seed);
          b = (uint8_t)lcg_next(&seed); a = (uint8_t)lcg_next(&seed);
          break;
        case 7:  /* alpha ramp, opaque colour */
          r = 40; g = 90; b = 200; a = (uint8_t)((x * 255) / (w > 1 ? w - 1 : 1));
          break;
        case 8:  /* two colours, transparent 4x2 cells */
          if (((x / 4) + (y / 2)) % 3 == 0) { r = 255; g = 255; b = 255; a = 0; }
          else { r = 16; g = 32; b = 48; }
          break;
        case 9:  /* photo-like colour with a varying alpha */
          r = (uint8_t)((x * 3 + y) + (lcg_next(&seed) & 15));
          g = (uint8_t)((x + y * 2) + (lcg_next(&seed) & 7));
          b = (uint8_t)((x * y) + (lcg_next(&seed) & 31));
          a = (uint8_t)((x * 9 + y * 4) & 255);
          break;
        case 10: { /* 8x8 blocks, some fully transparent with arbitrary colours */
          int bx = x / 8, by = y / 8;
          if ((bx + by) % 3 == 0) {
            r = (uint8_t)lcg_next(&seed); g = (uint8_t)lcg_next(&seed);
            b = (uint8_t)lcg_next(&seed); a = 0;
          } else {
            r = (uint8_t)(x * 5); g = (uint8_t)(y * 7); b = (uint8_t)(bx * 11); a = 255;
          }
          break;
        }
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

static void run_case(const char* name, int kind, int w, int h, int quality) {
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
  config.lossless = 0;
  config.method = 3;
  config.exact = 0;
  config.use_sharp_yuv = 0;
  if (!WebPValidateConfig(&config)) exit(1);
  if (!WebPPictureInit(&pic)) exit(1);
  pic.use_argb = 0;
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
    printf("%s %d %d %d error %d\n", name, w, h, quality, pic.error_code);
  } else {
    printf("%s %d %d %d %zu %016llx\n", name, w, h, quality, writer.size,
           (unsigned long long)fnv1a64(writer.mem, writer.size));
  }
  WebPPictureFree(&pic);
  WebPMemoryWriterClear(&writer);
  free(rgba);
}

int main(void) {
  static const struct { const char* name; int kind; int w; int h; } images[] = {
    {"noise_alpha_40x30", 4, 40, 30},
    {"alpha_ramp_64x8", 7, 64, 8},
    {"two_colours_alpha_41x23", 8, 41, 23},
    {"photo_alpha_64x48", 9, 64, 48},
    {"blocks_alpha_72x40", 10, 72, 40},
    {"blocks_alpha_75x37", 10, 75, 37},
    {"pixel_alpha_1x1", 4, 1, 1},
    {"row_alpha_5x1", 7, 5, 1},
  };
  static const int qualities[] = {0, 50, 100};
  size_t i, q;
  for (i = 0; i < sizeof(images) / sizeof(images[0]); ++i) {
    for (q = 0; q < sizeof(qualities) / sizeof(qualities[0]); ++q) {
      run_case(images[i].name, images[i].kind, images[i].w, images[i].h, qualities[q]);
    }
  }
  return 0;
}
