/*
 * Differential reference for the skia-rust VP8 lossy encoder (crates/skia-rust-libwebp/src/enc).
 *
 * Encodes the corpus below the way SkWebpEncoderImpl does for lossy: WebPConfigPreset(DEFAULT,
 * quality) with lossless = 0, method = 3 (Skia sets it), exact = 0, use_sharp_yuv = 0,
 * preprocessing = 0; the picture is imported as RGBA (WebPPictureImportRGBA, use_argb = 0) or
 * RGBX (WebPPictureImportRGBX, opaque images), and the output is one line per image:
 *   name width height quality bytes fnv1a64
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

static void run_case(const char* name, int kind, int w, int h, int quality, int rgbx) {
  uint8_t* rgba = (uint8_t*)malloc((size_t)w * h * 4);
  WebPConfig config;
  WebPPicture pic;
  WebPMemoryWriter writer;
  int ok;
  if (rgba == NULL) return;
  make_image(kind, w, h, rgba);
  if (rgbx) {
    int i;
    for (i = 0; i < w * h; ++i) rgba[4 * i + 3] = 255;
  }
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
  if (rgbx ? !WebPPictureImportRGBX(&pic, rgba, w * 4)
           : !WebPPictureImportRGBA(&pic, rgba, w * 4)) {
    fprintf(stderr, "import failed\n");
    exit(1);
  }
  WebPMemoryWriterInit(&writer);
  pic.writer = WebPMemoryWrite;
  pic.custom_ptr = &writer;
  ok = WebPEncode(&config, &pic);
  if (!ok) {
    printf("%s %d %d %d%s error %d\n", name, w, h, quality, rgbx ? " rgbx" : "", pic.error_code);
  } else {
    printf("%s %d %d %d%s %zu %016llx\n", name, w, h, quality, rgbx ? " rgbx" : "", writer.size,
           (unsigned long long)fnv1a64(writer.mem, writer.size));
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
    {"pixel_1x1", 0, 1, 1},
    {"row_2x1", 0, 2, 1},
    {"column_1x5", 2, 1, 5},
    {"gradient_300x200", 6, 300, 200},
  };
  static const int qualities[] = {0, 50, 100};
  size_t i, q;
  for (i = 0; i < sizeof(images) / sizeof(images[0]); ++i) {
    for (q = 0; q < sizeof(qualities) / sizeof(qualities[0]); ++q) {
      run_case(images[i].name, images[i].kind, images[i].w, images[i].h, qualities[q], 0);
      if (images[i].kind != 4 && images[i].kind != 7) {
        run_case(images[i].name, images[i].kind, images[i].w, images[i].h, qualities[q], 1);
      }
    }
  }
  return 0;
}
