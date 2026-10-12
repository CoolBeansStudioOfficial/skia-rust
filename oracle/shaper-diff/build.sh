#!/bin/sh
# Builds the shaper-diff driver against HarfBuzz at Skia's pin (9cb1fee5, 13.1.0 plus one build-only
# commit; Skia's DEPS#L55) with the defines of Skia's third_party/harfbuzz/BUILD.gn#L28-L36 (HAVE_OT,
# HAVE_CONFIG_OVERRIDE_H, HB_NO_FALLBACK_SHAPE, HB_NO_WIN1256; no ICU, no FreeType) and Skia's
# config-override.h (cpp/). `-ffp-contract=off` keeps the float code as written.
#
#   sh oracle/shaper-diff/build.sh <harfbuzz-src-dir> <output-binary>
#
# <harfbuzz-src-dir> is the `src/` directory that `cargo xtask text-deps fetch` prints. The single
# translation unit harfbuzz.cc takes about 75 s on 4 cores.
set -eu
HB="$1"
OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
BUILD="$(dirname "$OUT")/shaper-diff-build"
mkdir -p "$BUILD"
FLAGS="-O2 -std=c++17 -ffp-contract=off -fno-exceptions -fno-rtti -DHAVE_OT -DHB_NO_FALLBACK_SHAPE -DHB_NO_WIN1256 -DHAVE_CONFIG_OVERRIDE_H -I$HERE/cpp -I$HB"
if [ ! -f "$BUILD/harfbuzz.o" ] || [ "$HB/harfbuzz.cc" -nt "$BUILD/harfbuzz.o" ]; then
    g++ $FLAGS -c "$HB/harfbuzz.cc" -o "$BUILD/harfbuzz.o"
fi
g++ $FLAGS -c "$HERE/cpp/shaper_diff.cc" -o "$BUILD/driver.o"
g++ -o "$OUT" "$BUILD/driver.o" "$BUILD/harfbuzz.o" -lpthread
echo "built $OUT"
