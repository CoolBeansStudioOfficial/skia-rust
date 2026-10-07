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
| **rrect** | | | |
| `SkRRect::Type`, `SkRRect::Corner` | `rrect::Type`, `rrect::Corner` (`Empty`, `Rect`, ..; `UpperLeft`, ..) | same; `Type::LAST` for `kLastType` | same as skia-safe; `Corner` is `repr(usize)` so `corner as usize` indexes the radii |
| `SkRRect::getType` / `type` | `get_type` | `get_type` (`debug_assert!(is_valid())` as in C++) | `type` is a Rust keyword; one name |
| `SkRRect::getSimpleRadii` | `simple_radii` | `simple_radii` | same as skia-safe |
| `SkRRect::radii()` (span) | `radii_ref -> &[Vector; 4]` | `radii_ref` | same as skia-safe |
| `SkRRect::getBounds` / `rect` | `bounds`, `rect` | same | same as skia-safe |
| `SkRRect::Make*` | `new_rect`, `new_oval`, `new_rect_xy`, `new_rect_radii`, `new_nine_patch`, `new_empty` | same | same as skia-safe |
| `SkRRect::inset/outset(dx, dy[, dst])` | `inset(delta)`, `with_inset(delta)`, `outset`, `with_outset` | same | same as skia-safe; the `dst` overload is `with_*` |
| `SkRRect::makeOffset` | `with_offset` | `with_offset` | same as skia-safe |
| `SkRRect::contains(SkPoint)` / `contains(SkRect)` | `contains_point`, `contains` | same | overloads get distinct names |
| `SkRRect::writeToMemory` | `write_to_memory(&mut Vec<u8>)` | same; native-endian floats, replaces the vector's contents | same as skia-safe |
| `SkRRect::readFromMemory` | `read_from_memory(&[u8]) -> usize` | same | same as skia-safe |
| `SkRRect::kSizeInMemory` | `SIZE_IN_MEMORY` | `SIZE_IN_MEMORY` (`12 * 4`) | same as skia-safe |
| `SkRRect::transform` (both overloads) | `transform(&Matrix) -> Option<RRect>` | not ported yet | needs `SkMatrix` (and `SkPathPriv::DeduceRRectFromContour`) |
| `SkRRect::dump`, `dumpToString`, `dumpHex` | `dump`, `dump_to_string`, `dump_hex` | not ported yet | need `SkString` / `SkAppendScalar` |
| `SkRRect::operator==` / `!=` | `PartialEq` | `PartialEq` comparing the rect and the 8 radii as floats (not the type) | Skia's semantics |
| `SkRRectPriv` | not exposed | `#[doc(hidden)] rrect::rrect_priv` free functions | PORTING §3 |
| `SkRRectPriv::ReadFromBuffer`, `WriteToBuffer` | not exposed | not ported yet | need `SkRBuffer` / `SkWBuffer` |
| `SkRRectPriv::{IsNearlySimpleCircular, AllCornersCircular, AllCornersRelativelyCircular, IsRelativelyCircular}` default `tolerance` | not exposed | `tolerance: impl Into<Option<scalar>>`, `None` = `SK_ScalarNearlyZero` | C++ default argument |
| `SkScaleToSides::AdjustRadii`, `SkFloatingPoint<float, 4>::AlmostEquals` | not exposed | private helpers in `rrect.rs` | only used by `SkRRect` so far; move out if another module needs them |
| **region** | | | |
| `SkRegion::RunHead` (ref-counted run array) | internal | private `Arc<RunHead>` (`Vec<i32>` runs), rebuilt on write | safe replacement for the ref-counted malloc block; run array contents and `writeToMemory` bytes match Skia |
| `SkRegion::Op` | `RegionOp` | `region::Op` (+ `pub type RegionOp = Op`) | both spellings available |
| `SkRegion::translate(dx, dy, dst)` | not exposed | `Region::translate_to(dx, dy, &mut dst)` | mechanical name; `translate(d)` / `translated(d)` as in skia-safe |
| `SkRegion::op(rgna, rgnb, op)` | not exposed | `Region::op_region_region(a, b, op)` | the other `op` overloads keep skia-safe's `op_rect`, `op_region`, `op_rect_region`, `op_region_rect` |
| `SkRegion::writeToMemory(nullptr)` | not exposed | `Region::write_to_memory_size()` | size query split from the write; `write_to_memory(&mut Vec<u8>)` as in skia-safe (native-endian `i32`s) |
| `SkRegion::Iterator::reset` | `reset(self, &Region) -> Iterator` | same (consumes `self`) | the iterator borrows the region, so a reset may change its lifetime |
| `SkRegion::Spanerator::next(int*, int*)` | `Iterator<Item = (i32, i32)>` | same | out-parameters become a tuple |
| `QuickReject` | crate-root trait | `region::QuickReject` | `skia-safe` defines it in `core.rs`; lives in `region` until another port needs it elsewhere |
| `SkRegionPriv::VisitSpans`, `Validate`, `kRunTypeSentinel`, `SkRegionValueIsSentinel` | not exposed | `#[doc(hidden)] region::region_priv::{visit_spans, validate, RUN_TYPE_SENTINEL, region_value_is_sentinel}` | PORTING §3 |
| `SkRegion::setPath`, `addBoundaryPath`, `getBoundaryPath` (`SkRegion_path.cpp`) | `set_path`, `add_boundary_path`, `boundary_path` | not ported yet | need `SkPath` / `SkPathBuilder` / scan conversion |
| `SkRegion::toString` (Android framework only) | not exposed | not ported | `SK_BUILD_FOR_ANDROID_FRAMEWORK` only |
