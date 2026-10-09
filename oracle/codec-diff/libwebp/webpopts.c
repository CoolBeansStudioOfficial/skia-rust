/*
 * Differential driver for the WebPDecoderOptions that Skia uses (cropping, scaling, bypass of the
 * loop filter, no fancy upsampling), for skia-rust-libwebp (docs/design/codecs.md §9.1).
 *
 * For each file and each option variant, decodes with WebPDecode in each output mode and prints
 * one line per (file, variant, mode):
 *
 *   <basename> <variant> <mode> status=<VP8StatusCode> w=<width> h=<height> fnv=<16 hex digits>
 *
 * The variant string is self-describing so the Rust side can parse it:
 *   none | crop=L,T,W,H | scale=W,H | bypass | nofancy, joined by '+'.
 * The hash is FNV-1a 64 over each output row (width * bytes-per-pixel bytes, no padding), folded
 * row by row as webpdump.c does.
 *
 * Built by build.sh with the same flags as webpdump.c: no SIMD, -ffp-contract=off.
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "src/webp/decode.h"
#include "src/dsp/cpu.h"

extern VP8CPUInfo VP8GetCPUInfo;

static const struct {
  const char* name;
  WEBP_CSP_MODE mode;
  int bpp;
} kModes[] = {
  {"RGBA", MODE_RGBA, 4},     {"BGRA", MODE_BGRA, 4},   {"rgbA", MODE_rgbA, 4},
  {"bgrA", MODE_bgrA, 4},     {"RGB", MODE_RGB, 3},     {"RGB565", MODE_RGB_565, 2},
};

static const char* kVariants[] = {
  "none",
  "crop=3,5,100,77",
  "crop=0,0,1,1",
  "crop=10,20,150,120",
  "scale=100,0",
  "scale=0,50",
  "scale=193,198",
  "scale=800,600",
  "scale=37,41",
  "crop=10,20,150,120+scale=75,60",
  "nofancy",
  "bypass",
  "crop=0,0,386,50+nofancy",
  "scale=1,1",
};

static uint64_t Fnv1a(const uint8_t* p, size_t n) {
  uint64_t h = 1469598103934665603ULL;
  size_t i;
  for (i = 0; i < n; ++i) {
    h ^= p[i];
    h *= 1099511628211ULL;
  }
  return h;
}

static uint8_t* ReadFile(const char* path, size_t* size) {
  FILE* f = fopen(path, "rb");
  long n;
  uint8_t* buf;
  if (f == NULL) return NULL;
  fseek(f, 0, SEEK_END);
  n = ftell(f);
  fseek(f, 0, SEEK_SET);
  buf = (uint8_t*)malloc(n > 0 ? (size_t)n : 1);
  if (buf != NULL && fread(buf, 1, (size_t)n, f) != (size_t)n) {
    free(buf);
    buf = NULL;
  }
  fclose(f);
  *size = (size_t)n;
  return buf;
}

static const char* Basename(const char* path) {
  const char* s = strrchr(path, '/');
  return s ? s + 1 : path;
}

/* Applies the variant string to the decoder config. Parts are separated by '+'. */
static void ApplyVariant(const char* variant, WebPDecoderConfig* config) {
  char buf[128];
  char* part;
  strncpy(buf, variant, sizeof(buf) - 1);
  buf[sizeof(buf) - 1] = '\0';
  for (part = strtok(buf, "+"); part != NULL; part = strtok(NULL, "+")) {
    int a, b, c, d;
    if (strcmp(part, "none") == 0) {
      /* defaults */
    } else if (strcmp(part, "nofancy") == 0) {
      config->options.no_fancy_upsampling = 1;
    } else if (strcmp(part, "bypass") == 0) {
      config->options.bypass_filtering = 1;
    } else if (sscanf(part, "crop=%d,%d,%d,%d", &a, &b, &c, &d) == 4) {
      config->options.use_cropping = 1;
      config->options.crop_left = a;
      config->options.crop_top = b;
      config->options.crop_width = c;
      config->options.crop_height = d;
    } else if (sscanf(part, "scale=%d,%d", &a, &b) == 2) {
      config->options.use_scaling = 1;
      config->options.scaled_width = a;
      config->options.scaled_height = b;
    }
  }
}

int main(int argc, char** argv) {
  int i, v, m;
  /* Disable runtime CPU dispatch: every DSP entry point is the portable C one. */
  VP8GetCPUInfo = NULL;
  for (i = 1; i < argc; ++i) {
    size_t size = 0;
    uint8_t* data = ReadFile(argv[i], &size);
    if (data == NULL) {
      printf("%s unreadable\n", Basename(argv[i]));
      continue;
    }
    for (v = 0; v < (int)(sizeof(kVariants) / sizeof(kVariants[0])); ++v) {
      for (m = 0; m < (int)(sizeof(kModes) / sizeof(kModes[0])); ++m) {
        WebPDecoderConfig config;
        VP8StatusCode status;
        WebPDecBuffer* out;
        if (!WebPInitDecoderConfig(&config)) return 1;
        config.options.use_threads = 0;
        config.output.colorspace = kModes[m].mode;
        ApplyVariant(kVariants[v], &config);
        status = WebPDecode(data, size, &config);
        out = &config.output;
        if (status == VP8_STATUS_OK) {
          const size_t row_bytes = (size_t)out->width * kModes[m].bpp;
          const uint8_t* rgba = out->u.RGBA.rgba;
          const int stride = out->u.RGBA.stride;
          uint64_t h = 1469598103934665603ULL;
          int y;
          for (y = 0; y < out->height; ++y) {
            const uint8_t* row = rgba + (size_t)y * stride;
            uint64_t r = Fnv1a(row, row_bytes);
            int b;
            for (b = 0; b < 8; ++b) {
              h ^= (r >> (8 * b)) & 0xff;
              h *= 1099511628211ULL;
            }
          }
          printf("%s %s %s status=%d w=%d h=%d fnv=%016llx\n", Basename(argv[i]), kVariants[v],
                 kModes[m].name, (int)status, out->width, out->height,
                 (unsigned long long)h);
        } else {
          printf("%s %s %s status=%d w=0 h=0 fnv=0000000000000000\n", Basename(argv[i]),
                 kVariants[v], kModes[m].name, (int)status);
        }
        WebPFreeDecBuffer(out);
      }
    }
    free(data);
  }
  return 0;
}
