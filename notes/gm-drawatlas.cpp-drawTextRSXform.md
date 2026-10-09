# gm/drawatlas.cpp :: drawTextRSXform (failing)

Status: `failing`, registration ignored. Port: `tests/gm/src/gm/drawatlas.rs`.

## What the GM does
Draws the 24 glyphs of "ABCDFGHJKLMNOPQRSTUVWXYZ" (100 pt, default portable font) along an oval,
each glyph placed by an `RSXform` built from `SkPathMeasure::getPosTan` at the glyph centre, then
drawn as an RSX text blob (`SkTextBlob::MakeFromRSXformGlyphs`) with a `Mirror` linear gradient
shader (red to blue, 0..220). Twice, once filled and once stroked (2.25, round join).

## What was tried
1. Full port (text blob built with the new `TextBlob::from_rsxform_glyphs`, which mirrors
   `SkTextBlob::MakeFromRSXformGlyphs`). All 45 comparisons mismatch on every tier.
2. Debug output: the blob is built (`blob=true`), the transforms look right (for the first glyph
   `scos -0.1229, ssin 0.9924, tx 707.0, ty 430.7`), and the oval outline matches the golden.
3. Diff image (scalar, 565): the golden shows the glyphs in the gradient; ours shows **no glyphs at
   all** (only the oval and the stroke-rect outline).
4. Experiment: the same blob with the gradient shader removed (`paint` default, black). The glyphs
   then appear in the right places (diff image shows the shapes match, colours differ as expected).
   So the RSX positioning and glyph outlines are right; the shader on the glyphs is what vanishes.

## Hypothesis
Text drawn with a mirror-tiled linear-gradient shader is not rasterised (the glyph coverage is
composed with a shader that yields transparent or is culled). Candidates:
- `Shader` for text blobs in `skia-rust-raster` (glyph mask blitting with a shader paint) for a
  shader whose local matrix is identity and whose gradient spans x in 0..220 while the glyphs sit
  at x in 0..700 (the Mirror tile mode must repeat there).
- The mirror tile path itself (`TileMode::Mirror` in the gradient stage) for 1D gradients.
Check first: a plain filled `draw_str` at the same positions with the same shader, then the same
shader with `TileMode::Clamp`. Compare against `cpu-x64-scalar` goldens (`oracle/README.md`).

Not yet checked: whether `SkPathMeasure` positions differ (the rect and oval match, so probably not).
