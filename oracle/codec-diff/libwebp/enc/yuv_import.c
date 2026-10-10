/*
 * Differential reference for the YUV import of the skia-rust VP8 encoder
 * (crates/skia-rust-libwebp/src/enc/picture.rs).
 *
 * For each corpus image, imports the pixels the way SkWebpEncoderImpl does for lossy
 * (WebPPictureImportRGBA, or WebPPictureImportRGBX for opaque images, use_argb = 0) and prints
 * one line per image: name width height rgbx y_fnv1a64 u_fnv1a64 v_fnv1a64 a_fnv1a64 (or -).
 * The generators are those of encode_lossless.c, bit for bit.
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

static void print_plane(const uint8_t* p, size_t n) {
  printf(" %016llx", (unsigned long long)fnv1a64(p, n));
}

static void run_case(const char* name, int kind, int w, int h, int rgbx) {
  uint8_t* rgba = (uint8_t*)malloc((size_t)w * h * 4);
  WebPPicture pic;
  int uv_w = (w + 1) >> 1, uv_h = (h + 1) >> 1;
  if (rgba == NULL) return;
  make_image(kind, w, h, rgba);
  if (rgbx) {
    int i;
    for (i = 0; i < w * h; ++i) rgba[4 * i + 3] = 255;
  }
  if (!WebPPictureInit(&pic)) exit(1);
  pic.use_argb = 0;
  pic.width = w;
  pic.height = h;
  if (rgbx ? !WebPPictureImportRGBX(&pic, rgba, w * 4)
           : !WebPPictureImportRGBA(&pic, rgba, w * 4)) {
    fprintf(stderr, "import failed\n");
    exit(1);
  }
  printf("%s %d %d %d", name, w, h, rgbx);
  print_plane(pic.y, (size_t)pic.y_stride * h);
  print_plane(pic.u, (size_t)pic.uv_stride * uv_h);
  print_plane(pic.v, (size_t)pic.uv_stride * uv_h);
  if (pic.a != NULL) {
    print_plane(pic.a, (size_t)pic.a_stride * h);
  } else {
    printf(" -");
  }
  printf("\n");
  (void)uv_w;
  WebPPictureFree(&pic);
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
  size_t i;
  for (i = 0; i < sizeof(images) / sizeof(images[0]); ++i) {
    run_case(images[i].name, images[i].kind, images[i].w, images[i].h, 0);
    if (images[i].kind != 4 && images[i].kind != 7) {
      run_case(images[i].name, images[i].kind, images[i].w, images[i].h, 1);
    }
  }
  return 0;
}
