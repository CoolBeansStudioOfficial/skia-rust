#!/bin/sh
# Builds zlibdump against the pinned Chromium zlib sources (zlib@646b7f56, 1.3.0.1-motley) with
# clang, as the differential reference for skia-rust-zlib's deflate. Usage:
#   sh oracle/codec-diff/zlib/build.sh <zlib-checkout> <output-binary>
# The checkout is https://chromium.googlesource.com/chromium/src/third_party/zlib at 646b7f56 (the
# `DEPS` pin of Skia m156), with its contrib/optimizations/insert_string.h. Sources are compiled
# without SIMD (no ADLER32_SIMD/CRC32_SIMD defines), without chromeconf.h (the symbol prefixes do
# not change output), and without gzip (NO_GZIP: the port has no gzip framing, and zlib-format
# output does not depend on it). cpu_features.h is empty, since the scalar build does not use it.
set -eu
SRC="$1"
OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
STUB="$(dirname "$OUT")/zlib-stub"
mkdir -p "$STUB"
: > "$STUB/cpu_features.h"
clang -O2 -ffp-contract=off -DNO_GZIP -DCHROMIUM_ZLIB_NO_CHROMECONF -I"$SRC" -I"$STUB" -o "$OUT" \
    "$HERE/zlibdump.c" "$SRC/deflate.c" "$SRC/trees.c" "$SRC/zutil.c" "$SRC/adler32.c"
echo "built $OUT"
