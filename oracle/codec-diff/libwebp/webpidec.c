/*
 * Differential driver for the incremental decoder (idec_dec.c) of skia-rust-libwebp.
 *
 * For each file named on the command line and each RGB output mode of webpdump.c, the file is fed
 * to WebPIUpdate as a growing prefix in steps of 1, 7, 64 and 4096 bytes, as SkWebpCodec does with
 * a map-mode buffer. After each call a line is printed when the status, the last decoded row
 * (`WebPIDecGetRGB`'s last_y) or the output pointer's availability changes, and once more at the
 * end:
 *
 *   <basename> <mode> chunk=<n> fed=<bytes> status=<VP8StatusCode> last_y=<rows> fnv=<16 hex>
 *
 * fnv is FNV-1a 64 folded over the rows [0, last_y) at width * bytes-per-pixel each, as in
 * webpdump.c, so the Rust replay compares the emitted pixels at every step.
 *
 * Built with the pinned libwebp 1.4.0 sources (see build.sh).
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

static const int kChunks[] = {1, 7, 64, 4096};

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

/* FNV over the rows [0, rows) of an RGB buffer, folded per row as in webpdump.c. */
static uint64_t HashRows(const uint8_t* rgb, int stride, int row_bytes, int rows) {
  uint64_t acc = 1469598103934665603ULL;
  int y, b;
  for (y = 0; y < rows; ++y) {
    uint64_t r = Fnv1a(rgb + (size_t)y * stride, (size_t)row_bytes);
    for (b = 0; b < 8; ++b) {
      acc ^= (r >> (8 * b)) & 0xff;
      acc *= 1099511628211ULL;
    }
  }
  return acc;
}

static void RunOne(const char* base, const uint8_t* data, size_t size, int m, int chunk) {
  WebPDecoderConfig config;
  WebPIDecoder* idec;
  size_t fed = 0;
  int last_status = -1, last_y = -2, have_out = -1;
  VP8StatusCode status = VP8_STATUS_SUSPENDED;
  int ly = -1, w = 0, h = 0, stride = 0;
  uint8_t* rgb = NULL;

  if (!WebPInitDecoderConfig(&config)) return;
  config.output.colorspace = kModes[m].mode;
  idec = WebPIDecode(NULL, 0, &config);
  if (idec == NULL) {
    printf("%s %s chunk=%d fed=0 status=invalid last_y=-1 fnv=0000000000000000\n", base,
           kModes[m].name, chunk);
    return;
  }
  while (fed < size) {
    size_t n = (size - fed < (size_t)chunk) ? size - fed : (size_t)chunk;
    fed += n;
    status = WebPIUpdate(idec, data, fed);
    ly = -1;
    rgb = WebPIDecGetRGB(idec, &ly, &w, &h, &stride);
    if (status != VP8_STATUS_SUSPENDED || (int)status != last_status || ly != last_y ||
        (rgb != NULL) != have_out) {
      uint64_t fnv = (rgb != NULL) ? HashRows(rgb, stride, w * kModes[m].bpp, ly) : 0;
      printf("%s %s chunk=%d fed=%zu status=%d last_y=%d fnv=%016llx\n", base, kModes[m].name,
             chunk, fed, (int)status, ly, (unsigned long long)fnv);
      last_status = (int)status;
      last_y = ly;
      have_out = (rgb != NULL);
    }
    if (status != VP8_STATUS_SUSPENDED) break;
  }
  ly = -1;
  rgb = WebPIDecGetRGB(idec, &ly, &w, &h, &stride);
  {
    uint64_t fnv = (rgb != NULL) ? HashRows(rgb, stride, w * kModes[m].bpp, ly) : 0;
    printf("%s %s chunk=%d end fed=%zu status=%d last_y=%d fnv=%016llx\n", base,
           kModes[m].name, chunk, fed, (int)status, ly, (unsigned long long)fnv);
  }
  WebPIDelete(idec);
}

int main(int argc, char** argv) {
  int i, m, c;
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
      for (c = 0; c < (int)(sizeof(kChunks) / sizeof(kChunks[0])); ++c) {
        RunOne(Basename(argv[i]), data, size, m, kChunks[c]);
      }
    }
    free(data);
  }
  return 0;
}
