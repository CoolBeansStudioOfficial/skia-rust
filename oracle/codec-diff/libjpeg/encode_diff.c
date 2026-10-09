/*
 * Differential harness for skia-rust-libjpeg's compressor: drives libjpeg-turbo 3.1.0 (built by
 * build.sh, no SIMD, JPEG_LIB_VERSION 62 as in Skia's jconfig.h) through the calls Skia's
 * SkJpegEncoderImpl makes, over a synthetic corpus generated from integer hashes, and prints one
 * line per case:
 *
 *   <case>|<bytes>|<fnv1a-64 of the JPEG bytes>
 *
 * The corpus, the case names and the pixel generator are mirrored in
 * crates/skia-rust-libjpeg/tests/encode_diff.rs, which encodes the same cases with the port and
 * compares the lines. `expected/encode.txt` is generated from this program and committed.
 *
 * Call sequence (SkJpegEncoderImpl.cpp#L160-L195 and initializeCommon): jpeg_set_defaults, the
 * sampling factors, optimize_coding = TRUE, jpeg_set_quality(q, TRUE), jpeg_start_compress(TRUE),
 * jpeg_write_marker for each metadata segment, jpeg_write_scanlines in chunks, and
 * jpeg_finish_compress. The output is collected with jpeg_stdio_dest on a temporary file.
 *
 * Usage: encode_diff
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "jpeglib.h"

typedef struct {
  const char *fmt; /* "rgba", "bgra", "rgbx", "rgb", "ycc", "gray" */
  J_COLOR_SPACE in_cs;
  int nc;
} format_t;

static const format_t FORMATS[] = {
  {"rgba", JCS_EXT_RGBA, 4}, {"bgra", JCS_EXT_BGRA, 4}, {"rgbx", JCS_EXT_RGBX, 4},
  {"rgb", JCS_EXT_RGB, 3},   {"ycc", JCS_YCbCr, 3},     {"gray", JCS_GRAYSCALE, 1},
};

/* The pixel generator shared with the Rust replay. kind 0: a gradient with 5 bits of noise;
 * kind 1: uniform noise over all 256 values. */
static uint32_t mix(uint32_t h) {
  h ^= h >> 15;
  h *= 0x2C1B3C6Du;
  h ^= h >> 12;
  h *= 0x297A2D39u;
  h ^= h >> 15;
  return h;
}

static uint8_t pixel(uint32_t x, uint32_t y, uint32_t c, uint32_t seed, int kind) {
  uint32_t h = (x * 0x9E3779B1u) ^ (y * 0x85EBCA77u) ^ (c * 0xC2B2AE3Du) ^ (seed * 0x27D4EB2Fu);
  h = mix(h);
  if (kind == 1) return (uint8_t)(h & 255u);
  {
    uint32_t grad = (x * 5u + y * 3u + c * 40u) & 255u;
    return (uint8_t)((grad + (h & 31u)) & 255u);
  }
}

typedef struct {
  char name[160];
  const format_t *fmt;
  uint32_t w, h;
  int quality;
  int samp; /* 420, 422, 444, or 0 for grayscale */
  int smoothing;
  int markers;
  int chunk; /* rows per jpeg_write_scanlines call; 0 = all rows */
  int kind;
  uint32_t seed;
} case_t;

static int add_case(case_t *cs, int n, const format_t *f, uint32_t w, uint32_t h, int q,
                    int samp, int smoothing, int markers, int chunk, int kind) {
  case_t *c = &cs[n];
  snprintf(c->name, sizeof c->name, "%s/%ux%u/q%d/s%d/sm%d/m%d/c%d/k%d", f->fmt, (unsigned)w,
           (unsigned)h, q, samp, smoothing, markers, chunk, kind);
  c->fmt = f;
  c->w = w;
  c->h = h;
  c->quality = q;
  c->samp = samp;
  c->smoothing = smoothing;
  c->markers = markers;
  c->chunk = chunk;
  c->kind = kind;
  c->seed = (uint32_t)(n + 1);
  return n + 1;
}

static int build_cases(case_t *cs) {
  static const uint32_t SIZES[][2] = {{1, 1},  {3, 2},  {8, 8},   {9, 7},
                                      {16, 17}, {33, 31}, {64, 48}, {101, 67}};
  static const int QS[] = {1, 75, 100};
  static const int SAMPS[] = {420, 422, 444};
  int n = 0;
  size_t i, j, k, l;
  for (i = 0; i < sizeof SIZES / sizeof SIZES[0]; i++) {
    for (j = 0; j < sizeof FORMATS / sizeof FORMATS[0]; j++) {
      const format_t *f = &FORMATS[j];
      int ns = f->in_cs == JCS_GRAYSCALE ? 1 : 3;
      for (k = 0; k < (size_t)ns; k++) {
        for (l = 0; l < 3; l++) {
          int samp = f->in_cs == JCS_GRAYSCALE ? 0 : SAMPS[k];
          n = add_case(cs, n, f, SIZES[i][0], SIZES[i][1], QS[l], samp, 0, 0, 0, 0);
        }
      }
    }
  }
  /* Metadata segments, row chunking, input smoothing, and large noisy images. */
  n = add_case(cs, n, &FORMATS[0], 33, 31, 75, 420, 0, 1, 0, 0);
  n = add_case(cs, n, &FORMATS[4], 33, 31, 75, 420, 0, 1, 0, 0);
  n = add_case(cs, n, &FORMATS[5], 33, 31, 90, 0, 0, 1, 0, 0);
  n = add_case(cs, n, &FORMATS[0], 64, 48, 75, 420, 0, 0, 3, 0);
  n = add_case(cs, n, &FORMATS[4], 64, 48, 75, 422, 0, 0, 1, 0);
  n = add_case(cs, n, &FORMATS[1], 101, 67, 100, 444, 0, 0, 7, 0);
  n = add_case(cs, n, &FORMATS[4], 16, 17, 75, 420, 40, 0, 0, 0);
  n = add_case(cs, n, &FORMATS[4], 33, 31, 75, 444, 40, 0, 0, 0);
  n = add_case(cs, n, &FORMATS[0], 33, 31, 75, 422, 100, 0, 0, 0);
  n = add_case(cs, n, &FORMATS[4], 64, 48, 60, 420, 100, 0, 0, 0);
  n = add_case(cs, n, &FORMATS[5], 64, 48, 75, 0, 40, 0, 0, 0);
  n = add_case(cs, n, &FORMATS[0], 300, 200, 100, 420, 0, 0, 0, 1);
  n = add_case(cs, n, &FORMATS[4], 300, 200, 1, 420, 0, 0, 0, 1);
  n = add_case(cs, n, &FORMATS[5], 300, 200, 75, 0, 0, 0, 0, 1);
  n = add_case(cs, n, &FORMATS[2], 300, 200, 100, 444, 0, 0, 0, 1);
  return n;
}

static const uint8_t ICC_LIKE[40] = {
  'A', 'C', 'S', 'P', 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
  16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35,
};
static const uint8_t XMP_LIKE[60] = {
  'h', 't', 't', 'p', ':', '/', '/', 'n', 's', '.', 'a', 'd', 'o', 'b', 'e', '.', 'c', 'o',
  'm', '/', 'x', 'a', 'p', '/', '1', '.', '0', '/', 0, 'x', 'm', 'p', 'd', 'a', 't', 'a',
  0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
};
static const uint8_t EXIF_LIKE[30] = {
  'E', 'x', 'i', 'f', 0, 0, 'M', 'M', 0, 42, 0, 0, 0, 8, 0, 1, 1, 0, 0, 3,
  0, 0, 0, 1, 0, 1, 0, 0, 0, 0,
};

typedef struct {
  struct jpeg_error_mgr pub;
} err_mgr;

static void output_message(j_common_ptr cinfo) { (void)cinfo; }

/* Encodes one case; returns the bytes in *out (malloc'd) and their count. */
static int encode_case(const case_t *c, uint8_t **out, size_t *outlen) {
  struct jpeg_compress_struct cinfo;
  struct jpeg_error_mgr jerr;
  FILE *tmp = tmpfile();
  uint8_t *row = NULL;
  uint32_t rowbytes;
  uint32_t y;
  int ok = 0;
  if (!tmp) return 0;
  cinfo.err = jpeg_std_error(&jerr);
  jerr.output_message = output_message;
  jpeg_create_compress(&cinfo);
  jpeg_stdio_dest(&cinfo, tmp);
  cinfo.image_width = c->w;
  cinfo.image_height = c->h;
  cinfo.input_components = c->fmt->nc;
  cinfo.in_color_space = c->fmt->in_cs;
  jpeg_set_defaults(&cinfo);
  if (c->samp == 420) {
    cinfo.comp_info[0].h_samp_factor = 2;
    cinfo.comp_info[0].v_samp_factor = 2;
  } else if (c->samp == 422) {
    cinfo.comp_info[0].h_samp_factor = 2;
    cinfo.comp_info[0].v_samp_factor = 1;
  } else if (c->samp == 444) {
    cinfo.comp_info[0].h_samp_factor = 1;
    cinfo.comp_info[0].v_samp_factor = 1;
  }
  cinfo.smoothing_factor = c->smoothing;
  cinfo.optimize_coding = TRUE;
  jpeg_set_quality(&cinfo, c->quality, TRUE);
  jpeg_start_compress(&cinfo, TRUE);
  if (c->markers) {
    jpeg_write_marker(&cinfo, JPEG_APP0 + 2, ICC_LIKE, sizeof ICC_LIKE);
    jpeg_write_marker(&cinfo, JPEG_APP0 + 1, XMP_LIKE, sizeof XMP_LIKE);
    jpeg_write_marker(&cinfo, JPEG_APP0 + 1, EXIF_LIKE, sizeof EXIF_LIKE);
  }
  rowbytes = c->w * (uint32_t)c->fmt->nc;
  row = (uint8_t *)malloc(rowbytes);
  {
    uint32_t chunk = c->chunk ? (uint32_t)c->chunk : c->h;
    uint32_t done = 0;
    while (done < c->h) {
      uint32_t n = c->h - done < chunk ? c->h - done : chunk;
      JSAMPROW rows[64];
      uint32_t r;
      if (n > 64) n = 64;
      for (r = 0; r < n; r++) {
        uint32_t x, cc;
        y = done + r;
        for (x = 0; x < c->w; x++) {
          for (cc = 0; cc < (uint32_t)c->fmt->nc; cc++) {
            row[x * c->fmt->nc + cc] = pixel(x, y, cc, c->seed, c->kind);
          }
        }
        rows[r] = (JSAMPROW)malloc(rowbytes);
        memcpy(rows[r], row, rowbytes);
      }
      jpeg_write_scanlines(&cinfo, rows, n);
      for (r = 0; r < n; r++) free(rows[r]);
      done += n;
    }
  }
  jpeg_finish_compress(&cinfo);
  jpeg_destroy_compress(&cinfo);
  free(row);
  {
    long len = ftell(tmp);
    rewind(tmp);
    *out = (uint8_t *)malloc((size_t)len);
    if (*out && fread(*out, 1, (size_t)len, tmp) == (size_t)len) {
      *outlen = (size_t)len;
      ok = 1;
    }
  }
  fclose(tmp);
  return ok;
}

static uint64_t fnv1a(const uint8_t *p, size_t n) {
  uint64_t h = 0xcbf29ce484222325ull;
  size_t i;
  for (i = 0; i < n; i++) {
    h ^= p[i];
    h *= 0x100000001b3ull;
  }
  return h;
}

static case_t CASES[512];

int main(void) {
  int n = build_cases(CASES);
  int i;
  for (i = 0; i < n; i++) {
    uint8_t *bytes = NULL;
    size_t len = 0;
    if (encode_case(&CASES[i], &bytes, &len)) {
      printf("%s|%lu|%016llx\n", CASES[i].name, (unsigned long)len,
             (unsigned long long)fnv1a(bytes, len));
    } else {
      printf("%s|err|0\n", CASES[i].name);
    }
    free(bytes);
  }
  return 0;
}
