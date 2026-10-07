# API mapping

Deviations from the reference API (rust-skia's `skia-safe`, see `docs/PORTING.md` §3), and non-mechanical names for symbols `skia-safe` doesn't expose. One row per symbol, grouped by module, kept sorted.

| Skia | skia-safe | skia-rust | Why |
|---|---|---|---|
| `SkColor` | `Color` (no raw accessor) | `Color` + `From<Color> for u32` | safe replacement for skia-safe's crate-private `into_native` |
| `SkColor4f::toSkColor` | `Color4f::to_color` (truncates) | `Color4f::to_color` (Skia rounding via `Sk4f_toL32`) | skia-safe's version differs from Skia; skia-rust ports Skia |
| `SkPMColor4f` / `SkRGBA4f<kPremul>` | none | `PMColor4f` | alpha-type template modelled as two concrete types sharing a macro |
| `SkPMColor` helpers | none | `color::{pm_color_set_argb, pm_color_get_*}` | mechanical rule; SkPMColor is RGBA (`SK_R32_SHIFT == 0`) |
| `SkUnPreMultiply` | none | `un_pre_multiply::{get_scale, apply_scale, pm_color_to_color}` | class with only statics becomes a module |
