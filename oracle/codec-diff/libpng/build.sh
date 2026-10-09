#!/bin/sh
# Builds pngwdump against the pinned libpng sources (libpng 1.6.56, skia.googlesource.com/
# third_party/libpng@d5515b5b, the source list of Skia's third_party/libpng/BUILD.gn) and the
# pinned Chromium zlib (zlib@646b7f56, the same sources as oracle/codec-diff/zlib/build.sh). Usage:
#   sh oracle/codec-diff/libpng/build.sh <libpng-checkout> <zlib-checkout> <output-binary>
# The configuration is scripts/pnglibconf.h.prebuilt (PNG_Z_DEFAULT_STRATEGY 1 with filters and 0
# without, PNG_ZBUF_SIZE 8192, all write features on, PNG_USE_ABS undefined). The read sources are
# linked only so the library is complete: the harness calls the write API. The sources are compiled
# without SIMD (the write path has none), and zlib without gzip, as in the zlib harness.
set -eu
PNG="$1"
ZLIB="$2"
OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
BUILD="$(dirname "$OUT")/libpng-build"
mkdir -p "$BUILD/conf" "$BUILD/zlib-stub"
cp "$PNG/scripts/pnglibconf.h.prebuilt" "$BUILD/conf/pnglibconf.h"
: > "$BUILD/zlib-stub/cpu_features.h"
clang -O2 -ffp-contract=off -DNO_GZIP -DCHROMIUM_ZLIB_NO_CHROMECONF \
    -I"$BUILD/conf" -I"$PNG" -I"$ZLIB" -I"$BUILD/zlib-stub" -o "$OUT" \
    "$HERE/pngwdump.c" \
    "$PNG/png.c" "$PNG/pngerror.c" "$PNG/pngget.c" "$PNG/pngmem.c" "$PNG/pngset.c" \
    "$PNG/pngtrans.c" "$PNG/pngwio.c" "$PNG/pngwrite.c" "$PNG/pngwtran.c" "$PNG/pngwutil.c" \
    "$ZLIB/deflate.c" "$ZLIB/trees.c" "$ZLIB/zutil.c" "$ZLIB/adler32.c" "$ZLIB/crc32.c" \
    -lm
echo "built $OUT"
