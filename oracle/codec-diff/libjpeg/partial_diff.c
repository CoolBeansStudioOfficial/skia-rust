/*
 * Differential harness for jpeg_crop_scanline and jpeg_skip_scanlines (libjpeg-turbo 3.1.0, the
 * revision Skia m156 pins). Drives the C library through a list of operations per case and prints
 * one line per case:
 *
 *   <file>|<case>|<status>|<rows>|<fnv1a-64 of the output bytes>
 *
 * Each case first decodes the header and starts the output (buffered-image for progressive files,
 * as SkJpegCodec does), crops when crop_w != 0, then runs its operations in order:
 *   skip n: jpeg_skip_scanlines(n); the returned count is hashed as 4 little-endian bytes;
 *   read n: up to n rows through jpeg_read_scanlines, one at a time (n == 0: to the end).
 * The crop is given in sixteenths of output_width and clamped to it, the same way in the Rust test
 * (crates/skia-rust-libjpeg/tests/diff.rs, matches_libjpeg_partial_oracle). `rows` counts the rows
 * read.
 *
 * Usage: partial_diff <resources/images dir>
 * Built by oracle/codec-diff/libjpeg/build.sh, which writes expected/partial.txt.
 */
#include <dirent.h>
#include <setjmp.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include "jpeglib.h"

typedef struct {
  struct jpeg_error_mgr pub;
  jmp_buf jb;
} err_mgr;

typedef struct {
  struct jpeg_source_mgr pub;
  const JOCTET *data;
  size_t len;
} mem_src;

static void err_exit(j_common_ptr cinfo) {
  err_mgr *e = (err_mgr *)cinfo->err;
  longjmp(e->jb, 1);
}

static void output_message(j_common_ptr cinfo) { (void)cinfo; }

static void init_src(j_decompress_ptr c) {
  mem_src *s = (mem_src *)c->src;
  s->pub.next_input_byte = s->data;
  s->pub.bytes_in_buffer = s->len;
}

static boolean fill_src(j_decompress_ptr c) {
  mem_src *s = (mem_src *)c->src;
  /* Skia's memory source: no more data is available, so report suspension. */
  s->pub.next_input_byte = NULL;
  s->pub.bytes_in_buffer = 0;
  return FALSE;
}

static void skip_src(j_decompress_ptr c, long n) {
  mem_src *s = (mem_src *)c->src;
  if ((size_t)n > s->pub.bytes_in_buffer) {
    err_exit((j_common_ptr)c);
  }
  s->pub.next_input_byte += n;
  s->pub.bytes_in_buffer -= (size_t)n;
}

static void term_src(j_decompress_ptr c) { (void)c; }

typedef struct {
  uint64_t h;
} hasher;

static void hash_bytes(hasher *hs, const unsigned char *p, size_t n) {
  size_t i;
  for (i = 0; i < n; i++) {
    hs->h ^= p[i];
    hs->h *= 1099511628211ULL;
  }
}

static void hash_u32(hasher *hs, uint32_t v) {
  unsigned char b[4];
  b[0] = (unsigned char)(v & 0xff);
  b[1] = (unsigned char)((v >> 8) & 0xff);
  b[2] = (unsigned char)((v >> 16) & 0xff);
  b[3] = (unsigned char)((v >> 24) & 0xff);
  hash_bytes(hs, b, 4);
}

typedef struct {
  int skip; /* 1: skip n rows; 0: read n rows (n == 0: to the end of the image) */
  unsigned n;
} pop;

typedef struct {
  const char *name;
  int scale_num;
  J_COLOR_SPACE out_cs;
  unsigned crop_x; /* in sixteenths of output_width */
  unsigned crop_w; /* 0: no crop */
  const pop *ops;
  int nops;
} pcase;

static const char *status_for_failure = "err";

static void run_partial(const unsigned char *data, size_t len, const pcase *pc, const char **status,
                        unsigned *rows, uint64_t *hash) {
  struct jpeg_decompress_struct cinfo;
  mem_src src;
  err_mgr jerr;
  /* Static: read after longjmp, so it must not be an indeterminate automatic. */
  static hasher hs;
  static JSAMPARRAY rowbuf;
  unsigned int total_rows = 0;
  int i;

  hs.h = 14695981039346656037ULL;
  rowbuf = NULL;
  *rows = 0;
  *hash = hs.h;
  *status = status_for_failure;

  memset(&cinfo, 0, sizeof(cinfo));
  memset(&src, 0, sizeof(src));
  cinfo.err = jpeg_std_error(&jerr.pub);
  jerr.pub.error_exit = err_exit;
  jerr.pub.output_message = output_message;
  if (setjmp(jerr.jb)) {
    jpeg_destroy_decompress(&cinfo);
    *hash = hs.h;
    free(rowbuf);
    return;
  }
  jpeg_create_decompress(&cinfo);
  src.pub.init_source = init_src;
  src.pub.fill_input_buffer = fill_src;
  src.pub.skip_input_data = skip_src;
  src.pub.resync_to_restart = jpeg_resync_to_restart;
  src.pub.term_source = term_src;
  src.data = data;
  src.len = len;
  cinfo.src = &src.pub;

  if (jpeg_read_header(&cinfo, TRUE) == JPEG_SUSPENDED) {
    *status = "suspended";
    jpeg_destroy_decompress(&cinfo);
    return;
  }
  if (cinfo.arith_code) {
    *status = "skip";
    jpeg_destroy_decompress(&cinfo);
    return;
  }
  cinfo.scale_num = pc->scale_num;
  cinfo.scale_denom = 8;
  cinfo.out_color_space = pc->out_cs;
  /* SkJpegCodec.cpp: CMYK and YCCK sources are decoded to CMYK, and Skia's swizzler converts them. */
  if (cinfo.jpeg_color_space == JCS_CMYK || cinfo.jpeg_color_space == JCS_YCCK) {
    cinfo.out_color_space = JCS_CMYK;
  }
  if (pc->out_cs == JCS_RGB565) cinfo.dither_mode = JDITHER_NONE;

  if (cinfo.progressive_mode) {
    /* SkJpegCodec.cpp#L508-L540: keep consuming input until it stops, then output the last
     * complete scan. */
    unsigned int last_scan = 0;
    cinfo.buffered_image = TRUE;
    jpeg_start_decompress(&cinfo);
    while (!jpeg_input_complete(&cinfo)) {
      int res = jpeg_consume_input(&cinfo);
      if (res == JPEG_SUSPENDED) break;
      if (res == JPEG_SCAN_COMPLETED) last_scan = cinfo.input_scan_number;
    }
    if (last_scan == 0) {
      *status = "suspended";
      jpeg_destroy_decompress(&cinfo);
      *hash = hs.h;
      return;
    }
    jpeg_start_output(&cinfo, (int)last_scan);
  } else if (!jpeg_start_decompress(&cinfo)) {
    *status = "suspended";
    jpeg_destroy_decompress(&cinfo);
    *hash = hs.h;
    return;
  }

  if (pc->crop_w) {
    unsigned int W = cinfo.output_width;
    unsigned int x = W * pc->crop_x / 16;
    unsigned int w = W * pc->crop_w / 16;
    if (w == 0) w = 1;
    if (x + w > W) w = W - x;
    jpeg_crop_scanline(&cinfo, &x, &w);
  }

  {
    size_t rowbytes = cinfo.out_color_space == JCS_RGB565
                          ? (size_t)cinfo.output_width * 2
                          : (size_t)cinfo.output_width * cinfo.output_components;
    rowbuf = (JSAMPARRAY)malloc(sizeof(JSAMPROW));
    rowbuf[0] = (JSAMPROW)calloc(rowbytes, 1);
    for (i = 0; i < pc->nops; i++) {
      const pop *op = &pc->ops[i];
      if (op->skip) {
        JDIMENSION got = jpeg_skip_scanlines(&cinfo, op->n);
        hash_u32(&hs, (uint32_t)got);
      } else {
        unsigned int want = op->n ? op->n : cinfo.output_height;
        unsigned int k;
        for (k = 0; k < want && cinfo.output_scanline < cinfo.output_height; k++) {
          unsigned int got = jpeg_read_scanlines(&cinfo, rowbuf, 1);
          if (got == 0) break;
          hash_bytes(&hs, rowbuf[0], rowbytes);
          total_rows += got;
        }
      }
    }
    free(rowbuf[0]);
    free(rowbuf);
    rowbuf = NULL;
  }
  *rows = total_rows;
  *hash = hs.h;
  if (cinfo.output_scanline == cinfo.output_height) {
    if (cinfo.progressive_mode) jpeg_finish_output(&cinfo);
    jpeg_finish_decompress(&cinfo);
    *status = "ok";
  } else {
    *status = "partial";
  }
  jpeg_destroy_decompress(&cinfo);
}

/* Operation lists. */
static const pop OPS_S1[] = {{1, 1}, {0, 0}};
static const pop OPS_S3[] = {{1, 3}, {0, 0}};
static const pop OPS_S7[] = {{1, 7}, {0, 0}};
static const pop OPS_S16[] = {{1, 16}, {0, 0}};
static const pop OPS_SFAR[] = {{1, 1000}, {0, 0}};
static const pop OPS_R1S8[] = {{0, 1}, {1, 8}, {0, 0}};
static const pop OPS_REP[] = {{1, 2}, {1, 2}, {1, 2}, {1, 2}, {0, 0}};
static const pop OPS_MID[] = {{0, 2}, {1, 5}, {0, 0}};
static const pop OPS_MULTI[] = {{0, 3}, {1, 1}, {0, 2}, {1, 9}, {0, 0}};
static const pop OPS_READ[] = {{0, 0}};
static const pop OPS_S3R[] = {{1, 3}, {0, 0}};
static const pop OPS_R1S4[] = {{0, 1}, {1, 4}, {0, 0}};
static const pop OPS_R2S5_CMYK[] = {{0, 2}, {1, 5}, {0, 0}};

#define NOPS(a) ((int)(sizeof(a) / sizeof((a)[0])))

static const pcase PCASES[] = {
    {"skip1/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_S1, NOPS(OPS_S1)},
    {"skip3/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_S3, NOPS(OPS_S3)},
    {"skip7/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_S7, NOPS(OPS_S7)},
    {"skip16/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_S16, NOPS(OPS_S16)},
    {"skipfar/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_SFAR, NOPS(OPS_SFAR)},
    {"skipadj/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_R1S8, NOPS(OPS_R1S8)},
    {"skiprep/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_REP, NOPS(OPS_REP)},
    {"skipmid/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmulti/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"crop-left/s8", 8, JCS_EXT_RGBA, 0, 8, OPS_READ, NOPS(OPS_READ)},
    {"crop-right/s8", 8, JCS_EXT_RGBA, 8, 8, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s8", 8, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s8", 8, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-tiny/s8", 8, JCS_EXT_RGBA, 15, 1, OPS_READ, NOPS(OPS_READ)},
    {"crop-clamp/s8", 8, JCS_EXT_RGBA, 12, 9, OPS_READ, NOPS(OPS_READ)},
    {"crop-full/s8", 8, JCS_EXT_RGBA, 0, 16, OPS_READ, NOPS(OPS_READ)},
    {"crop-skip/s8", 8, JCS_EXT_RGBA, 5, 6, OPS_S3R, NOPS(OPS_S3R)},
    {"crop-skipmid/s8", 8, JCS_EXT_RGBA, 3, 7, OPS_R1S4, NOPS(OPS_R1S4)},
    {"skipmid/s1", 1, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmid/s2", 2, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmid/s3", 3, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmid/s4", 4, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmid/s5", 5, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmid/s6", 6, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmid/s7", 7, JCS_EXT_RGBA, 0, 0, OPS_MID, NOPS(OPS_MID)},
    {"skipmulti/s1", 1, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"skipmulti/s2", 2, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"skipmulti/s3", 3, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"skipmulti/s4", 4, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"skipmulti/s5", 5, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"skipmulti/s6", 6, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"skipmulti/s7", 7, JCS_EXT_RGBA, 0, 0, OPS_MULTI, NOPS(OPS_MULTI)},
    {"crop-mid/s1", 1, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s2", 2, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s3", 3, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s4", 4, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s5", 5, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s6", 6, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-mid/s7", 7, JCS_EXT_RGBA, 5, 6, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s1", 1, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s2", 2, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s3", 3, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s4", 4, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s5", 5, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s6", 6, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-odd/s7", 7, JCS_EXT_RGBA, 3, 7, OPS_READ, NOPS(OPS_READ)},
    {"crop-skip/s1", 1, JCS_EXT_RGBA, 5, 6, OPS_S3R, NOPS(OPS_S3R)},
    {"crop-skip/s4", 4, JCS_EXT_RGBA, 5, 6, OPS_S3R, NOPS(OPS_S3R)},
    {"crop-skip/s6", 6, JCS_EXT_RGBA, 5, 6, OPS_S3R, NOPS(OPS_S3R)},
    {"gray-crop-skip/s8", 8, JCS_GRAYSCALE, 3, 7, OPS_R1S4, NOPS(OPS_R1S4)},
    {"cmyk-skip/s8", 8, JCS_EXT_RGBA, 0, 0, OPS_R2S5_CMYK, NOPS(OPS_R2S5_CMYK)},
};

static int cmp_names(const void *a, const void *b) {
  return strcmp(*(char *const *)a, *(char *const *)b);
}

int main(int argc, char **argv) {
  const char *dir = argc > 1 ? argv[1] : ".";
  DIR *d;
  struct dirent *ent;
  char **names = NULL;
  size_t n = 0, cap = 0, fi, c;
  size_t ncases = sizeof(PCASES) / sizeof(PCASES[0]);

  d = opendir(dir);
  if (!d) {
    fprintf(stderr, "cannot open %s\n", dir);
    return 2;
  }
  while ((ent = readdir(d)) != NULL) {
    const char *nm = ent->d_name;
    size_t l = strlen(nm);
    if ((l > 4 && strcmp(nm + l - 4, ".jpg") == 0) || (l > 5 && strcmp(nm + l - 5, ".jpeg") == 0)) {
      if (n == cap) {
        cap = cap ? cap * 2 : 64;
        names = (char **)realloc(names, cap * sizeof(char *));
      }
      names[n++] = strdup(nm);
    }
  }
  closedir(d);
  qsort(names, n, sizeof(char *), cmp_names);

  for (fi = 0; fi < n; fi++) {
    char path[4096];
    FILE *f;
    unsigned char *data;
    size_t len;
    struct stat st;
    snprintf(path, sizeof(path), "%s/%s", dir, names[fi]);
    f = fopen(path, "rb");
    if (!f) {
      fprintf(stderr, "cannot read %s\n", path);
      return 2;
    }
    if (fstat(fileno(f), &st) != 0) {
      return 2;
    }
    len = (size_t)st.st_size;
    data = (unsigned char *)malloc(len);
    if (fread(data, 1, len, f) != len) {
      return 2;
    }
    fclose(f);
    for (c = 0; c < ncases; c++) {
      const char *status;
      unsigned rows;
      uint64_t hash;
      run_partial(data, len, &PCASES[c], &status, &rows, &hash);
      printf("%s|%s|%s|%u|%016llx\n", names[fi], PCASES[c].name, status, rows,
             (unsigned long long)hash);
    }
    free(data);
  }
  for (fi = 0; fi < n; fi++) {
    free(names[fi]);
  }
  free(names);
  return 0;
}
