# HarfRust 0.9.0 as vendored by skia-rust

This directory is HarfRust 0.9.0 (https://github.com/harfbuzz/harfrust, MIT, see `LICENSE`) plus the
patches below. It exists because Skia's oracle shapes with HarfBuzz at its pin (`9cb1fee5`, 13.1.0),
and `oracle/shaper-diff` (docs/design/modules.md §2.4, §2.5) found differences that Skia's font
callbacks do not absorb. docs/design/modules.md decision 5 allows a vendored patch for exactly
that. `harfrust` in the workspace `Cargo.toml` is a path dependency on this directory, and the
directory is excluded from the workspace (it is not linted with the workspace lints).

Everything not listed here is upstream, byte for byte, except that `Cargo.toml` has no
`criterion` dev-dependency and `tests/`, `benches/` are not copied. Every changed place is marked
with `skia-rust patch` and the class name below.

`oracle/shaper-diff` proves the patches: with them, every row of modes b (Skia's callbacks) and c
(the empty face) is exact. `known-divergences.txt` keeps only the rows of mode a (HarfRust's own
font functions) that Skia's callbacks replace.

| Class | Why HarfBuzz differs from HarfRust 0.9.0 | Files |
|---|---|---|
| `fallback-mark-gap` | `hb-ot-shape-fallback.cc#L220` computes the gap for fallback mark positioning as `font->y_scale / 16`; HarfRust scaled `units_per_em / 16`, which truncates first. | `face.rs` (`Scale::y_scale`), `ot_shape_fallback.rs` (`position_mark`) |
| `fallback-space-advance` | `hb-ot-shape-fallback.cc#L574-L588` divides the font's scale (`(x_scale + n/2) / n`, `x_scale * 4 / 18`); HarfRust divided the upem and then scaled it. | `face.rs` (`Scale::x_scale`), `ot_shape_fallback.rs` (`_hb_ot_shape_fallback_spaces`) |
| `anchor-rounding` | HarfBuzz 13.1 keeps GPOS anchors as floats (`AnchorFormat*.hh get_anchor`: `em_fscale_x (xCoordinate)`, a variation delta as `em_scalef_x`), subtracts them and rounds the result (`MarkArray.hh#L57-L58`, `CursivePosFormat1.hh#L168-L214`); HarfRust scaled the font-unit difference. HarfBuzz's `roundf` is `floorf (x + .5f)` (`hb-algs.hh#L329-L337`), which differs from `f32::round` on negative halves. | `face.rs` (`Scale::fscale_*`, `scalef_*`, `hb_roundf`), `ot/mod.rs` (`resolve_anchor_scaled`), `ot/gpos/mark.rs`, `ot/gpos/cursive.rs` |
| `arabic-fallback-shaping` | `arabic_fallback_shape` was a stub. HarfBuzz builds fallback lookups from the Arabic Presentation Forms when the font has no Arabic GSUB (`hb-ot-shaper-arabic-fallback.hh`). Ported (without the Windows-1256 variant, which Skia builds out with `HB_NO_WIN1256`). | `ot_shaper_arabic_fallback.rs` (new), `ot_shaper_arabic_fallback_table.rs` (new, the data of `hb-ot-shaper-arabic-table.hh`), `ot_shaper_arabic.rs` (`do_fallback`, `arabic_fallback_shape`), `ot_map.rs` (`needs_fallback`) |

One addition is not a divergence fix: `harfrust::unicode_script (u32) -> Script` (`hb/mod.rs`,
`lib.rs`) exposes HarfBuzz's built-in script data, which Skia's script run iterator calls
(`SkShaper_harfbuzz.cpp#L433-L435`).

To move to a newer HarfRust, apply the same patches to it (search for `skia-rust patch`), then
`cargo test -p skia-rust-shaper-diff`.
