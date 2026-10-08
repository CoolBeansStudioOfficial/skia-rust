/*
 * Calls libjpeg-turbo's inverse DCTs (jidctint.c / jidctred.c, C paths, 3.1.0) on a fixed
 * pseudo-random set of blocks and prints every output block. The Rust test
 * crates/skia-rust-libjpeg/tests/idct_c.rs regenerates the same inputs and compares.
 *
 * Input generator (both sides): 64-bit LCG, x = x * 6364136223846793005 + 1442695040888963407,
 * taking the high 32 bits. Each block: 64 coefficients, each zero with probability 1/2, else in
 * [-512, 511]; the 64 quantizers are in [1, 255].
 */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "jpeglib.h"

extern void jpeg_idct_islow(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_7x7(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_6x6(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_5x5(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_3x3(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_4x4(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_2x2(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);
extern void jpeg_idct_1x1(j_decompress_ptr, jpeg_component_info *, JCOEFPTR, JSAMPARRAY, JDIMENSION);

static uint64_t lcg_state = 0;
static uint32_t lcg(void) {
  lcg_state = lcg_state * 6364136223846793005ULL + 1442695040888963407ULL;
  return (uint32_t)(lcg_state >> 32);
}

/* prepare_range_limit_table, 8-bit case (jdmaster.c), copied: the table the IDCT indexes. */
static void build_range_limit(JSAMPLE *storage, JSAMPLE **out) {
  JSAMPLE *table = storage;
  int i;
  memset(table, 0, 5 * 256 + 128);
  table += 256; /* sample_range_limit */
  for (i = 0; i <= 255; i++) table[i] = (JSAMPLE)i;
  table += 128;
  for (i = 128; i < 512; i++) table[i] = 255;
  memset(table + 512, 0, 384);
  memcpy(table + 1024 - 128, table - 128, 128);
  *out = table - 128;
}

int main(void) {
  struct jpeg_decompress_struct cinfo;
  struct jpeg_error_mgr err;
  jpeg_component_info comp;
  JSAMPLE storage[5 * 256 + 128];
  JSAMPLE *srl;
  int size;
  int blk;
  int nblocks = 200;

  memset(&cinfo, 0, sizeof(cinfo));
  memset(&comp, 0, sizeof(comp));
  cinfo.err = jpeg_std_error(&err);
  build_range_limit(storage, &srl);
  cinfo.sample_range_limit = srl;

  for (size = 1; size <= 8; size++) {
    lcg_state = 12345 + size;
    comp.DCT_h_scaled_size = size;
    comp.DCT_v_scaled_size = size;
    for (blk = 0; blk < nblocks; blk++) {
      JCOEF coef[64];
      int dct_table[64];
      JSAMPLE outbuf[8][8];
      JSAMPLE *rows[8];
      int i, r, c;
      for (i = 0; i < 64; i++) {
        uint32_t v = lcg();
        coef[i] = (v & 1) ? (JCOEF)((int)((v >> 1) % 512) - 256) : 0;
        dct_table[i] = (int)(lcg() % 16) + 1;
      }
      for (r = 0; r < 8; r++) rows[r] = outbuf[r];
      memset(outbuf, 0, sizeof(outbuf));
      comp.dct_table = dct_table;
      switch (size) {
        case 1: jpeg_idct_1x1(&cinfo, &comp, coef, rows, 0); break;
        case 2: jpeg_idct_2x2(&cinfo, &comp, coef, rows, 0); break;
        case 3: jpeg_idct_3x3(&cinfo, &comp, coef, rows, 0); break;
        case 4: jpeg_idct_4x4(&cinfo, &comp, coef, rows, 0); break;
        case 5: jpeg_idct_5x5(&cinfo, &comp, coef, rows, 0); break;
        case 6: jpeg_idct_6x6(&cinfo, &comp, coef, rows, 0); break;
        case 7: jpeg_idct_7x7(&cinfo, &comp, coef, rows, 0); break;
        default: jpeg_idct_islow(&cinfo, &comp, coef, rows, 0); break;
      }
      printf("size %d block %d:", size, blk);
      for (r = 0; r < size; r++) {
        for (c = 0; c < size; c++) printf(" %d", outbuf[r][c]);
      }
      printf("\n");
    }
  }
  return 0;
}
