#!/bin/sh
# Builds webpdump against the pinned libwebp 1.4.0 decoder sources (845d5476) with clang, as the
# differential reference for skia-rust-libwebp. Usage:
#   sh oracle/codec-diff/libwebp/build.sh <libwebp-checkout> <output-binary>
# The checkout is the git clone of https://github.com/webmproject/libwebp at 845d5476 (the
# `DEPS` pin of Skia m156). Sources are compiled without SIMD (no WEBP_USE_* defines), with
# WEBP_SWAP_16BIT_CSP=1 as in Skia's build, and -ffp-contract=off.
set -eu
SRC="$1"
OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
OBJ="$(dirname "$OUT")/webpdump-obj"
mkdir -p "$OBJ"
CFLAGS="-O2 -ffp-contract=off -DWEBP_SWAP_16BIT_CSP=1 -I$SRC -I$SRC/src"
FILES="
src/dec/alpha_dec.c src/dec/buffer_dec.c src/dec/frame_dec.c src/dec/idec_dec.c
src/dec/io_dec.c src/dec/quant_dec.c src/dec/tree_dec.c src/dec/vp8_dec.c
src/dec/vp8l_dec.c src/dec/webp_dec.c
src/dsp/alpha_processing.c src/dsp/cpu.c src/dsp/dec.c src/dsp/dec_clip_tables.c
src/dsp/filters.c src/dsp/lossless.c src/dsp/rescaler.c src/dsp/upsampling.c src/dsp/yuv.c
src/utils/bit_reader_utils.c src/utils/color_cache_utils.c src/utils/filters_utils.c
src/utils/huffman_utils.c src/utils/quant_levels_dec_utils.c src/utils/random_utils.c
src/utils/rescaler_utils.c src/utils/thread_utils.c src/utils/utils.c src/utils/palette.c
src/dsp/alpha_processing_sse2.c src/dsp/dec_sse2.c src/dsp/filters_sse2.c src/dsp/lossless_sse2.c
src/dsp/rescaler_sse2.c src/dsp/upsampling_sse2.c src/dsp/yuv_sse2.c

src/dsp/alpha_processing_sse41.c src/dsp/dec_sse41.c src/dsp/upsampling_sse41.c src/dsp/yuv_sse41.c
src/dsp/lossless_sse41.c
src/demux/demux.c
"
OBJS=""
for f in $FILES; do
  o="$OBJ/$(echo "$f" | tr '/' '_').o"
  clang $CFLAGS -c "$SRC/$f" -o "$o"
  OBJS="$OBJS $o"
done
clang $CFLAGS -c "$HERE/webpdump.c" -o "$OBJ/webpdump.o"
clang -o "$OUT" "$OBJ/webpdump.o" $OBJS
