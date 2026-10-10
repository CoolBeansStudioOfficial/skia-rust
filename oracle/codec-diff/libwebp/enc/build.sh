#!/bin/sh
# Builds the encoder reference (encode_lossless.c) against the pinned libwebp 1.4.0 sources
# (845d5476). Usage:
#   sh oracle/codec-diff/libwebp/enc/build.sh <libwebp-checkout> <outdir>
# Produces <outdir>/encode_lossless_c (portable C, SSE2 and SSE4.1 kernels compiled out) and
# <outdir>/encode_lossless_sse (the default x86-64 build: SSE2 on, SSE4.1 on with -msse4.1).
# Both use -ffp-contract=off, as the differential contract requires.
set -eu
SRC="$1"
OUT="$2"
HERE="$(cd "$(dirname "$0")" && pwd)"
mkdir -p "$OUT"
BASE="-O2 -ffp-contract=off -DWEBP_SWAP_16BIT_CSP=1 -I$SRC -I$SRC/src"
FILES="$(ls "$SRC"/src/enc/*.c "$SRC"/src/dsp/*.c "$SRC"/src/utils/*.c "$SRC"/sharpyuv/*.c | grep -v -E '_(mips|msa|neon)[0-9a-z_]*\.c$')"

build() {
  name="$1"
  shift
  objs=""
  for f in $FILES; do
    o="$OUT/$name-$(echo "$f" | sed "s#$SRC/##; s#[/.]#_#g").o"
    gcc $BASE "$@" -c "$f" -o "$o"
    objs="$objs $o"
  done
  gcc $BASE "$@" -c "$HERE/encode_lossless.c" -o "$OUT/$name-main.o"
  gcc -o "$OUT/$name" "$OUT/$name-main.o" $objs -lm
}

build encode_lossless_c -U__SSE2__ -U__SSE4_1__
build_main() {
  name="$1"
  main_src="$2"
  shift 2
  gcc $BASE "$@" -c "$HERE/$main_src" -o "$OUT/$name-main2.o"
  objs=""
  for o in "$OUT/$name"-*.o; do
    case "$o" in *-main.o|*-main2.o) ;; *) objs="$objs $o" ;; esac
  done
  gcc -o "$OUT/$name-methods" "$OUT/$name-main2.o" $objs -lm
}
build_main encode_lossless_c encode_lossless_methods.c -U__SSE2__ -U__SSE4_1__
build encode_lossless_sse -msse4.1
