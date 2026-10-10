# ScaledEmojiPerspectiveGM(Test)

Status: failing on the 565 and f16 configs only; 8888 matches on every tier.

Mismatch (565, scalar tier): ours 848b8bc8a473..., golden d0fe24b7f782... (all tiers differ alike).
The diff image (target/gm-diffs/cpu-x64-scalar/565/scaledemojiperspective_test.png) shows only a
few pixels differing, on the two or three smallest glyphs near the centre of the perspective star.
Non-perspective emoji GMs (scaledemoji/pos/coloremoji/blendmodes, Test) match on all configs, so
the ARGB32 sprite path and the drawable deferral are fine.

Hypothesis: under perspective the glyphs are drawn as drawables (SVG paths) through
Canvas::finish_glyph_run_draw (save_layer + drawable.draw). The deviation from C++ is that the
layer/draw happens after the device call; for 565/f16 destinations the layer or the AA path blit of
very small perspective-transformed paths rounds differently. Next step: compare the
SkGlyphRunPainter drawable branch (matrix = position * strike scale, saveLayer bounds) with
PendingGlyphDraw, and dump the path verbs of the smallest glyph.

## Attempt 2 (port/svg-core2)

The draw order was redesigned to Skia's: `GlyphRunListPainter::draw_for_bitmap_device` now returns at
each drawable (`PendingGlyphDrawable`) with a `GlyphRunDrawCursor`, the canvas draws the
`saveLayer(bounds, paint); drawable->draw(m)` and calls `Device::on_draw_glyph_run_list_step` again
(path glyphs, then the run's drawables, then masks, as in SkGlyphRunPainter.cpp#L296-L322).
The hashes of 565 and f16 did not change at all (565 scalar still 848b8bc8..., golden d0fe24b7...),
so the order was not the cause. Facts established:

- The Test font (TestSVGTypeface "Emoji") draws through the drawable stage under perspective:
  every run has path acc=0 and drawable acc=2 (instrumented count: 1890 drawable stages over the
  scaledemoji GMs); the bitmap fallback stage is never reached.
- The 297 differing 565 pixels are single-LSB differences in one channel (e.g. g 9 vs 8, r 12 vs
  16 differ by one step per channel) on edge pixels of the two smallest glyphs near (642..675,
  663..690), not a geometry difference.
- The layer of a 565 canvas is N32 (image_filter_color_type) and of an f16 canvas F16, both
  restored onto the destination through RasterPipelineSpriteBlitter (ChooseL32 only handles an N32
  destination), which is the one thing 8888 (ChooseL32 / memcpy) does not share with 565/f16.
  Code review of choose_sprite / RasterPipelineSpriteBlitter::setup / drawSpecial against
  SkBlitter_Sprite.cpp / SkBitmapDevice.cpp found no deviation.
- Remaining hypothesis: the oracle's N32 is BGRA, and a BGRA layer composited onto 565/f16
  (lowp vs highp selection for load_bgra/srcover/store_565) rounds differently from the RGBA layer
  of this Linux host, or the sprite pipeline picks lowp on the oracle and highp here. Next step: dump
  the stage list and lowp/highp choice of the restore blit (needs an oracle dump; the oracle host is
  gone), or force a BGRA layer in the test harness to compare.
