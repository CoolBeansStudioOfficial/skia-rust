Paragraph test fonts for skia-rust (`--paragraph_fonts`), Skia chrome/m156.

A rebuild of Skia's `skparagraph` bot asset v4 (`infra/bots/assets/skparagraph/create.py`): the
`fonts/` directory of Rusino/textlayout at 9c1868e84da1db358807ebff5cf52327e53560a0, overlaid with
Skia's `resources/fonts` at 2f82ef6e77774dc4e8e382b2fb6159c58c0f8725 (without `abc/`, `svg/` and
`fonts.xml`), plus NotoNaskhArabic-Regular.ttf. Built by `tools/test-fonts/build.sh`.

One font of the bots' asset is not included: GoogleSans-Regular.ttf. Its own licence string says it
is not an open-source font, so it is not redistributed (only `SkParagraph_Intrinsic` asks for it).

The archive is a plain, reproducible ustar (sorted names, mtime 0, owner 0). Its SHA-256 and every
font's are in `inventory/test-fonts.lock`; the licences are listed in `NOTICE`.
