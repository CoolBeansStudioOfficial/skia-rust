# gm/emboss.cpp::EmbossGM

Ported 1:1 in `tests/gm/src/gm/emboss.rs` (registered, `#[ignore]`d). `smallemboss` in the same file
passes on every checkable tier.

Result with `gm-verify`: every config and tier mismatches (`8888` on the scalar tier and the
SSE/ML tiers, `565` and `f16` on all tiers), so the difference is not tier-specific.

The golden and our output differ in the text only (the circles and the stroked ring match):

- golden: "Hello" is blue (the `SkShaders::Color(SK_ColorBLUE)` shader) and "World" is green
  (`setColor(SK_ColorGREEN)` after `setShader(nullptr)`), both plain, with no emboss.
- ours: both words are drawn black, with no visible colour.

The `Hello` and `World` text is drawn after the stroked circle, with the emboss mask filter still set
on the paint (`setMaskFilter` is not cleared in the C++ either).

Hypothesis (not confirmed): text drawn with a non-blur mask filter (`SkEmbossMaskFilter`) takes a
different path in our port (glyph masks or path fallback) than Skia's `SkGlyphRunListPainter` /
`SkDraw::drawGlyphRunList`, and the colour and shader are lost. Next step: diff the rendered text
against the golden with `cargo run -p skia-rust-gm --bin gm-verify -- --match emboss::` and compare
the `target/gm-diffs` images. Finding the first divergence needs the oracle's raster-pipeline dump
(`cargo xtask oracle rp-dump`), which is not available on this host.
