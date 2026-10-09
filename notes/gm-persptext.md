# gm/persptext.cpp::PerspTextGM(false) and PerspTextGM(true)

Status: passing (`tests/gm/src/gm/persptext.rs`), both configs, every tier.

What was known before this attempt: 17 pixels differed (max channel 5) under perspective, and the
slice-5 port was not on the branch.

Root cause (found in this attempt)
- Every glyph of the GM takes the path branch of the CPU glyph painter (`SkGlyphRunPainter.cpp`,
  `ShouldDrawAsPath` is true under perspective), and `needsExactCTM` is false, so the painter does
  `canvas->concat(m); canvas->drawPath(path, paint)`.
- `SkCanvas::concat(const SkMatrix&)` builds an `SkM44` and calls `fMCRec->fMatrix.preConcat(m)`,
  so the CTM is composed in `SkM44`'s float arithmetic. The device then takes `asM33()` of that.
- The port composed the CTM with the 3x3 `Matrix::concat`, whose products are computed in double
  (`muladdmul`) and rounded once. Under perspective those rounding differences move a few
  anti-aliased path edges by one ulp, which is the 17-pixel difference.
- Fix: `crates/skia-rust-raster/src/draw.rs`, `draw_glyph_path_concat`, composes with
  `M44::concat(M44::from(ctm), M44::from(m)).to_m33()`. Checked both ways: with the 3x3 composition
  both configs fail (0/2), with the M44 composition both pass (2/2). All other text GMs still match.
