# SkParagraphTest.cpp::SkParagraph_EmojiParagraph

Status: `todo`, the port is `#[ignore]`d (`tests/src/modules/skparagraph/sk_paragraph_test.rs`).

## What fails

Under `FontConfig::NativeFontations` (the only configuration the paragraph tests run in, `docs/design/modules.md` §4),
with the fonts of `test-fonts-m156` (`SKIA_RUST_PARAGRAPH_FONTS=release`):

```
SkParagraph_EmojiParagraph: 7 failure(s)
  line.width() == 998.25 || (998.1 < line.width() && line.width() < 998.2): width: 996.09375   (x7, every full line)
```

`impl->lines().size() == 8`, `unresolvedGlyphs() == 0` and `line.height() == 59` pass. Each full line holds 16 emoji, so the
advance of one "Noto Color Emoji" glyph at size 50 is 996.09375 / 16 = 62.255859375 = 50 * 2550 / 2048.

## Where the expectation comes from

The C++ comment says: "The actual value is 50_size / 109_ppemX * 136_advance = ~62.385319; FreeType reports advances in 24.6
fixed point, so each is 62.390625". That is the advance of the CBDT bitmap strike (109 ppem, advance 136) that FreeType scales,
i.e. 16 * 62.390625 = 998.25. The paragraph code does not compute it: the run advances are the shaper's advances (HarfBuzz via
HarfRust in `skia-rust-shaper`) over the glyph advances of the Fontations typeface (`skia-rust-text`), and Fontations
(skrifa) scales the outline advance (2550 of 2048 units per em) instead of the strike's.

## Hypothesis

The assertion is specific to the FreeType port: Skia's own `NativeFonts_Fontations` bot would see 996.09375 as well. No oracle
run was possible (the oracle host is gone), so this is not confirmed. If a Fontations-backed C++ run gives 998.25, the
difference is in `skia-rust-text`'s advance for bitmap-only (CBDT) glyphs, not in `skia-rust-paragraph`.

To continue: run the C++ test under `NativeFonts_Fontations` (or dump `SkFont::getWidths` of glyph U+1F600 of
`NotoColorEmoji.ttf` at size 50 there) and compare with `Font::get_widths_bounds` of the Rust typeface.
