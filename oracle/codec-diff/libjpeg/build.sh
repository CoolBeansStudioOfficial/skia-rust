#!/bin/sh
# Builds oracle/codec-diff/libjpeg/jpeg_diff.c against the libjpeg-turbo revision Skia m156 pins
# (third_party/skia/DEPS: third_party/externals/libjpeg-turbo, chromium/deps/libjpeg_turbo.git @
# e14cbfaa85529d47f9f55b0f104a579c1061f9ad, libjpeg-turbo 3.1.0).
#
# Usage: build.sh <libjpeg-turbo checkout at that commit> <output dir> <resources/images dir>
#
# Skia's third_party/libjpeg-turbo/BUILD.gn settings for the x64 oracle: TURBO_FOR_WINDOWS,
# C_/D_ARITH_CODING_SUPPORTED=1, USE_CLZ_INTRINSIC, NO_GETENV, NO_PUTENV, and no WITH_SIMD (the
# SIMD sources are added only for non-Windows arm/arm64). The oracle ran on Windows x64, where
# `long` is 32 bits, and jpegint.h's `typedef long JLONG` is the 32-bit integer the arithmetic
# depends on. The copy of jpegint.h below uses `int` (same width) so that this Linux build matches.
# The checkout is not modified; the sources are copied into <output dir>/src.
set -eu
SRC=$1
OUT=$2
RES=$3
HERE=$(cd "$(dirname "$0")" && pwd)
CLANG=${CC:-clang}
mkdir -p "$OUT/src"
cp "$SRC"/src/*.c "$SRC"/src/*.h "$OUT/src/"
# JLONG_LONG=1 keeps `long` (64 bits on Linux) to check whether the width matters.
if [ -z "${JLONG_LONG:-}" ]; then
  sed -i 's/^typedef long JLONG;/typedef int JLONG;  \/* 32 bits, as `long` on Windows x64 *\//' "$OUT/src/jpegint.h"
  grep -q '^typedef int JLONG;' "$OUT/src/jpegint.h"
fi
# The sources of Skia's libjpeg_sources list that the 8-bit library uses.
SOURCES="jaricom jcapimin jcapistd jcarith jccoefct jccolor jcdctmgr jcdiffct jchuff jcicc
jcinit jclhuff jclossls jcmainct jcmarker jcmaster jcomapi jcparam jcphuff jcprepct jcsample
jctrans jdapimin jdapistd jdarith jdatadst jdatasrc jdcoefct jdcolor jddctmgr jddiffct jdhuff
jdicc jdinput jdlhuff jdlossls jdmainct jdmarker jdmaster jdmerge jdphuff jdpostct jdsample
jdtrans jerror jfdctflt jfdctfst jfdctint jidctflt jidctfst jidctint jidctred jmemmgr jmemnobs
jpeg_nbits jquant1 jquant2 jutils"
# BUILD.gn's libjpeg16_sources, and libjpeg12_sources (the 16 list plus the DCT and quantizers).
SOURCES_16="jcapistd jccolor jcdiffct jclossls jcmainct jcprepct jcsample jdapistd jdcolor
jddiffct jdlossls jdmainct jdpostct jdsample jutils"
SOURCES_12="$SOURCES_16 jccoefct jcdctmgr jdcoefct jddctmgr jdmerge jfdctfst jfdctint jidctflt
jidctfst jidctint jidctred jquant1 jquant2"
OBJS=""
for s in $SOURCES; do
  $CLANG -O2 -ffp-contract=off -std=c99 -w -c \
    -DNO_GETENV -DNO_PUTENV -DTURBO_FOR_WINDOWS -DC_ARITH_CODING_SUPPORTED=1 \
    -DD_ARITH_CODING_SUPPORTED=1 -DUSE_CLZ_INTRINSIC -DBITS_IN_JSAMPLE=8 \
    -I"$OUT/src" -o "$OUT/$s.o" "$OUT/src/$s.c"
  OBJS="$OBJS $OUT/$s.o"
done
# Skia also links the 12- and 16-bit copies of the sources (BUILD.gn's libjpeg12/libjpeg16); the
# 8-bit controllers call their mangled entry points (j12init_*, j16init_*), so they must exist.
for bits in 12 16; do
  if [ "$bits" = 12 ]; then LIST=$SOURCES_12; else LIST=$SOURCES_16; fi
  for s in $LIST; do
    $CLANG -O2 -ffp-contract=off -std=c99 -w -c \
      -DNO_GETENV -DNO_PUTENV -DTURBO_FOR_WINDOWS -DBITS_IN_JSAMPLE=$bits \
      -I"$OUT/src" -o "$OUT/b$bits-$s.o" "$OUT/src/$s.c"
    OBJS="$OBJS $OUT/b$bits-$s.o"
  done
done
for prog in jpeg_diff partial_diff encode_diff; do
  $CLANG -O2 -ffp-contract=off -std=gnu99 -w -DNO_GETENV -DNO_PUTENV -DTURBO_FOR_WINDOWS \
    -DC_ARITH_CODING_SUPPORTED=1 -DD_ARITH_CODING_SUPPORTED=1 -DUSE_CLZ_INTRINSIC \
    -I"$OUT/src" -o "$OUT/$prog" "$HERE/$prog.c" $OBJS
done
$CLANG -O2 -ffp-contract=off -std=gnu99 -w -DNO_GETENV -DNO_PUTENV -DTURBO_FOR_WINDOWS \
  -DC_ARITH_CODING_SUPPORTED=1 -DD_ARITH_CODING_SUPPORTED=1 -DUSE_CLZ_INTRINSIC \
  -I"$OUT/src" -o "$OUT/idct_check" "$HERE/idct_check.c" $OBJS
# expected/libjpeg.txt: the whole-image cases; expected/partial.txt: jpeg_crop_scanline and
# jpeg_skip_scanlines (partial_diff.c); expected/idct.txt: the C inverse DCTs (idct_check.c).
# The Rust tests check all three files.
"$OUT/jpeg_diff" "$RES" > "$OUT/libjpeg.txt"
"$OUT/partial_diff" "$RES" > "$OUT/partial.txt"
"$OUT/idct_check" > "$OUT/idct.txt"
echo "wrote $OUT/libjpeg.txt ($(wc -l < "$OUT/libjpeg.txt") lines)"
echo "wrote $OUT/partial.txt ($(wc -l < "$OUT/partial.txt") lines)"
# expected/encode.txt: the compressor cases (encode_diff.c), which take no resources.
"$OUT/encode_diff" > "$OUT/encode.txt"
echo "wrote $OUT/idct.txt ($(wc -l < "$OUT/idct.txt") lines)"
echo "wrote $OUT/encode.txt ($(wc -l < "$OUT/encode.txt") lines)"
