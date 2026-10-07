# API mapping

Deviations from the reference API (rust-skia's `skia-safe`, see `docs/PORTING.md` §3), and non-mechanical names for symbols `skia-safe` doesn't expose. One row per symbol, grouped by module, kept sorted.

| Skia | skia-safe | skia-rust | Why |
|---|---|---|---|
| **color** | | | |
| `SkColor` | `Color` (no raw accessor) | `Color` + `From<Color> for u32` | safe replacement for skia-safe's crate-private `into_native` |
| `SkColor4f::toSkColor` | `Color4f::to_color` (truncates) | `Color4f::to_color` (Skia rounding via `Sk4f_toL32`) | skia-safe's version differs from Skia; skia-rust ports Skia |
| `SkPMColor4f` / `SkRGBA4f<kPremul>` | none | `PMColor4f` | alpha-type template modelled as two concrete types sharing a macro |
| `SkPMColor` helpers | none | `color::{pm_color_set_argb, pm_color_get_*}` | mechanical rule; SkPMColor byte order is Skia's platform default (BGRA on Windows, RGBA elsewhere) |
| `SkUnPreMultiply` | none | `un_pre_multiply::{get_scale, apply_scale, pm_color_to_color}` | class with only statics becomes a module |
| **float_bits** | | | |
| `SkFloat2Bits`, `SkBits2Float`, `SkFloatAs2sCompliment`, ... (`SkFloatBits.h`) | not exposed | `float_bits::{float_to_bits, bits_to_float, float_as_2s_compliment, ...}` | mechanical names; needed by `ScalarTest` |
| **point** | | | |
| `SkIVector` | `pub use IPoint as IVector` | `pub type IVector = IPoint` | a type alias is equivalent |
| `SkIPoint` `+ - += -=` | plain `+`/`-` (panics on overflow) | saturating (`Sk32_sat_add` / `Sk32_sat_sub`) | Skia's semantics (PORTING §3) |
| `SkIPoint::operator-()` | `-x` | `wrapping_neg` | C++ negation wraps in practice; no debug panic |
| `SkPointPriv` | not exposed | `#[doc(hidden)] point::point_priv` free functions | PORTING §3 |
| `SkPointPriv::AsScalars` | not exposed | `point_priv::as_scalars(&Point) -> [scalar; 2]` | no `unsafe`: returns a copy, not a pointer |
| `SkPointPriv::DistanceToLine*BetweenSqd(..., Side*)` | not exposed | `point_priv::distance_to_line_between_sqd(.., Option<&mut Side>)` | optional out-param |
| `SkPointPriv::EqualsWithinTolerance` (2 overloads) | not exposed | `equals_within_tolerance`, `equals_within_tolerance_tol` | overloads get distinct names |
| `SkPointPriv::Negate` / `RotateCCW` / `RotateCW` | not exposed | `negate`, `rotate_ccw`, `rotate_ccw_in_place`, `rotate_cw`, `rotate_cw_in_place` | `src`/`dst` overloads split into by-value and in-place |
| `SkPointPriv::SetLengthFast` | not exposed | `point_priv::set_length_fast` | m156 shares the double-precision path with `setLength` |
| `SkPointPriv::SetRectFan`, `SetRectTriStrip` | not exposed | not ported | write through a byte `stride` into raw vertex memory; not expressible without `unsafe` |
| **point3** | | | |
| `SkPoint3::makeScale` | `Point3::scaled` | `Point3::scaled` | same as skia-safe |
| **rect** | | | |
| `SkIRect::asInt32s`, `SkRect::asScalars` | `&[i32]` / `&[f32; 4]` into the struct | `[i32; 4]` / `[scalar; 4]` by value | no `unsafe` |
| `SkIRect::inset` / `makeInset` | via `with_outset(-delta)` | direct `sat_add`/`sat_sub`, as in C++ | differs from skia-safe only at saturation |
| `SkIRect::MakePtSize` | non-saturating `From<(IPoint, ISize)>` | saturating `from_pt_size` / `From<(IPoint, ISize)>` | Skia's semantics |
| `SkIRect::offsetTo` | `with_offset_to` returns `(pin(..), pin(..), new_x, new_y)` (edges swapped) | `(new_x, new_y, pin(..), pin(..))` | skia-safe's ordering looks like a bug |
| `SkIRect::topLeft` | missing | `IRect::top_left` | mechanical name |
| `SkRect::Bounds` | via FFI | `Rect::bounds`, `Rect::bounds_or_empty`, `Rect::from_bounds` | 64-bit variant of the C++ (both variants compute the same numerics); `std::min/max` NaN semantics are kept for `join`, `intersect`, `sorted`, `set_bounds2` |
| `SkRect::centerX/centerY` | `left * 0.5 + right * 0.5` | `float_midpoint` (`sk_float_midpoint`, double math) | Skia's m156 formula |
| `SkRect::contains`, `SkIRect::contains` overloads | `Contains<T>` trait (crate root) | `rect::Contains<T>` | same shape; trait lives in `rect` |
| `SkRect::dump`, `dumpToString`, `dumpHex` | `dump`, `dump_to_string`, `dump_hex` | not ported yet | need `SkString` / `SkAppendScalar` |
| `SkRect::offsetTo` | `with_offset_to` returns `(x, y, x - left, y - top)` | `offset_to` / `with_offset_to` mirror C++ (`right += newX - left`, ...) | skia-safe's version looks like a bug |
| `SkRect::roundOut(SkIRect*)`, `roundOut(SkRect*)` | `RoundOut<R>` trait | `rect::RoundOut<R>` | same as skia-safe |
| `SkRect::set(SkPoint, SkPoint)`, `intersect(a, b)`, `join(a, b)` | `set_bounds2`, `intersect2`, `join2` | same | same as skia-safe |
| `SkRect::toQuad`, `copyToQuad` | `to_quad`, `copy_to_quad` | not ported yet | need `SkPathDirection` (`SkPathTypes.h`) |
| `SkRectPriv` | not exposed | `#[doc(hidden)] rect::rect_priv` free functions | PORTING §3 |
| `SkRectPriv::FitsInFixed` | not exposed | `rect_priv::fits_in_fixed_rect` | avoids clashing with `math_priv::fits_in_fixed` |
| `SkRectPriv::QuadContainsRect*` | not exposed | not ported yet | need `SkMatrix` / `SkM44` |
| `SkRectPriv::Subtract` (4 overloads) | not exposed | `subtract`, `subtract_irect` (bool + out-param), `subtract_diff`, `subtract_irect_diff` | overloads get distinct names |
