// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/PatchWriter.h (chrome/m156)

#![allow(clippy::float_cmp)] // the C++ compares control points and tangents with exact ==
#![allow(clippy::too_many_arguments)] // mirrors the C++ helper signatures
#![allow(clippy::cast_precision_loss)] // mirrors the implicit int -> float conversions in C++

//! `PatchWriter` writes tessellation patches, formatted with their specific attribs, into a GPU
//! buffer.
//!
//! C++ selects the attribute layout and the algorithm variants with a variadic list of traits
//! (`Required<A>`, `Optional<A>`, `TrackJoinControlPoints`, ...). Rust has no variadic generics,
//! so the same information lives in a [`PatchWriterTraits`] implementation: one associated
//! constant per attribute saying whether it is [`AttribMode::Required`], [`AttribMode::Optional`]
//! or [`AttribMode::Disabled`], and one `bool` per feature trait. Each configuration Skia
//! instantiates is a unit struct below (see the `*Traits` types at the end of this file).
//!
//! Attribute values are stored as [`AttribValue`]s: the value plus the runtime enabled bit taken
//! from the `PatchAttribs` mask. `Required` attributes are always written, `Optional` ones only
//! when enabled, `Disabled` ones never. C++ removes disabled members at compile time; here the
//! mode is an associated constant, so the match in [`write_attrib`] folds away.
//!
//! The `PatchAllocator` is the C++ `PatchAllocator` template parameter. The Ganesh and Graphite
//! allocators are not ported yet, so the writer is generic over the [`PatchAllocator`] trait.

use std::marker::PhantomData;

use skia_rust_core::color::PMColor4f;
use skia_rust_core::paint::Cap;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar_ceil_to_int;
use skia_rust_simd::vx::{Float2, Float4};

use crate::gpu::buffer_writer::{BufferWrite, BufferWriter, Repeat, VertexColor, VertexWriter};
use crate::tessellate::linear_tolerances::LinearTolerances;
use crate::tessellate::middle_out_polygon_triangulator::MiddleOutPolygonTriangulator;
use crate::tessellate::tessellation::{
    K_CONIC_CURVE_TYPE, K_CUBIC_CURVE_TYPE, K_MAX_PARAMETRIC_SEGMENTS_P4,
    K_MAX_SEGMENTS_PER_CURVE_P4, K_PRECISION, K_TRIANGULAR_CONIC_CURVE_TYPE, PatchAttribs,
    StrokeParams, patch_stride,
};
use crate::tessellate::wangs_formula::{self, VectorXform};

/// The largest stride a patch can have: four control points, then every attrib at its widest.
/// Sizes are the C++ `kMaxStride` terms; the wide-color term is `sizeof(SkPMColor4f)`.
// Port of: src/gpu/tessellate/PatchWriter.h#L187-L195 (chrome/m156), `PatchStorage::kMaxStride`.
const K_MAX_STRIDE: usize = 4 * size_of::<Point>()
    + size_of::<Point>()
    + size_of::<Point>()
    + size_of::<StrokeParams>()
    + 4 * size_of::<f32>()
    + size_of::<f32>()
    + size_of::<f32>()
    + 2 * size_of::<u32>();

/// Whether an attribute is written: always, when enabled at runtime, or never.
///
/// These map onto C++'s `Required<A>`, `Optional<A>`, and the absence of a trait for `A`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttribMode {
    /// The attribute is not part of the patch layout (`has_trait` is false for both
    /// `Required<A>` and `Optional<A>`).
    Disabled,
    /// `Optional<A>`: the attribute is written only when `PatchAttribs` includes it.
    Optional,
    /// `Required<A>`: the attribute is always part of the layout.
    Required,
}

/// The compile-time configuration of a [`PatchWriter`]: the C++ trait pack.
///
/// Every attribute defaults to [`AttribMode::Disabled`] and every feature to `false`, so an
/// implementation only lists what its instantiation enables.
// Port of: src/gpu/tessellate/PatchWriter.h#L48-L71 (chrome/m156), `Required`, `Optional`,
// `TrackJoinControlPoints`, `AddTrianglesWhenChopping`, `DiscardFlatCurves`, `ReplicateLineEndPoints`.
pub trait PatchWriterTraits {
    /// `Optional<kJoinControlPoint>` / `Required<kJoinControlPoint>`.
    const JOIN_CONTROL_POINT: AttribMode = AttribMode::Disabled;
    /// `Optional<kFanPoint>` / `Required<kFanPoint>`.
    const FAN_POINT: AttribMode = AttribMode::Disabled;
    /// `Optional<kStrokeParams>` / `Required<kStrokeParams>`.
    const STROKE_PARAMS: AttribMode = AttribMode::Disabled;
    /// `Optional<kColor>` / `Required<kColor>`.
    const COLOR: AttribMode = AttribMode::Disabled;
    /// `Optional<kWideColorIfEnabled>` / `Required<kWideColorIfEnabled>`.
    const WIDE_COLOR_IF_ENABLED: AttribMode = AttribMode::Disabled;
    /// `Optional<kPaintDepth>` / `Required<kPaintDepth>`.
    const PAINT_DEPTH: AttribMode = AttribMode::Disabled;
    /// `Optional<kExplicitCurveType>` / `Required<kExplicitCurveType>`.
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Disabled;
    /// `Optional<kSsboIndex>` / `Required<kSsboIndex>`.
    const SSBO_INDEX: AttribMode = AttribMode::Disabled;
    /// `TrackJoinControlPoints`: defers the patches of a stroke until the next join is known.
    const TRACK_JOIN_CONTROL_POINTS: bool = false;
    /// `AddTrianglesWhenChopping`: also emits triangles that fill the chopped-off interior.
    const ADD_TRIANGLES_WHEN_CHOPPING: bool = false;
    /// `DiscardFlatCurves`: drops curves whose tessellation needs no more than one segment.
    const DISCARD_FLAT_CURVES: bool = false;
    /// `ReplicateLineEndPoints`: writes lines as cubics with replicated end points.
    const REPLICATE_LINE_END_POINTS: bool = false;
}

/// The writer's storage for one buffer slot: `PatchAllocator::append` returns a [`VertexWriter`]
/// over the GPU buffer, or `None` when it is full.
// Port of: src/gpu/tessellate/PatchWriter.h#L213-L220 (chrome/m156), `PatchAllocator` (concept).
pub trait PatchAllocator {
    /// Returns a writer for one instance worth of data, with the tolerances of the curve that
    /// will be written to it, or `None` if no space is left.
    fn append(&mut self, tolerances: &LinearTolerances) -> Option<VertexWriter<'_>>;
}

/// One attribute of a patch: its value, and whether `PatchAttribs` enables it at runtime.
// Port of: src/gpu/tessellate/PatchWriter.h#L77-L116 (chrome/m156), `AttribValue`.
#[derive(Clone, Copy, Debug, Default)]
struct AttribValue<T> {
    value: T,
    enabled: bool,
}

/// Writes one attribute if its mode says so.
// Port of: src/gpu/tessellate/PatchWriter.h#L118-L129 (chrome/m156), `operator<<(VertexWriter&, AttribValue)`.
fn write_attrib<T: BufferWrite>(w: &mut VertexWriter<'_>, mode: AttribMode, v: &AttribValue<T>) {
    match mode {
        AttribMode::Required => {
            w.put(&v.value);
        }
        AttribMode::Optional => {
            if v.enabled {
                w.put(&v.value);
            }
        }
        AttribMode::Disabled => {}
    }
}

/// The runtime enabled bit of an attribute. The C++ constructor asserts that a `Required`
/// attribute is present and a disabled one is absent.
// Port of: src/gpu/tessellate/PatchWriter.h#L84-L103 (chrome/m156), `AttribValue::AttribValue`.
fn attrib_enabled(mode: AttribMode, attribs: PatchAttribs, bit: PatchAttribs) -> bool {
    match mode {
        AttribMode::Required => {
            debug_assert!(attribs.contains(bit));
            true
        }
        AttribMode::Optional => attribs.contains(bit),
        AttribMode::Disabled => {
            debug_assert!(!attribs.contains(bit));
            false
        }
    }
}

/// The attribute values of the writer, apart from the allocator and the deferred patch.
// Port of: src/gpu/tessellate/PatchWriter.h#L380-L398 (chrome/m156), the `f*` attrib members of `PatchWriter`.
#[derive(Clone, Copy, Debug)]
struct Attributes {
    attribs: PatchAttribs,
    join: AttribValue<Point>,
    fan_point: AttribValue<Point>,
    stroke_params: AttribValue<StrokeParams>,
    color: AttribValue<VertexColor>,
    depth: AttribValue<f32>,
    ssbo_index: AttribValue<u32>,
}

impl Attributes {
    // Port of: src/gpu/tessellate/PatchWriter.h#L392-L404 (chrome/m156), `PatchWriter` constructor.
    fn new<C: PatchWriterTraits>(attribs: PatchAttribs) -> Self {
        Self {
            attribs,
            join: AttribValue {
                value: Point::default(),
                enabled: attrib_enabled(
                    C::JOIN_CONTROL_POINT,
                    attribs,
                    PatchAttribs::JOIN_CONTROL_POINT,
                ),
            },
            fan_point: AttribValue {
                value: Point::default(),
                enabled: attrib_enabled(C::FAN_POINT, attribs, PatchAttribs::FAN_POINT),
            },
            stroke_params: AttribValue {
                value: StrokeParams::default(),
                enabled: attrib_enabled(C::STROKE_PARAMS, attribs, PatchAttribs::STROKE_PARAMS),
            },
            color: AttribValue {
                value: VertexColor::default(),
                enabled: attrib_enabled(C::COLOR, attribs, PatchAttribs::COLOR),
            },
            depth: AttribValue {
                value: 0.0,
                enabled: attrib_enabled(C::PAINT_DEPTH, attribs, PatchAttribs::PAINT_DEPTH),
            },
            ssbo_index: AttribValue {
                value: 0,
                enabled: attrib_enabled(C::SSBO_INDEX, attribs, PatchAttribs::SSBO_INDEX),
            },
        }
    }
}

/// Whether the wide color flag applies to the color attrib, from the trait and `PatchAttribs`.
// Port of: src/gpu/tessellate/PatchWriter.h#L318-L329 (chrome/m156), the wide-color branches of `updateColorAttrib`.
fn wide_color<C: PatchWriterTraits>(attribs: PatchAttribs) -> bool {
    match C::WIDE_COLOR_IF_ENABLED {
        AttribMode::Required => {
            debug_assert!(attribs.contains(PatchAttribs::WIDE_COLOR_IF_ENABLED));
            true
        }
        AttribMode::Optional => attribs.contains(PatchAttribs::WIDE_COLOR_IF_ENABLED),
        AttribMode::Disabled => {
            debug_assert!(!attribs.contains(PatchAttribs::WIDE_COLOR_IF_ENABLED));
            false
        }
    }
}

/// The bytes of a deferred patch: the control points and attribs of the patch that waits for its
/// join control point.
// Port of: src/gpu/tessellate/PatchWriter.h#L153-L186 (chrome/m156), `PatchStorage`.
#[derive(Clone, Copy, Debug)]
struct PatchStorage {
    // This is the first control point of the contour, e.g. moveTo.
    first_control_point: Point,
    // The last control point written within the contour.
    last_control_point: Point,
    // The explicit curve type passed to writePatch().
    curve_type: f32,
    // The parametric segment value to restore on LinearTolerances.
    n_p4: f32,
    data: [u8; K_MAX_STRIDE],
}

impl Default for PatchStorage {
    fn default() -> Self {
        Self {
            first_control_point: Point::default(),
            last_control_point: Point::default(),
            curve_type: -1.0,
            n_p4: -1.0,
            data: [0; K_MAX_STRIDE],
        }
    }
}

impl PatchStorage {
    // Port of: src/gpu/tessellate/PatchWriter.h#L160-L163 (chrome/m156), `PatchStorage::disableDeferral`.
    fn disable_deferral(&mut self) {
        // do nothing if we already have a deferred patch recorded
        // std::max(fN_p4, 0.f)
        self.n_p4 = if self.n_p4 < 0.0 { 0.0 } else { self.n_p4 };
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L164-L166 (chrome/m156), `PatchStorage::mustDefer`.
    fn must_defer(&self) -> bool {
        self.n_p4 < 0.0
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L167-L169 (chrome/m156), `PatchStorage::hasVerb`.
    fn has_verb(&self) -> bool {
        self.curve_type >= 0.0
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L170-L172 (chrome/m156), `PatchStorage::hasPending`.
    fn has_pending(&self) -> bool {
        self.n_p4 > 0.0
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L173-L178 (chrome/m156), `PatchStorage::reset`.
    fn reset(&mut self, move_to: Point) {
        self.curve_type = -1.0;
        self.n_p4 = -1.0;
        self.first_control_point = move_to;
        self.last_control_point = move_to;
    }
}

/// `std::max(a, b)`: returns `b` only when `a < b`.
fn cpp_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: returns `b` only when `b < a`.
fn cpp_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

fn f2(p: Point) -> Float2 {
    Float2::new(p.x, p.y)
}

fn pt(v: Float2) -> Point {
    Point::new(v[0], v[1])
}

/// Writes the two lanes of a float2, as `VertexWriter << float2` does.
fn put_f2(vw: &mut VertexWriter<'_>, v: Float2) {
    vw.put(&v[0]);
    vw.put(&v[1]);
}

fn splat2(s: f32) -> Float2 {
    Float2::splat(s)
}

fn splat4(s: f32) -> Float4 {
    Float4::splat(s)
}

/// Exact componentwise equality of two float2s (`all(a == b)`).
fn eq2(a: Float2, b: Float2) -> bool {
    a[0] == b[0] && a[1] == b[1]
}

/// `mix(a, b, T) = (b - a) * T + a`, lane by lane.
// Port of: src/gpu/tessellate/PatchWriter.h#L434-L438 (chrome/m156), `PatchWriter::mix`.
fn mix4(a: Float4, b: Float4, t: Float4) -> Float4 {
    (b - a) * t + a
}

/// `TangentPoint(p0, p1, p2, p3)`: the first of p1, p2, p3 that is not a repeat.
// Port of: src/gpu/tessellate/PatchWriter.h#L724-L732 (chrome/m156), `PatchWriter::TangentPoint`.
fn tangent_point(p0: Float2, p1: Float2, p2: Float2, p3: Float2) -> Float2 {
    if !eq2(p0, p1) {
        p1
    } else if !eq2(p2, p1) {
        p2
    } else {
        p3
    }
}

fn read_f32(data: &[u8], offset: usize) -> f32 {
    f32::from_ne_bytes(data[offset..offset + 4].try_into().expect("four bytes"))
}

/// Writes a `float4` made of four floats starting at `offset` in `data`, as `float4::Load` does.
fn load4(data: &[u8], offset: usize) -> Float4 {
    Float4::new(
        read_f32(data, offset),
        read_f32(data, offset + 4),
        read_f32(data, offset + 8),
        read_f32(data, offset + 12),
    )
}

/// Writes one patch: appends to the allocator, or to the deferred patch when it must wait.
// Port of: src/gpu/tessellate/PatchWriter.h#L577-L588 (chrome/m156), `PatchWriter::appendPatch`.
fn append_patch<'a, C: PatchWriterTraits, A: PatchAllocator>(
    allocator: &'a mut A,
    deferred: &'a mut PatchStorage,
    tolerances: &LinearTolerances,
    stride: usize,
    explicit_curve_type: f32,
) -> Option<VertexWriter<'a>> {
    if C::TRACK_JOIN_CONTROL_POINTS && deferred.must_defer() {
        debug_assert!(stride <= K_MAX_STRIDE);
        deferred.curve_type = explicit_curve_type;
        deferred.n_p4 = tolerances.num_parametric_segments_p4();
        debug_assert!(!deferred.must_defer() && deferred.has_pending());
        return Some(VertexWriter::new(&mut deferred.data[..stride]));
    }
    allocator.append(tolerances)
}

/// Writes the attribs that follow the control points of a patch, in the C++ order:
/// join, fan point, stroke params, color, depth, curve type, SSBO index.
// Port of: src/gpu/tessellate/PatchWriter.h#L536-L541 (chrome/m156), `PatchWriter::emitPatchAttribs`.
fn emit_patch_attribs<C: PatchWriterTraits>(
    attrs: &Attributes,
    vw: &mut VertexWriter<'_>,
    join: AttribValue<Point>,
    explicit_curve_type: f32,
) {
    write_attrib(vw, C::JOIN_CONTROL_POINT, &join);
    write_attrib(vw, C::FAN_POINT, &attrs.fan_point);
    write_attrib(vw, C::STROKE_PARAMS, &attrs.stroke_params);
    write_attrib(vw, C::COLOR, &attrs.color);
    write_attrib(vw, C::PAINT_DEPTH, &attrs.depth);
    let curve_type = AttribValue {
        value: explicit_curve_type,
        enabled: attrs.attribs.contains(PatchAttribs::EXPLICIT_CURVE_TYPE),
    };
    write_attrib(vw, C::EXPLICIT_CURVE_TYPE, &curve_type);
    write_attrib(vw, C::SSBO_INDEX, &attrs.ssbo_index);
}

/// Writes tessellation patches, formatted with their attribs, into the allocator's buffers.
///
/// `C` selects the attribute layout and algorithm variants; `A` allocates the instance data.
// Port of: src/gpu/tessellate/PatchWriter.h#L222-L915 (chrome/m156), `PatchWriter`.
pub struct PatchWriter<C: PatchWriterTraits, A: PatchAllocator> {
    attrs: Attributes,
    approx_transform: VectorXform,
    max_scale: f32,
    tolerances: LinearTolerances,
    allocator: A,
    // Only used when C::TRACK_JOIN_CONTROL_POINTS is set. (C++ uses `std::monostate` otherwise.)
    deferred: PatchStorage,
    _traits: PhantomData<C>,
}

impl<C: PatchWriterTraits, A: PatchAllocator> std::fmt::Debug for PatchWriter<C, A> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PatchWriter")
            .field("attribs", &self.attrs.attribs)
            .field("max_scale", &self.max_scale)
            .finish_non_exhaustive()
    }
}

impl<C: PatchWriterTraits, A: PatchAllocator> Drop for PatchWriter<C, A> {
    // Port of: src/gpu/tessellate/PatchWriter.h#L297-L301 (chrome/m156), `PatchWriter::~PatchWriter`.
    fn drop(&mut self) {
        if C::TRACK_JOIN_CONTROL_POINTS {
            debug_assert!(!self.deferred.has_pending());
        }
    }
}

impl<C: PatchWriterTraits, A: PatchAllocator> PatchWriter<C, A> {
    /// Creates a writer for `attribs`. `make_allocator` receives the patch stride in bytes, as the
    /// C++ `PatchAllocator` constructor does.
    // Port of: src/gpu/tessellate/PatchWriter.h#L268-L296 (chrome/m156), `PatchWriter::PatchWriter`.
    pub fn new(attribs: PatchAttribs, make_allocator: impl FnOnce(usize) -> A) -> Self {
        const {
            assert!(
                !C::TRACK_JOIN_CONTROL_POINTS
                    || matches!(C::JOIN_CONTROL_POINT, AttribMode::Required),
                "Deferred patches and auto-updating joins requires kJoinControlPoint attrib"
            );
        }
        let attrs = Attributes::new::<C>(attribs);
        // The wide-color attrib is checked by the same rules as the other attribs.
        let _ = wide_color::<C>(attribs);
        Self {
            attrs,
            approx_transform: VectorXform::default(),
            max_scale: 1.0,
            tolerances: LinearTolerances::default(),
            allocator: make_allocator(patch_stride(attribs)),
            deferred: PatchStorage::default(),
            _traits: PhantomData,
        }
    }

    /// The `PatchAttribs` this writer was created with.
    // Port of: src/gpu/tessellate/PatchWriter.h#L302 (chrome/m156), `PatchWriter::attribs`.
    #[must_use]
    pub fn attribs(&self) -> PatchAttribs {
        self.attrs.attribs
    }

    /// Sets the transform used to estimate tessellation density, and the largest scale it applies.
    // Port of: src/gpu/tessellate/PatchWriter.h#L303-L307 (chrome/m156), `PatchWriter::setShaderTransform`.
    pub fn set_shader_transform(&mut self, xform: VectorXform, max_scale: f32) {
        self.approx_transform = xform;
        self.max_scale = max_scale;
    }

    /// Writes the deferred stroke patch that starts at the current contour, closing it with `cap`
    /// (or with the current join when `cap` is `None`), and resets the deferral to `move_to`.
    /// C++ `writeDeferredStrokePatch(SkPoint moveTo, std::optional<SkPaint::Cap> cap)`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L331-L375 (chrome/m156), `PatchWriter::writeDeferredStrokePatch`.
    pub fn write_deferred_stroke_patch(&mut self, move_to: Point, cap: Option<Cap>) {
        const { assert!(C::TRACK_JOIN_CONTROL_POINTS) };
        if self.deferred.has_pending() {
            debug_assert!(self.deferred.has_verb());
            let join: Point = match cap {
                None => self.attrs.join.value,
                Some(cap) => {
                    let join = self.deferred.first_control_point;
                    if cap == Cap::Round {
                        self.write_circle(self.deferred.last_control_point);
                        self.write_circle(self.deferred.first_control_point);
                    } else if cap == Cap::Square {
                        self.write_square(
                            self.deferred.last_control_point,
                            Some(self.attrs.join.value),
                        );
                        let p01 = load4(&self.deferred.data, 0);
                        // fData + 4*sizeof(float): the floats of p2 and p3.
                        let p23 = load4(&self.deferred.data, 4 * size_of::<f32>());
                        let last = if self.deferred.curve_type == K_CUBIC_CURVE_TYPE {
                            Float2::new(p23[2], p23[3])
                        } else {
                            Float2::new(p23[0], p23[1])
                        };
                        let cap_pt = tangent_point(
                            Float2::new(p01[0], p01[1]),
                            Float2::new(p01[2], p01[3]),
                            Float2::new(p23[0], p23[1]),
                            last,
                        );
                        self.write_square(Point::new(p01[0], p01[1]), Some(pt(cap_pt)));
                    }
                    join
                }
            };
            // memcpy(fData + 4 * sizeof(SkPoint), &join, sizeof(SkPoint))
            let join_bytes = join_bytes(join);
            self.deferred.data[4 * size_of::<Point>()..5 * size_of::<Point>()]
                .copy_from_slice(&join_bytes);
            self.tolerances.set_parametric_segments(self.deferred.n_p4);
            let stride = patch_stride(self.attrs.attribs);
            if let Some(mut vw) = self.allocator.append(&self.tolerances) {
                vw.put(&self.deferred.data[..stride]);
            }
        } else if let (Some(cap), true) = (cap, self.deferred.has_verb()) {
            match cap {
                Cap::Butt => {}
                Cap::Round => self.write_circle(self.deferred.first_control_point),
                Cap::Square => self.write_square(self.deferred.first_control_point, None),
            }
            debug_assert!(!self.deferred.has_pending());
        }
        self.deferred.reset(move_to);
    }

    /// Closes the current contour with a line from its last point back to its first, then writes
    /// the deferred patch with `cap`. C++ `closeDeferredStrokePatch(SkPaint::Cap cap)`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L376-L384 (chrome/m156), `PatchWriter::closeDeferredStrokePatch`.
    pub fn close_deferred_stroke_patch(&mut self, cap: Cap) {
        const { assert!(C::TRACK_JOIN_CONTROL_POINTS) };
        self.write_line(
            self.deferred.last_control_point,
            self.deferred.first_control_point,
        );
        if self.deferred.has_pending() {
            self.write_deferred_stroke_patch(self.deferred.first_control_point, None);
        } else {
            debug_assert!(self.deferred.has_verb());
            self.write_deferred_stroke_patch(self.deferred.first_control_point, Some(cap));
        }
    }

    /// The no-argument `writeDeferredStrokePatch()` of C++: writes the pending patch with the
    /// current join and no cap, leaving the origin as the move-to.
    // Port of: src/gpu/tessellate/PatchWriter.h#L385-L387 (chrome/m156), `PatchWriter::writeDeferredStrokePatch()`.
    pub fn write_pending_deferred_stroke_patch(&mut self) {
        const {
            assert!(matches!(
                C::JOIN_CONTROL_POINT,
                AttribMode::Required | AttribMode::Optional
            ));
        };
        self.write_deferred_stroke_patch(Point::new(0.0, 0.0), None);
    }

    /// Updates the join control point. The next deferred patch can use it, so nothing needs to
    /// wait for it any more.
    // Port of: src/gpu/tessellate/PatchWriter.h#L388-L393 (chrome/m156), `PatchWriter::updateJoinControlPointAttrib`.
    pub fn update_join_control_point_attrib(&mut self, last_control_point: Point) {
        const { assert!(!matches!(C::JOIN_CONTROL_POINT, AttribMode::Disabled)) };
        debug_assert!(
            self.attrs
                .attribs
                .contains(PatchAttribs::JOIN_CONTROL_POINT)
        );
        self.attrs.join.value = last_control_point;
        // fJoin is valid, so no need to defer next real patch
        self.deferred.disable_deferral();
    }

    /// Sets the fan point attrib of subsequent patches.
    // Port of: src/gpu/tessellate/PatchWriter.h#L394-L399 (chrome/m156), `PatchWriter::updateFanPointAttrib`.
    pub fn update_fan_point_attrib(&mut self, fan_point: Point) {
        const { assert!(!matches!(C::FAN_POINT, AttribMode::Disabled)) };
        debug_assert!(self.attrs.attribs.contains(PatchAttribs::FAN_POINT));
        self.attrs.fan_point.value = fan_point;
    }

    /// Sets the stroke params attrib of subsequent patches, and the tolerances they imply.
    // Port of: src/gpu/tessellate/PatchWriter.h#L400-L406 (chrome/m156), `PatchWriter::updateStrokeParamsAttrib`.
    pub fn update_stroke_params_attrib(&mut self, stroke_params: StrokeParams) {
        const { assert!(!matches!(C::STROKE_PARAMS, AttribMode::Disabled)) };
        debug_assert!(self.attrs.attribs.contains(PatchAttribs::STROKE_PARAMS));
        self.attrs.stroke_params.value = stroke_params;
        self.tolerances.set_stroke(&stroke_params, self.max_scale);
    }

    /// Sets the stroke params that every patch uses, when the attrib itself is not written.
    // Port of: src/gpu/tessellate/PatchWriter.h#L407-L411 (chrome/m156), `PatchWriter::updateUniformStrokeParams`.
    pub fn update_uniform_stroke_params(&mut self, stroke_params: StrokeParams) {
        const { assert!(!matches!(C::STROKE_PARAMS, AttribMode::Disabled)) };
        debug_assert!(!self.attrs.attribs.contains(PatchAttribs::STROKE_PARAMS));
        self.tolerances.set_stroke(&stroke_params, self.max_scale);
    }

    /// Sets the color attrib of subsequent patches. Wide colors are written as four floats.
    // Port of: src/gpu/tessellate/PatchWriter.h#L412-L424 (chrome/m156), `PatchWriter::updateColorAttrib`.
    pub fn update_color_attrib(&mut self, color: &PMColor4f) {
        const { assert!(!matches!(C::COLOR, AttribMode::Disabled)) };
        debug_assert!(self.attrs.attribs.contains(PatchAttribs::COLOR));
        self.attrs.color.value = VertexColor::new(color, wide_color::<C>(self.attrs.attribs));
    }

    /// Sets the paint depth attrib of subsequent patches.
    // Port of: src/gpu/tessellate/PatchWriter.h#L425-L430 (chrome/m156), `PatchWriter::updatePaintDepthAttrib`.
    pub fn update_paint_depth_attrib(&mut self, depth: f32) {
        const { assert!(!matches!(C::PAINT_DEPTH, AttribMode::Disabled)) };
        debug_assert!(self.attrs.attribs.contains(PatchAttribs::PAINT_DEPTH));
        self.attrs.depth.value = depth;
    }

    /// Sets the SSBO index attrib of subsequent patches.
    // Port of: src/gpu/tessellate/PatchWriter.h#L431-L436 (chrome/m156), `PatchWriter::updateSsboIndexAttrib`.
    pub fn update_ssbo_index_attrib(&mut self, ssbo_index: u32) {
        const { assert!(!matches!(C::SSBO_INDEX, AttribMode::Disabled)) };
        debug_assert!(self.attrs.attribs.contains(PatchAttribs::SSBO_INDEX));
        self.attrs.ssbo_index.value = ssbo_index;
    }

    // ---- writeX: every geometric type is converted to an equivalent cubic or conic. ----

    /// Writes a cubic from its four control points.
    // Port of: src/gpu/tessellate/PatchWriter.h#L438-L449 (chrome/m156), `PatchWriter::writeCubic(float2 ...)`.
    pub fn write_cubic_f2(&mut self, p0: Float2, p1: Float2, p2: Float2, p3: Float2) {
        let n4 = wangs_formula::cubic_p4_vec(K_PRECISION, p0, p1, p2, p3, &self.approx_transform);
        if C::DISCARD_FLAT_CURVES && n4 <= 1.0 {
            return;
        }
        let num_patches = self.account_for_curve(n4);
        if num_patches != 0 {
            self.chop_and_write_cubics(p0, p1, p2, p3, num_patches);
        } else {
            self.write_cubic_patch(p0, p1, p2, p3);
        }
    }

    /// Writes a cubic from an `SkPoint[4]`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L450-L455 (chrome/m156), `PatchWriter::writeCubic(const SkPoint[4])`.
    pub fn write_cubic(&mut self, pts: &[Point; 4]) {
        self.write_cubic_f2(f2(pts[0]), f2(pts[1]), f2(pts[2]), f2(pts[3]));
    }

    /// Writes a conic from its three control points and weight. The conic is stored as a cubic
    /// with `{w, inf}` in its last control point.
    // Port of: src/gpu/tessellate/PatchWriter.h#L456-L467 (chrome/m156), `PatchWriter::writeConic(float2 ...)`.
    pub fn write_conic_f2(&mut self, p0: Float2, p1: Float2, p2: Float2, w: f32) {
        let n2 = wangs_formula::conic_p2_vec(K_PRECISION, p0, p1, p2, w, &self.approx_transform);
        if C::DISCARD_FLAT_CURVES && n2 <= 1.0 {
            return;
        }
        let num_patches = self.account_for_curve(n2 * n2);
        if num_patches != 0 {
            self.chop_and_write_conics(p0, p1, p2, w, num_patches);
        } else {
            self.write_conic_patch(p0, p1, p2, w);
        }
    }

    /// Writes a conic from an `SkPoint[3]` and its weight.
    // Port of: src/gpu/tessellate/PatchWriter.h#L468-L474 (chrome/m156), `PatchWriter::writeConic(const SkPoint[3], float)`.
    pub fn write_conic(&mut self, pts: &[Point; 3], w: f32) {
        self.write_conic_f2(f2(pts[0]), f2(pts[1]), f2(pts[2]), w);
    }

    /// Writes a quadratic, converted to an equivalent cubic.
    // Port of: src/gpu/tessellate/PatchWriter.h#L475-L487 (chrome/m156), `PatchWriter::writeQuadratic(float2 ...)`.
    pub fn write_quadratic_f2(&mut self, p0: Float2, p1: Float2, p2: Float2) {
        let n4 = wangs_formula::quadratic_p4_vec(K_PRECISION, p0, p1, p2, &self.approx_transform);
        if C::DISCARD_FLAT_CURVES && n4 <= 1.0 {
            return;
        }
        let num_patches = self.account_for_curve(n4);
        if num_patches != 0 {
            self.chop_and_write_quads(p0, p1, p2, num_patches);
        } else {
            self.write_quad_patch(p0, p1, p2);
        }
    }

    /// Writes a quadratic from an `SkPoint[3]`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L488-L493 (chrome/m156), `PatchWriter::writeQuadratic(const SkPoint[3])`.
    pub fn write_quadratic(&mut self, pts: &[Point; 3]) {
        self.write_quadratic_f2(f2(pts[0]), f2(pts[1]), f2(pts[2]));
    }

    /// Writes a line, as a cubic whose control points lie on it (or replicate its end points).
    // Port of: src/gpu/tessellate/PatchWriter.h#L494-L509 (chrome/m156), `PatchWriter::writeLine(float4)`.
    pub fn write_line_f4(&mut self, p0p1: Float4) {
        if C::DISCARD_FLAT_CURVES {
            return;
        }
        self.tolerances.set_parametric_segments(1.0);
        let p0 = Float2::new(p0p1[0], p0p1[1]);
        let p1 = Float2::new(p0p1[2], p0p1[3]);
        if C::REPLICATE_LINE_END_POINTS {
            self.write_cubic_patch(p0, p0, p1, p1);
        } else {
            let mid = (p0p1.zwxy() - p0p1) * splat4(1.0 / 3.0) + p0p1;
            self.write_cubic_patch_mid(p0, mid, p1);
        }
    }

    /// `writeLine(float2 p0, float2 p1)`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L510-L511 (chrome/m156), `PatchWriter::writeLine(float2, float2)`.
    pub fn write_line_f2(&mut self, p0: Float2, p1: Float2) {
        self.write_line_f4(Float4::from_xy_zw(p0, p1));
    }

    /// `writeLine(SkPoint p0, SkPoint p1)`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L512-L516 (chrome/m156), `PatchWriter::writeLine(SkPoint, SkPoint)`.
    pub fn write_line(&mut self, p0: Point, p1: Point) {
        self.write_line_f2(f2(p0), f2(p1));
    }

    /// Writes a triangle, as a conic with `w = inf` (a `kTriangularConicCurveType` patch).
    // Port of: src/gpu/tessellate/PatchWriter.h#L517-L524 (chrome/m156), `PatchWriter::writeTriangle(float2 ...)`.
    pub fn write_triangle_f2(&mut self, p0: Float2, p1: Float2, p2: Float2) {
        // 2 * 2 * 2 * 2
        const K_TRIANGLE_SEGMENTS_P4: f32 = 2.0 * 2.0 * 2.0 * 2.0;
        self.tolerances
            .set_parametric_segments(K_TRIANGLE_SEGMENTS_P4);
        self.write_patch(
            p0,
            p1,
            p2,
            Float2::new(f32::INFINITY, f32::INFINITY),
            K_TRIANGULAR_CONIC_CURVE_TYPE,
        );
    }

    /// Writes a triangle from three `SkPoint`s.
    // Port of: src/gpu/tessellate/PatchWriter.h#L525-L531 (chrome/m156), `PatchWriter::writeTriangle(SkPoint ...)`.
    pub fn write_triangle(&mut self, p0: Point, p1: Point, p2: Point) {
        self.write_triangle_f2(f2(p0), f2(p1), f2(p2));
    }

    /// Writes a stroke-width circle at `p`, as a cubic with four copies of `p` as control points.
    // Port of: src/gpu/tessellate/PatchWriter.h#L532-L540 (chrome/m156), `PatchWriter::writeCircle`.
    pub fn write_circle(&mut self, p: Point) {
        self.tolerances.set_parametric_segments(1.0);
        if let Some(mut vw) = self.allocator.append(&self.tolerances) {
            // p0,p1,p2,p3 = p -> 4 copies
            vw.put(&Repeat::<4, Point> { value: &p });
            let join = AttribValue {
                value: p,
                enabled: self
                    .attrs
                    .attribs
                    .contains(PatchAttribs::JOIN_CONTROL_POINT),
            };
            emit_patch_attribs::<C>(&self.attrs, &mut vw, join, K_CUBIC_CURVE_TYPE);
        }
    }

    /// Writes a square cap at `p`, as a conic with `w = 1` whose joins point to `join_to` (or `p`).
    // Port of: src/gpu/tessellate/PatchWriter.h#L541-L551 (chrome/m156), `PatchWriter::writeSquare(SkPoint, optional)`.
    pub fn write_square(&mut self, p: Point, join_to: Option<Point>) {
        self.tolerances.set_parametric_segments(1.0);
        if let Some(mut vw) = self.allocator.append(&self.tolerances) {
            // p0,p1,p2 = p -> 3 copies, then the conic with w = 1
            vw.put(&Repeat::<3, Point> { value: &p });
            vw.put(&1.0_f32).put(&f32::INFINITY);
            let join_to = join_to.unwrap_or(p);
            let join = AttribValue {
                value: join_to,
                enabled: self
                    .attrs
                    .attribs
                    .contains(PatchAttribs::JOIN_CONTROL_POINT),
            };
            emit_patch_attribs::<C>(&self.attrs, &mut vw, join, K_CONIC_CURVE_TYPE);
        }
    }

    /// `writeSquare(float2 p, float2 joinTo)`.
    // Port of: src/gpu/tessellate/PatchWriter.h#L552-L554 (chrome/m156), `PatchWriter::writeSquare(float2, float2)`.
    pub fn write_square_f2(&mut self, p: Float2, join_to: Float2) {
        self.write_square(pt(p), Some(pt(join_to)));
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L556-L571 (chrome/m156), `PatchWriter::writePatch`.
    fn write_patch(
        &mut self,
        p0: Float2,
        p1: Float2,
        p2: Float2,
        p3: Float2,
        explicit_curve_type: f32,
    ) {
        if C::TRACK_JOIN_CONTROL_POINTS {
            self.deferred.curve_type = explicit_curve_type;
        }
        let last = if explicit_curve_type == K_CUBIC_CURVE_TYPE {
            p3
        } else {
            p2
        };
        if eq2(p0, p1) && eq2(p1, p2) && eq2(p2, last) {
            return;
        }
        let stride = patch_stride(self.attrs.attribs);
        let join_before = self.attrs.join;
        {
            let Some(mut vw) = append_patch::<C, A>(
                &mut self.allocator,
                &mut self.deferred,
                &self.tolerances,
                stride,
                explicit_curve_type,
            ) else {
                return;
            };
            put_f2(&mut vw, p0);
            put_f2(&mut vw, p1);
            put_f2(&mut vw, p2);
            put_f2(&mut vw, p3);
            emit_patch_attribs::<C>(&self.attrs, &mut vw, join_before, explicit_curve_type);
        }
        if C::TRACK_JOIN_CONTROL_POINTS {
            self.deferred.last_control_point = pt(last);
            self.attrs.join.value = pt(tangent_point(last, p2, p1, p0));
        }
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L572-L574 (chrome/m156), `PatchWriter::writeCubicPatch(float2 x4)`.
    fn write_cubic_patch(&mut self, p0: Float2, p1: Float2, p2: Float2, p3: Float2) {
        self.write_patch(p0, p1, p2, p3, K_CUBIC_CURVE_TYPE);
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L575-L576 (chrome/m156), `PatchWriter::writeCubicPatch(float2, float4, float2)`.
    fn write_cubic_patch_mid(&mut self, p0: Float2, p1p2: Float4, p3: Float2) {
        self.write_cubic_patch(
            p0,
            Float2::new(p1p2[0], p1p2[1]),
            Float2::new(p1p2[2], p1p2[3]),
            p3,
        );
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L589-L591 (chrome/m156), `PatchWriter::writeQuadPatch`.
    fn write_quad_patch(&mut self, p0: Float2, p1: Float2, p2: Float2) {
        let mid = mix4(
            Float4::from_xy_zw(p0, p2),
            Float4::from_xy_zw(p1, p1),
            splat4(2.0 / 3.0),
        );
        self.write_cubic_patch_mid(p0, mid, p2);
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L592-L594 (chrome/m156), `PatchWriter::writeConicPatch`.
    fn write_conic_patch(&mut self, p0: Float2, p1: Float2, p2: Float2, w: f32) {
        self.write_patch(
            p0,
            p1,
            p2,
            Float2::new(w, f32::INFINITY),
            K_CONIC_CURVE_TYPE,
        );
    }

    /// Sets the tolerances for a curve with `n4` parametric segments to the most one patch can
    /// hold, and returns how many patches it must be chopped into (0 when it fits in one).
    // Port of: src/gpu/tessellate/PatchWriter.h#L595-L605 (chrome/m156), `PatchWriter::accountForCurve`.
    fn account_for_curve(&mut self, n4: f32) -> i32 {
        let max_p4 = K_MAX_PARAMETRIC_SEGMENTS_P4 as f32;
        if n4 <= max_p4 {
            self.tolerances.set_parametric_segments(cpp_max(1.0, n4));
            0
        } else {
            self.tolerances.set_parametric_segments(max_p4);
            scalar_ceil_to_int(wangs_formula::root4(
                cpp_min(n4, K_MAX_SEGMENTS_PER_CURVE_P4) / max_p4,
            ))
        }
    }

    /// Pushes a vertex into `tri`, and writes the triangles it pops.
    // Port of: src/gpu/tessellate/PatchWriter.h#L856-L862 (chrome/m156), `PatchWriter::writeTriangleStack`, with `pushVertex`.
    fn push_vertex_and_write(&mut self, tri: &mut Option<MiddleOutPolygonTriangulator>, p: Point) {
        if let Some(tri) = tri {
            let stack = tri.push_vertex(p);
            self.write_triangle_stack(stack.iter());
        }
    }

    /// Closes `tri`, and writes the triangles it pops.
    // Port of: src/gpu/tessellate/PatchWriter.h#L856-L862 (chrome/m156), `PatchWriter::writeTriangleStack`, with `close`.
    fn close_and_write(&mut self, tri: &mut Option<MiddleOutPolygonTriangulator>) {
        if let Some(tri) = tri {
            let stack = tri.close();
            self.write_triangle_stack(stack.iter());
        }
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L856-L862 (chrome/m156), `PatchWriter::writeTriangleStack`.
    fn write_triangle_stack<'s>(
        &mut self,
        triangles: impl Iterator<Item = (Point, Point, Point)> + 's,
    ) {
        for (p0, p1, p2) in triangles {
            self.write_triangle(p0, p1, p2);
        }
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L620-L655 (chrome/m156), `PatchWriter::chopAndWriteQuads`.
    fn chop_and_write_quads(
        &mut self,
        mut p0: Float2,
        mut p1: Float2,
        p2: Float2,
        mut num_patches: i32,
    ) {
        let mut triangulator = C::ADD_TRIANGLES_WHEN_CHOPPING
            .then(|| MiddleOutPolygonTriangulator::new(num_patches, pt(p0)));
        while num_patches >= 3 {
            let t = Float4::new(1.0, 1.0, 2.0, 2.0) / splat4(num_patches as f32);
            let ab = mix4(xyxy(p0), xyxy(p1), t);
            let bc = mix4(xyxy(p1), xyxy(p2), t);
            let abc = mix4(ab, bc, t);
            let middle = mix4(ab, bc, mix4(t, t.zwxy(), splat4(2.0 / 3.0)));
            // Write the 1st quad.
            self.write_quad_patch(p0, lo(ab), lo(abc));
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.write_triangle_f2(p0, lo(abc), hi(abc));
            }
            // Write the 2nd quad (already a cubic).
            self.write_cubic_patch_mid(lo(abc), middle, hi(abc));
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.push_vertex_and_write(&mut triangulator, pt(hi(abc)));
            }
            // Save the 3rd quad.
            p0 = hi(abc);
            p1 = hi(bc);
            num_patches -= 2;
        }
        if num_patches == 2 {
            let ab = (p0 + p1) * splat2(0.5);
            let bc = (p1 + p2) * splat2(0.5);
            let abc = (ab + bc) * splat2(0.5);
            // Write the 1st quad.
            self.write_quad_patch(p0, ab, abc);
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.write_triangle_f2(p0, abc, p2);
            }
            // Write the 2nd quad.
            self.write_quad_patch(abc, bc, p2);
        } else {
            debug_assert_eq!(num_patches, 1);
            // Write the single remaining quad.
            self.write_quad_patch(p0, p1, p2);
        }
        if C::ADD_TRIANGLES_WHEN_CHOPPING {
            self.push_vertex_and_write(&mut triangulator, pt(p2));
            self.close_and_write(&mut triangulator);
        }
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L656-L690 (chrome/m156), `PatchWriter::chopAndWriteConics`.
    fn chop_and_write_conics(
        &mut self,
        p0: Float2,
        p1: Float2,
        p2: Float2,
        w: f32,
        mut num_patches: i32,
    ) {
        let mut triangulator = C::ADD_TRIANGLES_WHEN_CHOPPING
            .then(|| MiddleOutPolygonTriangulator::new(num_patches, pt(p0)));
        // Homogeneous coordinates: (x, y, w) with a fourth lane that is always 1.
        let mut h0 = Float4::from_xy_zw(p0, Float2::new(1.0, 1.0));
        let mut h1 = Float4::from_xy_zw(p1, Float2::new(1.0, 1.0)) * splat4(w);
        let h2 = Float4::from_xy_zw(p2, Float2::new(1.0, 1.0));
        while num_patches >= 2 {
            let t = 1.0_f32 / num_patches as f32;
            let ab = mix4(h0, h1, splat4(t));
            let bc = mix4(h1, h2, splat4(t));
            let abc = mix4(ab, bc, splat4(t));
            let midpoint = lo(abc) / splat2(abc[3]);
            self.write_conic_patch(
                lo(h0) / splat2(h0[3]),
                lo(ab) / splat2(ab[3]),
                midpoint,
                ab[3] / (h0[3] * abc[3]).sqrt(),
            );
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.push_vertex_and_write(&mut triangulator, pt(midpoint));
            }
            // Save the 2nd conic (in homogeneous space).
            h0 = abc;
            h1 = bc;
            num_patches -= 1;
        }
        debug_assert_eq!(num_patches, 1);
        self.write_conic_patch(
            lo(h0) / splat2(h0[3]),
            lo(h1) / splat2(h1[3]),
            lo(h2), // h2.w == 1
            h1[3] / h0[3].sqrt(),
        );
        if C::ADD_TRIANGLES_WHEN_CHOPPING {
            self.push_vertex_and_write(&mut triangulator, pt(lo(h2)));
            self.close_and_write(&mut triangulator);
        }
    }

    // Port of: src/gpu/tessellate/PatchWriter.h#L691-L723 (chrome/m156), `PatchWriter::chopAndWriteCubics`.
    fn chop_and_write_cubics(
        &mut self,
        mut p0: Float2,
        mut p1: Float2,
        mut p2: Float2,
        p3: Float2,
        mut num_patches: i32,
    ) {
        let mut triangulator = C::ADD_TRIANGLES_WHEN_CHOPPING
            .then(|| MiddleOutPolygonTriangulator::new(num_patches, pt(p0)));
        while num_patches >= 3 {
            let t = Float4::new(1.0, 1.0, 2.0, 2.0) / splat4(num_patches as f32);
            let ab = mix4(xyxy(p0), xyxy(p1), t);
            let bc = mix4(xyxy(p1), xyxy(p2), t);
            let cd = mix4(xyxy(p2), xyxy(p3), t);
            let abc = mix4(ab, bc, t);
            let bcd = mix4(bc, cd, t);
            let abcd = mix4(abc, bcd, t);
            // p1 & p2 of the middle cubic.
            let middle = mix4(abc, bcd, t.zwxy());
            // Write the 1st cubic.
            self.write_cubic_patch(p0, lo(ab), lo(abc), lo(abcd));
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.write_triangle_f2(p0, lo(abcd), hi(abcd));
            }
            // Write the 2nd cubic.
            self.write_cubic_patch_mid(lo(abcd), middle, hi(abcd));
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.push_vertex_and_write(&mut triangulator, pt(hi(abcd)));
            }
            // Save the 3rd cubic.
            p0 = hi(abcd);
            p1 = hi(bcd);
            p2 = hi(cd);
            num_patches -= 2;
        }
        if num_patches == 2 {
            let ab = (p0 + p1) * splat2(0.5);
            let bc = (p1 + p2) * splat2(0.5);
            let cd = (p2 + p3) * splat2(0.5);
            let abc = (ab + bc) * splat2(0.5);
            let bcd = (bc + cd) * splat2(0.5);
            let abcd = (abc + bcd) * splat2(0.5);
            // Write the 1st cubic.
            self.write_cubic_patch(p0, ab, abc, abcd);
            if C::ADD_TRIANGLES_WHEN_CHOPPING {
                self.write_triangle_f2(p0, abcd, p3);
            }
            // Write the 2nd cubic.
            self.write_cubic_patch(abcd, bcd, cd, p3);
        } else {
            debug_assert_eq!(num_patches, 1);
            // Write the single remaining cubic.
            self.write_cubic_patch(p0, p1, p2, p3);
        }
        if C::ADD_TRIANGLES_WHEN_CHOPPING {
            self.push_vertex_and_write(&mut triangulator, pt(p3));
            self.close_and_write(&mut triangulator);
        }
    }
}

/// `float4(p, p)`: the x and y of `p` in both halves.
fn xyxy(p: Float2) -> Float4 {
    Float4::new(p[0], p[1], p[0], p[1])
}

/// The first two lanes of a float4 (`.lo` / `.xy()`).
fn lo(v: Float4) -> Float2 {
    Float2::new(v[0], v[1])
}

/// The last two lanes of a float4 (`.hi` / `.zw()`).
fn hi(v: Float4) -> Float2 {
    Float2::new(v[2], v[3])
}

/// The bytes of a `Point` as stored in a patch: x, then y, native endian.
fn join_bytes(p: Point) -> [u8; 8] {
    let mut out = [0u8; 8];
    out[..4].copy_from_slice(&p.x.to_ne_bytes());
    out[4..].copy_from_slice(&p.y.to_ne_bytes());
    out
}

impl BufferWrite for Point {
    // Port of: src/gpu/BufferWriter.h (chrome/m156), writing an `SkPoint` as two floats.
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        self.x.write_to(w);
        self.y.write_to(w);
    }
}

impl BufferWrite for StrokeParams {
    // Port of: src/gpu/tessellate/Tessellation.h#L157-L175 (chrome/m156), `StrokeParams` written as two floats.
    fn write_to(&self, w: &mut BufferWriter<'_>) {
        self.radius.write_to(w);
        self.join_type.write_to(w);
    }
}

// ---- The configurations Skia instantiates. ----

/// `PatchWriter<DynamicInstancesPatchAllocator<FixedCountCurves>, ...>` in Graphite's curves
/// render step. The allocator is not ported yet.
// Port of: src/gpu/graphite/render/TessellateCurvesRenderStep.cpp#L52-L57 (chrome/m156), `Writer`.
#[derive(Debug, Clone, Copy)]
pub struct GraphiteCurvesTraits;

impl PatchWriterTraits for GraphiteCurvesTraits {
    const PAINT_DEPTH: AttribMode = AttribMode::Required;
    const SSBO_INDEX: AttribMode = AttribMode::Required;
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Optional;
    const ADD_TRIANGLES_WHEN_CHOPPING: bool = true;
    const DISCARD_FLAT_CURVES: bool = true;
}

/// `PatchWriter<DynamicInstancesPatchAllocator<FixedCountWedges>, ...>` in Graphite's wedges
/// render step.
// Port of: src/gpu/graphite/render/TessellateWedgesRenderStep.cpp#L53-L58 (chrome/m156), `Writer`.
#[derive(Debug, Clone, Copy)]
pub struct GraphiteWedgesTraits;

impl PatchWriterTraits for GraphiteWedgesTraits {
    const FAN_POINT: AttribMode = AttribMode::Required;
    const PAINT_DEPTH: AttribMode = AttribMode::Required;
    const SSBO_INDEX: AttribMode = AttribMode::Required;
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Optional;
}

/// `PatchWriter<DynamicInstancesPatchAllocator<FixedCountStrokes>, ...>` in Graphite's strokes
/// render step.
// Port of: src/gpu/graphite/render/TessellateStrokesRenderStep.cpp#L57-L65 (chrome/m156), `Writer`.
#[derive(Debug, Clone, Copy)]
pub struct GraphiteStrokesTraits;

impl PatchWriterTraits for GraphiteStrokesTraits {
    const JOIN_CONTROL_POINT: AttribMode = AttribMode::Required;
    const STROKE_PARAMS: AttribMode = AttribMode::Required;
    const PAINT_DEPTH: AttribMode = AttribMode::Required;
    const SSBO_INDEX: AttribMode = AttribMode::Required;
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Optional;
    const REPLICATE_LINE_END_POINTS: bool = true;
    const TRACK_JOIN_CONTROL_POINTS: bool = true;
}

/// `PatchWriter<VertexChunkPatchAllocator, ...>` in Ganesh's stroke tessellator.
// Port of: src/gpu/ganesh/tessellate/StrokeTessellator.cpp#L41-L48 (chrome/m156), `StrokeWriter`.
#[derive(Debug, Clone, Copy)]
pub struct GaneshStrokeTraits;

impl PatchWriterTraits for GaneshStrokeTraits {
    const JOIN_CONTROL_POINT: AttribMode = AttribMode::Required;
    const STROKE_PARAMS: AttribMode = AttribMode::Optional;
    const COLOR: AttribMode = AttribMode::Optional;
    const WIDE_COLOR_IF_ENABLED: AttribMode = AttribMode::Optional;
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Optional;
    const REPLICATE_LINE_END_POINTS: bool = true;
    const TRACK_JOIN_CONTROL_POINTS: bool = true;
}

/// `PatchWriter<VertexChunkPatchAllocator, ...>` in Ganesh's curve tessellator.
// Port of: src/gpu/ganesh/tessellate/PathTessellator.cpp#L39-L44 (chrome/m156), `CurveWriter`.
#[derive(Debug, Clone, Copy)]
pub struct GaneshCurveTraits;

impl PatchWriterTraits for GaneshCurveTraits {
    const COLOR: AttribMode = AttribMode::Optional;
    const WIDE_COLOR_IF_ENABLED: AttribMode = AttribMode::Optional;
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Optional;
    const ADD_TRIANGLES_WHEN_CHOPPING: bool = true;
    const DISCARD_FLAT_CURVES: bool = true;
}

/// `PatchWriter<VertexChunkPatchAllocator, ...>` in Ganesh's wedge tessellator.
// Port of: src/gpu/ganesh/tessellate/PathTessellator.cpp#L87-L92 (chrome/m156), `WedgeWriter`.
#[derive(Debug, Clone, Copy)]
pub struct GaneshWedgeTraits;

impl PatchWriterTraits for GaneshWedgeTraits {
    const FAN_POINT: AttribMode = AttribMode::Required;
    const COLOR: AttribMode = AttribMode::Optional;
    const WIDE_COLOR_IF_ENABLED: AttribMode = AttribMode::Optional;
    const EXPLICIT_CURVE_TYPE: AttribMode = AttribMode::Optional;
}

#[cfg(test)]
mod tests {
    use super::{GaneshCurveTraits, PatchAllocator, PatchWriter};
    use crate::gpu::buffer_writer::VertexWriter;
    use crate::tessellate::linear_tolerances::LinearTolerances;
    use crate::tessellate::tessellation::PatchAttribs;
    use skia_rust_core::point::Point;

    /// Appends each patch to the end of a byte vector the test owns.
    struct Sink<'a> {
        out: &'a mut Vec<u8>,
        stride: usize,
    }

    impl PatchAllocator for Sink<'_> {
        fn append(&mut self, _tolerances: &LinearTolerances) -> Option<VertexWriter<'_>> {
            let start = self.out.len();
            self.out.resize(start + self.stride, 0);
            Some(VertexWriter::new(&mut self.out[start..]))
        }
    }

    // A circle is four copies of its center, then the explicit curve type (cubic = 0). With only
    // the curve-type attrib enabled, the patch is 4 * 8 + 4 bytes.
    #[test]
    fn write_circle_emits_four_centers_then_curve_type() {
        let mut out = Vec::new();
        {
            let mut writer = PatchWriter::<GaneshCurveTraits, _>::new(
                PatchAttribs::EXPLICIT_CURVE_TYPE,
                |stride| Sink {
                    out: &mut out,
                    stride,
                },
            );
            writer.write_circle(Point::new(3.0, 4.0));
        }
        assert_eq!(out.len(), 36);
        for i in 0..4 {
            assert_eq!(out[i * 8..i * 8 + 4], 3.0_f32.to_ne_bytes());
            assert_eq!(out[i * 8 + 4..i * 8 + 8], 4.0_f32.to_ne_bytes());
        }
        assert_eq!(out[32..36], 0.0_f32.to_ne_bytes());
    }
}
