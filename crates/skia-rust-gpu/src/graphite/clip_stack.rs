// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ClipStack.h, src/gpu/graphite/ClipStack.cpp

//! `ClipStack`: the clip state of a Graphite `Device`.
//!
//! The stack is two parallel stacks: [`SaveRecord`]s (one per `save()` that was followed by a
//! clip change) and [`RawElement`]s (the geometric clip operations, from every record that is
//! still on the stack). Newer elements can make older ones redundant, in which case the older ones
//! are marked invalid (and become valid again if the record that invalidated them is popped).
//! Draws query the stack with [`ClipStack::visit_clip_stack_for_draw`], which clips the draw
//! geometrically where it can, tightens the scissor, or hands back an analytic clip; whatever is
//! left are the *effective elements*, which [`ClipStack::update_clip_state_for_draw`] turns into
//! depth-only draws that the `Device` records ([`ClipDrawHooks`]).
//!
//! Deviations from the C++:
//!
//! - `ClipStack` does not hold a `Device*`. The calls that need the device take a
//!   [`ClipDrawHooks`] (the `friend class ClipStack` of `Device`: `drawClipShape()`,
//!   `drawClipShapeImmediate()`, `updateNextDepthForClipping()`, the `useDrawListLayer()` cap and
//!   the writes to the `DrawParams` of a captured clip draw). The device size is given at
//!   construction (it never changes).
//! - The effective elements ([`ElementList`]) are indices into the element stack instead of
//!   `const Element*`; [`ClipStack::element`] reads one. The indices stay valid until the stack
//!   is modified, which is the lifetime of the pointers in C++.
//! - `DrawParams*` and `Layer*` back-pointers are [`DrawParamsId`] and [`LayerId`] (see
//!   `draw_list_types`).
//! - `ClipStack::clipShader()`'s shader blending, `SaveRecord::shader()` and `Clip::shader()` are
//!   kept as `Shader` handles; the `Clip` only records whether one is present (the shader itself
//!   is read from [`ClipStack::clip_shader_ref`]).
//! - `Geometry::maskToDevice()` is always null here: the geometries that carry one
//!   (`CoverageMaskShape`, analytic blurs) are not ported yet.
//! - `SK_GRAPHITE_USE_LEGACY_RRECT_CLIP_SHADER` is not built, so `AnalyticClip` is the xform +
//!   per-corner-radii variant.
//!
//! Seams:
//!
//! - The atlas clip (rasterizing the remaining effective elements into a clip mask in an atlas
//!   when MSAA is unavailable) stops at [`ClipAtlasManager`]: the stack calls it exactly as
//!   `ClipAtlasManager::findOrCreateEntry()` is called in C++. The device passes the clip atlas of
//!   the recorder when the raster path strategy is in use; otherwise the remaining elements are
//!   drawn as depth-only clip draws.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::floating_point::{ieee_float_divide, is_finite, is_finite_all};
use skia_rust_core::paint::Cap;
use skia_rust_core::path_priv::W0_PLANE_DISTANCE;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect as SkRect, rect_priv};
use skia_rust_core::rrect::{Corner, RRect, rrect_priv};
use skia_rust_core::scalar::Scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_simd::vx::{self, Float2, Float4};

use crate::graphite::draw_list_types::{DrawParamsId, LayerId};
use crate::graphite::draw_order::{CompressedPaintersOrder, DrawOrder, PaintersDepth};
use crate::graphite::draw_params::Clip;
use crate::graphite::geom::bounds_manager::BoundsManager;
use crate::graphite::geom::edge_aa_quad::{EdgeAAQuad, Flags as EdgeFlags};
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::non_msaa_clip::{AnalyticClip, NonMSAAClip};
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::geom::transform::{Transform, Type as TransformType};
use crate::graphite::texture_proxy::TextureProxy;
use skia_rust_core::m44::M44;

/// `ClipStack::ClipState`.
// Port of: src/gpu/graphite/ClipStack.h#L41-L43 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClipStack::ClipState")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClipState {
    /// `kEmpty`.
    Empty,
    /// `kWideOpen`.
    WideOpen,
    /// `kDeviceRect`.
    DeviceRect,
    /// `kDeviceRRect`.
    DeviceRRect,
    /// `kComplex`.
    Complex,
}

/// `ClipStack::PixelSnapping`: if `Yes` and the right conditions are met, the clip geometry is
/// adjusted to align with the pixel grid to emulate some aspects of non-AA behavior.
// Port of: src/gpu/graphite/ClipStack.h#L62-L65 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClipStack::PixelSnapping")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixelSnapping {
    /// `kNo`.
    No,
    /// `kYes`.
    Yes,
}

/// All data describing a geometric modification to the clip (`ClipStack::Element`).
// Port of: src/gpu/graphite/ClipStack.h#L48-L53 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClipStack::Element")]
#[derive(Clone, Debug)]
pub struct ClipElement {
    /// `fShape`.
    pub shape: Shape,
    /// `fLocalToDevice`.
    pub local_to_device: Transform,
    /// `fOp`.
    pub op: ClipOp,
}

/// The clip elements that affect a draw and must be drawn as depth-only draws
/// (`ClipStack::ElementList`): indices for [`ClipStack::element`].
pub type ElementList = Vec<usize>;

/// What the clip stack calls on its device (`Device::drawClipShape()`,
/// `Device::drawClipShapeImmediate()`, `Device::updateNextDepthForClipping()`, and the pieces of
/// the device's recorder and `DrawContext` that `ClipStack` reaches through `fDevice`).
// Port of: src/gpu/graphite/Device.h#L246-L257 (chrome/m156)
pub trait ClipDrawHooks {
    /// `drawClipShape(localToDevice, shape, clip, order)`.
    #[doc(alias = "drawClipShape")]
    fn draw_clip_shape(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        order: DrawOrder,
    );

    /// `drawClipShapeImmediate(localToDevice, shape, clip, order)`: records a draw and returns a
    /// backpointer to the `DrawParams` of the draw.
    #[doc(alias = "drawClipShapeImmediate")]
    fn draw_clip_shape_immediate(
        &mut self,
        local_to_device: &Transform,
        shape: &Shape,
        clip: &Clip,
        order: DrawOrder,
    ) -> (Option<DrawParamsId>, Option<LayerId>);

    /// `updateNextDepthForClipping(depth)`.
    #[doc(alias = "updateNextDepthForClipping")]
    fn update_next_depth_for_clipping(&mut self, depth: PaintersDepth);

    /// `fRecorder->priv().caps()->useDrawListLayer()`.
    #[doc(alias = "useDrawListLayer")]
    fn use_draw_list_layer(&self) -> bool;

    /// The writes `RawElement::drawClip()` makes through `fCaptureParams` in the layered draw
    /// list: the final order, draw bounds and scissor of a captured depth-only clip draw.
    fn update_clip_draw(
        &mut self,
        params: DrawParamsId,
        order: DrawOrder,
        draw_bounds: Rect,
        scissor: IRect,
    );

    /// `Layer::fOrder` of a layer returned by [`ClipDrawHooks::draw_clip_shape_immediate`].
    fn layer_order(&self, layer: LayerId) -> Option<CompressedPaintersOrder>;
}

/// The seam to `ClipAtlasManager` (G12a): flattens the remaining effective elements of a draw
/// into one clip mask in an atlas.
// Port of: src/gpu/graphite/ClipAtlasManager.h (chrome/m156), `findOrCreateEntry`
pub trait ClipAtlasManager {
    /// `findOrCreateEntry(stackRecordID, elementList, maskBounds, outPos)`: the atlas texture
    /// holding the mask (and where in it the mask starts through `out_pos`), or `None` if the
    /// mask could not be placed.
    #[doc(alias = "findOrCreateEntry")]
    fn find_or_create_entry(
        &mut self,
        stack_record_id: u32,
        element_list: &[&ClipElement],
        mask_bounds: IRect,
        out_pos: &mut IPoint,
    ) -> Option<Arc<TextureProxy>>;
}

// ===========================================================================================
// Free helpers
// ===========================================================================================

// Port of: src/gpu/graphite/ClipStack.cpp#L52-L62 (chrome/m156)
fn subtract(a: &Rect, b: &Rect, exact: bool) -> Rect {
    let mut diff = SkRect::new_empty();
    if rect_priv::subtract(&a.as_sk_rect(), &b.as_sk_rect(), &mut diff) || !exact {
        // Either A-B is exactly the rectangle stored in diff, or we don't need an exact answer
        // and can settle for the subrect of A excluded from B (which is also 'diff')
        Rect::from_sk_rect(&diff)
    } else {
        // For our purposes, we want the original A when A-B cannot be exactly represented
        *a
    }
}

const INVALID_GEN_ID: u32 = 0;
const EMPTY_GEN_ID: u32 = 1;
const WIDE_OPEN_GEN_ID: u32 = 2;

// Port of: src/gpu/graphite/ClipStack.cpp#L68-L78 (chrome/m156)
fn next_gen_id() -> u32 {
    // 0-2 are reserved for invalid, empty & wide-open
    const FIRST_UNRESERVED_GEN_ID: u32 = 3;
    static NEXT_ID: AtomicU32 = AtomicU32::new(FIRST_UNRESERVED_GEN_ID);

    loop {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        if id >= FIRST_UNRESERVED_GEN_ID {
            return id;
        }
    }
}

// Port of: src/gpu/graphite/ClipStack.cpp#L80-L123 (chrome/m156)
fn oriented_bbox_intersection(
    a: &Rect,
    a_xform: &Transform,
    b: &Rect,
    b_xform: &Transform,
) -> bool {
    // NOTE: We intentionally exclude projected bounds for two reasons:
    //   1. We can skip the division by w and worring about clipping to w = 0.
    //   2. W/o the projective case, the separating axes are simpler to compute (see below).
    debug_assert!(
        a_xform.type_() != TransformType::Perspective
            && b_xform.type_() != TransformType::Perspective
    );
    let quad_a = a_xform.map_points_rect(a);
    let quad_b = b_xform.map_points_rect(b);

    // There are 4 separating axes, defined by the two normals from quadA and from quadB, but
    // since they were produced by transforming a rectangle by an affine transform, we know the
    // normals are orthoganal to the basis vectors of upper 2x2 of their two transforms.
    let am = a_xform.matrix();
    let bm = b_xform.matrix();
    let axes_x = Float4::new(-am.rc(1, 0), -am.rc(1, 1), -bm.rc(1, 0), -bm.rc(1, 1));
    let axes_y = Float4::new(am.rc(0, 0), am.rc(0, 1), bm.rc(0, 0), bm.rc(0, 1));

    // Projections of the 4 corners of each quadrilateral vs. the 4 axes. For orthonormal
    // transforms, the projections of a quad's corners to its own normal axes should work out
    // to the original dimensions of the rectangle, but this code handles skew and scale factors
    // without branching.
    let proj = |q: &Float4| axes_x * q.x() + axes_y * q.y();
    let a_proj0 = proj(&quad_a[0]);
    let a_proj1 = proj(&quad_a[1]);
    let a_proj2 = proj(&quad_a[2]);
    let a_proj3 = proj(&quad_a[3]);

    let b_proj0 = proj(&quad_b[0]);
    let b_proj1 = proj(&quad_b[1]);
    let b_proj2 = proj(&quad_b[2]);
    let b_proj3 = proj(&quad_b[3]);

    // Minimum and maximum projected values against the 4 axes, for both quadA and quadB, which
    // gives us four pairs of intervals to test for separation.
    let min_a = a_proj0.min(a_proj1).min(a_proj2.min(a_proj3));
    let max_a = a_proj0.max(a_proj1).max(a_proj2.max(a_proj3));
    let min_b = b_proj0.min(b_proj1).min(b_proj2.min(b_proj3));
    let max_b = b_proj0.max(b_proj1).max(b_proj2.max(b_proj3));

    let overlaps = min_b.le_mask(max_a) & min_a.le_mask(max_b);
    vx::all(overlaps) // any non-overlapping interval would imply no intersection
}

// LTRB are set in returned bitmask if other's LTRB edge is coincident or inside `shape`'s edge.
// Port of: src/gpu/graphite/ClipStack.cpp#L125-L135 (chrome/m156)
fn clipped_edges(shape: &Rect, other: &Rect) -> EdgeFlags {
    // Since RB are stored negated in vals(), this works out to
    //     [other.LT >= shape.LT, other.RB <= shape.RB]
    let inside_mask = other.vals().ge_mask(shape.vals());
    let flag = |lane: usize, flag: EdgeFlags| {
        if inside_mask[lane] != 0 {
            flag
        } else {
            EdgeFlags::NONE
        }
    };
    flag(0, EdgeFlags::LEFT)
        | flag(1, EdgeFlags::TOP)
        | flag(2, EdgeFlags::RIGHT)
        | flag(3, EdgeFlags::BOTTOM)
}

// Tries to intersect `otherShape` transformed by `otherToDevice` directly into `shape` assuming
// that `shape` is transformed by localToDevice. If possible (true), `shape` represents the exact
// intersection of the two original shapes. Returns true if `shape` is modified, false otherwise.
// Port of: src/gpu/graphite/ClipStack.cpp#L137-L316 (chrome/m156)
#[allow(clippy::too_many_lines)] // mirrors the C++ function
#[allow(clippy::similar_names)] // mirrors the C++ names localOtherRect/localOtherRRect
fn intersect_shape(
    other_to_device: &Transform,
    other_shape: &Shape,
    local_to_device: &Transform,
    shape: &mut Shape,
    edge_flags: &mut EdgeFlags,
) -> bool {
    // There are only a subset of shape types that we can analytically intersect with each other,
    // assuming a simple fill style (always the case for clip shapes):
    //
    //  rects, rrects, flood-fills (empty+inverse-fill)
    //
    // Flood-fills only appear as part of a draw, so it's only checked for `shape` and not
    // `otherShape`. In theory, per-edge AA quads could also be included but they do not appear as
    // clip shapes.
    //
    // Paths and arcs have complex intersection logic, so are skipped under the assumption that
    // simple cases have already been mapped to a rect or rrect. Lines are only ever stroked, so
    // are incompatible with this function.
    //
    // EdgeAAQuads that are rectangular can be intersected by being treated as a rect shape and
    // adjusting edge flags as non-AA edges are clipped out.
    let shape_intersectable = shape.is_rect() || shape.is_rrect() || shape.is_flood_fill();
    let other_intersectable = other_shape.is_rect() || other_shape.is_rrect();
    // Only clip shapes are used for `otherShape`, so we shouldn't see any flood fills here
    debug_assert!(!other_shape.is_flood_fill());
    // Only rects should have edge flags other than kAll
    debug_assert!(*edge_flags == EdgeFlags::ALL || shape.is_rect());

    if !shape_intersectable || !other_intersectable {
        // Technically if shapeIntersectable was true for empty+inverse, we could turn the flood
        // fill into `otherShape` regardless of its type, but those other types are more expensive
        // to render and in the situation where many draws fill against a clip path, we'd want to
        // draw the clip a single time vs. drawing the path multiple times.
        return false;
    }

    // In order to combine, otherShape must be able to map into `localToDevice` without changing
    // shape class (e.g. to a path when rotated) in order for shading to apply in the same
    // coordinate space. This is possible if the relative transform between otherToDevice and
    // localToDevice is rectStaysRect.
    let storage: Transform;

    // We track `local` to `other` and use the `inverseMapRect` functions to map the `otherShape`
    // into local space when possible. Using `localToOther` instead of `otherToLocal` allows the
    // common case of a device-space clip (otherToDevice == I) and an axis-aligned draw to
    // simply use `localToDevice` as `localToOther`.
    let local_to_other: Option<&Transform> = if other_to_device == local_to_device {
        // No coordinate space conversion, so set to null to signal identity mapping is skippable.
        // NOTE: This case arises in clip-clip combinations when both were axis-aligned and pre-
        // transformed to device space.
        None
    } else if other_to_device.type_() == TransformType::Identity
        && local_to_device.type_() <= TransformType::RectStaysRect
    {
        // Relative transform is (otherToDevice)^-1*localToDevice = localToDevice
        Some(local_to_device)
    } else if other_to_device.type_() <= TransformType::RectStaysRect
        && local_to_device.type_() == TransformType::Identity
    {
        // Relative transform is otherToDevice^-1*localToDevice = otherToDevice^-1
        // (which may not occur in a common scenario but is harmless). Inverse() is mostly
        // shuffling bytes around, not recomputing the inverse.
        storage = Transform::inverse(other_to_device);
        Some(&storage)
    } else {
        // Calculate (otherToDevice)^-1*localToDevice and see if the relative transform is
        // of the right type.
        storage = Transform::new(M44::concat(
            other_to_device.inverse_matrix(),
            local_to_device.matrix(),
        ));
        if storage.type_() <= TransformType::RectStaysRect {
            Some(&storage)
        } else {
            // `otherShape` can't be trivially mapped to the local coordinate space
            return false;
        }
    };

    // Since `otherShape` is either a rect or a round rect, bounds() is tight to the linear edges.
    let mut local_other_rect = other_shape.bounds();
    if let Some(local_to_other) = local_to_other {
        // In this block, `localOtherRect` is defined in the other coord space and `mapped` is in
        // the local coord space. At the end of the block, `localOtherRect` is set to `mapped` so
        // that afterwards it is always defined in local space.
        let mapped = local_to_other.inverse_map_rect(&local_other_rect);
        // If we don't have enough precision, the other shape might not map back to the geometry.
        // Allow up to 1/1000th of a pixel in tolerance when mapping between coordinate spaces,
        // otherwise we'll have to clip the shapes independently.
        let other_tol =
            Shape::DEFAULT_PIXEL_TOLERANCE * other_to_device.local_aa_radius(&local_other_rect);
        if local_other_rect.is_empty_negative_or_nan()
            || !local_to_other
                .map_rect(&mapped)
                .nearly_equals(&local_other_rect, other_tol)
        {
            return false;
        }
        local_other_rect = mapped;
    }
    // Remember the edges that get clipped by the intersection
    let clipped_edges = clipped_edges(&shape.bounds(), &local_other_rect);
    if !shape.is_flood_fill() {
        // And now it's tight to the intersection with `shape`, sans any corner rounding
        local_other_rect.intersect(shape.bounds());
    }
    // Make sure that the intersected shape does not become subpixel in size, since drawing a
    // subpixel/hairline shape produces a different result than something that's clipped.
    let local_aa_radius = local_to_device.local_aa_radius(&local_other_rect);
    if !is_finite(local_aa_radius)
        || vx::any(
            local_other_rect
                .size()
                .le_mask(Float2::from(local_aa_radius)),
        )
    {
        return false;
    }

    let local_other_rrect: RRect;
    if other_shape.is_rect() {
        if shape.is_rect() || shape.is_flood_fill() {
            debug_assert!(*edge_flags == EdgeFlags::ALL || !shape.is_flood_fill());
            // Assuming that non-AA edges seam with non-AA edges other quads to create a uniform
            // coverage field, we turn on the AA edge flag when coincident or clipped. This will
            // create a nice AA edge from this draw while the other non-AA quad is discarded.
            *edge_flags |= clipped_edges; // This is a no-op if shape was a flood fill
            shape.set_rect(local_other_rect);
            return true;
        }
        // Fall back to rrect+rrect intersection
        local_other_rrect = RRect::new_rect(local_other_rect.as_sk_rect());
    } else {
        debug_assert!(other_shape.is_rrect());
        if let Some(local_to_other) = local_to_other {
            if let Some(rr) = other_shape
                .rrect()
                .transform(&local_to_other.inverse_matrix().to_m33())
            {
                local_other_rrect = rr;
            } else {
                // Transformation produced invalid geometry
                return false;
            }
        } else {
            local_other_rrect = *other_shape.rrect();
        }

        if shape.is_rect() && *edge_flags != EdgeFlags::ALL {
            // When combining a mixed edge AA quad with a rounded rectangle, we require that all
            // non-AA edges be clipped out entirely.
            if (clipped_edges | *edge_flags) != EdgeFlags::ALL {
                // The intersection shows AA'ed round corners and non-AA'ed edges, which can't be
                // represented by just Geometry or Shape.
                return false;
            }
        } else if shape.is_flood_fill() {
            debug_assert_eq!(*edge_flags, EdgeFlags::ALL);
            shape.set_rrect(local_other_rrect);
            return true;
        } // Else continue with rrect+rrect intersection
    }

    // `shape` can only be rect or rrect at this point, flood fill should already have returned.
    // If we've made it this far, we've also determined that the edge flags should be set to kAll
    // on a successful rrect+rrect intersection.
    debug_assert!(shape.is_rect() || shape.is_rrect());

    let shape_rrect = if shape.is_rect() {
        RRect::new_rect(shape.rect().as_sk_rect())
    } else {
        *shape.rrect()
    };
    let local_rrect = rrect_priv::conservative_intersect(&local_other_rrect, &shape_rrect);
    if local_rrect.is_rect() {
        // Valid shape that can be simplified to rect
        shape.set_rect(Rect::from_sk_rect(local_rrect.rect()));
        *edge_flags = EdgeFlags::ALL;
        true
    } else if !local_rrect.is_empty() {
        // Intersection is representable as a rrect still
        shape.set_rrect(local_rrect);
        *edge_flags = EdgeFlags::ALL;
        true
    } else {
        // Intersection is complex, leave edge flags unmodified
        false
    }
}

// Port of: src/gpu/graphite/ClipStack.cpp#L318-L329 (chrome/m156)
fn snap_scissor(a: &Rect, device_bounds: &Rect) -> Rect {
    // Snapping to 4 pixel boundaries seems to give a good tradeoff between rasterizing slightly
    // more (but being clipped by the depth test), vs. setting a tight scissor that forces a state
    // change.
    // NOTE: This rounds out to the *next* multiple of 4, so that if the input rectangle happens
    // to land on a multiple of 4 we still create some padding to avoid scissoring just AA
    // outsets.
    const RES: f32 = 4.0;
    let snapped = a.make_outset(RES - 1.0);
    let snapped = Rect::from_vals(snapped.vals() * (1.0 / RES)).make_round_out();
    Rect::from_vals(snapped.vals() * RES).make_intersect(*device_bounds)
}

// ===========================================================================================
// TransformedShape, Simplify
// ===========================================================================================

/// A flyweight object describing geometry, subject to a local-to-device transform
/// (`ClipStack::TransformedShape`). This can be used by `SaveRecord`s, Elements, and draws to
/// determine how two shape operations interact with each other, without needing to share a base
/// class, friend each other, or have a template for each combination of two types.
// Port of: src/gpu/graphite/ClipStack.cpp#L331-L355 (chrome/m156)
#[derive(Clone, Copy)]
struct TransformedShape<'a> {
    local_to_device: &'a Transform,
    shape: &'a Shape,
    outer_bounds: Rect,
    inner_bounds: Rect,
    op: ClipOp,

    // contains() performs a fair amount of work to be as accurate as possible since it can mean
    // greatly simplifying the clip stack. However, in some contexts this isn't worth doing because
    // the actual shape is only an approximation (save records), or there's no current way to take
    // advantage of knowing this shape contains another (draws containing a clip hypothetically
    // could replace their geometry to draw the clip directly, but that isn't implemented now).
    contains_checks_only_bounds: bool,
}

impl TransformedShape<'_> {
    // Port of: src/gpu/graphite/ClipStack.cpp#L357-L389 (chrome/m156)
    fn intersects(&self, o: &TransformedShape<'_>) -> bool {
        if !self.outer_bounds.intersects(o.outer_bounds) {
            return false;
        }

        if self.local_to_device.type_() <= TransformType::RectStaysRect
            && o.local_to_device.type_() <= TransformType::RectStaysRect
        {
            // The two shape's coordinate spaces are different but both rect-stays-rect or
            // simpler. This means, though, that their outer bounds approximations are tight to
            // their transormed shape bounds. There's no point to do further tests given that and
            // that we already found that these outer bounds *do* intersect.
            true
        } else if self.local_to_device == o.local_to_device {
            // Since the two shape's local coordinate spaces are the same, we can compare shape
            // bounds directly for a more accurate intersection test. We intentionally do not go
            // further and do shape-specific intersection tests since these could have unknown
            // complexity (for paths) and limited utility (e.g. two round rects that are disjoint
            // solely from their corner curves).
            self.shape.bounds().intersects(o.shape.bounds())
        } else if self.local_to_device.type_() != TransformType::Perspective
            && o.local_to_device.type_() != TransformType::Perspective
        {
            // The shapes don't share the same coordinate system, and their approximate 'outer'
            // bounds in device space could have substantial outsetting to contain the transformed
            // shape (e.g. 45 degree rotation). Perform a more detailed check on their oriented
            // bounding boxes.
            oriented_bbox_intersection(
                &self.shape.bounds(),
                self.local_to_device,
                &o.shape.bounds(),
                o.local_to_device,
            )
        } else {
            // Else multiple perspective transforms are involved, so assume intersection and allow
            // the rasterizer to handle perspective clipping.
            true
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L391-L441 (chrome/m156)
    fn contains(&self, o: &TransformedShape<'_>) -> bool {
        if self.inner_bounds.contains(o.outer_bounds) {
            return true;
        }
        // Skip more expensive contains() checks if configured not to, or if the extent of 'o'
        // exceeds this shape's outer bounds. When that happens there must be some part of 'o'
        // that cannot be contained in this shape.
        if self.contains_checks_only_bounds || !self.outer_bounds.contains(o.outer_bounds) {
            return false;
        }

        if self.local_to_device == o.local_to_device {
            // Test the shapes directly against each other, with a special check for a rrect+rrect
            // containment (a intersect b == a implies b contains a) and paths (same gen ID, or
            // same path for small paths means they contain each other).
            const MAX_PATH_COMPARE_POINTS: usize = 16;
            if self.shape.is_rrect() && o.shape.is_rrect() {
                rrect_priv::conservative_intersect(self.shape.rrect(), o.shape.rrect())
                    == *o.shape.rrect()
            } else if self.shape.is_path() && o.shape.is_path() {
                // TODO: Is this worth doing still if clips only cost as much as a single draw?
                self.shape.path().generation_id() == o.shape.path().generation_id()
                    || (self.shape.path().count_points() <= MAX_PATH_COMPARE_POINTS
                        && self.shape.path() == o.shape.path())
            } else {
                self.shape.conservative_contains_rect(o.shape.bounds())
            }
        } else if self.local_to_device.type_() <= TransformType::RectStaysRect
            && o.local_to_device.type_() <= TransformType::RectStaysRect
        {
            // Optimize the common case where o's bounds can be mapped tightly into this
            // coordinate space and then tested against our shape.
            let local_bounds = self
                .local_to_device
                .inverse_map_rect(&o.local_to_device.map_rect(&o.shape.bounds()));
            self.shape.conservative_contains_rect(local_bounds)
        } else if self.shape.convex(true) {
            // Since this shape is convex, if all four corners of o's bounding box are inside it
            // then the entirety of o is also guaranteed to be inside it.
            let device_quad = o.local_to_device.map_points_rect(&o.shape.bounds());
            let mut local_quad = [Float4::default(); 4];
            self.local_to_device
                .inverse_map_points(&device_quad, &mut local_quad);
            for i in 0..4 {
                // TODO: Would be nice to make this consistent with how the GPU clips NDC w.
                if device_quad[i].w() < W0_PLANE_DISTANCE || local_quad[i].w() < W0_PLANE_DISTANCE {
                    // Something in O actually projects behind the W = 0 plane and would be
                    // clipped to infinity, so it's extremely unlikely that this contains O.
                    return false;
                }
                if !self.shape.conservative_contains_point(
                    Float2::new(local_quad[i].x(), local_quad[i].y()) / local_quad[i].w(),
                ) {
                    return false;
                }
            }
            true
        } else {
            // Else not an easily comparable pair of shapes so assume this doesn't contain O
            false
        }
    }
}

/// This captures which of the two elements in (A op B) would be required when they are combined,
/// where op is intersect or difference (`ClipStack::SimplifyResult`).
// Port of: src/gpu/graphite/ClipStack.h#L196-L201 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SimplifyResult {
    Empty,
    AOnly,
    BOnly,
    Both,
}

/// How a clip element affects a draw after more detailed analysis (`ClipStack::DrawInfluence`).
// Port of: src/gpu/graphite/ClipStack.h#L205-L210 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DrawInfluence {
    /// The element causes the draw shape to be entirely clipped out.
    ClipsOutDraw,
    /// The element is fully covered, so the draw's shape can be ignored.
    ReplacesDraw,
    /// The element does not affect the draw.
    None,
    /// The element affects the draw shape in a complex way.
    ComplexInteraction,
}

// Port of: src/gpu/graphite/ClipStack.cpp#L443-L511 (chrome/m156)
fn simplify(a: &TransformedShape<'_>, b: &TransformedShape<'_>) -> SimplifyResult {
    match (a.op, b.op) {
        (ClipOp::Intersect, ClipOp::Intersect) => {
            // Intersect (A) + Intersect (B)
            if !a.intersects(b) {
                // Regions with non-zero coverage are disjoint, so intersection = empty
                SimplifyResult::Empty
            } else if b.contains(a) {
                // B's full coverage region contains entirety of A, so intersection = A
                SimplifyResult::AOnly
            } else if a.contains(b) {
                // A's full coverage region contains entirety of B, so intersection = B
                SimplifyResult::BOnly
            } else {
                // The shapes intersect in some non-trivial manner
                SimplifyResult::Both
            }
        }
        (ClipOp::Intersect, ClipOp::Difference) => {
            // Intersect (A) + Difference (B)
            if !a.intersects(b) {
                // A only intersects B's full coverage region, so intersection = A
                SimplifyResult::AOnly
            } else if b.contains(a) {
                // B's zero coverage region completely contains A, so intersection = empty
                SimplifyResult::Empty
            } else {
                // Intersection cannot be simplified. Note that the combination of a intersect
                // and difference op in this order cannot produce kBOnly
                SimplifyResult::Both
            }
        }
        (ClipOp::Difference, ClipOp::Intersect) => {
            // Difference (A) + Intersect (B) - the mirror of Intersect(A) + Difference(B),
            // but combining is commutative so this is equivalent barring naming.
            if !b.intersects(a) {
                // B only intersects A's full coverage region, so intersection = B
                SimplifyResult::BOnly
            } else if a.contains(b) {
                // A's zero coverage region completely contains B, so intersection = empty
                SimplifyResult::Empty
            } else {
                // Cannot be simplified
                SimplifyResult::Both
            }
        }
        (ClipOp::Difference, ClipOp::Difference) => {
            // Difference (A) + Difference (B)
            if a.contains(b) {
                // A's zero coverage region contains B, so B doesn't remove any extra
                // coverage from their intersection.
                SimplifyResult::AOnly
            } else if b.contains(a) {
                // Mirror of the above case, intersection = B instead
                SimplifyResult::BOnly
            } else {
                // Intersection of the two differences cannot be simplified. Note that for
                // this op combination it is not possible to produce kEmpty.
                SimplifyResult::Both
            }
        }
    }
}

// Port of: src/gpu/graphite/ClipStack.cpp#L513-L532 (chrome/m156)
fn simplify_for_draw(clip: &TransformedShape<'_>, draw: &TransformedShape<'_>) -> DrawInfluence {
    // Given the mapping below, we can just recast the SimplifyResult returned from
    // Simplify(A=clip, B=draw):
    match simplify(clip, draw) {
        // If the result is kEmpty, the draw is clipped out.
        SimplifyResult::Empty => DrawInfluence::ClipsOutDraw,
        // If the result is kAOnly, only the clip's shape provides coverage and the draw could be
        // replaced with something that just covers the clip bounds.
        SimplifyResult::AOnly => DrawInfluence::ReplacesDraw,
        // If the result is kBOnly, the clip's shape doesn't impact the draw's coverage at all.
        SimplifyResult::BOnly => DrawInfluence::None,
        // If the result is kBoth, the clip and the draw combine in a complex manner
        SimplifyResult::Both => DrawInfluence::ComplexInteraction,
    }
}

// ===========================================================================================
// RawElement
// ===========================================================================================

/// What `RawElement::drawClip()` reaches on the device: the hooks and the device bounds.
struct ClipDevice<'a> {
    hooks: &'a mut dyn ClipDrawHooks,
    bounds: Rect,
}

/// Wraps the geometric [`ClipElement`] data with logic for containment and bounds testing
/// (`ClipStack::RawElement`).
// Port of: src/gpu/graphite/ClipStack.h#L215-L309 (chrome/m156)
#[derive(Clone, Debug)]
struct RawElement {
    element: ClipElement,

    // Device space bounds. These bounds are not snapped to pixels with the assumption that if
    // a relation (intersects, contains, etc.) is true for the bounds it will be true for the
    // rasterization of the coordinates that produced those bounds.
    inner_bounds: Rect,
    outer_bounds: Rect,

    // State tracking how this clip element needs to be recorded into the draw context. As the
    // clip stack is applied to additional draws, the clip's Z and usage bounds grow to account
    // for it; its compressed painter's order is selected the first time a draw is affected.
    usage_bounds: Rect,
    order: CompressedPaintersOrder,
    max_z: PaintersDepth,

    // Elements are invalidated by SaveRecords as the record is updated with new elements that
    // override old geometry. An invalidated element stores the index of the first element of
    // the save record that invalidated it. This makes it easy to undo when the save record is
    // popped from the stack, and is stable as the current save record is modified.
    invalidated_by_index: i32,

    // Used exclusively by the drawListLayer path to track depth-only draws.
    //
    // capture_params: the drawParams of the depth-only draw. Allows: 1) The initial coarse bounds
    //                 (recorded in drawClipImmediate) to be updated to tighter bounds later.
    //                 2) Deferred assignment of the Z value the draw will use.
    // insertion: The layer holding capture_params for this element's depth-only draw. This acts
    //            as a dependency barrier; clipped draws affected by this rawElement must be
    //            inserted into or after the latest layer+list across all their dependcy depth
    //            draws.
    capture_params: Option<DrawParamsId>,
    insertion: Option<LayerId>,
}

impl RawElement {
    // Port of: src/gpu/graphite/ClipStack.cpp#L534-L616 (chrome/m156)
    fn new(
        device_bounds: &Rect,
        local_to_device: &Transform,
        shape: &Shape,
        op: ClipOp,
        snapping: PixelSnapping,
    ) -> Self {
        let mut e = RawElement {
            element: ClipElement {
                shape: shape.clone(),
                local_to_device: *local_to_device,
                op,
            },
            inner_bounds: Rect::infinite_inverted(),
            outer_bounds: Rect::default(),
            usage_bounds: Rect::infinite_inverted(),
            order: DrawOrder::K_NO_INTERSECTION,
            max_z: DrawOrder::K_CLEAR_DEPTH,
            invalidated_by_index: -1,
            capture_params: None,
            insertion: None,
        };
        // Discard shapes that don't have any area (including when a transform can't be inverted,
        // since it means the two dimensions are collapsed to 0 or 1 dimension in device space).
        if e.element.shape.is_line() || !local_to_device.valid() {
            e.element.shape.reset();
        }
        // Make sure the shape is not inverted. An inverted shape is equivalent to a non-inverted
        // shape with the clip op toggled.
        if e.element.shape.inverted() {
            e.element.op = if e.element.op == ClipOp::Intersect {
                ClipOp::Difference
            } else {
                ClipOp::Intersect
            };
        }

        e.outer_bounds = e
            .element
            .local_to_device
            .map_rect(&e.element.shape.bounds())
            .make_intersect(*device_bounds);
        e.inner_bounds = Rect::infinite_inverted();

        // Apply rect-stays-rect transforms to rects and round rects to reduce the number of
        // unique local coordinate systems that are in play.
        if !e.outer_bounds.is_empty_negative_or_nan()
            && e.element.local_to_device.type_() <= TransformType::RectStaysRect
        {
            if e.element.shape.is_rect() {
                // The actual geometry can be updated to the device-intersected bounds and we know
                // the inner bounds are equal to the outer.
                if snapping == PixelSnapping::Yes {
                    e.outer_bounds.round();
                }
                e.element.shape.set_rect(e.outer_bounds);
                e.element.local_to_device = Transform::identity();
                e.inner_bounds = e.outer_bounds;
            } else if e.element.shape.is_rrect() {
                // Can't transform in place and must still check transform result since some very
                // ill-formed scale+translate matrices can cause invalid rrect radii.
                if let Some(mut xformed) = e
                    .element
                    .shape
                    .rrect()
                    .transform(&e.element.local_to_device.to_matrix())
                {
                    if snapping == PixelSnapping::Yes {
                        // The rounded corners will still be anti-aliased, but snap the horizontal
                        // and vertical edges to pixel values.
                        let radii = *xformed.radii_ref();
                        let rounded = SkRect::from(xformed.rect().round());
                        xformed.set_rect_radii(rounded, &radii);
                    }
                    e.element.shape.set_rrect(xformed);
                    e.element.local_to_device = Transform::identity();
                    // Refresh outer bounds to match the transformed round rect in case
                    // SkRRect::transform produces slightly different results from
                    // Transform::mapRect.
                    e.outer_bounds = e.element.shape.bounds().make_intersect(*device_bounds);
                    e.inner_bounds = Rect::from_sk_rect(&rrect_priv::inner_bounds(&xformed))
                        .make_intersect(e.outer_bounds);
                }
            }
        }

        if e.outer_bounds.is_empty_negative_or_nan() {
            // Either was already an empty shape or a non-empty shape is offscreen, so treat it
            // as such.
            e.element.shape.reset();
            e.inner_bounds = Rect::infinite_inverted();
        }

        // Now that fOp and fShape are canonical, set the shape's fill type to match how it needs
        // to be drawn as a depth-only shape everywhere that is clipped out (intersect is thus
        // inverse-filled)
        e.element
            .shape
            .set_inverted(e.element.op == ClipOp::Intersect);

        // Post-conditions on inner and outer bounds
        debug_assert!(e.element.shape.is_empty() || device_bounds.contains(e.outer_bounds));
        e.validate();
        e
    }

    // `operator TransformedShape()`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L618-L620 (chrome/m156)
    fn transformed(&self) -> TransformedShape<'_> {
        TransformedShape {
            local_to_device: &self.element.local_to_device,
            shape: &self.element.shape,
            outer_bounds: self.outer_bounds,
            inner_bounds: self.inner_bounds,
            op: self.element.op,
            contains_checks_only_bounds: false,
        }
    }

    fn has_pending_draw(&self) -> bool {
        self.order != DrawOrder::K_NO_INTERSECTION
    }

    fn shape(&self) -> &Shape {
        &self.element.shape
    }

    fn local_to_device(&self) -> &Transform {
        &self.element.local_to_device
    }

    fn op(&self) -> ClipOp {
        self.element.op
    }

    // As new elements are pushed on to the stack, they may make older elements redundant.
    // The old elements are marked invalid so they are skipped during clip application, but may
    // become active again when a save record is restored.
    fn is_invalid(&self) -> bool {
        self.invalidated_by_index >= 0
    }

    // Record a depth-only draw to the given device, restricted to the portion of the clip that
    // is actually required based on prior recorded draws. Resets usage tracking for subsequent
    // passes.
    // Port of: src/gpu/graphite/ClipStack.cpp#L622-L707 (chrome/m156)
    fn draw_clip(&mut self, device: &mut ClipDevice<'_>) {
        self.validate();

        // Skip elements that have not affected any draws
        if !self.has_pending_draw() {
            debug_assert!(self.usage_bounds.is_empty_negative_or_nan());
            // TODO (thomsmit): worth to set scissor to empty and check downstream? or better to
            // allow noop draw?
            return;
        }

        debug_assert!(!self.usage_bounds.is_empty_negative_or_nan());
        // For clip draws, the usage bounds is the scissor.
        let device_bounds = device.bounds;
        let mut scissor = self.usage_bounds; // all joined usage bounds are pre-snapped

        // snappedOuterBounds was the rectangle used in updateForDraw() to query the Z order the
        // clip's draw will be inserted at. The scissor must enforce that rendering doesn't happen
        // outside of those bounds.
        let snapped_outer_bounds = snap_scissor(&self.outer_bounds, &device_bounds);
        scissor.intersect(snapped_outer_bounds);
        // But if the overlap is sufficiently large, just rasterize out to the snapped bounds
        // instead of adding a tight scissor. A factor of 1/2 is used because that corresponds to
        // the area change caused by a 45-degree rotation.
        if 0.5 * snapped_outer_bounds.area() < scissor.area() {
            scissor = snapped_outer_bounds;
        }

        let draw_bounds = if self.element.op == ClipOp::Intersect {
            scissor
        } else {
            self.outer_bounds.make_intersect(scissor)
        };
        if !draw_bounds.is_empty_negative_or_nan() {
            // Although we are recording this clip draw after all the draws it affects, 'fOrder'
            // was determined at the first usage, so after sorting by DrawOrder the clip draw will
            // be in the right place. Unlike regular draws that use their own "Z", by writing
            // (1 + max Z this clip affects), it will cause those draws to fail either GREATER and
            // GEQUAL depth tests where they need to be clipped.
            let order = DrawOrder::with_paint_order(self.max_z.next(), self.order);
            // An element's clip op is encoded in the shape's fill type. Inverse fills are
            // intersect ops and regular fills are difference ops. This means fShape is already in
            // the right state to draw directly.
            debug_assert!(
                (self.element.op == ClipOp::Difference && !self.element.shape.inverted())
                    || (self.element.op == ClipOp::Intersect && self.element.shape.inverted())
            );

            // NOTE: We use fOuterBounds as the transformed shape bounds as that hasn't been
            // clipped by the scissor. It has been clipped by the device bounds, but that
            // shouldn't impact any decisions at this point. If that becomes not the case, we can
            // either recompute the shape's device-space bounds
            // (fLocalToDevice.mapRect(fShape.bounds())) or store a fully unclipped shape bounds
            // on the RawElement.
            if device.hooks.use_draw_list_layer() {
                // TODO (thomsmit), rename this function to updateDeferredClip when
                // drawListLayer is used
                let params = self
                    .capture_params
                    .expect("a pending clip draw of the layered draw list was captured");
                device
                    .hooks
                    .update_clip_draw(params, order, draw_bounds, scissor.as_sk_irect());
                device.hooks.update_next_depth_for_clipping(order.depth());
            } else {
                device.hooks.draw_clip_shape(
                    &self.element.local_to_device,
                    &self.element.shape,
                    &Clip::new(
                        draw_bounds,
                        self.outer_bounds,
                        scissor.as_sk_irect(),
                        NonMSAAClip::default(),
                        /* shader= */ false,
                    ),
                    order,
                );
            }
        }

        // After the clip shape is drawn, reset its state. If the clip element is being popped off
        // the stack or overwritten because a new clip invalidated it, this won't matter. But if
        // the clips were drawn because the Device had to flush pending work while the clip stack
        // was not empty, subsequent draws will still need to be clipped to the elements. In this
        // case, the usage accumulation process will begin again and automatically use the
        // Device's post-flush Z values and BoundsManager state.
        self.usage_bounds = Rect::infinite_inverted();
        self.order = DrawOrder::K_NO_INTERSECTION;
        self.max_z = DrawOrder::K_CLEAR_DEPTH;
        self.capture_params = None;
        self.insertion = None;
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L709-L731 (chrome/m156)
    fn draw_clip_immediate(&mut self, device: &mut ClipDevice<'_>, snapped_outer_bounds: &Rect) {
        // We can't call validate but we need to make sure we have something to draw here.
        debug_assert!(!self.element.shape.is_empty());

        // We shouldn't be drawing if we already drew this clip element
        debug_assert!(self.insertion.is_none());
        debug_assert!(self.capture_params.is_none());

        // Note, passing fOuterBounds here may influence the preference for wedges here.
        let (params, insertion) = device.hooks.draw_clip_shape_immediate(
            &self.element.local_to_device,
            &self.element.shape,
            &Clip::new(
                *snapped_outer_bounds,
                *snapped_outer_bounds,
                snapped_outer_bounds.as_sk_irect(),
                NonMSAAClip::default(),
                /* shader= */ false,
            ),
            DrawOrder::with_paint_order(self.max_z, self.order),
        );
        self.capture_params = params;
        self.insertion = insertion;

        debug_assert!(self.insertion.is_some());
        debug_assert!(self.capture_params.is_some());
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L733-L744 (chrome/m156)
    fn validate(&self) {
        // If the shape type isn't empty, the outer bounds shouldn't be empty; if the inner bounds
        // are not empty, they must be contained in outer.
        debug_assert!(
            (self.element.shape.is_empty() || !self.outer_bounds.is_empty_negative_or_nan())
                && (self.inner_bounds.is_empty_negative_or_nan()
                    || self.outer_bounds.contains(self.inner_bounds))
        );
        debug_assert!(
            (self.element.op == ClipOp::Difference && !self.element.shape.inverted())
                || (self.element.op == ClipOp::Intersect && self.element.shape.inverted())
        );
        debug_assert!(!self.has_pending_draw() || !self.usage_bounds.is_empty_negative_or_nan());
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L746-L753 (chrome/m156)
    // `first_active` is `current.firstActiveElementIndex()`.
    fn mark_invalid(&mut self, first_active: i32) {
        debug_assert!(!self.is_invalid());
        self.invalidated_by_index = first_active;
        // NOTE: We don't draw the accumulated clip usage when the element is marked invalid. Some
        // invalidated elements are part of earlier save records so can become re-active after a
        // restore in which case they should continue to accumulate. Invalidated elements that are
        // part of the active save record are removed at the end of the stack modification, which
        // is when they are explicitly drawn.
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L755-L759 (chrome/m156)
    fn restore_valid(&mut self, first_active: i32) {
        if first_active < self.invalidated_by_index {
            self.invalidated_by_index = -1;
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L761-L798 (chrome/m156)
    fn combine(&mut self, other: &RawElement) -> bool {
        // Don't combine elements that have collected draw usage, since that changes their
        // geometry.
        if self.has_pending_draw() || other.has_pending_draw() {
            return false;
        }
        // To reduce the number of possibilities, only consider intersect+intersect. Difference
        // and mixed op cases could be analyzed to simplify one of the shapes, but that is a rare
        // occurrence and the math is much more complicated.
        if other.element.op != ClipOp::Intersect || self.element.op != ClipOp::Intersect {
            return false;
        }

        // NOTE: intersect_shape operates on the underlying geometry and ignores the fill rule,
        // which because these are intersect clip ops, is the inverse fill. If the shape is
        // updated, the resulting geometry is set to a regular fill so it must be re-inverted to
        // represent the pixels rasterized for a depth-only clip draw.
        let mut edge_flags = EdgeFlags::ALL;
        let shape_updated = intersect_shape(
            &other.element.local_to_device,
            &other.element.shape,
            &self.element.local_to_device,
            &mut self.element.shape,
            &mut edge_flags,
        );
        debug_assert_eq!(edge_flags, EdgeFlags::ALL);

        if shape_updated {
            // This logic works under the assumption that both combined elements were intersect.
            debug_assert!(
                self.element.op == ClipOp::Intersect && other.element.op == ClipOp::Intersect
            );
            self.outer_bounds.intersect(other.outer_bounds);
            self.inner_bounds.intersect(other.inner_bounds);
            // Inner bounds can become empty, but outer bounds should not be able to.
            debug_assert!(!self.outer_bounds.is_empty_negative_or_nan());
            self.element.shape.set_inverted(true); // Undo intersect_shape setting it to non-inverse
            self.validate();
            true
        } else {
            false
        }
    }

    // 'added' represents a new op added to the element stack. Its combination with this element
    // can result in a number of possibilities:
    //  1. The entire clip is empty (signaled by both this and 'added' being invalidated).
    //  2. The 'added' op supercedes this element (this element is invalidated).
    //  3. This op supercedes the 'added' element (the added element is marked invalidated).
    //  4. Their combination can be represented by a single new op (in which case this element
    //     should be invalidated, and the combined shape stored in 'added').
    //  5. Or both elements remain needed to describe the clip (both are valid and unchanged).
    //
    // The calling element will only modify its invalidation index since it could belong to part
    // of the inactive stack (that might be restored later). All merged state/geometry is handled
    // by modifying 'added'.
    // Port of: src/gpu/graphite/ClipStack.cpp#L800-L835 (chrome/m156)
    fn update_for_element(&mut self, added: &mut RawElement, first_active: i32) {
        if self.is_invalid() {
            // Already doesn't do anything, so skip this element
            return;
        }

        // 'A' refers to this element, 'B' refers to 'added'.
        match simplify(&self.transformed(), &added.transformed()) {
            SimplifyResult::Empty => {
                // Mark both elements as invalid to signal that the clip is fully empty
                self.mark_invalid(first_active);
                added.mark_invalid(first_active);
            }
            SimplifyResult::AOnly => {
                // This element already clips more than 'added', so mark 'added' is invalid to
                // skip it
                added.mark_invalid(first_active);
            }
            SimplifyResult::BOnly => {
                // 'added' clips more than this element, so mark this as invalid
                self.mark_invalid(first_active);
            }
            SimplifyResult::Both => {
                // Else the bounds checks think we need to keep both, but depending on the
                // combination of the ops and shape kinds, we may be able to do better.
                if added.combine(self) {
                    // 'added' now fully represents the combination of the two elements
                    self.mark_invalid(first_active);
                }
            }
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L837-L844 (chrome/m156)
    fn test_for_draw(&self, draw: &TransformedShape<'_>) -> DrawInfluence {
        if self.is_invalid() {
            // Cannot affect the draw
            return DrawInfluence::None;
        }

        simplify_for_draw(&self.transformed(), draw)
    }

    // Updates usage tracking to incorporate the bounds and Z value for the new draw call.
    // If this element hasn't affected any prior draws, it will use the bounds manager to
    // assign itself a compressed painters order for later rendering.
    //
    // This method assumes that this element affects the draw in a complex way, such that
    // calling `testForDraw()` on the same draw would return `DrawInfluence::kIntersect`. It is
    // assumed that `testForDraw()` was called beforehand to ensure that this is the case.
    //
    // Assuming that this element does not clip out the draw, returns the painters order the
    // draw must sort after.
    // Port of: src/gpu/graphite/ClipStack.cpp#L846-L917 (chrome/m156)
    #[allow(clippy::if_not_else)] // mirrors the C++ branch order
    fn update_for_draw(
        &mut self,
        device: &mut ClipDevice<'_>,
        bounds_manager: &dyn BoundsManager,
        device_bounds: &Rect,
        snapped_draw_bounds: &Rect,
        draw_z: PaintersDepth,
    ) -> (CompressedPaintersOrder, Option<LayerId>) {
        debug_assert!(!self.is_invalid());

        if !self.has_pending_draw() {
            // No usage yet so we need an order that we will use when drawing to just the depth
            // attachment. It is sufficient to use the next CompressedPaintersOrder after the
            // most recent draw under this clip's outer bounds. It is necessary to use the
            // entire clip's outer bounds because the order has to be determined before the
            // final usage bounds are known and a subsequent draw could require a completely
            // different portion of the clip than this triggering draw.
            //
            // Lazily determining the order has several benefits to computing it when the clip
            // element was first created:
            //  - Elements that are invalidated by nested clips before draws are made do not
            //    waste time in the BoundsManager.
            //  - Elements that never actually modify a draw (e.g. a defensive clip) do not
            //    waste time in the BoundsManager.
            //  - A draw that triggers clip usage on multiple elements will more likely assign
            //    the same order to those elements, meaning their depth-only draws are more
            //    likely to batch in the final DrawPass.
            //
            // However, it does mean that clip elements can have the same order as each other,
            // or as later draws (e.g. after the clip has been popped off the stack). Any
            // overlap between clips or draws is addressed when the clip is drawn by selecting
            // an appropriate DisjointStencilIndex value. Stencil-aside, this order assignment
            // logic, max Z tracking, and the depth test during rasterization are able to
            // resolve everything correctly even if clips have the same order value.
            // See go/clip-stack-order for a detailed analysis of why this works.
            let snapped_outer_bounds = snap_scissor(&self.outer_bounds, device_bounds);
            self.usage_bounds = *snapped_draw_bounds;
            self.max_z = draw_z;

            if device.hooks.use_draw_list_layer() {
                self.draw_clip_immediate(device, &snapped_outer_bounds);
                // Use this value to force hasPendingDraw() to return true.
                // TODO (thomsmit): Change this to a bool when drawListLayer is implemented.
                self.order = DrawOrder::K_NO_INTERSECTION.next();
            } else {
                self.order = bounds_manager
                    .get_most_recent_draw(snapped_outer_bounds)
                    .next();
            }
        } else {
            // Earlier draws have already used this element so we cannot change where the
            // depth-only draw will be sorted to, but we need to ensure we cover the new draw's
            // bounds and use a Z value that will clip out its pixels as appropriate.
            self.usage_bounds.join(*snapped_draw_bounds);
            if draw_z > self.max_z {
                self.max_z = draw_z;
            }
        }

        (self.order, self.insertion)
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L919-L945 (chrome/m156)
    fn clip_type(&self) -> ClipState {
        // Map from the internal shape kind to the clip state enum
        use crate::graphite::geom::shape::Type as ShapeType;
        match self.element.shape.type_() {
            ShapeType::Empty => ClipState::Empty,

            ShapeType::Rect => {
                if self.element.op == ClipOp::Intersect
                    && self.element.local_to_device.type_() == TransformType::Identity
                {
                    ClipState::DeviceRect
                } else {
                    ClipState::Complex
                }
            }

            ShapeType::RRect => {
                if self.element.op == ClipOp::Intersect
                    && self.element.local_to_device.type_() == TransformType::Identity
                {
                    ClipState::DeviceRRect
                } else {
                    ClipState::Complex
                }
            }

            // These types should never become RawElements, but call them kComplex in release
            // builds
            ShapeType::Arc | ShapeType::Line => {
                debug_assert!(false, "arcs and lines never become RawElements");
                ClipState::Complex
            }

            ShapeType::Path => ClipState::Complex,
        }
    }
}

// ===========================================================================================
// SaveRecord
// ===========================================================================================

/// Represents a saved point in the clip stack, and manages the life time of elements added to
/// stack within the record's life time. Also provides the logic for determining active elements
/// given a draw query (`ClipStack::SaveRecord`).
// Port of: src/gpu/graphite/ClipStack.h#L315-L376 (chrome/m156)
#[derive(Clone, Debug)]
struct SaveRecord {
    // Inner bounds is always contained in outer bounds, or it is empty. All bounds will be
    // contained in the device bounds.
    inner_bounds: Rect, // Inside is full coverage (stack op == intersect) or 0 cov (diff)
    outer_bounds: Rect, // Outside is 0 coverage (op == intersect) or full cov (diff)

    // A save record can have up to one shader, multiple shaders are automatically blended
    shader: Option<Shader>,

    starting_element_index: usize, // First element owned by this save record
    oldest_valid_index: i32,       // Index of oldest element that's valid for this record
    deferred_save_count: i32,      // Number of save() calls without modifications (yet)

    // Will be kIntersect unless every valid element is kDifference, which is significant
    // because if kDifference then there is an implicit extra outer bounds at the device edges.
    stack_op: ClipOp,
    state: ClipState,
    gen_id: u32,
}

impl SaveRecord {
    // Port of: src/gpu/graphite/ClipStack.cpp#L951-L960 (chrome/m156)
    fn new(device_bounds: &Rect) -> Self {
        Self {
            inner_bounds: *device_bounds,
            outer_bounds: *device_bounds,
            shader: None,
            starting_element_index: 0,
            oldest_valid_index: 0,
            deferred_save_count: 0,
            stack_op: ClipOp::Intersect,
            state: ClipState::WideOpen,
            gen_id: INVALID_GEN_ID,
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L962-L977 (chrome/m156)
    fn from_prior(prior: &SaveRecord, starting_element_index: usize) -> Self {
        // If the prior record added an element, this one will insert into the same index
        // (that's okay since we'll remove it when this record is popped off the stack).
        debug_assert!(starting_element_index >= prior.starting_element_index);
        Self {
            inner_bounds: prior.inner_bounds,
            outer_bounds: prior.outer_bounds,
            shader: prior.shader.clone(),
            starting_element_index,
            oldest_valid_index: prior.oldest_valid_index,
            deferred_save_count: 0,
            stack_op: prior.stack_op,
            state: prior.state,
            gen_id: INVALID_GEN_ID,
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L979-L990 (chrome/m156)
    fn gen_id(&self) -> u32 {
        if self.state == ClipState::Empty {
            EMPTY_GEN_ID
        } else if self.state == ClipState::WideOpen {
            WIDE_OPEN_GEN_ID
        } else {
            // The gen ID shouldn't be empty or wide open, since they are reserved for the above
            // if-cases. It may be kInvalid if the record hasn't had any elements added to it yet.
            debug_assert!(self.gen_id != EMPTY_GEN_ID && self.gen_id != WIDE_OPEN_GEN_ID);
            self.gen_id
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L992-L998 (chrome/m156)
    fn state(&self) -> ClipState {
        if self.shader.is_some() && self.state != ClipState::Empty {
            ClipState::Complex
        } else {
            self.state
        }
    }

    fn first_active_element_index(&self) -> i32 {
        i32::try_from(self.starting_element_index).expect("the element stack is small")
    }

    fn can_be_updated(&self) -> bool {
        self.deferred_save_count == 0
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1000-L1007 (chrome/m156)
    fn test_for_draw(&self, draw: &TransformedShape<'_>) -> DrawInfluence {
        let identity = Transform::identity();
        let outer_save_bounds = Shape::from_rect(self.outer_bounds);
        let save = TransformedShape {
            local_to_device: &identity,
            shape: &outer_save_bounds,
            outer_bounds: self.outer_bounds,
            inner_bounds: self.inner_bounds,
            op: self.stack_op,
            contains_checks_only_bounds: true,
        };

        simplify_for_draw(&save, draw)
    }

    // Deferred save manipulation
    fn push_save(&mut self) {
        debug_assert!(self.deferred_save_count >= 0);
        self.deferred_save_count += 1;
    }

    // Returns true if the record should stay alive. False means the ClipStack must delete it
    fn pop_save(&mut self) -> bool {
        self.deferred_save_count -= 1;
        debug_assert!(self.deferred_save_count >= -1);
        self.deferred_save_count >= 0
    }

    // Remove the elements owned by this save record, which must happen before the save record
    // itself is removed from the clip stack. Records draws for any removed elements that have
    // draw usages.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1009-L1016 (chrome/m156)
    fn remove_elements(&self, elements: &mut Vec<RawElement>, device: &mut ClipDevice<'_>) {
        while elements.len() > self.starting_element_index {
            // Since the element is being deleted now, it won't be in the ClipStack when the
            // Device calls recordDeferredClipDraws(). Record the clip's draw now (if it needs
            // it).
            elements
                .last_mut()
                .expect("the stack is not empty")
                .draw_clip(device);
            elements.pop();
        }
    }

    // Restore element validity now that this record is the new top of the stack.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1018-L1030 (chrome/m156)
    fn restore_elements(&self, elements: &mut [RawElement]) {
        // Presumably this SaveRecord is the new top of the stack, and so it owns the elements
        // from its starting index to restoreCount - 1. Elements from the old save record have
        // been destroyed already, so their indices would have been >= restoreCount, and any
        // still-present element can be un-invalidated based on that.
        let first_active = self.first_active_element_index();
        let mut i = i32::try_from(elements.len()).expect("the element stack is small") - 1;
        for e in elements.iter_mut().rev() {
            if i < self.oldest_valid_index {
                break;
            }
            e.restore_valid(first_active);
            i -= 1;
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1032-L1044 (chrome/m156)
    fn add_shader(&mut self, shader: Shader) {
        debug_assert!(self.can_be_updated());
        match self.shader.take() {
            None => self.shader = Some(shader),
            Some(old) => {
                // The total coverage is computed by multiplying the coverage from each element
                // (shape or shader), but since multiplication is associative, we can use kSrcIn
                // blending to make a new shader that represents 'shader' * 'fShader'
                self.shader = Some(shaders::blend(BlendMode::SrcIn, shader, old));
            }
        }
    }

    // Return true if the element was added to 'elements', or otherwise affected the save record
    // (e.g. turned it empty).
    // Port of: src/gpu/graphite/ClipStack.cpp#L1046-L1163 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn add_element(
        &mut self,
        to_add: RawElement,
        elements: &mut Vec<RawElement>,
        device: &mut ClipDevice<'_>,
    ) -> bool {
        // Validity check the element's state first
        to_add.validate();

        // And we shouldn't be adding an element if we have a deferred save
        debug_assert!(self.can_be_updated());

        if self.state == ClipState::Empty {
            // The clip is already empty, and we only shrink, so there's no need to record this
            // element.
            return false;
        } else if to_add.shape().is_empty() {
            // An empty difference op should have been detected earlier, since it's a no-op
            debug_assert_eq!(to_add.op(), ClipOp::Intersect);
            self.state = ClipState::Empty;
            self.remove_elements(elements, device);
            return true;
        }

        // Here we treat the SaveRecord as a "TransformedShape" with the identity transform, and a
        // shape equal to its outer bounds. This lets us get accurate intersection tests against
        // the new element, but we pass true to skip more detailed contains checks because the
        // SaveRecord's shape is potentially very different from its aggregate outer bounds.
        let simplified = {
            let outer_save_bounds = Shape::from_rect(self.outer_bounds);
            let identity = Transform::identity();
            let save = TransformedShape {
                local_to_device: &identity,
                shape: &outer_save_bounds,
                outer_bounds: self.outer_bounds,
                inner_bounds: self.inner_bounds,
                op: self.stack_op,
                contains_checks_only_bounds: true,
            };

            // In this invocation, 'A' refers to the existing stack's bounds and 'B' refers to the
            // new element.
            simplify(&save, &to_add.transformed())
        };
        match simplified {
            SimplifyResult::Empty => {
                // The combination results in an empty clip
                self.state = ClipState::Empty;
                self.remove_elements(elements, device);
                return true;
            }

            SimplifyResult::AOnly => {
                // The combination would not be any different than the existing clip
                return false;
            }

            SimplifyResult::BOnly => {
                // The combination would invalidate the entire existing stack and can be replaced
                // with just the new element.
                self.replace_with_element(to_add, elements, device);
                return true;
            }

            SimplifyResult::Both => {
                // The new element combines in a complex manner, so update the stack's bounds
                // based on the combination of its and the new element's ops (handled below)
            }
        }

        if self.state == ClipState::WideOpen {
            // When the stack was wide open and the clip effect was kBoth, the "complex" manner is
            // simply to keep the element and update the stack bounds to be the element's
            // intersected with the device.
            self.replace_with_element(to_add, elements, device);
            return true;
        }

        // Some form of actual clip element(s) to combine with.
        if self.stack_op == ClipOp::Intersect {
            if to_add.op() == ClipOp::Intersect {
                // Intersect (stack) + Intersect (toAdd)
                //  - Bounds updates is simply the paired intersections of outer and inner.
                self.outer_bounds.intersect(to_add.outer_bounds);
                self.inner_bounds.intersect(to_add.inner_bounds);
                // Outer should not have become empty, but is allowed to if there's no
                // intersection.
                debug_assert!(!self.outer_bounds.is_empty_negative_or_nan());
            } else {
                // Intersect (stack) + Difference (toAdd)
                //  - Shrink the stack's outer bounds if the difference op's inner bounds
                //    completely cuts off an edge.
                //  - Shrink the stack's inner bounds to completely exclude the op's outer bounds.
                self.outer_bounds = subtract(&self.outer_bounds, &to_add.inner_bounds, true);
                self.inner_bounds = subtract(&self.inner_bounds, &to_add.outer_bounds, false);
            }
        } else if to_add.op() == ClipOp::Intersect {
            // Difference (stack) + Intersect (toAdd)
            //  - Bounds updates are just the mirror of Intersect(stack) + Difference(toAdd)
            let old_outer = self.outer_bounds;
            self.outer_bounds = subtract(&to_add.outer_bounds, &self.inner_bounds, true);
            self.inner_bounds = subtract(&to_add.inner_bounds, &old_outer, false);
        } else {
            // Difference (stack) + Difference (toAdd)
            //  - The updated outer bounds is the union of outer bounds and the inner becomes the
            //    largest of the two possible inner bounds
            self.outer_bounds.join(to_add.outer_bounds);
            if to_add.inner_bounds.area() > self.inner_bounds.area() {
                self.inner_bounds = to_add.inner_bounds;
            }
        }

        // If we get here, we're keeping the new element and the stack's bounds have been updated.
        // We ought to have caught the cases where the stack bounds resemble an empty or wide open
        // clip, so assert that's the case.
        debug_assert!(
            !self.outer_bounds.is_empty_negative_or_nan()
                && (self.inner_bounds.is_empty_negative_or_nan()
                    || self.outer_bounds.contains(self.inner_bounds))
        );

        self.append_element(to_add, elements, device)
    }

    // These functions modify 'elements' and element-dependent state of the record (such as valid
    // index and fState). Records draws for any clips that have deferred usages that are
    // inactivated and cannot be restored (i.e. part of the active save record).
    // Port of: src/gpu/graphite/ClipStack.cpp#L1165-L1292 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn append_element(
        &mut self,
        mut to_add: RawElement,
        elements: &mut Vec<RawElement>,
        device: &mut ClipDevice<'_>,
    ) -> bool {
        let count = i32::try_from(elements.len()).expect("the element stack is small");
        let first_active = self.first_active_element_index();

        // Update past elements to account for the new element
        let mut i = count - 1;

        // After the loop, elements between [max(youngestValid, startingIndex)+1, count-1] can be
        // removed from the stack (these are the active elements that have been invalidated by the
        // newest element; since it's the active part of the stack, no restore() can bring them
        // back).
        let mut youngest_valid = first_active - 1;
        // After the loop, elements between [0, oldestValid-1] are all invalid. The value of
        // oldestValid becomes the save record's new fLastValidIndex value.
        let mut oldest_valid = count;
        // After the loop, this is the earliest active element that was invalidated. It may be
        // older in the stack than earliestValid, so cannot be popped off, but can be used to
        // store the new element instead of allocating more.
        let mut oldest_active_invalid: Option<usize> = None;
        let mut oldest_active_invalid_index = count;

        for existing in elements.iter_mut().rev() {
            if i < self.oldest_valid_index {
                break;
            }
            // We don't need to pass the actual index that toAdd will be saved to; just the
            // minimum index of this save record, since that will result in the same restoration
            // behavior later.
            existing.update_for_element(&mut to_add, first_active);

            if to_add.is_invalid() {
                if existing.is_invalid() {
                    // Both new and old invalid implies the entire clip becomes empty
                    self.state = ClipState::Empty;
                    return true;
                }
                // The new element doesn't change the clip beyond what the old element already
                // does
                return false;
            } else if existing.is_invalid() {
                // The new element cancels out the old element. The new element may have been
                // modified to account for the old element's geometry.
                if i >= first_active {
                    // Still active, so the invalidated index could be used to store the new
                    // element
                    oldest_active_invalid =
                        Some(usize::try_from(i).expect("the element index is not negative"));
                    oldest_active_invalid_index = i;
                }
            } else {
                // Keep both new and old elements
                oldest_valid = i;
                if i > youngest_valid {
                    youngest_valid = i;
                }
            }

            i -= 1;
        }

        // Post-iteration validity check
        debug_assert!(
            oldest_valid == count
                || (oldest_valid >= self.oldest_valid_index && oldest_valid < count)
        );
        debug_assert!(
            youngest_valid == first_active - 1
                || (youngest_valid >= first_active && youngest_valid < count)
        );
        debug_assert!(
            (oldest_active_invalid.is_some()
                && oldest_active_invalid_index >= first_active
                && oldest_active_invalid_index < count)
                || oldest_active_invalid.is_none()
        );

        // Update final state
        debug_assert!(oldest_valid >= self.oldest_valid_index);
        self.oldest_valid_index = oldest_valid.min(oldest_active_invalid_index);
        self.state = if oldest_valid == count {
            to_add.clip_type()
        } else {
            ClipState::Complex
        };
        if self.stack_op == ClipOp::Difference && to_add.op() == ClipOp::Intersect {
            // The stack remains in difference mode only as long as all elements are difference
            self.stack_op = ClipOp::Intersect;
        }

        let mut target_count = youngest_valid + 1;
        if oldest_active_invalid.is_none() || oldest_active_invalid_index >= target_count {
            // toAdd will be stored right after youngestValid
            target_count += 1;
            oldest_active_invalid = None;
        }
        let target_count = usize::try_from(target_count).expect("the target count is positive");
        while elements.len() > target_count {
            // shouldn't delete what we'll reuse
            debug_assert_ne!(oldest_active_invalid, Some(elements.len() - 1));
            elements
                .last_mut()
                .expect("the stack is not empty")
                .draw_clip(device);
            elements.pop();
        }
        if let Some(index) = oldest_active_invalid {
            elements[index].draw_clip(device);
            elements[index] = to_add;
        } else if elements.len() < target_count {
            elements.push(to_add);
        } else {
            let back = elements.last_mut().expect("the stack is not empty");
            back.draw_clip(device);
            *back = to_add;
        }

        // Changing this will prompt ClipStack to invalidate any masks associated with this
        // record.
        self.gen_id = next_gen_id();
        true
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1294-L1322 (chrome/m156)
    fn replace_with_element(
        &mut self,
        to_add: RawElement,
        elements: &mut Vec<RawElement>,
        device: &mut ClipDevice<'_>,
    ) {
        // The aggregate state of the save record mirrors the element
        self.inner_bounds = to_add.inner_bounds;
        self.outer_bounds = to_add.outer_bounds;
        self.stack_op = to_add.op();
        self.state = to_add.clip_type();

        // All prior active element can be removed from the stack: [startingIndex, count - 1]
        let target_count = self.starting_element_index + 1;
        while elements.len() > target_count {
            elements
                .last_mut()
                .expect("the stack is not empty")
                .draw_clip(device);
            elements.pop();
        }
        if elements.len() < target_count {
            elements.push(to_add);
        } else {
            let back = elements.last_mut().expect("the stack is not empty");
            back.draw_clip(device);
            *back = to_add;
        }

        debug_assert_eq!(elements.len(), self.starting_element_index + 1);

        // This invalidates all older elements that are owned by save records lower in the clip
        // stack.
        self.oldest_valid_index = self.first_active_element_index();
        self.gen_id = next_gen_id();
    }
}

// ===========================================================================================
// DrawShape
// ===========================================================================================

/// `DrawShape` represents the approximate shape that is being drawn in order to compare it against
/// the clip stack's `RawElement`s. It is able to map to a `TransformedShape` to be simplified with
/// either the `SaveRecord` or each element. For non-Shape geometries and stroked shapes, it
/// represents the oriented bounding box. For filled Shapes, it preserves the original shape for
/// more accurate contains/intersect checks and geometrically combining `RawElement`s into the
/// shape (`ClipStack::DrawShape`).
// Port of: src/gpu/graphite/ClipStack.cpp#L1324-L1415 (chrome/m156)
struct DrawShape {
    local_to_device: Transform,

    // When 'style' isn't fill, the original geometry describes the pre-stroked shape, so 'fShape'
    // is updated to include the bounds post-stroking. `fShape` may also include local AA outsets
    // under certain circumstances:
    //  1. If it's a hairline, the AA outset can be added in local space to preserve a tighter
    //     oriented bbox compared to device bounds outset by 1px.
    //  2. If it's subpixel, the rendered geometry is often treated as a hairline with an adjusted
    //     coverage ramp.
    // Notably, the local AA outset is not included in `styledShape` for other cases to maximize
    // the cases where a draw is contained in a clip, or can be clipped geometrically. This
    // assumes that rendering an AA'ed non-hairline/subpixel edge produces a 1px feathered edge
    // that's not qualitatively different from the 1px feathered edge a clip would enforce.
    shape: Shape,
    edge_flags: EdgeFlags,

    // Not valid until after applyStyle() is called, although applyScissor() can shrink the inner
    // and outer bounds.
    transformed_shape_bounds: Rect,
    outer_bounds: Rect,
    inner_bounds: Rect,

    scissor: Rect,

    // Whether or not the shape matches the original geometry to draw (with style)
    shape_matches_geometry: bool,
    // Whether or not the clip stack can modify this shape in place (and if it has already done
    // so).
    shape_compatible_with_intersect_shape: bool,
    shape_was_modified: bool,
}

impl DrawShape {
    // Port of: src/gpu/graphite/ClipStack.cpp#L1384-L1415 (chrome/m156)
    fn new(local_to_device: &Transform, geometry: &Geometry) -> Self {
        // `geometry.maskToDevice()` is null for every ported geometry (see the module docs).
        let mut shape = Shape::default();
        let mut edge_flags = EdgeFlags::ALL;
        let shape_matches_geometry;
        if let Geometry::Shape(s) = geometry {
            shape = s.clone();
            shape_matches_geometry = true;
        } else {
            // The geometry is something special like text or vertices, in which case it's
            // definitely not a shape that could simplify cleanly with the clip stack, so just
            // track its bounds. The exception is EdgeAA quads that are rectangular, in which
            // case we can clip its edges and adjust edge flags.
            shape.set_rect(geometry.bounds());
            if let Geometry::EdgeAAQuad(quad) = geometry {
                edge_flags = quad.edge_flags();
                shape_matches_geometry = quad.is_rect();
            } else {
                shape_matches_geometry = false;
            }
            // If geometry is not a shape, it is not inverted.
            debug_assert!(!shape.inverted());
        }

        let shape_compatible_with_intersect_shape =
            shape.is_flood_fill() || (!shape.inverted() && (shape.is_rect() || shape.is_rrect()));

        Self {
            local_to_device: *local_to_device,
            shape,
            edge_flags,
            transformed_shape_bounds: Rect::default(),
            outer_bounds: Rect::default(),
            inner_bounds: Rect::default(),
            scissor: Rect::infinite(),
            shape_matches_geometry,
            shape_compatible_with_intersect_shape,
            shape_was_modified: false,
        }
    }

    // `operator TransformedShape()`
    // Port of: src/gpu/graphite/ClipStack.cpp#L1355-L1366 (chrome/m156)
    fn transformed(&self) -> TransformedShape<'_> {
        // A regular draw is a transformed shape that "intersects" the clip. An inverse-filled
        // draw is equivalent to "difference". For simple convex shapes we provide an inner bounds
        // because we can geometrically intersect clip elements with the draw geometry and not
        // really impact the choice of Renderer (given the family of renderers used for simple
        // shapes). In theory any convex shape could provide an inner bounds and/or use the
        // detailed contains check, but that would cause path rendering draws to potentially
        // change in hard to predict ways.
        let op = if self.shape.inverted() {
            ClipOp::Difference
        } else {
            ClipOp::Intersect
        };
        TransformedShape {
            local_to_device: &self.local_to_device,
            shape: &self.shape,
            outer_bounds: self.outer_bounds,
            inner_bounds: self.inner_bounds,
            op,
            contains_checks_only_bounds: !self.shape_can_be_modified(),
        }
    }

    fn shape_can_be_modified(&self) -> bool {
        self.shape_compatible_with_intersect_shape && self.shape_matches_geometry
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1417-L1522 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    #[allow(clippy::if_not_else)] // mirrors the C++ branch order
    fn apply_style(&mut self, style: &StrokeRec, device_bounds: &Rect) -> bool {
        self.transformed_shape_bounds = self.shape.bounds(); // not scissor'ed, regular fill rule bounds
        let orig_size = self.transformed_shape_bounds.size();
        if !is_finite_all(orig_size.x(), &[orig_size.y()]) {
            // Discard all non-finite geometry as if it were clipped out
            return false;
        }

        // Discard fills and strokes that cannot produce any coverage: an empty fill, or a
        // zero-length stroke that has butt caps. Otherwise the stroke style applies to a vertical
        // or horizontal line (making it non-empty), or it's a zero-length path segment that
        // must produce round or square caps (making it non-empty):
        //     https://www.w3.org/TR/SVG11/implnote.html#PathElementImplementationNotes
        if !self.shape.inverted()
            && (self.shape.is_line() || vx::any(orig_size.eq_mask(Float2::from(0.0))))
            && (style.is_fill_style()
                || (style.cap() == Cap::Butt && vx::all(orig_size.eq_mask(Float2::from(0.0)))))
        {
            return false;
        }

        // Anti-aliasing makes shapes larger than their original coordinates, but we only care
        // about that for local clip checks in certain cases (see below).
        // NOTE: After this if-else block, `transformedShapeBounds` will be in device space.
        let local_aa_outset = self
            .local_to_device
            .local_aa_radius(&self.transformed_shape_bounds);
        if !is_finite(local_aa_outset) {
            // We cannot calculate an accurate local shape bounds, and transformedShapeBounds is
            // meant to be unclipped. This is to maximize atlas reuse for mostly unclipped draws
            // and to detect when a scissor state change is required. Setting
            // transformedShapeBounds to deviceBounds is harmless in this case as these benefits
            // are unlikely to apply for this transform.
            self.transformed_shape_bounds = *device_bounds;
            self.shape.set_rect(*device_bounds);
            self.local_to_device = Transform::identity();
            self.shape_matches_geometry = false;
        } else {
            // SkStrokeRec::GetInflationRadius() returns a device-space inflation for hairlines.
            let mut local_outset = 0.0_f32;
            if !style.is_fill_style() && !style.is_hairline_style() {
                // Rectangles, rounded rectangles, and lines do not produce miters so don't count
                // the pessimistic limit against their draw bounds.
                let effective_miter_limit = if self.shape.is_path() {
                    style.miter()
                } else {
                    1.0
                };
                // Rectangles and rounded rectangles don't have caps, so don't count that against
                // their draw bounds (if we could efficiently know a path was a closed contour, it
                // could be included here too).
                let effective_cap = if self.shape.is_rect() || self.shape.is_rrect() {
                    Cap::Butt
                } else {
                    style.cap()
                };
                local_outset = StrokeRec::inflation_radius_from_params(
                    style.join(),
                    effective_miter_limit,
                    effective_cap,
                    style.width(),
                );
            }

            if style.is_hairline_style()
                || (!style.is_fill_style() && style.width() < local_aa_outset)
                || (style.is_fill_style()
                    && !self.shape.inverted()
                    && vx::any(orig_size.lt_mask(Float2::from(local_aa_outset))))
            {
                // The geometry is a hairline or projects to a subpixel shape, so rendering will
                // not follow the typical 1/2px outset anti-aliasing that is compatible with
                // clipping. In this case, apply the local AA radius to the shape to have a
                // conservative clip query while preserving the oriented bounding box.
                local_outset += local_aa_outset;
            }

            if local_outset > 0.0 {
                // Propagate style and AA outset into styledShape so clip queries reflect style.
                self.transformed_shape_bounds.outset(local_outset);
                let inverted = self.shape.inverted();
                if self.shape.is_rrect() {
                    // Try to preserve the rounded corners, which can reduce the chance of
                    // clipping stroked rounded rects that are clipped to a round rect matching
                    // their outer edge.
                    let mut rrect = *self.shape.rrect();
                    rrect.outset((local_outset, local_outset));
                    self.shape.set_rrect(rrect);
                } else {
                    self.shape.set_rect(self.transformed_shape_bounds); // it's still local at this point
                }
                self.shape.set_inverted(inverted); // preserve original inversion state
                self.shape_matches_geometry = false;
            }

            self.transformed_shape_bounds = self
                .local_to_device
                .map_rect(&self.transformed_shape_bounds);
        }

        self.outer_bounds = self.transformed_shape_bounds;
        self.inner_bounds = Rect::infinite_inverted();

        if self.shape_can_be_modified()
            && self.local_to_device.type_() <= TransformType::RectStaysRect
        {
            if self.shape.is_rect() {
                self.inner_bounds = self.outer_bounds;
            } else if self.shape.is_rrect() {
                let rrect_inner_bounds = rrect_priv::inner_bounds(self.shape.rrect());
                if !rrect_inner_bounds.is_empty() {
                    self.inner_bounds = self
                        .local_to_device
                        .map_rect(&Rect::from_sk_rect(&rrect_inner_bounds));
                }
            }
            // Otherwise it's a flood fill, but should have empty bounds anyways
        }
        // Otherwise we either don't need the inner bounds, or the inner bounds can't be computed
        // for a non-axis-aligned transform

        true // Something can be drawn based on style (might still be clipped out)
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1524-L1533 (chrome/m156)
    fn apply_scissor(&mut self, scissor: &Rect) {
        // Apply the scissor to the outer bounds because it restricts rasterization and will allow
        // the SaveRecord::testForDraw() case to detect no clip influence if only the scissor is
        // needed.
        debug_assert_eq!(*scissor, Rect::from_sk_irect(&scissor.as_sk_irect())); // `scissor` must be integer valued
        self.scissor.intersect(*scissor); // For first call, fScissor is infinite so this is assignment
        self.outer_bounds.intersect(*scissor);
        self.inner_bounds.intersect(*scissor);
    }

    // Sync any modifications back to `geometry` and return a Clip object encapsulating the
    // tracked bounds of the now-clipped draw.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1535-L1568 (chrome/m156)
    fn finish_clip(
        &mut self,
        geometry: &mut Geometry,
        non_msaa_clip: NonMSAAClip,
        has_clip_shader: bool,
    ) -> Clip {
        if self.shape_was_modified {
            // Sync back to the geometry that will be drawn
            debug_assert!(self.shape_can_be_modified());
            if geometry.is_edge_aa_quad() && self.shape.is_rect() {
                // Preserve the EdgeAAQuad geometry type and sync updated edge flags
                debug_assert!(geometry.edge_aa_quad().is_rect());
                *geometry = Geometry::EdgeAAQuad(EdgeAAQuad::from_rect(
                    *self.shape.rect(),
                    self.edge_flags,
                ));
            } else {
                debug_assert_eq!(self.edge_flags, EdgeFlags::ALL);
                *geometry = Geometry::Shape(self.shape.clone());
            }
            // Reconstruct new transformedShapeBounds and outer bounds
            self.transformed_shape_bounds = self.local_to_device.map_rect(&self.shape.bounds());
            self.outer_bounds = self.transformed_shape_bounds.make_intersect(self.scissor);
        }

        let draw_bounds = if self.shape.inverted() {
            self.scissor
        } else {
            self.outer_bounds
        };
        // If the draw isn't clipped out (empty drawBounds), it should be in the scissor rect
        debug_assert!(draw_bounds.is_empty_negative_or_nan() || self.scissor.contains(draw_bounds));
        // If the scissor is empty, the draw bounds must also be empty
        debug_assert!(
            !self.scissor.is_empty_negative_or_nan() || draw_bounds.is_empty_negative_or_nan()
        );
        // fScissor.asSkIRect() must be equivalent
        debug_assert_eq!(
            self.scissor,
            Rect::from_sk_irect(&self.scissor.as_sk_irect())
        );
        Clip::new(
            draw_bounds,
            self.transformed_shape_bounds,
            self.scissor.as_sk_irect(),
            non_msaa_clip,
            has_clip_shader,
        )
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1570-L1578 (chrome/m156)
    fn reset_to_flood_fill(&mut self) {
        if self.shape_can_be_modified() && !self.shape.is_flood_fill() {
            self.shape.reset();
            self.shape.set_inverted(true);
            self.edge_flags = EdgeFlags::ALL;
            self.outer_bounds = Rect::infinite_inverted();
            self.inner_bounds = Rect::infinite_inverted();
            self.shape_was_modified = true;
        }
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1580-L1604 (chrome/m156)
    fn intersect_clip_element(&mut self, clip: &RawElement) -> bool {
        debug_assert_eq!(clip.op(), ClipOp::Intersect);
        if self.shape_can_be_modified()
            && intersect_shape(
                clip.local_to_device(),
                clip.shape(),
                &self.local_to_device,
                &mut self.shape,
                &mut self.edge_flags,
            )
        {
            debug_assert!(!self.shape.inverted());
            if self.outer_bounds.is_empty_negative_or_nan() {
                // Changing from a flood fill to the clip's shape
                self.outer_bounds = clip.outer_bounds;
                self.inner_bounds = clip.inner_bounds;
            } else {
                // Restricting the shape's geometry by the clip
                self.outer_bounds.intersect(clip.outer_bounds);
                self.inner_bounds.intersect(clip.inner_bounds);
                debug_assert!(!self.outer_bounds.is_empty_negative_or_nan()); // Should have been caught earlier
            }

            self.shape_was_modified = true;
            return true;
        }

        false
    }
}

// ===========================================================================================
// Analytic clips
// ===========================================================================================

// The corners of an `SkRRect`, in the order of `SkRRect::Corner`.
const CORNERS: [Corner; 4] = [
    Corner::UpperLeft,
    Corner::UpperRight,
    Corner::LowerRight,
    Corner::LowerLeft,
];

// Decide whether we can use this shape to do analytic clipping. Only rects and certain
// rrects are supported. We assume these have been pre-transformed by the RawElement
// constructor, so only identity transforms are allowed.
// Port of: src/gpu/graphite/ClipStack.cpp#L1757-L1845 (chrome/m156)
#[allow(clippy::similar_names)] // mirrors the C++ names rect/rrect
fn can_apply_analytic_clip(shape: &Shape, local_to_device: &Transform) -> AnalyticClip {
    if local_to_device.type_() > TransformType::Affine {
        return AnalyticClip::default();
    }

    // Since the transformation is affine, it can be represented as a 2x2 matrix and a
    // translation. To minimize data sent to the GPU, the translation is pre-applied to the
    // rectangle coordinates and only the 2x2 needs to be sent. The analytic clip is invoked with
    // device coordinates so the inverse's 2x2 is used.
    let dev_to_local = local_to_device.inverse_matrix();
    let mut xform = Float4::new(
        dev_to_local.rc(0, 0),
        dev_to_local.rc(1, 0), // column-major
        dev_to_local.rc(0, 1),
        dev_to_local.rc(1, 1),
    );
    // Applying this translation to the local rrect geometry moves it into a new coordinate space
    // where just the 2x2 of localToDevice is needed to map to device space (conversely, where
    // just `xform` is needed to map device coords to the rrect's new space).
    let m = local_to_device.matrix();
    let tx = xform[0] * m.rc(0, 3) + xform[2] * m.rc(1, 3);
    let ty = xform[1] * m.rc(0, 3) + xform[3] * m.rc(1, 3);

    // Can handle Rect directly.
    if shape.is_rect() {
        return AnalyticClip {
            bounds: shape.rect().make_offset(Float2::new(tx, ty)).as_sk_rect(),
            radii: Float4::new(0.0, 0.0, 0.0, 0.0),
            xform,
            inverted: shape.inverted(),
        };
    }

    // Otherwise we only handle certain kinds of RRects, specifically only rrects with circular
    // corners (although each corner can differ). We don't just check AllCornersRelativelyCircular
    // because we can fold an Y-axis scale factor into the 2x2 transform if that non-uniform
    // scaling could make all corners effectively circular.
    if !shape.is_rrect() {
        return AnalyticClip::default();
    }

    let tolerance =
        local_to_device.local_aa_radius(&shape.bounds()) * Shape::DEFAULT_PIXEL_TOLERANCE;

    let rrect = shape.rrect();
    let mut scale_y_axis: Option<f32> = None;
    let mut radii = Float4::new(0.0, 0.0, 0.0, 0.0);
    for (i, corner) in CORNERS.iter().enumerate() {
        let r = rrect.radii(*corner);
        let corner_scale = if rrect_priv::is_relatively_circular(r.x, r.y, tolerance) {
            1.0
        } else {
            ieee_float_divide(r.x, r.y)
        };

        if r.x < tolerance || r.y < tolerance {
            radii[i] = 0.0; // Clamp to a square corner, so doesn't impact scale factor
        } else if let Some(scale) = scale_y_axis {
            // We already have a scale factor from some other corner, so we need to agree.
            if !<f32 as Scalar>::nearly_equal(corner_scale, scale, tolerance) {
                return AnalyticClip::default(); // Would not pass AllCornersRelativelyCircular after scaling
            }
            radii[i] = r.x;
        } else {
            // We haven't encountered a non-circular corner yet. Set the scale factor to the
            // current radii ratio (which will be 1 if it's already circular).
            scale_y_axis = Some(corner_scale);
            radii[i] = r.x;
        }
    }

    let mut rect = Rect::from_sk_rect(rrect.rect()).make_offset(Float2::new(tx, ty));
    if let Some(s) = scale_y_axis
        && !<f32 as Scalar>::nearly_equal(s, 1.0, tolerance)
    {
        rect.set_top(rect.top() * s);
        rect.set_bot(rect.bot() * s);

        // Since we scaled the rrect by s, we should scale its local-to-device matrix by 1/s to
        // remain the same shape. However, `xform` is the device-to-local matrix so as the
        // inverse, we also just have to multiply by s.
        xform[1] *= s;
        xform[3] *= s;
    }

    AnalyticClip {
        bounds: rect.as_sk_rect(),
        radii,
        xform,
        inverted: shape.inverted(),
    }
}

// ===========================================================================================
// ClipStack
// ===========================================================================================

/// The clip state of a `Device` (`skgpu::graphite::ClipStack`).
// Port of: src/gpu/graphite/ClipStack.h#L35-L426 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClipStack")]
#[derive(Debug)]
pub struct ClipStack {
    // SaveRecords and Elements are stored in two parallel stacks. The top-most SaveRecord is the
    // active record, older records represent earlier save points and aren't modified until they
    // become active again. Elements may be owned by the active SaveRecord, in which case they are
    // fully mutable, or they may be owned by a prior SaveRecord. However, Elements from both the
    // active SaveRecord and older records can be valid and affect draw operations. Elements are
    // marked inactive when new elements are determined to supersede their effect completely.
    // Inactive elements of the active SaveRecord can be deleted immediately; inactive elements of
    // older SaveRecords may become active again as the save stack is popped back.
    //
    // See go/grclipstack-2.0 for additional details and visualization of the data structures.
    elements: Vec<RawElement>,
    saves: Vec<SaveRecord>, // always has one wide open record at the top

    // The size of the device this clip stack is coupled with.
    width: i32,
    height: i32,
}

// The panics are SkASSERT-style invariants of the C++ (the base save record is never popped).
#[allow(clippy::missing_panics_doc)]
impl ClipStack {
    /// `ClipStack(owningDevice)`: a wide-open clip for a device of `width` x `height` pixels.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1627-L1633 (chrome/m156)
    #[must_use]
    pub fn new(width: i32, height: i32) -> Self {
        let mut stack = Self {
            elements: Vec::new(),
            saves: Vec::new(),
            width,
            height,
        };
        // Start with a save record that is wide open
        let device_bounds = stack.device_bounds();
        stack.saves.push(SaveRecord::new(&device_bounds));
        stack
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1668-L1670 (chrome/m156)
    #[allow(clippy::cast_precision_loss)] // device sizes are small
    fn device_bounds(&self) -> Rect {
        Rect::wh(self.width as f32, self.height as f32)
    }

    fn current_save_record(&self) -> &SaveRecord {
        self.saves
            .last()
            .expect("the base save record is never popped")
    }

    /// `clipState()`.
    #[doc(alias = "clipState")]
    #[must_use]
    pub fn clip_state(&self) -> ClipState {
        self.current_save_record().state()
    }

    /// `maxDeferredClipDraws()`: the most clip draws a flush can record.
    #[doc(alias = "maxDeferredClipDraws")]
    #[must_use]
    pub fn max_deferred_clip_draws(&self) -> usize {
        self.elements.len()
    }

    /// `conservativeBounds()`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1672-L1689 (chrome/m156)
    #[doc(alias = "conservativeBounds")]
    #[must_use]
    pub fn conservative_bounds(&self) -> Rect {
        let current = self.current_save_record();
        if current.state() == ClipState::Empty {
            Rect::infinite_inverted()
        } else if current.state() == ClipState::WideOpen {
            self.device_bounds()
        } else if current.stack_op == ClipOp::Difference {
            // The outer/inner bounds represent what's cut out, so full bounds remains the device
            // bounds, minus any fully clipped content that spans the device edge.
            subtract(&self.device_bounds(), &current.inner_bounds, true)
        } else {
            debug_assert!(self.device_bounds().contains(current.outer_bounds));
            current.outer_bounds
        }
    }

    /// Provides for-range over active, valid clip elements from most recent to oldest
    /// (`begin()`/`end()`).
    // Port of: src/gpu/graphite/ClipStack.h#L388-L424 (chrome/m156)
    pub fn elements(&self) -> impl Iterator<Item = &ClipElement> {
        let current = self.current_save_record();
        let range = if current.state() == ClipState::Empty || current.state() == ClipState::WideOpen
        {
            // No visible clip elements when empty or wide open
            0..0
        } else {
            usize::try_from(current.oldest_valid_index)
                .expect("the oldest valid index is not negative")..self.elements.len()
        };
        self.elements[range]
            .iter()
            .rev()
            .filter(|e| !e.is_invalid())
            .map(|e| &e.element)
    }

    /// The element at `index` of the element stack (an entry of an [`ElementList`]).
    ///
    /// # Panics
    /// If `index` is not an index of the element stack.
    #[must_use]
    pub fn element(&self, index: usize) -> &ClipElement {
        &self.elements[index].element
    }

    /// The pending usage of the element at `index`, for tests: whether it has pending draws and
    /// its usage bounds.
    #[must_use]
    pub fn element_usage(&self, index: usize) -> (bool, Rect) {
        let e = &self.elements[index];
        (e.has_pending_draw(), e.usage_bounds)
    }

    /// How many elements the element stack holds, valid or not (for tests).
    #[must_use]
    pub fn element_stack_len(&self) -> usize {
        self.elements.len()
    }

    /// How many save records the stack holds, including the base one (for tests).
    #[must_use]
    pub fn save_record_count(&self) -> usize {
        self.saves.len()
    }

    /// Whether the element at `index` of the element stack is invalid, i.e. superseded by a newer
    /// element (for tests).
    #[must_use]
    pub fn element_is_invalid(&self, index: usize) -> bool {
        self.elements[index].is_invalid()
    }

    /// The bounds of the active save record: `(inner, outer)` (for tests).
    #[must_use]
    pub fn save_record_bounds(&self) -> (Rect, Rect) {
        let current = self.current_save_record();
        (current.inner_bounds, current.outer_bounds)
    }

    /// The generation ID of the active save record, which changes whenever its elements do
    /// (`SaveRecord::genID()`).
    #[must_use]
    pub fn gen_id(&self) -> u32 {
        self.current_save_record().gen_id()
    }

    /// `save()`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1635-L1638 (chrome/m156)
    pub fn save(&mut self) {
        self.saves
            .last_mut()
            .expect("the base save record is never popped")
            .push_save();
    }

    /// `restore()`: pending clip draws of the removed elements are recorded through `hooks`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1640-L1660 (chrome/m156)
    pub fn restore(&mut self, hooks: &mut dyn ClipDrawHooks) {
        let mut device = ClipDevice {
            hooks,
            bounds: self.device_bounds(),
        };
        let current = self
            .saves
            .last_mut()
            .expect("the base save record is never popped");
        if current.pop_save() {
            // This was just a deferred save being undone, so the record doesn't need to be
            // removed yet
            return;
        }

        // When we remove a save record, we delete all elements >= its starting index and any
        // masks that were rasterized for it.
        current.remove_elements(&mut self.elements, &mut device);

        self.saves.pop();
        // Restore any remaining elements that were only invalidated by the now-removed save
        // record.
        self.saves
            .last()
            .expect("restore() without a matching save()")
            .restore_elements(&mut self.elements);
    }

    // Will return the current save record, properly updating deferred saves and initializing a
    // first record if it were empty. Returns whether the save was deferred.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1691-L1702 (chrome/m156)
    fn writable_save_record(&mut self) -> bool {
        let current = self
            .saves
            .last_mut()
            .expect("the base save record is never popped");
        if current.can_be_updated() {
            // Current record is still open, so it can be modified directly
            false
        } else {
            // Must undefer the save to get a new record.
            let stays_alive = current.pop_save();
            debug_assert!(stays_alive);
            let new_record = SaveRecord::from_prior(current, self.elements.len());
            self.saves.push(new_record);
            true
        }
    }

    /// `clipShader(shader)`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1704-L1718 (chrome/m156)
    #[doc(alias = "clipShader")]
    pub fn clip_shader(&mut self, shader: Shader) {
        // Shaders can't bring additional coverage
        if self.current_save_record().state() == ClipState::Empty {
            return;
        }

        self.writable_save_record();
        self.saves
            .last_mut()
            .expect("the base save record is never popped")
            .add_shader(shader);
        // Geometry elements are not invalidated by updating the clip shader
        // TODO(b/238763003): Integrating clipShader into graphite needs more thought,
        // particularly how to handle the shader explosion and where to put the effects in the
        // GraphicsPipelineDesc. One idea is to use sample locations and draw the clipShader into
        // the depth buffer. Another is resolve the clip shader into an alpha mask image that is
        // sampled by the draw.
    }

    /// The shader of the active clip (`SaveRecord::shader()`), if any.
    #[must_use]
    pub fn clip_shader_ref(&self) -> Option<&Shader> {
        self.current_save_record().shader.as_ref()
    }

    /// `clipShape(localToDevice, shape, op, snapping)`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1720-L1763 (chrome/m156)
    #[doc(alias = "clipShape")]
    pub fn clip_shape(
        &mut self,
        hooks: &mut dyn ClipDrawHooks,
        local_to_device: &Transform,
        shape: &Shape,
        op: ClipOp,
        snapping: PixelSnapping,
    ) {
        if self.current_save_record().state() == ClipState::Empty {
            return;
        }

        // This will apply the transform if it's shape-type preserving, and clip the element's
        // bounds to the device bounds (NOT the conservative clip bounds, since those are based on
        // the net effect of all elements while device bounds clipping happens implicitly. During
        // addElement, we may still be able to invalidate some older elements).
        // NOTE: Does not try to simplify the shape type by inspecting the SkPath.
        let device_bounds = self.device_bounds();
        let element = RawElement::new(&device_bounds, local_to_device, shape, op, snapping);

        // An empty op means do nothing (for difference), or close the save record, so we try and
        // detect that early before doing additional unnecessary save record allocation.
        if element.shape().is_empty() && element.op() == ClipOp::Difference {
            // If the shape is empty and we're subtracting, this has no effect on the clip
            return;
        }
        // else we will make the clip empty, but we need a new save record to record that change
        // in the clip state; fall through to below and updateForElement() will handle it.

        let was_deferred = self.writable_save_record();
        let element_count = self.elements.len();
        let mut device = ClipDevice {
            hooks,
            bounds: device_bounds,
        };
        let added = self
            .saves
            .last_mut()
            .expect("the base save record is never popped")
            .add_element(element, &mut self.elements, &mut device);
        if !added && was_deferred {
            // We made a new save record, but ended up not adding an element to the stack.
            // So instead of keeping an empty save record around, pop it off and restore the
            // counter
            debug_assert_eq!(element_count, self.elements.len());
            self.saves.pop();
            self.saves
                .last_mut()
                .expect("the base save record is never popped")
                .push_save();
        }
    }

    /// Computes the bounds and the effective elements of the clip stack when applied to the draw
    /// described by the provided transform, shape, and stroke (`visitClipStackForDraw`).
    ///
    /// Applying clips to a draw is a mostly lazy operation except for what is returned:
    ///  - The Clip's scissor is set to `conservativeBounds()`.
    ///  - The Clip stores the draw's clipped bounds, taking into account its transform, styling,
    ///    and the above scissor.
    ///  - The Clip also stores the draw's fill-style invariant clipped bounds which is used in
    ///    atlas draws and may differ from the draw bounds.
    ///  - The Clip may contain an analytic clip (geometry or texture mask) that must be included
    ///    in the draw's `PaintParams`.
    ///  - The draw's Geometry may be intersected geometrically with clip elements, potentially
    ///    impacting the final choice of Renderer.
    ///
    /// All remaining clip elements that affect the draw will be returned in
    /// `out_effective_elements`. The per-clip element state has to be explicitly updated by
    /// calling `update_clip_state_for_draw()` which prepares the clip stack for later rendering.
    ///
    /// `clip_atlas` is the seam to the clip atlas (see [`ClipAtlasManager`]); `None` when the
    /// device has no clip atlas.
    // Port of: src/gpu/graphite/ClipStack.cpp#L1847-L2002 (chrome/m156)
    #[doc(alias = "visitClipStackForDraw")]
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    pub fn visit_clip_stack_for_draw(
        &self,
        local_to_device: &Transform,
        geometry: &mut Geometry,
        style: &StrokeRec,
        out_effective_elements: &mut ElementList,
        clip_atlas: Option<&mut dyn ClipAtlasManager>,
    ) -> Clip {
        let clipped_out = || {
            Clip::new(
                Rect::infinite_inverted(),
                Rect::infinite_inverted(),
                IRect::new_empty(),
                NonMSAAClip::default(),
                /* shader= */ false,
            )
        };

        let cs = self.current_save_record();
        if cs.state() == ClipState::Empty {
            // We know the draw is clipped out so don't bother computing the base draw bounds.
            return clipped_out();
        }
        // Compute draw bounds, clipped only to our device bounds since we need to return that
        // even if the clip stack is known to be wide-open.
        let device_bounds = self.device_bounds();

        let mut draw = DrawShape::new(local_to_device, geometry);
        if !draw.apply_style(style, &device_bounds) {
            return clipped_out();
        }

        // For intersect clips, the scissor rectangle is snapped outer bounds (to loosely restrict
        // rasterization if absolutely necessary). Cases where the draw is fully inside the
        // scissor are automatically handled during GPU command generation.
        //
        // For difference clips, a tight scissor could be `subtract(drawBounds, cs.innerBounds())`
        // but this is only useful when the clip spans across an axis of the draw and can
        // otherwise lead to scissor state thrashing since it's connected to the draw's bounds as
        // well. So just use the device bounds for simplicity.
        draw.apply_scissor(&if cs.stack_op == ClipOp::Intersect {
            snap_scissor(&cs.outer_bounds, &device_bounds)
        } else {
            device_bounds
        });

        let has_clip_shader = cs.shader.is_some();
        match cs.test_for_draw(&draw.transformed()) {
            DrawInfluence::ClipsOutDraw => {
                // The draw is offscreen or clipped out, so there is no need to visit the clip
                // elements.
                return clipped_out();
            }

            DrawInfluence::None => {
                // The draw is unaffected by the clip stack (except possibly `scissor`), and
                // there's no need to visit each clip element.
                return draw.finish_clip(geometry, NonMSAAClip::default(), has_clip_shader);
            }

            DrawInfluence::ReplacesDraw => {
                // The draw covers the clip entirely. Replace the shape with a flood fill, which
                // can intersect with shapes efficiently.
                draw.reset_to_flood_fill();
                // Check each element's influence on the draw below
            }

            DrawInfluence::ComplexInteraction => {
                // Check each element's influence on the draw below
            }
        }

        debug_assert_eq!(out_effective_elements.as_slice(), []);
        let mut non_msaa_clip = NonMSAAClip::default();
        for (i, e) in self.elements.iter().enumerate().rev() {
            if i32::try_from(i).expect("the element stack is small") < cs.oldest_valid_index {
                // All earlier elements have been invalidated by elements already processed so the
                // draw can't be affected by them and cannot contribute to their usage bounds.
                break;
            }

            match e.test_for_draw(&draw.transformed()) {
                DrawInfluence::ClipsOutDraw => {
                    // Per-element check was able to completely reject the draw.
                    out_effective_elements.clear();
                    return clipped_out();
                }

                DrawInfluence::None => {
                    // This element does not interact, so continue to the next
                    continue;
                }

                DrawInfluence::ReplacesDraw => {
                    // This element is covered entirely by the draw, so the draw's geometry can be
                    // replaced assuming the coordinate spaces are compatible. To facilitate this,
                    // we switch the drawn geometry to a flood fill and then fall through to
                    // intersection. Even if the coordinate spaces aren't in alignment, this
                    // eliminates the draw's source of analytic coverage.
                    draw.reset_to_flood_fill();
                }

                DrawInfluence::ComplexInteraction => {}
            }

            // First try to handle the clip geometrically
            if e.op() == ClipOp::Intersect && draw.intersect_clip_element(e) {
                continue;
            }
            // Second try to tighten the scissor, which is lighter weight than adding an analytic
            // clip pipeline variation or triggering MSAA.
            if e.clip_type() == ClipState::DeviceRect {
                let scissor = e.shape().rect().make_round();
                if e.shape()
                    .rect()
                    .nearly_equals(&scissor, Shape::DEFAULT_PIXEL_TOLERANCE)
                {
                    // Pass in `scissor` since these need to be integral values while
                    // nearlyEquals allows the original rect coordinates to be slightly different
                    // (causing problems later with asSkIRect()).
                    draw.apply_scissor(&scissor);
                    continue;
                }
            }
            // Third try to handle the clip analytically in the shader
            if non_msaa_clip.analytic_clip.is_empty() {
                non_msaa_clip.analytic_clip =
                    can_apply_analytic_clip(e.shape(), e.local_to_device());
                if !non_msaa_clip.analytic_clip.is_empty() {
                    continue;
                }
            }

            // Fourth, remember the element for later, either to be a depth-only draw or to be
            // flattened into a clip mask.
            // Otherwise, accumulate it for later. Depending on how many elements are collected
            // we may use the scissor, analytic clip, or MSAA/atlas.
            out_effective_elements.push(i);
        }

        // If there is no MSAA supported, rasterize any remaining elements by flattening them
        // into a single mask and storing in an atlas. Otherwise these will be handled by
        // Device::drawClip().
        if let Some(clip_atlas) = clip_atlas
            && !out_effective_elements.is_empty()
        {
            let i_mask_bounds = cs.outer_bounds.make_round_out().as_sk_irect();
            let element_refs: Vec<&ClipElement> = out_effective_elements
                .iter()
                .map(|&i| &self.elements[i].element)
                .collect();
            let mut out_pos = IPoint::new(0, 0);
            let proxy = clip_atlas.find_or_create_entry(
                cs.gen_id(),
                &element_refs,
                i_mask_bounds,
                &mut out_pos,
            );
            if let Some(proxy) = proxy {
                // Add to Clip
                let atlas_clip = &mut non_msaa_clip.atlas_clip;
                atlas_clip.out_pos = out_pos;
                atlas_clip.mask_bounds = i_mask_bounds;
                atlas_clip.atlas_texture = Some(proxy);

                // Elements are represented in the clip atlas, discard.
                out_effective_elements.clear();
            }
        }

        draw.finish_clip(geometry, non_msaa_clip, has_clip_shader)
    }

    /// Updates the per-clip element state for later rendering using pre-computed clip state data
    /// for a particular draw (`updateClipStateForDraw`). The provided `z` value is the depth
    /// value that the draw will use if it's not clipped out entirely.
    ///
    /// The returned `CompressedPaintersOrder` is the largest order that will be used by any of
    /// the clip elements that affect the draw, and the layer is the latest layer a depth-only
    /// clip draw was inserted into (layered draw list only).
    ///
    /// If the provided `clip` indicates that the draw will be clipped out, then this method has
    /// no effect and returns `DrawOrder::kNoIntersection`.
    // Port of: src/gpu/graphite/ClipStack.cpp#L2004-L2058 (chrome/m156)
    #[doc(alias = "updateClipStateForDraw")]
    pub fn update_clip_state_for_draw(
        &mut self,
        hooks: &mut dyn ClipDrawHooks,
        clip: &Clip,
        effective_elements: &ElementList,
        bounds_manager: &dyn BoundsManager,
        z: PaintersDepth,
    ) -> (CompressedPaintersOrder, Option<LayerId>) {
        if clip.is_clipped_out() {
            return (DrawOrder::K_NO_INTERSECTION, None);
        }

        debug_assert_ne!(self.current_save_record().state(), ClipState::Empty);

        // In the drawListLayer approach, each clipped draw needs to know the *latest* insertion
        // across the depth only draws that affect it. (This is the layer pointer returned by this
        // function). To facilitate this, each clip element records the latest layer it inserted
        // into across its render steps. Although effectiveElements may contain a mixture of drawn
        // and undrawn elements, taking the max across the indexes (which are monotonically
        // increasing) associated with each fDeferredLayer yields the latest layer.
        let mut latest_insertion: Option<LayerId> = None;
        let device_bounds = self.device_bounds();
        debug_assert!(!clip.draw_bounds().is_empty_negative_or_nan());

        // Always record snapped draw bounds to avoid scissor thrashing since these bounds will be
        // used to determine the scissor applied to the depth-only draw for the clip element.
        let snapped_draw_bounds = snap_scissor(&clip.draw_bounds(), &device_bounds);
        let mut max_clip_order = DrawOrder::K_NO_INTERSECTION;
        let mut device = ClipDevice {
            hooks,
            bounds: device_bounds,
        };
        for &index in effective_elements {
            let (order, insertion) = self.elements[index].update_for_draw(
                &mut device,
                bounds_manager,
                &device_bounds,
                &snapped_draw_bounds,
                z,
            );
            max_clip_order = max_clip_order.max(order);
            // Note, the > operator on the insertion only considers the layer. In the case that
            // both insertions reside on the same layer, we arbitrarily skip the update, meaning
            // that the list is the one associated with the earlier draw. Because depth draws are
            // added to the head of each layer, and thus in reverse order, the insertion of an
            // earlier draw will always be the latest depth only draw in a layer, even in the case
            // that the clip stack draws elements out of order.
            //
            // (I.e. Even you get a clipstack traversal like DrawnA UndrawnB DrawnC, if B inserts
            // into A or C's layer it either does not match and is added to the head, and thus the
            // existing A or C insertion must be the latest bindingList, or it matches A or C and
            // thus has an identical insertion. Even if this reverse ordering property was not
            // true, a clipped shading draw cannot match on a depth draw, so it would either match
            // on an existing draw or is added to to the tail, preserving the ordering).
            match (latest_insertion, insertion) {
                (None, _) => latest_insertion = insertion,
                (Some(latest), Some(insertion)) => {
                    if device.hooks.layer_order(insertion) > device.hooks.layer_order(latest) {
                        latest_insertion = Some(insertion);
                    }
                }
                (Some(_), None) => {}
            }
        }

        (max_clip_order, latest_insertion)
    }

    /// `recordDeferredClipDraws()`: records the depth-only draws of all elements with pending
    /// usage.
    // Port of: src/gpu/graphite/ClipStack.cpp#L2060-L2064 (chrome/m156)
    #[doc(alias = "recordDeferredClipDraws")]
    pub fn record_deferred_clip_draws(&mut self, hooks: &mut dyn ClipDrawHooks) {
        let mut device = ClipDevice {
            hooks,
            bounds: self.device_bounds(),
        };
        for e in &mut self.elements {
            e.draw_clip(&mut device);
        }
    }
}
