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
