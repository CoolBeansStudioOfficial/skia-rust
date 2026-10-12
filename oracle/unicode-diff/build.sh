#!/bin/sh
# Builds the unicode-diff driver against Chromium's ICU 78.2 at Skia's pin (chromium/deps/icu d578f2e8;
# Skia's DEPS#L57): every source/common/*.cpp, with the defines of Skia's third_party/icu/BUILD.gn#L66-L88
# (U_COMMON_IMPLEMENTATION, U_STATIC_IMPLEMENTATION, U_ENABLE_DYLOAD=0, U_DISABLE_RENAMING=1,
# U_USING_ICU_NAMESPACE=0). The data is not linked: the driver installs common/icudtl.dat with
# udata_setCommonData, as the pilot of docs/design/modules.md Appendix A did.
#
#   sh oracle/unicode-diff/build.sh <icu common dir> <output-binary>
#
# <icu common dir> is the `icu common:` line of `cargo xtask text-deps fetch --only icu`. The ~240
# translation units take about 150 s on 4 cores; objects are cached next to the binary and the
# directory (`<output-binary>-icu-obj`) can be deleted afterwards.
set -eu
COMMON="$1"
OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
OBJ="${OUT}-icu-obj"
mkdir -p "$OBJ"
FLAGS="-O1 -std=c++17 -ffp-contract=off -fPIC -w -DU_COMMON_IMPLEMENTATION -DU_STATIC_IMPLEMENTATION -DU_ENABLE_DYLOAD=0 -DU_DISABLE_RENAMING=1 -DU_USING_ICU_NAMESPACE=0 -I$COMMON"
JOBS="$(nproc 2>/dev/null || echo 4)"
export FLAGS OBJ
ls "$COMMON"/*.cpp | xargs -P "$JOBS" -I{} sh -c '
    src="{}"; o="$OBJ/$(basename "$src" .cpp).o"
    if [ ! -f "$o" ] || [ "$src" -nt "$o" ]; then g++ $FLAGS -c "$src" -o "$o"; fi'
g++ -O1 -std=c++17 -ffp-contract=off -DU_STATIC_IMPLEMENTATION -DU_DISABLE_RENAMING=1 -DU_USING_ICU_NAMESPACE=0 -I"$COMMON" -I"$HERE/cpp" -c "$HERE/cpp/unicode_diff.cc" -o "$OBJ/unicode_diff_driver.obj"
g++ -o "$OUT" "$OBJ"/*.o "$OBJ/unicode_diff_driver.obj" -lpthread
echo "built $OUT"
