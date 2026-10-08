# Design: Text (Phase 5)

Status: proposed (2026-10-08). Pin: `chrome/m156` (`95ee33d7ec77`). Audience: every agent porting
Skia's text stack (typefaces, fonts, scaler contexts, strikes, text blobs, the CPU glyph painter),
the test font tooling, the Fontations backend, and the text GM sweeps.

All Skia references are `path#Lx-Ly` in the pinned tree (`third_party/skia`). Manifest counts are
from `inventory/manifest.toml` at `6d79587`; golden facts are from `hashes-m156.json`
(`inventory/goldens.lock`).

## Decisions at a glance

1. **The goldens were rendered with Skia's portable test fonts, not Fontations.** The oracle builds
   use default GN args (`skia_use_fontations = false`, `gn/skia.gni#L59`) and DM runs with
   `--nativeFonts false` (`xtask/src/oracle.rs#L525`), which is also the config of Skia's own
   default bots (`--nonativeFonts`, `infra/bots/gen_tasks_logic/dm_flags.go#L1820-L1828`). So
   every text golden comes from pure Skia code: `TestTypeface` path glyphs, `TestSVGTypeface` color
   glyphs, `SkCustomTypeface`, or the empty typeface. Everything that renders them is in the pinned
   source, so text GMs can match exactly with no new oracle run (§1).
2. **Resource fonts are `None` in the GM configuration.** The portable `SkFontMgr` returns
   `nullptr` from every `makeFrom*` (`tools/fonts/TestFontMgr.cpp#L142-L154`), so
   `ToolUtils::CreateTypefaceFromResource` was null in every golden. Those GMs skipped
   (`colrv1`, `palette`, non-`Test` emoji formats: no golden) or drew with the empty typeface. Our
   test tooling reproduces this exactly. Skip-matching GMs pass by "both skipped" (§1.2).
3. **The text engine goes in `skia-rust-core`, the typeface backends in `skia-rust-text`.** This
   revises PLAN §3.1. `SkFont`, `SkTypeface`, `SkScalerContext`, `SkStrike*`, `SkGlyph` and
   `SkTextBlob` live in Skia's `src/core`, and `Canvas::draw_str` and `Font::measure_str` need
   them. The one raster dependency of the engine (rasterizing a glyph path into a mask,
   `SkScalerContext::GenerateImageFromPath`) is a trait seam that the raster crate implements and
   backends hand to the scaler context (§3.2).
4. **Fontations is a real dependency at Skia's exact pins:** `skrifa = "=0.43.2"`,
   `read-fonts = "=0.40.1"`, `font-types = "=0.12.0"` (`bazel/external/fontations/Cargo.toml`,
   `MODULE.bazel.lock`). All three are on crates.io (MIT OR Apache-2.0, MSRV 1.85). We call them
   directly. Skia's own Rust bridge (`src/ports/fontations/src/*.rs`) is ported as plain Rust
   modules without `cxx`, and `SkTypeface_fontations.cpp` is ported function by function (§2.2).
5. **FreeType, FontConfig, Android, DirectWrite/GDI and CoreText are out of scope.** Their tests
   (81 entries) are excluded with a reason. No golden was rendered with them, because portable fonts
   bypass the native backends (§2.3).
6. **Fontations pixels cannot be verified until a Fontations oracle exists.** Its unit tests
   (`FontationsTest`, `FontScanner_Fontations`, Fontations-config runs of `TypefaceTest`/
   `FontMgrTest`) gate it now. The 6 `fontations*.cpp` GMs stay `todo` with the reason
   `needs-oracle: no golden (oracle built with skia_use_fontations=false)` (§7.3).
7. **One process-wide strike cache, by necessity.** `StrikeCache::global()` is a `LazyLock`
   holding a `Mutex`-guarded cache, like `SkStrikeCache::GlobalStrikeCache()`
   (`src/core/SkStrikeCache.cpp#L35`). This is the one sanctioned exception to "no global mutable
   state": the cache memoizes a pure function of the descriptor, so its contents never change a
   result. Every Skia entry point that takes an explicit `SkStrikeCache*` keeps that parameter
   (§5.2).
8. **Ownership:** `Typeface` is a cheap-clone handle over `Arc<dyn TypefaceBase>`, like `Shader`.
   Strikes are `Arc<Strike>` with an internal `Mutex`. Glyph masks are blitted while the strike lock
   is held. Paths and drawables are cloned out before they are drawn. No static-initializer
   registration: typeface decoders are an explicit list (§5).
9. **The CPU draw path is a straight port** of `SkCanvas::onDrawGlyphRunList` → `SkDevice` →
   `skcpu::GlyphRunListPainter::drawForBitmapDevice` → `skcpu::Draw::paintMasks`, onto the D3/D4
   blitters that already handle A8, LCD16 and 3D masks. ARGB32 color glyphs go through
   `drawSprite`. SDF and Slugs are GPU-only: on raster `convertGlyphRunListToSlug` returns null
   (§6).
10. **Test fonts live in a new test-only crate, `tests/tools` (`skia-rust-tools`).** It holds
    `TestTypeface` (data generated from the `.inc` files), the portable `FontMgr`, `FontToolUtils`
    and later `TestSVGTypeface`. Both the unit-test and GM crates use it. GMs always run the portable
    configuration. Unit tests that call `TestFontMgr()` run in both configurations Skia's bots use,
    portable and native-Fontations (§8).
11. **The first wave is about 40 rendered GMs plus 76 skip-matching ones.** It needs no `TextBlob`:
    portable typeface, `drawString`, the strike machinery and the CPU painter (§9, waves T-A/T-B).
    `TextBlob` adds about 17. `TestSVGTypeface` (blocked on an SVG DOM) adds about 13. About 330
    more text GM entries wait on Phase 3/4 features (gradients, images, blur, image filters) and
    unlock when those land.

---

## 1. What the goldens contain

### 1.1 Oracle configuration (facts from the pinned sources and the oracle recipe)

| Fact | Source | Consequence |
|---|---|---|
| `skia_use_fontations` not set; default `false` | `oracle/tiers.toml` `[gn] args`; `gn/skia.gni#L59` | No Fontations code in the oracle. `gm/fontations*.cpp` are not even compiled (`gn/gm.gni#L421-L426`) |
| `skia_use_freetype = is_android \|\| is_linux \|\| is_canvaskit`, so `false` on Windows | `gn/skia.gni#L61` | No FreeType |
| `skia_enable_fontmgr_win = is_win` (DirectWrite compiled in) | `gn/skia.gni#L27` | Only reachable via `--nativeFonts`, which the oracle did not pass |
| DM runs with `--nativeFonts false --resourcePath resources` | `xtask/src/oracle.rs#L515-L525` | `ToolUtils::TestFontMgr()` = `MakePortableFontMgr()` (`tools/fonts/FontToolUtils.cpp#L300-L306`) |
| `skia_enable_svg = !is_component_build`, so `true`; `tool_utils` depends on `modules/svg` | `gn/skia.gni#L33`; `BUILD.gn#L2822-L2861` | The portable mgr has the `Emoji` and `Planet` families (`TestSVGTypeface`; `TestFontMgr.cpp#L80-L88`). Verify with the first emoji `_test` hash (§7.3) |
| `SK_GAMMA_APPLY_TO_A8` defined; `SK_GAMMA_EXPONENT` = 0 (sRGB), `SK_GAMMA_CONTRAST` = 0.5 (no `skia_use_fixed_gamma_text` off Android) | `BUILD.gn#L88-L101`; `include/core/SkTypes.h#L85-L93` | A8 glyph masks get the gamma/contrast pre-blend LUT |
| `skia_use_lcd_text_3x_filter = false` | `gn/skia.gni#L62` | LCD masks use `pack4xHToMask` (4 samples per pixel) |
| Raster sink surface props: `SkSurfaceProps(0, kRGB_H_SkPixelGeometry)` | `dm/DMSrcSink.cpp#L2170`, `#L2258`; our `tests/gm/src/sink.rs` | `kSubpixelAntiAlias` fonts get real LCD16 masks on `8888` (N32 + src-over); `565`/`f16` fall back to A8-from-LCD (`SkGlyphRunPainter.cpp#L234-L238`) |
| All three CPU configs have no color space | `tests/gm/src/sink.rs#L281-L303` | Scaler context flags are `kFakeGammaAndBoostContrast` everywhere (`SkGlyphRunPainter.cpp#L32-L41`) |

### 1.2 Where each GM's typeface came from

| How the GM gets its typeface | In the goldens | Our tooling must return |
|---|---|---|
| `DefaultPortableTypeface/Font`, `CreatePortableTypeface`, `DefaultTypeface`, `DefaultFont`, `CreateTestTypeface` | `TestTypeface` (path glyphs from `test_font_*.inc`; serif Normal is the default) | the same `TestTypeface` |
| `CreateTypefaceFromResource`, `TestFontMgr()->makeFromStream/makeFromData` | `nullptr` (`TestFontMgr.cpp#L142-L154`) | `None` |
| `SkFont()` / `setTypeface(nullptr)` | `SkTypeface::MakeEmpty()` (`src/core/SkFont.cpp#L60-L62`, `#L92-L95`): draws nothing, measures 0 | the empty typeface |
| `EmojiSample(Cbdt/Sbix/ColrV0/Svg)` | null (resources), so the GM returns `kSkip` (`gm/coloremoji.cpp#L115-L118`, `gm/scaledemoji.cpp#L57-L60`) | `None`, GM skips |
| `EmojiSample()` / `EmojiSample(Test)` | falls back to `CreatePortableTypeface("Emoji")` = `TestSVGTypeface::Default()` (`FontToolUtils.cpp#L125-L167`) | `TestSVGTypeface` (T20) |
| `PlanetTypeface()` | `planetcolr.ttf` is null on Windows, so it falls back to the `Planet` family (`TestSVGTypeface::Planets()`) | `TestSVGTypeface` (T20) |
| `SkCustomTypefaceBuilder` (`gm/userfont.cpp`) | custom typeface (`8888/gm/user_typeface` exists) | `CustomTypeface` (T17) |
| `SkTypeface_Make_Fontations` | not compiled | no golden |

Golden evidence (`cpu-x64-sse2`, `8888`): present: `coloremoji_test`, `coloremoji_blendmodes_test`,
`scaledemoji_test`, `scaledemojipos_test`, `scaledemojiperspective_test`, `scaledemoji_rendering`,
`user_typeface`, `typefacestyles`, `typefacestyles_kerning`, `typeface_styling`, `fontmgr_iter`,
`fontmgr_match`, `fontmgr_bounds*`, `lcdtext`, `stroketext`, `bigtext`, `mixedtextblobs`.
Absent, i.e. skipped or not built: every `colrv1_*`, every `palette`, `typefacerendering*`,
`coloremoji_colrv0` and the other non-`test` emoji formats, all `fontations*`.

**Skip-matching.** The GM harness counts "we skipped and the oracle has no result" as a match
(`tests/gm/src/check.rs`, `Outcome`). A faithfully ported `colrv1` GM therefore passes now: it skips
because its typeface is `None`, exactly as the oracle did. That is a correct statement about the
oracle configuration, not coverage of COLRv1. The PR that flips these entries must say so, and the
COLR code is tested separately (T21, unit tests, a future Fontations oracle).

### 1.3 What cannot be known without a fresh oracle run, and how we proceed anyway

| Unknown | Impact | Plan |
|---|---|---|
| Fontations pixels (any GM rendered with a resource font through Fontations) | No GM can verify T19/T21/T22 | Port faithfully; gate on unit tests. Keep those GMs `todo` with `needs-oracle`. Propose the oracle config in §7.3 for when an oracle host exists |
| Whether DM's `tool_utils` really had `SK_ENABLE_SVG` (the `Emoji`/`Planet` families) | Decides whether emoji `_test` goldens show SVG glyphs or fall back to serif | Very likely (GN default, DM sets `SkGraphics::SetOpenTypeSVGDecoderFactory` under the same define, `dm/DM.cpp#L1692-L1693`). The first `TestSVGTypeface` GM hash settles it |
| Nothing else on the portable path | — | Every input is in the pinned source: font data, gamma constants, props, blitters |

---

## 2. Typeface backends

### 2.1 What we port

| Backend | Skia source | Crate | Why |
|---|---|---|---|
| Empty | `src/core/SkTypeface.cpp` (`SkEmptyTypeface`), `SkScalerContext::MakeEmpty` | core | `SkFont`'s null typeface |
| Test (portable) | `tools/fonts/TestTypeface.{h,cpp}`, `test_font_*.inc`, `TestFontMgr.cpp`, `FontToolUtils.cpp`, `TestEmptyTypeface.h` | tools | renders almost every text golden |
| Test SVG (`Emoji`, `Planet`) | `tools/fonts/TestSVGTypeface.cpp#L1-L420` (not the COLR/CBDT font generators after `#L420`) | tools | emoji `_test` goldens; needs an SVG DOM (T20) |
| Random scaler context | `tools/fonts/RandomScalerContext.cpp` | tools | remote glyph cache tests |
| Custom (user) typeface | `src/utils/SkCustomTypeface.cpp` | text | `user_typeface`, `SampleUserTypeface` |
| Fontations | `src/ports/SkTypeface_fontations.cpp`, `SkFontScanner_fontations.cpp`, `SkFontMgr_fontations_empty.cpp`, `src/ports/fontations/src/*.rs` | text | PLAN decision; the only real-font backend in scope |
| Remote/proxy | `src/core/SkTypeface_remote.cpp`, `src/text/gpu/SkChromeRemoteGlyphCache.cpp` | text | non-Ganesh `SkRemoteGlyphCacheTest` entries |

### 2.2 Fontations: dependency, not port

- **Versions.** Skia m156 pins `read-fonts 0.40.1`, `font-types 0.12.0`, `skrifa 0.43.2`
  (`src/ports/fontations/Cargo.toml`, resolved in `MODULE.bazel.lock`). `read-fonts 0.40.2` and
  `skrifa 0.43.3` exist, and skrifa's requirement is `read-fonts ^0.40.1`. So all three must be
  `=`-pinned in `[workspace.dependencies]`, or Cargo resolves newer patch releases than Skia's.
  `bytemuck` (Skia locks `1.23.2`) comes in transitively; pin it the same way if `cargo tree`
  shows it changes outlines (it doesn't: it's only used for casts).
- **Why depend instead of port.** skrifa is Skia's implementation for this backend, not something
  Skia wraps around its own arithmetic. Depending on the exact version runs the same Rust source.
  Rust never contracts FMAs and has no fast-math, so rustc in Bazel and rustc in Cargo give
  identical arithmetic. This satisfies PORTING §8 ("no dependency may replace ported Skia
  behaviour"): the dependency *is* the behaviour. The PR adding it states this and the licences.
- **No `cxx`.** Skia's bridge (`base.rs` 770 lines, `hinting.rs` 129, `names.rs` 81,
  `verbs_points_pen.rs` 166, `colr.rs` 524, `bitmap.rs` 331; `ffi.rs` is only the `#[cxx::bridge]`
  declaration) is BSD Skia code. Port it into `skia_rust_text::ports::fontations::{base, hinting,
  names, pen, colr, bitmap}` with Rust types instead of FFI wrappers (`&Path`-building pen instead of
  `SkPathWrapper`, `Vec`/slices instead of `rust::Slice`, `Option` instead of the `bool` + out-param
  returns). Keep every function's logic and its `// Port of:` link. `SkTypeface_fontations.cpp`
  (1,761 lines) becomes `ports::fontations::typeface` and `::scaler_context`.
- **`unsafe` in dependencies.** `read-fonts` uses `bytemuck`. The workspace `unsafe_code = "deny"`
  covers our crates, not third-party ones, so PLAN §4 is unaffected. `cargo deny` checks licences
  and advisories as for every dependency.
- **Pin bumps.** A Skia milestone bump re-reads `bazel/external/fontations/Cargo.toml` and moves the
  pins with it (add this to `cargo xtask bump`'s checklist).

### 2.3 Out of scope, and how their tests are classified

`T0` sets these to `status = "excluded"`. The reason names the backend: "platform font backend
(FreeType/FontConfig/Android/CoreText) out of scope; skia-rust ships Fontations only (docs/design/text.md §2.3)".

| Entries | Count |
|---|---|
| `tests/FCITest.cpp::*` | 4 |
| `tests/FontMgrAndroidParserTest.cpp::*` | 7 |
| `tests/FontMgrFontConfigTest.cpp::*` (non-excluded) | 4 |
| `tests/FontScanner_FreeTypeTest.cpp::*` | 4 |
| `tests/TypefaceMacTest.cpp::TypefaceMacVariation` | 1 |
| `tests/FontationsFtCompTest.cpp::*` (compares against FreeType) | 26 |
| `gm/fontations_ft_compare.cpp::*` (compares against FreeType) | 35 |

No GM golden was rendered by FreeType, DirectWrite or CoreText (§1.1), so no GM needs a
"native backend" classification. GMs whose typeface was null match by skipping or by drawing
nothing. Benches with system fonts (`ShaperBench`, 34) belong to the shaper module (Phase 7).

---

## 3. Crates and layering

### 3.1 Placement (revises PLAN §3.1)

| Skia | skia-rust | Notes |
|---|---|---|
| `include/core/SkFontStyle.h`, `SkFontTypes.h`, `SkFontMetrics.h`, `SkFontArguments.h`, `SkFontParameters.h`, `src/core/SkFontMetricsPriv.cpp` | `skia_rust_core::{font_style, font_types, font_metrics, font_metrics_priv, font_arguments, font_parameters}` | value types |
| `SkTypeface.{h,cpp}`, `SkTypefaceCache.cpp`, `SkFontDescriptor.cpp`, `SkFontStream.cpp`, `src/sfnt/SkOTUtils.cpp` (name iterator) | `core::{typeface, typeface_cache, font_descriptor, font_stream, sfnt::ot_utils}` | `Typeface` handle + `TypefaceBase` trait |
| `SkFontMgr.{h,cpp}` | `core::font_mgr` | `FontMgr` handle + trait, `FontStyleSet`, `match_style_css3`, `FontMgr::empty()` |
| `SkFont.cpp`, `SkFontPriv.h`, `SkFont_serial.cpp`, `src/utils/SkCharToGlyphCache.cpp` | `core::{font, font_priv, utils::char_to_glyph_cache}` | |
| `SkDescriptor.cpp`, `SkScalerContext.{h,cpp}`, `SkMaskGamma.{h,cpp}`, `src/utils/SkMatrix22.cpp`, `src/utils/SkFloatUtils.h` | `core::{descriptor, scaler_context, mask_gamma, utils::matrix22, utils::float_utils}` | engine |
| `SkGlyph.{h,cpp}`, `SkStrike.cpp`, `SkStrikeCache.cpp`, `SkStrikeSpec.cpp`, `SkStrikeRef.cpp`, `src/text/StrikeForGPU.cpp` | `core::{glyph, strike, strike_cache, strike_spec, strike_ref, text::strike_for_gpu}` | engine |
| `SkTextBlob.cpp`, `SkTextBlobPriv.h`, `src/text/GlyphRun.cpp`, `src/utils/SkTextUtils.cpp` | `core::{text_blob, text::glyph_run, utils::text_utils}` | |
| `SkDrawable.cpp`, `SkPictureBackedGlyphDrawable` (in `SkGlyph.cpp`) | `core::{drawable, glyph::PictureBackedGlyphDrawable}` | if not already ported |
| `SkCanvas.cpp` text entry points, `SkDevice.cpp#L424-L488` | `core::canvas`, `core::device` | |
| `SkGlyphRunPainter.cpp` (`skcpu::GlyphRunListPainter`), `SkDraw_text.cpp`, `SkBitmapDevice.cpp#L540-L545`, `SkScalerContext::GenerateImageFromPath` + its static helpers (`SkScalerContext.cpp#L316-L697`) | `skia_rust_raster::{glyph_run_painter, draw_text, bitmap_device, glyph_image}` | needs `Draw`, A8 blitter, `RasterClip` |
| `src/utils/SkCustomTypeface.cpp`, `src/ports/*fontations*`, `src/core/SkTypeface_remote.cpp`, `src/text/gpu/SkChromeRemoteGlyphCache.cpp` | `skia_rust_text::{utils::custom_typeface, ports::fontations, remote_glyph_cache}` | backends |
| `tools/fonts/*`, `FontToolUtils`, `tools/Resources.cpp` | `skia-rust-tools` (`tests/tools`, `publish = false`) | test-only |

Layering after this note: `simd → core → skcms/raster → effects → (codec) → text → gpu → …`.
`skia-rust-text` depends on `core`, `raster` (glyph image seam, canvases for drawable/COLR glyphs),
`effects` (gradients for COLRv1) and, once it exists, `codec` (PNG bitmap glyphs, T22). The facade's
`text` feature turns on `skia-rust-text`. Without it, `Font`, `Typeface::empty()` and the
canvas API still exist, as they do in a Skia build with no font backend.

### 3.2 The rasterizer seam

`SkScalerContext::getImage` (`SkScalerContext.cpp#L699-L850`) and backends' `generateImage`
(e.g. `TestTypeface.cpp#L274-L276`) call `GenerateImageFromPath`, which draws the glyph path with
`skcpu::Draw` + `SkA8Blitter_Choose` into an A8 pixmap and then packs it to BW/A8/LCD16
(`SkScalerContext.cpp#L586-L697`). Core cannot depend on raster. So:

```rust
// core::scaler_context
pub trait GlyphPathRasterizer: Send + Sync + fmt::Debug {
    /// Port of `SkScalerContext::GenerateImageFromPath`.
    fn generate_image_from_path(&self, dst: &mut MaskBuilder<'_>, path: &Path,
                                pre_blend: &PreBlend, do_bgr: bool, vertical_lcd: bool,
                                a8_from_lcd: bool, hairline: bool);
}

pub struct ScalerContext {               // the non-virtual half of SkScalerContext
    rec: ScalerContextRec, typeface: Typeface, path_effect: Option<PathEffect>,
    mask_filter: Option<MaskFilter>, generate_image_from_path: bool, pre_blend: PreBlend,
    rasterizer: &'static dyn GlyphPathRasterizer,
    imp: Box<dyn ScalerContextImpl>,     // the virtuals: generate_metrics/image/path/drawable/font_metrics
}
```

`skia_rust_raster::glyph_image::GLYPH_PATH_RASTERIZER` is an immutable `static` unit struct. Each
backend's `on_create_scaler_context` (tools, text) passes `&GLYPH_PATH_RASTERIZER`. The
`ScalerContextImpl` methods get `&ScalerContextBase` (rec, rasterizer, pre-blend), so a backend's
`generate_image` can call `base.generate_image_from_path(glyph, image)` the way C++ calls the
protected helper. The empty typeface in core passes `&NO_PATH_RASTERIZER`, whose method is
`unreachable!()`: empty glyphs have no path, so `getImage` never reaches it. That is the only stub,
and it is documented.

*Rejected alternatives:* (a) moving scan conversion into core (a huge, non-mechanical move; `Draw`
pulls in the whole blitter stack); (b) putting the engine in raster (`Font::measure_str` and
`Canvas::draw_str` are core APIs and need strikes); (c) a registration `OnceLock` that raster fills
at first use (global mutable state, order-dependent).

---

## 4. Public API (skia-safe shape)

Read `third_party/rust-skia/skia-safe/src/core/{font,typeface,font_mgr,font_style,font_metrics,
font_arguments,font_parameters,font_types,text_blob,drawable,graphics}.rs`,
`core/canvas.rs#L1800-L2000` and `utils/{text_utils,custom_typeface}.rs` before naming anything.
Record deviations in `docs/API_MAPPING.md`.

| Skia | skia-rust (path as re-exported by the facade) | Shape |
|---|---|---|
| `sk_sp<SkTypeface>` | `Typeface` (`Clone` = `Arc` clone; `PartialEq` = `SkTypeface::Equal`), `TypefaceId`, `typeface::SerializeBehavior` | `Typeface(Arc<dyn TypefaceBase>)` like `Shader`; `Typeface::empty()`; methods from skia-safe (`font_style`, `unichars_to_glyphs`, `str_to_glyphs`, `count_glyphs`, `table_tags`, `get_table_data`, `units_per_em`, `family_name`, `clone_with_arguments`, `serialize`, `make_deserialize`, …) |
| `SkFont` | `Font` (value type, `Clone`) with `font::Edging` | `Font::new(typeface, size)`, `Font::default()` (empty typeface, 12pt), `measure_str`, `get_widths_bounds`, `get_pos`, `get_x_pos`, `get_path`, `get_paths`, `metrics`, `str_to_glyphs_vec`, … |
| `SkFontStyle`, `SkFontMetrics`, `SkFontArguments`, `SkFontParameters::Variation::Axis` | `FontStyle` (+`font_style::{Weight, Width, Slant}`), `FontMetrics`, `FontArguments`, `font_parameters::VariationAxis` | as skia-safe |
| `SkFontMgr`, `SkFontStyleSet` | `FontMgr` (handle over `Arc<dyn FontMgrBase>`), `FontStyleSet` | `FontMgr::empty()`; skia-safe's `FontMgr::new()` (system default) returns the Fontations empty manager under the `text` feature (open question Q2) |
| `SkTextBlob`, `SkTextBlobBuilder`, `SkTextBlob::Iter` | `TextBlob` (`Arc`), `TextBlobBuilder`, `text_blob::Iter` | `TextBlob::from_str/from_text/from_pos_text_h/from_pos_text/from_rsxform`, builder `alloc_run*` returning `&mut [GlyphId]` slices |
| `SkCanvas::drawSimpleText`, `drawString`, `drawGlyphs` (3 overloads), `drawGlyphsRSXform`, `drawTextBlob` | `Canvas::draw_str`, `draw_simple_text`, `draw_glyphs_at(glyphs, GlyphPositions::{Points, RSXforms}, origin, font, paint)`, `draw_glyphs_utf8`, `draw_text_blob` | `&self -> &Self`, as skia-safe |
| `SkTextUtils::Draw`, `DrawString` | `utils::text_utils::{draw_str_align, draw_text_align}` | |
| `SkCustomTypefaceBuilder` | `utils::CustomTypefaceBuilder` | |
| `SkGraphics::SetFontCacheLimit` etc. | `graphics::{set_font_cache_limit, font_cache_limit, font_cache_used, purge_font_cache, …}` | act on `StrikeCache::global()` |
| `SkTypeface_Make_Fontations`, `SkFontMgr_New_Fontations_Empty`, `SkFontScanner_Make_Fontations` | `typeface::fontations::make(data, args)`, `FontMgr::new_fontations_empty()`, `FontScanner::new_fontations()` | text crate |
| Private: `SkStrikeSpec`, `SkStrikeCache`, `SkStrike`, `SkGlyph`, `SkScalerContext*`, `SkDescriptor`, `sktext::GlyphRunList` | `#[doc(hidden)] pub` modules in core | needed by tests and the raster/gpu crates |

---

## 5. Ownership, caches and threading

### 5.1 Objects

- **Typeface.** `pub struct Typeface(Arc<dyn TypefaceBase>)`. `TypefaceBase: Any + Send + Sync`
  carries the `on*` virtuals of `SkTypeface` (`include/core/SkTypeface.h`). The common state
  (`fUniqueID`, `fStyle`, `fIsFixedPitch`) sits in a `TypefaceCore` struct that every
  implementation embeds and returns from `fn core(&self)`. Unique IDs come from a relaxed
  `AtomicU32` starting at 1, like `SkTypefaceCache::NewTypefaceID` (same rule as `PathData` ids,
  `docs/design/path.md`).
- **Font.** Plain value: `Option`-free `Typeface` (empty when null, as C++), size, scale, skew, flags,
  edging, hinting.
- **ScalerContext.** Owned by its `Strike` (`Box`), `Send`, never shared.
- **Strike.** `Arc<Strike>`. `Strike { desc: AutoDescriptor, strike_spec_key, inner: Mutex<StrikeInner> }`
  where `StrikeInner` holds the scaler context, `HashMap<PackedGlyphId, GlyphDigest>`,
  `Vec<Glyph>` (the C++ `fGlyphForIndex`: a digest stores its glyph's index, `SkGlyphDigest`), font
  metrics, memory counters and the pinner. A `Glyph` owns its image (`Box<[u8]>`), path
  (`Option<Path>`, an `Arc`-backed clone) and drawable (`Option<Drawable>`).
- **Borrowing during draws.** C++ locks the strike only inside `prepare_for_*` and then uses arena
  pointers after unlocking (`SkGlyphRunPainter.cpp#L46-L65`). Rust can't. So the painter keeps the
  `StrikeInner` guard for one run's **mask** pass: prepare, then `paint_masks`, then drop. A blit
  never re-enters a strike. For **paths and drawables** it clones the `Path`/`Drawable` (cheap
  `Arc` clones) into the accepted buffer and drops the guard before calling back into the canvas,
  because drawables may draw text. Output is identical: a strike's glyph data is immutable once set.
- **TextBlob.** `TextBlob(Arc<TextBlobData>)`. Runs keep Skia's record layout logically
  (`SkTextBlob::RunRecord`: glyph buffer, position buffer by `Positioning`, optional UTF-8 + clusters
  for `kExtended`) as owned `Vec`s, not a byte arena. Unique IDs come from an `AtomicU32`. The purge
  listener (`notifyAddedToCache`) is GPU-only; skip it until Phase 6.
- **GlyphRunList.** `GlyphRunList<'a>` borrows from a `GlyphRunBuilder`. The `Canvas` owns one
  scratch builder in a `RefCell` (C++ `fScratchGlyphRunBuilder`; `Canvas` methods take `&self`).
  Re-entrancy (a drawable glyph drawing text on the same canvas) takes a fresh builder, as
  `AutoGlyphRunBuilder` does (`SkCanvas.cpp#L2418-L2435`).

### 5.2 Process-wide state (all justified exceptions; record in `docs/API_MAPPING.md`)

| Skia global | skia-rust | Why it's acceptable |
|---|---|---|
| `SkStrikeCache::GlobalStrikeCache()` | `StrikeCache::global() -> &'static StrikeCache` (`static LazyLock<StrikeCache>`, internals in a `Mutex`) | memoization of a pure function; skia-safe's `Font::measure_str` has no cache parameter. Tests use local `StrikeCache::new()` exactly where C++ does (`SkStrikeCacheTest.cpp`). `SkGraphics::SetFontCacheLimit` changes only purging |
| typeface/blob/strike unique-id counters | `AtomicU32` / `AtomicU64` | ids, as for paths |
| `SkScalerContextRec::CachedMaskGamma` (`SkScalerContext.cpp#L115-L162`) | the default `(contrast, gamma)` `MaskGamma` in a `LazyLock` (immutable); other values built per scaler context | a pure function; no mutable cache |
| `SkTypeface::Register` / static `decoders()` (`SkTypeface.cpp#L162-L200`) | no registry. `Typeface::make_deserialize(stream, last_resort_mgr)` tries core's built-ins (empty), then `last_resort_mgr.typeface_decoders()` (a new `FontMgrBase` method; the Fontations and portable managers list theirs: custom typeface, Fontations, test typefaces), then `last_resort_mgr.make_from_stream` / `legacy_make_typeface` as C++ does | no life-before-main in Rust; behaviour matches whenever the manager knows the factory (open question Q3) |
| `SkFontMgr::RefEmpty()` singleton | `FontMgr::empty()` from a `OnceLock` | immutable |

### 5.3 Threading

`Typeface`, `Font`, `TextBlob`, `FontMgr`: `Send + Sync`. `Strike`: `Send + Sync` through its
`Mutex`. `StrikeCache`: one `Mutex` over the LRU list, hash map and totals, ported from
`SkStrikeCache.cpp`, including the purge order (it's observable in `SkStrikeCache_CachePurge` and
`SkGraphics_Limits`). Memory accounting uses the C++ `sizeof` values as named constants
(`SkGlyph`, `SkPath` estimates, `SkStrike` overhead), because tests compare totals. Never use
`std::mem::size_of` of our structs. `fontcache-mt` and `SkStrikeMultiThread` exercise this with
real threads.

---

## 6. The draw path

```
Canvas::draw_str / draw_simple_text / draw_glyphs_at / draw_text_blob          (core, T13)
  └─ GlyphRunBuilder::{text_to_glyph_run_list, blob_to_glyph_run_list, make_glyph_run_list}  (T12)
     Canvas::on_draw_glyph_run_list: quick-reject on source bounds, about_to_draw(kSkipMaskFilterAutoLayer)
                                                                                SkCanvas.cpp#L2443-L2455
  └─ Device::draw_glyph_run_list: finite CTM, RSXform runs → simplify_glyph_run_rsxform_and_redraw
                                                                                SkDevice.cpp#L424-L479
  └─ BitmapDevice::on_draw_glyph_run_list → LOOP_TILER → Draw::draw_glyph_run_list (raster, T14)
  └─ GlyphRunListPainter::draw_for_bitmap_device                       SkGlyphRunPainter.cpp#L219-L424
       props = N32 && src-over ? device props : props with kUnknown geometry  (LCD → A8 fallback)
       per run:
         ShouldDrawAsPath? → StrikeSpec::make_path → paths (canvas.draw_path, exact-CTM variant)
                             → drawables (save_layer + drawable.draw)
         !perspective     → StrikeSpec::make_mask(position matrix) → digest_for(kDirectMaskCPU)
                             → Draw::paint_masks(accepted, paint)                SkDraw_text.cpp#L53-L130
         leftovers        → scaled masks via bulk metrics + mapRectToQuad scale → draw as bitmaps
  └─ Draw::paint_masks: blitter_choose + AAClipBlitterWrapper; A8/BW/LCD16/3D → blitter.blit_mask
                        (D3 RasterPipelineBlitter, D4 legacy ARGB32/A8 blitters: already ported);
                        ARGB32 → draw_sprite (color glyphs)
```

- **Glyph images** come from `Strike::digest_for(action, id)` → `Glyph::set_image` →
  `ScalerContext::get_image`. That is either the backend's `generate_image` or the seam (§3.2),
  then the mask filter (`MaskFilterBase::filter_mask`: blur arrives with Phase 3; until then
  blurred text GMs stay blocked), then `pre_blend` (A8 gamma LUT, `applyLUTToA8Mask`; LCD:
  `pack4xHToMask` applies it per channel).
- **Path glyphs.** `ScalerContext::internal_get_path` applies the font's fake bold (stroke for
  `useStrokeForFakeBold` backends), the paint's path effect and the stroke rec
  (`SkScalerContext.cpp#L867-L947`). The stroker and dash effect are already ported (C6/C7). Text
  is drawn as paths for large sizes (`SkStrikeSpec::ShouldDrawAsPath`, 256-pixel limit), perspective,
  and paint effects that need the exact CTM. This is the path most label GMs take for big titles,
  and it reuses the whole C-wave scan conversion.
- **LCD and subpixel.** `compute_mask_format` (`SkScalerContext.cpp#L1143`) plus
  `MakeRecAndEffects` (`#L1177-L1290`) pick LCD16 for `kSubpixelAntiAlias` and RGB_H/BGR_H/V
  flags. Subpixel positioning packs 2 fractional bits per axis into `SkPackedGlyphID`
  (`SkGlyph.h`), with axis alignment from `SkStrikeSpec`'s rounding spec
  (`roundingSpec().ignorePositionFieldMask`, `halfAxisSampleFreq`). The painter adds the half-sample
  bias and floors (`SkGlyphRunPainter.cpp#L102-L141`). Port the packing arithmetic exactly
  (`SkFixed` 16.16 and `SkScalarToFixed`).
- **Color glyphs.** ARGB32 masks: `TestSVGTypeface` (renders its SVG into an N32 bitmap,
  `TestSVGTypeface.cpp#L224-L245`), Fontations COLRv0/v1 (`drawCOLRGlyph`) and bitmap glyphs
  (CBDT/sbix/EBDT). Large color glyphs go to `generateDrawable` → drawables (`SVGGlyphDrawable`,
  COLR drawables). All of them render through a raster `Canvas` on a `Bitmap`, which is why their
  backends sit above raster.
- **SDF / Slug.** Not on CPU. `SkDevice::convertGlyphRunListToSlug` returns null and
  `SkCanvas::drawSlug(nullptr)` does nothing (`SkDevice.cpp#L481-L488`, `SkCanvas.cpp#L2481-L2486`).
  The `slug` GM still has a golden (its other draws), so port `Canvas::convert_blob_to_slug ->
  Option<Slug>` returning `None` and `draw_slug(Option<&Slug>)`, with `Slug` an opaque type the
  GPU phase fills in. `dftext*` and `textblobmixedsizes_df` GMs render through the normal mask/path
  painter on raster.
- **Pictures.** `SkRecorder::onDrawTextBlob`/`onDrawGlyphRunList`/`onDrawSlug` and their `SkRecords`
  (D7 has the framework) record blobs by reference. Playback calls `draw_text_blob`. Picture
  serialization of typefaces (`SkPictureData` typeface set, `SkTypefacePlayback`) comes with T16.

---

## 7. Hinting, anti-aliasing and edging

### 7.1 Portable path (what the goldens need)

- `TestTypeface::onFilterRec` forces `SkFontHinting::kNone` and `useStrokeForFakeBold`
  (`TestTypeface.cpp#L145-L148`); `TestSVGTypeface` forces `kNone`. So hinting never changes
  outlines. Only the rec's hinting field (part of the descriptor) and baseline snapping matter.
- The `SkFont` defaults: edging `kAntiAlias`, hinting `kNormal` (`SkPaintDefaults_Hinting`), flags
  `kBaselineSnap`, size 12 (`SkFont.cpp#L43-L69`). `kAlias` edging gives BW masks via `packA8ToA1`.
- Gamma: luminance from `SkPaintPriv::ComputeLuminanceColor`, canonicalized by
  `SkScalerContextRec::setLuminanceColor`/`getLuminanceColor`. The tables come from `SkMaskGamma`
  (contrast 0.5, sRGB device gamma, `kFakeGammaAndBoostContrast` on all three configs). Port the
  table construction bit for bit (`SkMaskGamma.cpp`, `SkTMaskGamma` in the header, 8 luminance
  buckets). It's `f32` math with `sk_float_*` helpers: check against `SkMaskGamma` dumps by
  re-deriving them in a unit test from the C++ formulas.

### 7.2 Fontations path (no golden; port `SkTypeface_fontations.cpp` exactly)

- `onFilterRec` (`#L310-L331`): stroke fake bold; clear `kGenA8FromLCD_Flag`; `kFull → kNormal`
  unless LCD; `kNone` unless axis-aligned.
- Hinting instance selection (`#L377-L459`): hinting-reliant fonts force mono hinting; BW uses mono
  hinting unless `kNone`; `kSlight` = light autohint (`ForceForGlyf` unless forced/off);
  `kNormal` = full hinting without LCD; `kFull` = LCD-aware. `fDoLinearMetrics` follows each case.
  `SK_BUILD_FOR_ANDROID_FRAMEWORK` is never defined for us.
- `computeMatrices(kVertical)` splits scale and the remaining matrix. Outlines come from skrifa at
  `fScale.y()` and are transformed by `fRemainingMatrix`, exactly as in `generatePathImpl`.
- Embedded bitmaps vs outlines (`#L539-L563`): `kEmbeddedBitmapText_Flag`, PNG availability and
  `embeddedBitmapStrikeCloseEnough` (`#L40-L49`).

### 7.3 Proposed Fontations oracle (for when an oracle host exists; not blocking)

Add a build `x64-sse2-fontations` (`skia_use_fontations=true`). A small oracle-patch flag
`--fontMgr fontations_empty` makes `ToolUtils::TestFontMgr()` return
`SkFontMgr_New_Fontations_Empty()`, so resource fonts load through Fontations while named families
still fall back to the portable manager via `CreateTestTypeface`. Run it as its own tier
(`cpu-x64-sse2-fontations`). On Windows, plain `--nativeFonts --fontations` would pick DirectWrite
for `TestFontMgr` and Fontations only for the font scanner, and on Linux it would read system
fonts, so neither is hermetic. File this as a `needs-oracle` issue. Until then, the `fontations*`
GMs, Fontations-rendered `colrv1`/`palette` and the CBDT/sbix emoji variants have no verifiable
pixels.

---

## 8. Test tooling: `skia-rust-tools` and font configurations

- New workspace member `tests/tools` (`skia-rust-tools`, `publish = false`), used by `tests` and
  `tests/gm`. It holds `fonts::{test_typeface, test_font_data, test_font_mgr, test_empty_typeface,
  random_scaler_context, test_svg_typeface (T20)}`, `font_tool_utils` and `resources` (moved from
  `tests/src/resources.rs` if that's the only copy).
- **Font data.** `cargo xtask gen-test-fonts` reads `tools/fonts/test_font_{monospace,sans_serif,
  serif,index}.inc` and writes `tests/tools/src/fonts/test_font_data/*.rs`, committed. Point arrays
  stay `f32` literals copied verbatim (`0.225098f` → `0.225098_f32`; both languages round decimal
  literals correctly). Widths stay `SkFixed` `i32`, verbs stay `u8`. The generator is the only
  writer; CI checks it's up to date (re-run and `git diff --exit-code`). Never `include!` the
  `.inc` files: no paths through `..` or `third_party`.
- **Portable `FontMgr`.** Port `TestFontMgr.cpp` with families in Skia's order: monospace,
  sans-serif, serif, the three "Toy Liberation" families, then `Emoji` and `Planet` (indices 6
  and 7 are hard-coded in `onMatchFamily`). Until T20 lands, build it as Skia does without
  `SK_ENABLE_SVG`: no `Emoji`/`Planet` families, and no `oji`/`Planet` match branches. GMs that
  touch them won't match until T20; nothing else changes.
- **Two configurations, chosen per test, no global flag.**
  `font_tool_utils::FontConfig::{Portable, NativeFontations}`. `test_font_mgr()` reads it from a
  test-tools `thread_local!` that the harness sets around each test body (`with_font_config(cfg,
  || …)`). Test-only, and it never leaks into library crates.
  - **GMs:** always `Portable`. That's the oracle and Skia's default bots (`--nonativeFonts`).
  - **Unit tests** that call `ToolUtils::TestFontMgr()` / `CreateTypefaceFromResource` (TypefaceTest,
    FontMgrTest, SerializationTest, FontHost*, FontNames, …): `def_test!` runs them under both
    configurations, mirroring Skia's default bots and its `NativeFonts_Fontations` bots
    (`infra/bots/jobs.json`, e.g. `Test-Ubuntu24.04-…-NativeFonts_Fontations`). Under
    `NativeFontations`, `test_font_mgr()` is `SkFontMgr_New_Fontations_Empty()`. `DefaultTypeface`
    and `CreateTestTypeface` still fall back to portable faces through C++'s own fallback
    (`FontToolUtils.cpp#L361-L373`). An entry is `passing` only if both runs pass. This avoids hollow
    passes from `if (!typeface) return;`.
- `CreateTypefaceFromResource(path)` = `test_font_mgr().make_from_stream(resource(path))`. It is
  `None` under `Portable`, matching §1.2.

---

## 9. Work breakdown

Sizes: S < 500 lines, M 500–1,500, L > 1,500 (Rust, excluding tests and generated data). Every task
is a `port/<name>` branch and PR. "Unlocks" lists manifest entries (`tests/<File>.cpp::<Name>`)
and GM groups; GM counts are approximate (a file scan for text APIs; check golden presence with the
harness). Tasks in one wave are parallel unless "Depends" says otherwise. Agents start on Haiku
(PLAN §8.3), so each task is one Skia file pair or a tight cluster.

### Wave T-0 — bookkeeping

| ID | Task | Sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| T0 | Manifest hygiene: exclusions of §2.3 (81 entries) with the reason; `reason = "needs-oracle: no golden (oracle built with skia_use_fontations=false)"` on the 6 `gm/fontations{,_cbdt,_ebdt}.cpp` entries; `module = "text"` on text GMs filed elsewhere where the first porting PR touches them | manifest only | — | S | honest denominators |

### Wave T-A — types and engine (core). T1, T2, T4 start immediately; the rest follow the arrows.

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| T1 | Font value types: `FontStyle`, `font_types` (`TextEncoding`, `FontHinting`, `GlyphId`, `Unichar`), `FontMetrics` + `font_metrics_priv`, `FontArguments`, `font_parameters` | `include/core/SkFont{Style,Types,Metrics,Arguments,Parameters}.h`, `src/core/SkFontMetricsPriv.cpp` | — | S | — |
| T2 | `Descriptor`/`AutoDescriptor` with the C++ byte layout (12-byte header, 8-byte entries, `SkChecksum` checksum), `ScalerContextRec` and its C++ byte image (field order and padding as `sizeof(SkScalerContextRec)`) | `SkDescriptor.cpp`, `SkScalerContext.h#L69-L250` | T1 | M | `DescriptorTest` (9) |
| T3 | `Typeface` handle + `TypefaceBase` trait + `TypefaceCore`, unique ids, `EmptyTypeface`, `TypefaceCache`, `FontDescriptor` (serialize), `Typeface::serialize/make_deserialize`, `LocalizedStrings` + `SkOTUtils::LocalizedStrings_SingleName` | `SkTypeface.cpp`, `SkTypefaceCache.cpp`, `SkFontDescriptor.cpp`, `src/sfnt/SkOTUtils.cpp` (name parts) | T1 | M | `TypefaceTest::TypefaceCache`, `TypefaceTest::FontDescriptorNegativeVariationSerialize` |
| T4 | `MaskGamma` + `PreBlend` + `SkColorSpaceLuminance` (sRGB/gamma/linear) | `SkMaskGamma.{h,cpp}` | — | S | prerequisite of T6 |
| T5 | `Glyph`: `PackedGlyphId` (subpixel packing), `GlyphRect`, `GlyphDigest`/`GlyphAction`/`ActionType`, image/path/drawable state, flatten/unflatten; `Drawable` (if missing) + `PictureBackedGlyphDrawable` | `SkGlyph.{h,cpp}`, `SkDrawable.cpp` | T1, D7 (pictures) | M | `SkGlyphTest::SkGlyphRectBasic`, `SkGlyphTest::SkPictureBackedGlyphDrawable_Basic` |
| T6 | `ScalerContext` (non-virtual half) + `ScalerContextImpl` trait + `GlyphPathRasterizer` seam: `MakeRecAndEffects`, `computeMatrices` (with `utils::matrix22`, `utils::float_utils` ULPs), `makeGlyph`, `internalGetPath`, `getImage` (mask filter + intersection copy), font metrics, `MakeEmpty`, `GetMaskPreBlend` | `SkScalerContext.{h,cpp}` minus `#L316-L697`, `src/utils/SkMatrix22.cpp`, `SkFloatUtils.h` | T2, T3, T4, T5 | L | — |
| T7 | Raster glyph images: `generate_image_from_path`, `packA8ToA1`, `pack4xHToMask`, `pack3xHToMask` (behind the never-set 3x flag, for completeness), `applyLUTToA8Mask`; `GLYPH_PATH_RASTERIZER` | `SkScalerContext.cpp#L316-L697` | T6, D5 | S | — |
| T8 | Strikes: `Strike`, `StrikeCache` (+`global()`, LRU, purge, limits, C++ memory constants), `StrikeSpec` (`make_mask/make_path/make_canonicalized/make_with_no_device/should_draw_as_path`), `BulkGlyphMetrics{,AndPaths,AndDrawables,AndImages}`, `StrikeRef`, `graphics` font-cache API, `StrikePromise` | `SkStrike.cpp`, `SkStrikeCache.cpp`, `SkStrikeSpec.cpp`, `SkStrikeRef.cpp`, `src/text/StrikeForGPU.cpp`, `SkGraphics.cpp` (font parts) | T6 | M | (with T10) `SkStrikeCacheTest::SkStrikeCache_CachePurge`, `SkStrikeTest::SkStrikeMultiThread`, `SkStrikeTest::SkStrike_FlattenByType`, `SkRemoteGlyphCacheTest::SkGraphics_Limits`, `StrikeForGPUTest::SkStrikePromise_Basic` |
| T9 | `Font` (all measuring, `text_to_glyphs`, `get_path(s)`, `get_pos`/`get_x_pos`, `metrics`, `get_bounds`) + `font_priv` + flatten/unflatten + `CharToGlyphCache` | `SkFont.cpp`, `SkFontPriv.h`, `SkFont_serial.cpp`, `src/utils/SkCharToGlyphCache.cpp` | T8 | M | `PaintTest::Font_getpos`, `PaintTest::Paint_regression_measureText`, `FontTest::Font_flatten`, `SkFontMetricsPrivTest::SkFontMetricsPriv_Basic`, `CharToGlyphCache::chartoglyph_cache` |
| T10 | `skia-rust-tools` crate; `xtask gen-test-fonts`; `TestTypeface` + `SkTestScalerContext`; `TestEmptyTypeface`; `font_tool_utils` (`default_portable_typeface/font`, `create_portable_typeface`, `default_typeface/font`, `create_test_typeface`, `create_typeface_from_resource`, `emoji_sample`, `name_for_font_format`, `create_string_image`); `ToolUtils::VariationSliders` (used by `colrv1`); `FontConfig` | `tools/fonts/{TestTypeface,FontToolUtils}.cpp`, `TestEmptyTypeface.h`, `*.inc` | T3, T6, T7, T11 | M (+ generated data) | everything below |
| T11 | `FontMgr` handle + `FontMgrBase` trait, `FontStyleSet` (`match_style_css3`), `FontMgr::empty()`, `legacy_make_typeface`, `fallback(Request)`, `typeface_decoders()`; portable `FontMgr` (in tools, without SVG families) | `SkFontMgr.cpp`, `tools/fonts/TestFontMgr.cpp` | T3 | M | `FontMgrTest` (7, with T10; dual config needs T19 for the second half), `TypefaceTest::LegacyMakeTypeface`, `TypefaceTest::Typeface`, `TypefaceTest::TypefaceStyle` |

### Wave T-B — drawing (core + raster) → **first GM wave**

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| T12 | `GlyphRun`, `GlyphRunList` (bounds, origin, RSXform flag), `GlyphRunBuilder` (`text_to_glyph_run_list`, `make_glyph_run_list`, `blob_to_glyph_run_list` stub until T15) | `src/text/GlyphRun.{h,cpp}` | T9 | M | — |
| T13 | Canvas text API + `Device::draw_glyph_run_list` + `simplify_glyph_run_rsxform_and_redraw` + `Device::scaler_context_flags`; `Slug` placeholder (`convert_blob_to_slug → None`, `draw_slug`) | `SkCanvas.cpp#L2418-L2600`, `SkDevice.cpp#L424-L500` | T12, D6 | M | `DrawTextTest` (4) |
| T14 | CPU painter + `Draw::paint_masks` + `Draw::draw_glyph_run_list` + `BitmapDevice::on_draw_glyph_run_list` | `SkGlyphRunPainter.cpp`, `SkDraw_text.cpp`, `SkBitmapDevice.cpp#L540-L545` | T13, T8, T7 | M | GM wave 1 (below) |
| T15a | `TextBlob` + `TextBlobBuilder` (`alloc_run*`, `make`, conservative/tight bounds, `Iter`/`RunIterator`), `from_*` constructors, `blob_to_glyph_run_list`; `ToolUtils::add_to_text_blob(_w_len)` | `SkTextBlob.cpp#L1-L700`, `SkTextBlobPriv.h`, `tools/ToolUtils.cpp` | T12 | M | `TextBlobTest::{TextBlob_builder, TextBlob_extended, TextBlob_iter, TextBlob_paint, TextBlob_MakeAsDrawText}`, `TextBlobTest::SkCanvas_drawTextBlob_b513820666` |
| T15b | `TextBlob::get_intercepts` (+ `Font::get_intercepts`), serialize/deserialize (typeface procs), `utils::text_utils`, `ToolUtils::get_text_path` | `SkTextBlob.cpp#L700-L1021`, `src/utils/SkTextUtils.cpp`, `SkFont.cpp` intercepts | T15a | M | `TextBlobTest::{TextBlob_getIntercepts, TextBlob_serialize}`, `SerializationTest::WriteBuffer_external_memory_textblob` |
| T16 | Text in pictures: records, `SkRecorder`/`RecordDraw` for blobs and glyph run lists, typeface sets in picture serialization | `SkRecorder.cpp`, `SkRecordDraw.cpp`, `SkPictureData.cpp` (typefaces), `SkPictureFlat` | T15a, D7 | S | `SerializationTest::Serialization_PictureTypeface`, text GMs drawn via pictures |
| T17 | `CustomTypefaceBuilder` (paths + drawables, metrics, serialization, decoder) + `ToolUtils::SampleUserTypeface` | `src/utils/SkCustomTypeface.cpp`, `FontToolUtils.cpp#L182-L218` | T14, T5 | M | `TypefaceTest::CustomTypeface_invalid_glyphid`, `SkGlyphTest::{SkGlyph_SendMetrics, SkGlyph_SendWithImage, SkGlyph_SendWithPath, SkGlyph_SendWithDrawable}`, GM `user_typeface` |

**GM wave 1 (after T14; TextBlob not needed): about 40 rendered entries.** `aaxfermodes`,
`complexclip3` (2), `cubicpaths` (`CubicPathGM`, `CubicClosePathGM`; the shader variant waits on
gradients), `daa`, `dashing` (`dashtextcaps`), `degeneratesegments`, `drawglyphs` (RSXform),
`emptypath`, `fontscaler`, `glyph_pos` (6), `lcdtext` (3), `linepaths` (2), `mac_aa_explorer`
(`macaa_colors`), `pathreverse` (result `path-reverse`), `persptext` (2), `poly2poly` (`Em.ttf` is
null), `quadpaths` (2), `text_scale_skew` (2), `variedtext` (4), `bigtext` (2),
`fontscalerdistortable`. Port them as one Haiku task per 3–5 files. **Plus 76 skip-matching
entries**, which need only the API surface and `create_typeface_from_resource → None`:
`colrv1.cpp` (59), `palette.cpp` (5), `scaledemoji.cpp` non-`Test` formats (12). Port these as one
task. The PR states that they match the oracle's skips (§1.2).

**GM wave 2 (after T15a/b): about 17.** `textblob`, `textblobblockreordering`,
`textblobcolortrans`, `textblobtransforms`, `textblobuseaftergpufree`, `texteffects` (2),
`getpostextpath`, `lcdoverlap`, `skbug_12212`, `skbug_5321`, `skbug_8955`, `slug`,
`crbug_478659067` (no golden: check it skips), `pdf_never_embed.cpp` (3, raster results exist).

### Wave T-C — backends

| ID | Task | Skia sources | Depends | Size | Unlocks |
|---|---|---|---|---|---|
| T18a | `skia-rust-text` crate; pinned Fontations deps (§2.2); bridge `base.rs` + `names.rs` ported without `cxx` | `src/ports/fontations/src/{base,names}.rs`, `ffi.rs` (signatures only) | T3 | M | — |
| T18b | Bridge `verbs_points_pen.rs` (pen → `PathBuilder`) + `hinting.rs` | `src/ports/fontations/src/{verbs_points_pen,hinting}.rs` | T18a | S | — |
| T19a | `SkTypeface_Fontations` typeface half: make from stream/data, collection index, variations, palettes, tables, names, style, glyph mapping, `onFilterRec`; `FontScanner_Fontations`; `FontMgr::new_fontations_empty` | `SkTypeface_fontations.cpp#L1-L357`, `SkFontScanner_fontations.cpp`, `SkFontMgr_fontations_empty.cpp` | T18a, T11 | M | `FontationsTest::{Fontations_DoNotMakeFromNull, …_NonSfnt, …_Invalid_Index, Fontations_MakeFromFont, Fontations_MakeFromCollection*, Fontations_TableData, Fontations_TableTags, Fontations_VariationPosition, Fontations_VariationParameters*}`, `FontScanner_FontationsTest` (4) |
| T19b | Fontations scaler context, outline path: hinting instances, linear metrics, metrics, path generation, `getContourHeightForLetter` (synthetic x/cap height) | `SkTypeface_fontations.cpp#L358-L800` (outline parts) | T19a, T18b, T6 | L | `FontationsTest::{Fontations_SyntheticXHeight, Fontations_SyntheticCapHeight}`; `NativeFontations` halves of `TypefaceTest` (16), `FontMgrTest`, `FontHostTest`, `FontHostStreamTest`, `FontNamesTest`, `SerializationTest::Serialization_Typeface*` |
| T20 | `TestSVGTypeface` (`Default` + `Planets`, scaler context, `SVGGlyphDrawable`) + `Emoji`/`Planet` families in the portable mgr | `tools/fonts/TestSVGTypeface.cpp#L1-L420` | T14, **svg DOM** (open question Q1) | M | `coloremoji_test`, `scaledemoji{,pos,perspective}_test`, `scaledemoji_rendering`, `mixedtextblobs`, `fontmgr` (5), `dftext`, `textblobrandomfont`, `coloremoji_blendmodes_test` (with Phase 3) |
| T21 | Fontations COLRv0/v1: `colr.rs` + C++ `ColorPainter`/`BoundsPainter`, `drawCOLRGlyph`, COLR drawables, palette overrides | `src/ports/fontations/src/colr.rs`, `SkTypeface_fontations.cpp#L853-L1761` (+ COLR branches of `generateMetrics`) | T19b, Phase 3 gradients, T5 drawables | L | unit only (`colrv1` GMs skip in the oracle); `needs-oracle` for pixels |
| T22 | Fontations bitmap glyphs (CBDT/sbix/EBDT, alpha masks, PNG) | `src/ports/fontations/src/bitmap.rs`, `SkTypeface_fontations.cpp#L527-L852` (bitmap parts) | T19b, Phase 4 PNG codec | M | `needs-oracle` (`fontations_cbdt`, `fontations_ebdt`) |
| T23 | Remote glyph cache: `SkTypefaceProxy`, `SkStrikeServer`/`SkStrikeClient` (non-GPU parts), `RandomScalerContext` (tools) | `src/core/SkTypeface_remote.cpp`, `src/text/gpu/SkChromeRemoteGlyphCache.cpp`, `tools/fonts/RandomScalerContext.cpp` | T8, T15b | L | `SkRemoteGlyphCacheTest::{SkRemoteGlyphCache_ClientMemoryAccounting, _PurgesServerEntries, _StrikeDeletionServer, _StrikeLockingServer, _StrikePinningClient, _b513780208}`, `SkTypefaceProxy_Basic_Serial` (and `SkGraphics_Limits` with T8) |

### Wave T-D — sweeps

- **T24** GM waves 1 and 2 (above), one Haiku task per 3–5 GM files.
- **T25** Text GMs gated on other phases: about 330 entries in about 100 files whose other
  blockers are gradients (`gradients.cpp` alone has 37), images, blur mask filters, image filters,
  color filters, runtime effects or codecs. Each Phase 3/4 sweep picks them up once T14/T15 have
  landed. The text part is only label drawing (`DefaultPortableFont` + `drawString`).
- **T26** Benches: `TextBlobBench` (3), `FontCacheBench` (3), `SkGlyphCacheBench` (5),
  `CmapBench` (8), `TypefaceBench` (7), `GlyphQuadFillBench`, `GlyphRunRSXformBench`.

### Dependency summary

```
T1 ─┬─ T2 ─┐
    ├─ T3 ─┼─ T11 ─────────────────────────┐
T4 ─┤      │                                │
    └─ T5 ─┴─ T6 ─┬─ T7 ─┐                  ├─ T10 ─┐
                  └─ T8 ─┴─ T9 ─ T12 ─ T13 ─ T14 ───┴─ GM wave 1 (T24)
                                  └─ T15a ─ T15b ─ GM wave 2, T23
                                       └─ T16          T14 ─ T17, T20 (+svg)
T3 ─ T18a ─ T18b ─ T19a ─ T19b ─ T21 (+Phase 3), T22 (+Phase 4)
```

Critical path to the first GM: T1 → T5 → T6 → T8 → T9 → T12 → T13 → T14, with T10 alongside.
T2/T3/T4/T7/T11 run in parallel with it.

---

## 10. Risks and open questions

| # | Item | Plan |
|---|---|---|
| Q1 | **`TestSVGTypeface` needs an SVG DOM** (`SkSVGDOM::MakeFromStream`, shapes, `g`, `transform`, inline `style` fill/stroke). The svg module is Phase 7 and needs an XML parser decision (Skia uses expat via `SkDOM`). | Either pull the svg core forward (circle/ellipse/path/rect/g + presentation attributes covers `fonts/svg/*.svg`), or leave the ~13 emoji/`fontmgr` entries until Phase 7. Decide in the svg design note. T20 stays unscheduled until then |
| Q2 | skia-safe's `FontMgr::new()` / `FontMgr::default()` means "the platform default". m156 has no implicit default (`SkFontMgr::RefDefault` is gone). | Proposed: `FontMgr::new()` = `new_fontations_empty()` under the `text` feature, `empty()` without it; a Fontations-backed directory manager (`SkFontMgr_custom_directory` minus FreeType) is a later, separate decision. Record in `API_MAPPING.md` |
| Q3 | Typeface deserialization without Skia's static decoder registry (§5.2). A blob serialized with a Fontations typeface and deserialized with `last_resort_mgr = None` would fail where C++ succeeds. | Accept and document: the decoders of the custom typeface and Fontations live in the text crate, so they are reachable only through a manager that lists them. Revisit if a ported test deserializes with a null manager and expects a non-portable typeface back |
| R1 | **Hollow passes.** Skip-matching GMs (76) and early-return unit tests look like coverage. | PR descriptions name them; unit tests run in two font configs (§8) |
| R2 | **Exact `sizeof` dependence** (`DescriptorTest`, memory accounting). | Named C++-size constants (x64/arm64/wasm32 layouts agree for these PODs; `SkScalerContextRec` has no pointers). A unit test pins each constant to the C++ field arithmetic |
| R3 | **skrifa float codegen differences** (`f32::mul_add` or `libm` calls inside skrifa compile to the same Rust on both sides; wasm vs x64 `libm` could differ). | Same versions everywhere; any Fontations golden comes from a native x64 oracle; scalar/wasm caveats as R5 in the raster-pipeline design |
| R4 | **Strike lock held during mask blits** (§5.1) serializes concurrent draws of one strike. | Performance only. If the perf gate trips, store images as `Arc<[u8]>` and drop the guard after prepare |
| R5 | **Layering change** (engine in core) contradicts PLAN §3.1's comment for `skia-rust-text`. | This note updates PLAN §3.1 |
| R6 | `crbug_478659067`, `HelloBazelWorld` and `macaatest` have no golden. | Check each GM's skip conditions / build inclusion before porting; a missing golden with our render is a mismatch, never a pass |
