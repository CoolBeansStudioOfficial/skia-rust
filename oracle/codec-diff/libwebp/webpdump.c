/*
 * Differential driver for skia-rust-libwebp (oracle/codec-diff, see docs/design/codecs.md §9.1).
 *
 * Decodes each file named on the command line with WebPDecode in every output mode Skia asks
 * for, and prints one line per (file, mode):
 *
 *   <basename> <mode> status=<VP8StatusCode> w=<width> h=<height> fnv=<16 hex digits>
 *
 * The hash is FNV-1a 64 over the decoded bytes (width * bytes-per-pixel per row, no padding), so
 * the Rust port's `oracle/codec-diff/libwebp/expected.txt` can be compared line by line.
 *
 * Built with the pinned libwebp 1.4.0 sources (see build.sh), `-DWEBP_SWAP_16BIT_CSP=1` as in
 * Skia's build, `-ffp-contract=off`, and no SIMD (the C paths).
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
  {"RGB", MODE_RGB, 3},         {"RGBA", MODE_RGBA, 4},     {"BGR", MODE_BGR, 3},
  {"BGRA", MODE_BGRA, 4},       {"ARGB", MODE_ARGB, 4},     {"rgbA", MODE_rgbA, 4},
  {"bgrA", MODE_bgrA, 4},       {"Argb", MODE_Argb, 4},     {"RGB565", MODE_RGB_565, 2},
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

int main(int argc, char** argv) {
  int i, m;
  /* Disable runtime CPU dispatch: every DSP entry point is the portable C one. */
  VP8GetCPUInfo = NULL;
  for (i = 1; i < argc; ++i) {
    size_t size = 0;
    uint8_t* data = ReadFile(argv[i], &size);
    if (data == NULL) {
      printf("%s unreadable\n", Basename(argv[i]));
      continue;
    }
    for (m = 0; m < (int)(sizeof(kModes) / sizeof(kModes[0])); ++m) {
      WebPDecoderConfig config;
      VP8StatusCode status;
      WebPDecBuffer* out;
      if (!WebPInitDecoderConfig(&config)) return 1;
      config.options.use_threads = 0;
      config.output.colorspace = kModes[m].mode;
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
          /* Fold each row's hash into the running hash, byte by byte, to keep it order-sensitive. */
          for (b = 0; b < 8; ++b) {
            h ^= (r >> (8 * b)) & 0xff;
            h *= 1099511628211ULL;
          }
        }
        printf("%s %s status=%d w=%d h=%d fnv=%016llx\n", Basename(argv[i]), kModes[m].name,
               (int)status, out->width, out->height, (unsigned long long)h);
      } else {
        printf("%s %s status=%d w=0 h=0 fnv=0000000000000000\n", Basename(argv[i]), kModes[m].name,
               (int)status);
      }
      WebPFreeDecBuffer(out);
    }
    free(data);
  }
  return 0;
}
