#!/bin/sh
# Builds the `test-fonts-m156` release asset (inventory/test-fonts.lock): a rebuild of Skia's
# `skparagraph` bot asset, version 4, exactly as infra/bots/assets/skparagraph/create.py makes it
# (that script at the pin is the reference):
#   1. the `fonts/` directory of Rusino/textlayout at 9c1868e84da1db358807ebff5cf52327e53560a0,
#   2. over it Skia's `resources/fonts` at 2f82ef6e77774dc4e8e382b2fb6159c58c0f8725, without
#      `abc/`, `svg/` and `fonts.xml`,
#   3. NotoNaskhArabic-Regular.ttf (SHA-256 checked against create.py's ARABIC_SHA256).
# Differences from the bots' asset: GoogleSans-Regular.ttf is left out (its licence forbids
# redistribution; see the [excluded] table of the lock), and the result is a plain, reproducible
# tar (sorted names, mtime 0, owner 0) instead of a CIPD package.
#
# Usage: sh tools/test-fonts/build.sh <work-dir> [<output-dir>]
# Needs git, curl, tar (GNU tar: --sort), sha256sum. Prints the archive's SHA-256, which must equal
# the one in inventory/test-fonts.lock. Publishing (maintainer only):
#   gh release create test-fonts-m156 <output-dir>/test-fonts-m156.tar --repo CoolBeansStudioOfficial/skia-rust \
#     --title test-fonts-m156 --notes-file tools/test-fonts/RELEASE_NOTES.md
set -eu
WORK="$1"
OUT="${2:-$WORK/out}"
TEXTLAYOUT_REV=9c1868e84da1db358807ebff5cf52327e53560a0
SKIA_REV=2f82ef6e77774dc4e8e382b2fb6159c58c0f8725
ARABIC_URL='https://fonts.gstatic.com/s/notonaskharabic/v33/RrQ5bpV-9Dd1b1OAGA6M9PkyDuVBePeKNaxcsss0Y7bwvc5krK0z9_Mnuw.ttf'
ARABIC_SHA256=b957e8c71a24e50c1aad4df775c46282bbe5e62e2b2b2ca72b153d75b6a15edd

mkdir -p "$WORK" "$OUT"
rm -rf "$WORK/textlayout" "$WORK/skia" "$WORK/fonts"
mkdir "$WORK/textlayout" "$WORK/skia" "$WORK/fonts"

fetch_rev() { # dir url rev
    git -C "$1" init -q
    git -C "$1" remote add origin "$2"
    git -C "$1" fetch -q --depth 1 origin "$3"
    git -C "$1" checkout -q FETCH_HEAD
}
fetch_rev "$WORK/textlayout" https://github.com/Rusino/textlayout "$TEXTLAYOUT_REV"
fetch_rev "$WORK/skia" https://skia.googlesource.com/skia/ "$SKIA_REV"

cp -r "$WORK/textlayout/fonts/." "$WORK/fonts/"
cp -r "$WORK/skia/resources/fonts/." "$WORK/fonts/"
rm -rf "$WORK/fonts/abc" "$WORK/fonts/svg" "$WORK/fonts/fonts.xml"
curl -fsSL -o "$WORK/fonts/NotoNaskhArabic-Regular.ttf" "$ARABIC_URL"
echo "$ARABIC_SHA256  $WORK/fonts/NotoNaskhArabic-Regular.ttf" | sha256sum -c -
rm -f "$WORK/fonts/GoogleSans-Regular.ttf"
chmod 644 "$WORK"/fonts/*

(cd "$WORK/fonts" && LC_ALL=C tar --sort=name --mtime=@0 --owner=0 --group=0 --numeric-owner \
    --format=ustar -cf "$OUT/test-fonts-m156.tar" .)
sha256sum "$OUT/test-fonts-m156.tar"
