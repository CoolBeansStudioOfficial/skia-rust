/*
 * Differential harness for skia-rust-libjpeg: drives libjpeg-turbo 3.1.0 (the C library, built
 * with clang, no SIMD, JPEG_LIB_VERSION 80) through the calls Skia's SkJpegCodec makes, and prints
 * one line per case:
 *
 *   <file>|<case>|<status>|<rows>|<fnv1a-64 of the output bytes>
 *
 * The Rust side (crates/skia-rust-libjpeg/tests/diff.rs) prints the same lines from the port.
 * `expected/libjpeg.txt` is generated from this program and committed (text only).
 *
 * Source semantics mirror Skia's SkJpegSourceMgr for memory streams: init_source exposes the
 * whole buffer, fill_input_buffer reports exhaustion by returning FALSE (suspension), and
 * skip_input_data fails fatally past the end. Failures use longjmp to the caller, as Skia does.
 *
 * Usage: jpeg_diff <resources/images dir> [dump dir]
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

/* FNV-1a 64 over every output byte, in order. */
typedef struct {
  uint64_t h;
  FILE *dump;
} hasher;

static void hash_bytes(hasher *hs, const unsigned char *p, size_t n) {
  size_t i;
  for (i = 0; i < n; i++) {
    hs->h ^= p[i];
    hs->h *= 1099511628211ULL;
  }
  if (hs->dump) {
    fwrite(p, 1, n, hs->dump);
  }
}

typedef struct {
  const char *name;
  int scale_num;
  J_COLOR_SPACE out_cs;
  int raw;
} casedef;

static const char *status_for_failure = "err";

/* Runs one decode. Writes the status and the number of rows, and the hash. */
static void run_case(const unsigned char *data, size_t len, const casedef *cd, FILE *dump,
                     const char **status, unsigned *rows, uint64_t *hash) {
  struct jpeg_decompress_struct cinfo;
  mem_src src;
  err_mgr jerr;
  /* Static: read after longjmp, so it must not be an indeterminate automatic. */
  static hasher hs;
  static JSAMPARRAY rowbuf;
  unsigned int r;
  unsigned int total_rows = 0;
  int i;

  hs.h = 14695981039346656037ULL;
  hs.dump = dump;
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
    /* Failure: destroy the decompressor and report err, keeping the hash of what was written. */
    jpeg_destroy_decompress(&cinfo);
    *hash = hs.h;
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
    free(rowbuf);
    return;
  }
  /* Arithmetic coding is not decoded; progressive images are decoded through the buffered-image
   * path Skia uses (SkJpegCodec.cpp#L508-L540), and the raw YUV path skips them. */
  if (cinfo.arith_code || (cinfo.progressive_mode && cd->raw)) {
    *status = "skip";
    jpeg_destroy_decompress(&cinfo);
    free(rowbuf);
    return;
  }
  cinfo.scale_num = cd->scale_num;
  cinfo.scale_denom = 8;
  if (cd->raw) {
    cinfo.raw_data_out = TRUE;
  } else {
    cinfo.out_color_space = cd->out_cs;
    /* SkJpegCodec.cpp#L320-L330: RGB565 output is decoded with JDITHER_NONE. */
    if (cd->out_cs == JCS_RGB565) cinfo.dither_mode = JDITHER_NONE;
  }

  if (cd->raw) {
    /* Planes: per component, v_samp * DCTSIZE rows of width_in_blocks * DCTSIZE samples. */
    JSAMPIMAGE planes;
    int nrows[MAX_COMPONENTS];
    if (!jpeg_start_decompress(&cinfo)) {
      *status = "suspended";
      jpeg_destroy_decompress(&cinfo);
      *hash = hs.h;
      return;
    }
    planes = (JSAMPIMAGE)malloc(sizeof(JSAMPARRAY) * cinfo.num_components);
    for (i = 0; i < cinfo.num_components; i++) {
      jpeg_component_info *c = &cinfo.comp_info[i];
      int lines = c->v_samp_factor * DCTSIZE;
      int width = (int)c->width_in_blocks * DCTSIZE;
      int k;
      nrows[i] = lines;
      planes[i] = (JSAMPARRAY)malloc(sizeof(JSAMPROW) * lines);
      for (k = 0; k < lines; k++) {
        /* calloc: bytes the decoder does not write must read as zero, as in the Rust port. */
        planes[i][k] = (JSAMPROW)calloc((size_t)width, 1);
      }
    }
    while (cinfo.output_scanline < cinfo.output_height) {
      unsigned int got = jpeg_read_raw_data(&cinfo, planes, cinfo.max_v_samp_factor * DCTSIZE);
      int k;
      if (got == 0) {
        break;
      }
      for (i = 0; i < cinfo.num_components; i++) {
        int lines = nrows[i];
        int width = (int)cinfo.comp_info[i].width_in_blocks * DCTSIZE;
        for (k = 0; k < lines; k++) {
          hash_bytes(&hs, planes[i][k], (size_t)width);
        }
      }
      total_rows += got;
    }
    for (i = 0; i < cinfo.num_components; i++) {
      int k;
      for (k = 0; k < nrows[i]; k++) {
        free(planes[i][k]);
      }
      free(planes[i]);
    }
    free(planes);
    *rows = total_rows;
    *hash = hs.h;
    /* jpeg_read_raw_data advances by whole iMCU rows, so the end may overshoot the height. */
    if (cinfo.output_scanline >= cinfo.output_height) {
      jpeg_finish_decompress(&cinfo);
      *status = "ok";
    } else {
      *status = "partial";
    }
    jpeg_destroy_decompress(&cinfo);
    return;
  }

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
  {
    /* RGB565 rows are two bytes per pixel, whatever output_components says. */
    size_t rowbytes = cinfo.out_color_space == JCS_RGB565
                          ? (size_t)cinfo.output_width * 2
                          : (size_t)cinfo.output_width * cinfo.output_components;
    rowbuf = (JSAMPARRAY)malloc(sizeof(JSAMPROW));
    rowbuf[0] = (JSAMPROW)calloc(rowbytes, 1);
    while (cinfo.output_scanline < cinfo.output_height) {
      unsigned int got = jpeg_read_scanlines(&cinfo, rowbuf, 1);
      if (got == 0) {
        break;
      }
      hash_bytes(&hs, rowbuf[0], rowbytes);
      total_rows += got;
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
  (void)r;
}

static int cmp_names(const void *a, const void *b) {
  return strcmp(*(char *const *)a, *(char *const *)b);
}

int main(int argc, char **argv) {
  const char *dir = argc > 1 ? argv[1] : ".";
  const char *dumpdir = argc > 2 ? argv[2] : NULL;
  DIR *d;
  struct dirent *ent;
  char **names = NULL;
  size_t n = 0, cap = 0, fi;
  casedef cases[32];
  int ncases = 0;
  int s;

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

  for (s = 1; s <= 8; s++) {
    char buf[32];
    snprintf(buf, sizeof(buf), "rgba/s%d", s);
    cases[ncases].name = strdup(buf);
    cases[ncases].scale_num = s;
    cases[ncases].out_cs = JCS_EXT_RGBA;
    cases[ncases].raw = 0;
    ncases++;
  }
  cases[ncases] = (casedef){"bgra/s8", 8, JCS_EXT_BGRA, 0};
  ncases++;
  cases[ncases] = (casedef){"rgb/s8", 8, JCS_RGB, 0};
  ncases++;
  cases[ncases] = (casedef){"gray/s8", 8, JCS_GRAYSCALE, 0};
  ncases++;
  cases[ncases] = (casedef){"cmyk/s8", 8, JCS_CMYK, 0};
  ncases++;
  cases[ncases] = (casedef){"rgb565/s8", 8, JCS_RGB565, 0};
  ncases++;
  cases[ncases] = (casedef){"raw/s8", 8, JCS_UNKNOWN, 1};
  ncases++;

  for (fi = 0; fi < n; fi++) {
    char path[4096];
    FILE *f;
    unsigned char *data;
    size_t len;
    struct stat st;
    int c;
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
      FILE *dump = NULL;
      if (dumpdir) {
        char dp[4096];
        char cn[32];
        size_t k;
        for (k = 0; k < sizeof(cn) - 1 && cases[c].name[k]; k++) {
          cn[k] = cases[c].name[k] == '/' ? '_' : cases[c].name[k];
        }
        cn[k] = 0;
        snprintf(dp, sizeof(dp), "%s/%s.%s.bin", dumpdir, names[fi], cn);
        dump = fopen(dp, "wb");
      }
      /* Replace '/' in the case name for the dump file name. */
      run_case(data, len, &cases[c], dump, &status, &rows, &hash);
      if (dump) {
        fclose(dump);
      }
      printf("%s|%s|%s|%u|%016llx\n", names[fi], cases[c].name, status, rows, (unsigned long long)hash);
    }

    /* Truncations: 1/16 .. 15/16 of the file, decoded as rgba/s8. */
    {
      int k;
      casedef cd = {"rgba/s8", 8, JCS_EXT_RGBA, 0};
      for (k = 1; k <= 15; k++) {
        size_t tl = len * (size_t)k / 16;
        const char *status;
        unsigned rows;
        uint64_t hash;
        char label[128];
        snprintf(label, sizeof(label), "%s#trunc%d", names[fi], k);
        run_case(data, tl, &cd, NULL, &status, &rows, &hash);
        printf("%s|%s|%s|%u|%016llx\n", label, cd.name, status, rows, (unsigned long long)hash);
      }
      /* Corruptions: one byte XOR 0x5A at len*k/8. */
      for (k = 1; k <= 7; k++) {
        unsigned char *copy = (unsigned char *)malloc(len);
        size_t pos = len * (size_t)k / 8;
        const char *status;
        unsigned rows;
        uint64_t hash;
        char label[128];
        memcpy(copy, data, len);
        copy[pos] ^= 0x5A;
        snprintf(label, sizeof(label), "%s#flip%d", names[fi], k);
        run_case(copy, len, &cd, NULL, &status, &rows, &hash);
        printf("%s|%s|%s|%u|%016llx\n", label, cd.name, status, rows, (unsigned long long)hash);
        free(copy);
      }
    }
    free(data);
  }
  for (fi = 0; fi < n; fi++) {
    free(names[fi]);
  }
  free(names);
  return 0;
}
