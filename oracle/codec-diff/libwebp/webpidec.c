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
 * With `-v <variant>...` the files are decoded with each WebPDecoderOptions variant (the strings of
 * webpopts.c: crop=L,T,W,H, scale=W,H, nofancy, bypass, joined by '+'), and the variant is printed
 * after the basename:
 *
 *   <basename> <variant> <mode> chunk=<n> fed=<bytes> status=<VP8StatusCode> last_y=<rows> fnv=<hex>
 *
 * The rows are those of the cropped or scaled output (`WebPIDecGetRGB`'s width and height).
 *
 * Built with the pinned libwebp 1.4.0 sources (see build.sh, which links this driver as webpidec).
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

/* The option variants of the incremental differential with options (`-v`). The first two are
 * the cropping and scaling that SkWebpCodec sets for a subset and a scaled decode. */
static const char* kVariants[] = {
  "crop=10,20,150,120",
  "scale=37,41",
  "scale=800,600",
  "crop=10,20,150,120+scale=75,60",
  "crop=3,5,100,77+nofancy",
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

/* Applies a variant string (parts separated by '+') to the decoder options, as webpopts.c does. */
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

static void RunOne(const char* base, const char* variant, const uint8_t* data, size_t size,
                   int m, int chunk) {
  WebPDecoderConfig config;
  WebPIDecoder* idec;
  size_t fed = 0;
  int last_status = -1, last_y = -2, have_out = -1;
  VP8StatusCode status = VP8_STATUS_SUSPENDED;
  int ly = -1, w = 0, h = 0, stride = 0;
  uint8_t* rgb = NULL;
  /* The line prefix: the basename, then the variant when there is one. */
  const char* v = variant ? variant : "";
  char prefix[256];
  if (variant) {
    snprintf(prefix, sizeof(prefix), "%s %s", base, v);
  } else {
    snprintf(prefix, sizeof(prefix), "%s", base);
  }

  if (!WebPInitDecoderConfig(&config)) return;
  config.output.colorspace = kModes[m].mode;
  if (variant) ApplyVariant(variant, &config);
  idec = WebPIDecode(NULL, 0, &config);
  if (idec == NULL) {
    printf("%s %s chunk=%d fed=0 status=invalid last_y=-1 fnv=0000000000000000\n", prefix,
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
      printf("%s %s chunk=%d fed=%zu status=%d last_y=%d fnv=%016llx\n", prefix, kModes[m].name,
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
    printf("%s %s chunk=%d end fed=%zu status=%d last_y=%d fnv=%016llx\n", prefix,
           kModes[m].name, chunk, fed, (int)status, ly, (unsigned long long)fnv);
  }
  WebPIDelete(idec);
}

int main(int argc, char** argv) {
  int i, m, c, v, first_file = 1, variants = 0;
  /* Disable runtime CPU dispatch: every DSP entry point is the portable C one. */
  VP8GetCPUInfo = NULL;
  if (argc > 1 && strcmp(argv[1], "-v") == 0) {
    variants = 1;
    first_file = 2;
  }
  for (i = first_file; i < argc; ++i) {
    size_t size = 0;
    uint8_t* data = ReadFile(argv[i], &size);
    if (data == NULL) {
      printf("%s unreadable\n", Basename(argv[i]));
      continue;
    }
    for (m = 0; m < (int)(sizeof(kModes) / sizeof(kModes[0])); ++m) {
      for (c = 0; c < (int)(sizeof(kChunks) / sizeof(kChunks[0])); ++c) {
        if (!variants) {
          RunOne(Basename(argv[i]), NULL, data, size, m, kChunks[c]);
        } else {
          for (v = 0; v < (int)(sizeof(kVariants) / sizeof(kVariants[0])); ++v) {
            RunOne(Basename(argv[i]), kVariants[v], data, size, m, kChunks[c]);
          }
        }
      }
    }
    free(data);
  }
  return 0;
}
