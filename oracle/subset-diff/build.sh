#!/bin/sh
# Builds the subset-diff driver against the pinned HarfBuzz (9cb1fee5, 13.1.0 + one build-only commit;
# Skia's DEPS#L55) with the defines of Skia's third_party/harfbuzz/BUILD.gn (HAVE_OT, HB_NO_FALLBACK_SHAPE,
# HB_NO_WIN1256, HAVE_CONFIG_OVERRIDE_H; no ICU, no FreeType). Usage:
#   sh oracle/subset-diff/build.sh <harfbuzz-checkout> <skia-checkout> <output-binary>
set -eu
HB="$1"
SKIA="$2"
OUT="$3"
HERE="$(cd "$(dirname "$0")" && pwd)"
BUILD="$(dirname "$OUT")/subset-diff-build"
mkdir -p "$BUILD"
FLAGS="-O2 -std=c++17 -ffp-contract=off -fno-exceptions -fno-rtti -DHAVE_OT -DHB_NO_FALLBACK_SHAPE -DHB_NO_WIN1256 -DHAVE_CONFIG_OVERRIDE_H -I$SKIA/third_party/harfbuzz -I$HB/src"
g++ $FLAGS -c "$HB/src/harfbuzz-subset.cc" -o "$BUILD/hbs.o"
gcc -O2 -I"$HB/src" -c "$HERE/subset_diff.c" -o "$BUILD/driver.o"
g++ -o "$OUT" "$BUILD/driver.o" "$BUILD/hbs.o" -lpthread
echo "built $OUT"
