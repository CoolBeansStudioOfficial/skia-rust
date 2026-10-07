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
| **m44** | | | |
| `SkM44` | `M44` (`Clone`) | `M44` (`Copy + Clone`) | plain data, so `Copy` is a superset of skia-safe's API |
| `SkM44::operator==` | `PartialEq` via FFI | `PartialEq`, with the C++ `this == &other` shortcut (`std::ptr::eq`) | `m == m` is true even for NaN members, as in C++ |
| `SkM44::operator*` | `Mul` for `&M44` / `&V3` / `&V4` | same (`&M44 * &M44`, `&M44 * V3`, `&M44 * V4`) | same as skia-safe |
| `SkM44::preConcat(const SkMatrix&)` | not exposed | `M44::pre_concat_matrix` | overloads get distinct names |
| `SkM44::dump` | `dump` | not ported | needs `SkDebugf` |
| `SkM44::kUninitialized_Constructor` | none | not ported | no uninitialised values in safe Rust |
| `SkMatrixPriv::MapRect(const SkM44&, ...)` | not exposed | `matrix_priv::map_rect` | defined in `SkM44.cpp`, declared in `SkMatrixPriv.h` |
| `SkMatrixInvert.h` | not exposed | `matrix_invert::{invert_2x2_matrix, invert_3x3_matrix, invert_4x4_matrix}` | `outMatrix` may be null: `Option<&mut [scalar; N]>`; the determinant is returned as a `scalar` (a tiny double determinant underflows to 0, as in C++) |
| `SkV2` / `SkV3` / `SkV4` | `V2` / `V3` / `V4` | same | `as_array` / `as_mut_array` (unsafe in skia-safe) become `to_array` (by value); `SkV4::operator[]` is `Index` + `IndexMut`; the static `Dot` / `Cross` / `Normalize` are the same methods |
| **matrix** | | | |
| `SkMatrix` | `Matrix` (`Copy + Clone`) | `Matrix` (`Clone`, not `Copy`) | `fTypeMask` is `mutable` in C++ and is updated through `const` methods (`getType()`, ...). Rust stores it in an `AtomicU32` (relaxed), so `Matrix` is `Send + Sync` and `&self` getters keep the exact C++ caching behaviour, but it cannot be `Copy` |
| `SkMatrix::operator[]` (non-const) | `IndexMut<usize>` / `IndexMut<Member>` | same | dirties the type cache, as in C++ |
| `SkMatrix::kASkewY`, ... | `AffineMember` (+ `Index<AffineMember>`) | `AffineMember` (no `Index` impl) | skia-safe's `Index<AffineMember>` indexes the 3x3 array with the affine index, which is wrong; use `to_affine()` |
| `SkMatrix::get(int)`, `set(int, v)` | `IndexGet` / `IndexSet` traits | inherent `get` / `set` taking `impl Into<usize>` (`usize`, `Member`, `AffineMember`) | |
| `SkMatrix::setScale(sx, sy[, px, py])` and the other pivot overloads (`setRotate`, `setSkew`, `setSinCos`, `preScale`, ... `postSkew`) | `(.., pivot: impl Into<Option<Point>>)`; `None` is passed as pivot `(0, 0)` to the 4-arg C++ overload | same signature, but `None` calls the C++ overload *without* pivot | the two C++ overloads give different type masks (and NaN results), so skia-rust keeps them distinct |
| `SkMatrix::isSimilarity(tol)`, `preservesRightAngles(tol)` | no `tol` | `is_similarity` / `is_similarity_tol`, `preserves_right_angles` / `preserves_right_angles_tol` | overloads get distinct names |
| `SkMatrix::getMinMaxScales` | `min_max_scales() -> (scalar, scalar)` (ignores the `bool`) | `min_max_scales() -> Option<(scalar, scalar)>` | `None` when the C++ returns false |
| `SkMatrix::mapRadius` | `map_radius() -> Option<scalar>` (`None` with perspective) | `map_radius() -> scalar` | C++ also handles perspective |
| `SkMatrix::mapRect` | `map_rect() -> (Rect, bool)` | same, but panics (`unimplemented!`) for matrices with perspective | needs `SkPathBuilder::transform` and `SkPathPriv::PerspectiveClip` (SkPath, SkEdgeClipper); `Matrix_mapRect_skbug12335` stays `todo` |
| `SkMatrix::mapRectScaleTranslate` | `map_rect_scale_translate() -> Option<Rect>` | same | same as skia-safe |
| `SkMatrix::mapPoints` (span overloads) | `map_points(dst, src)` (asserts `dst.len() >= src.len()`), `map_points_inplace` | same names; maps `min(dst.len(), src.len())` points | C++ `min_count`; same for `map_vectors`, `map_homogeneous_points`, `map_points_to_homogeneous` |
| `SkMatrix::RectToRect`, `MakeRectToRect`, `setRectToRect` (`SK_SUPPORT_LEGACY_MATRIX_RECTTORECT`) | deprecated `rect_to_rect -> Option` / `from_rect_to_rect` | `make_rect_to_rect -> Matrix` (identity on failure), `set_rect_to_rect -> bool` (resets on failure) | C++ semantics; use `rect_2_rect` / `rect_to_rect_or_identity` in new code |
| `SkMatrix::PolyToPoly`, `setPolyToPoly` | `poly_to_poly`, `from_poly_to_poly`, `set_poly_to_poly` | `poly_to_poly`, `set_poly_to_poly` | `from_poly_to_poly` is a duplicate of `poly_to_poly` |
| `SkMatrix::postIDiv` (private) | deprecated `post_idiv` | `matrix_priv::post_i_div` | `SkMatrixPriv::PostIDiv` |
| `SkMatrix::I`, `InvalidMatrix` | `Matrix::i()`, `invalid_matrix()`, `matrix::IDENTITY` (`const`) | same, `matrix::IDENTITY` is a `static` | `Matrix` holds an atomic, so a `const` would trip `declare_interior_mutable_const` |
| `SkMatrix::setRSXform` | `set_rsxform` | not ported | needs `SkRSXform` |
| `SkMatrix::dump` | `dump` | not ported | needs `SkString` / `SkDebugf` |
| `SkMatrix::getMapPtsProc`, `SkMatrixPriv::GetMapPtsProc` | not exposed | not ported | function-pointer table; `map_points` dispatches on the type mask |
| `SkMatrixPriv::MapPointsWithStride` (2 overloads), `MapHomogeneousPointsWithStride` | not exposed | not ported | walk raw memory by byte stride: not expressible without `unsafe` |
| `SkMatrixPriv` | not exposed | `#[doc(hidden)] matrix_priv` free functions | PORTING §3 |
| `SkMatrixPriv::WriteToMemory`, `ReadFromMemory` | not exposed | `matrix_priv::write_to_memory(&Matrix, Option<&mut [u8]>) -> usize`, `read_from_memory(&mut Matrix, &[u8]) -> usize` | `Option` for the null buffer; native-endian floats; panics if `buffer` is shorter than the returned size |
| `SkMatrixPriv::InverseMapRect` | not exposed | `matrix_priv::inverse_map_rect(&Matrix, &Rect) -> Option<Rect>` | `bool` + out-param becomes `Option` |
| `SkMatrixPriv::CheapEqual` | not exposed | `matrix_priv::cheap_equal` | bitwise compare of the nine members (`memcmp`) |
| `SkMatrixPriv::M44ColMajor` | not exposed | `matrix_priv::m44_col_major` (returns `[scalar; 16]` by value) | no pointer into the matrix |
| `SkMatrixPriv::NearlyAffine` | not exposed | `matrix_priv::nearly_affine(m, bounds, tolerance)` | no default argument: pass `SCALAR_NEARLY_ZERO` |
| `SkMatrixPriv::kMaxFlattenSize` | not exposed | `matrix_priv::MAX_FLATTEN_SIZE` | |
| `SkPathPriv::kW0PlaneDistance` | not exposed | `matrix_priv::W0_PLANE_DISTANCE` | needed by `matrix_priv::map_rect`; move to `path_priv` when SkPath is ported |
| `SkDecomposeUpper2x2` (`SkMatrixUtils.h`) | not exposed | `#[doc(hidden)] matrix_utils::decompose_upper_2x2(&Matrix, Option<&mut Point>, Option<&mut Point>, Option<&mut Point>) -> bool` | null out-params become `Option` |
| `SkTreatAsSprite` (`SkMatrixUtils.h`) | not exposed | not ported | needs `SkSamplingOptions` |
| **geometry** | | | |
| `SkGeometry.h` free functions (`SkChopCubicAt`, `SkEvalQuadAt`, ...) | not exposed | `geometry::{chop_cubic_at, eval_quad_at, ...}` | mechanical names; `src`/`dst` pointers become slices, a nullable `dst` becomes `Option<&mut [Point]>`, out-arrays stay `&mut [scalar; N]` parameters and counts are `usize` |
| `SkChopCubicAt` (3 overloads) | not exposed | `chop_cubic_at`, `chop_cubic_at_t0_t1`, `chop_cubic_at_ts` | overloads get distinct names; `tCount` is `t_values.len()` |
| `SkEvalQuadAt(src, t, SkPoint*, SkVector*)` | not exposed | `eval_quad_at_pos_tangent` (with `eval_quad_at`, `eval_quad_tangent_at`) | overloads get distinct names; the pointers are `Option<&mut _>` |
| `SkClassifyCubic(p, t, s, d)` | not exposed | `classify_cubic(p)`, `classify_cubic_with(p, Option<&mut [f64; 2]>, ..)` | default arguments split into two functions |
| `SkCubicType`, `SkCubicIsDegenerate`, `SkCubicTypeName` | not exposed | `CubicType` (+ `is_degenerate`, `name`) | enum with methods |
| `SkConic` | not exposed | `geometry::Conic { pts, w }` | public fields as in C++; `set` overloads are `set` / `set_points`; the constructors are `new` / `from_points` |
| `SkConic::evalAt(t, SkPoint*, SkVector*)`, `chopAt(t1, t2, SkConic*)` | not exposed | `eval_at_pos_tangent`, `chop_at_interval` | overloads get distinct names |
| `SkConic::findXExtrema/findYExtrema(SkScalar*)`, `computeAsQuadError(SkVector*)`, `computeTightBounds(SkRect*)`, `computeFastBounds(SkRect*)` | not exposed | return `Option<scalar>` / `Vector` / `Rect` | out-parameters become return values |
| `SkConic::TransformW`, `SkConic::BuildUnitArc` | not exposed | not ported yet | need `SkMatrix` and `SkPathDirection` |
| `SkAutoConicToQuads::computeQuads` (3 overloads) | not exposed | `AutoConicToQuads::{compute_quads, compute_quads_with_weight}` | the pointer and `SkSpan` overloads merge; the storage is a `Vec<Point>` instead of `AutoSTMalloc` |
| `SkQuadCoeff`, `SkConicCoeff`, `SkCubicCoeff` | not exposed | `geometry::{QuadCoeff, ConicCoeff, CubicCoeff}` with public `a, b, c, d` / `numer, denom` fields | public so `CubicMapTest` can use them |
| `skgpu::tess::FindCubicConvex180Chops(pts, T, bool*)` | not exposed | `tessellation::find_cubic_convex_180_chops(pts, &mut t, &mut are_cusps)` | lives in `skia-rust-core` until a GPU crate exists |
| `SkQuads`, `SkCubics` (classes of statics) | not exposed | modules `quads`, `cubics` (`roots_real`, `roots_valid_t`, `eval_at`, ...) | class with only statics becomes a module; `solution` out-arrays stay `&mut [f64; N]` |
| `SkBezierCubic`, `SkBezierQuad` | not exposed | `bezier_curves::{BezierCubic, BezierQuad}` (unit structs with associated functions) | `SkSpan<const float>` results are slices of the caller's storage array |
| `SkCubicClipper::ChopMonoAtY(pts, y, SkScalar*)` | not exposed | `CubicClipper::chop_mono_at_y(pts, y) -> Option<scalar>` | bool + out-param becomes `Option` |
| `SkCubicMap` | `CubicMap` | `cubic_map::CubicMap` | same API as skia-safe (`new`, `is_linear`, `compute_y_from_x`, `compute_from_t`) |
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
| `SkRectPriv::QuadContainsRect` (2 overloads), `QuadContainsRectMask` | not exposed | `rect_priv::quad_contains_rect` (`&Matrix`, `&IRect`), `quad_contains_rect_m44`, `quad_contains_rect_mask` (returns `vx::Int4`) | overloads get distinct names; the C++ default `tol = 0.f` is an explicit `tol` argument |
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
