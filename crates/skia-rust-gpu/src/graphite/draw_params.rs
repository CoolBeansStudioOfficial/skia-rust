// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawParams.h

//! [`StrokeStyle`], [`Clip`] and [`DrawParams`]: the per-draw parameters a `RenderStep` reads.

use skia_rust_core::paint::{Cap, Join};
use skia_rust_core::rect::IRect;

use crate::graphite::draw_order::DrawOrder;
use crate::graphite::draw_types::BarrierType;
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::non_msaa_clip::NonMSAAClip;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::transform::Transform;

/// `std::max(a, b)` with the C++ argument order: returns `b` only when `a < b`. This differs from
/// `f32::max` for NaN and signed zeros, so the C++ form is kept.
#[inline]
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// The stroke parameters of a stroked draw (`StrokeStyle`).
// Port of: src/gpu/graphite/DrawParams.h#L17-L51 (chrome/m156)
#[doc(alias = "skgpu::graphite::StrokeStyle")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct StrokeStyle {
    // >0: relative to transform; ==0: hairline, 1px in device space.
    half_width: f32,
    // >0: miter join; ==0: bevel join; <0: round join.
    join_limit: f32,
    cap: Cap,
}

impl Default for StrokeStyle {
    // `StrokeStyle()`: zero width, zero join limit and a butt cap.
    fn default() -> Self {
        Self {
            half_width: 0.0,
            join_limit: 0.0,
            cap: Cap::Butt,
        }
    }
}

impl StrokeStyle {
    /// `StrokeStyle(width, miterLimit, join, cap)`.
    // Port of: src/gpu/graphite/DrawParams.h#L22-L30 (chrome/m156)
    #[must_use]
    pub fn new(width: f32, miter_limit: f32, join: Join, cap: Cap) -> Self {
        Self {
            half_width: std_max(0.0, 0.5 * width),
            join_limit: match join {
                Join::Miter => std_max(0.0, miter_limit),
                Join::Bevel => 0.0,
                Join::Round => -1.0,
            },
            cap,
        }
    }

    /// `isMiterJoin()`.
    // Port of: src/gpu/graphite/DrawParams.h#L32 (chrome/m156)
    #[must_use]
    pub fn is_miter_join(&self) -> bool {
        self.join_limit > 0.0
    }

    /// `isBevelJoin()`.
    // Port of: src/gpu/graphite/DrawParams.h#L33 (chrome/m156)
    #[must_use]
    pub fn is_bevel_join(&self) -> bool {
        self.join_limit == 0.0
    }

    /// `isRoundJoin()`.
    // Port of: src/gpu/graphite/DrawParams.h#L34 (chrome/m156)
    #[must_use]
    pub fn is_round_join(&self) -> bool {
        self.join_limit < 0.0
    }

    /// `halfWidth()`.
    // Port of: src/gpu/graphite/DrawParams.h#L36 (chrome/m156)
    #[must_use]
    pub const fn half_width(&self) -> f32 {
        self.half_width
    }

    /// `width()`.
    // Port of: src/gpu/graphite/DrawParams.h#L37 (chrome/m156)
    #[must_use]
    pub fn width(&self) -> f32 {
        2.0 * self.half_width
    }

    /// `miterLimit()`.
    // Port of: src/gpu/graphite/DrawParams.h#L38 (chrome/m156)
    #[must_use]
    pub fn miter_limit(&self) -> f32 {
        std_max(0.0, self.join_limit)
    }

    /// `cap()`.
    // Port of: src/gpu/graphite/DrawParams.h#L39 (chrome/m156)
    #[must_use]
    pub const fn cap(&self) -> Cap {
        self.cap
    }

    /// `join()`.
    // Port of: src/gpu/graphite/DrawParams.h#L40-L44 (chrome/m156)
    #[must_use]
    pub fn join(&self) -> Join {
        if self.join_limit > 0.0 {
            Join::Miter
        } else if self.join_limit == 0.0 {
            Join::Bevel
        } else {
            Join::Round
        }
    }

    /// `joinLimit()`: the raw join limit, compatible with `tess::StrokeParams`.
    // Port of: src/gpu/graphite/DrawParams.h#L47 (chrome/m156)
    #[must_use]
    pub const fn join_limit(&self) -> f32 {
        self.join_limit
    }
}

/// The clip state of a draw that a `RenderStep` can see (`Clip`).
///
/// Skia also carries a `const SkShader*` clip shader. Its contents are consumed by the paint's
/// key and uniforms (`PaintParams`, G6), so only whether one is present is kept here.
// Port of: src/gpu/graphite/DrawParams.h#L53-L116 (chrome/m156)
#[doc(alias = "skgpu::graphite::Clip")]
#[derive(Clone, Debug, Default)]
// `non_msaa_clip` mirrors Skia's `fNonMSAAClip` member name.
#[allow(clippy::struct_field_names)]
pub struct Clip {
    draw_bounds: Rect,
    transformed_shape_bounds: Rect,
    scissor: IRect,
    non_msaa_clip: NonMSAAClip,
    has_shader: bool,
}

impl Clip {
    /// `Clip(drawBounds, shapeBounds, scissor, nonMSAAClip, shader)`.
    // Port of: src/gpu/graphite/DrawParams.h#L58-L66 (chrome/m156)
    #[must_use]
    pub fn new(
        draw_bounds: Rect,
        shape_bounds: Rect,
        scissor: IRect,
        non_msaa_clip: NonMSAAClip,
        has_shader: bool,
    ) -> Self {
        Self {
            draw_bounds,
            transformed_shape_bounds: shape_bounds,
            scissor,
            non_msaa_clip,
            has_shader,
        }
    }

    /// `drawBounds()`.
    // Port of: src/gpu/graphite/DrawParams.h#L72 (chrome/m156)
    #[must_use]
    pub const fn draw_bounds(&self) -> Rect {
        self.draw_bounds
    }

    /// `scissor()`.
    // Port of: src/gpu/graphite/DrawParams.h#L80 (chrome/m156)
    #[must_use]
    pub const fn scissor(&self) -> IRect {
        self.scissor
    }

    /// `transformedShapeBounds()`.
    // Port of: src/gpu/graphite/DrawParams.h#L89 (chrome/m156)
    #[must_use]
    pub const fn transformed_shape_bounds(&self) -> Rect {
        self.transformed_shape_bounds
    }

    /// `nonMSAAClip()`.
    // Port of: src/gpu/graphite/DrawParams.h#L96 (chrome/m156)
    #[must_use]
    pub const fn non_msaa_clip(&self) -> &NonMSAAClip {
        &self.non_msaa_clip
    }

    /// Whether a clip shader is present (`shader()` is non-null).
    // Port of: src/gpu/graphite/DrawParams.h#L99 (chrome/m156), presence only
    #[must_use]
    pub const fn has_shader(&self) -> bool {
        self.has_shader
    }

    /// `isClippedOut()`.
    // Port of: src/gpu/graphite/DrawParams.h#L102 (chrome/m156)
    #[must_use]
    pub fn is_clipped_out(&self) -> bool {
        self.draw_bounds.is_empty_negative_or_nan()
    }

    /// `needsCoverage()`.
    // Port of: src/gpu/graphite/DrawParams.h#L103 (chrome/m156)
    #[must_use]
    pub fn needs_coverage(&self) -> bool {
        self.has_shader || !self.non_msaa_clip.is_empty()
    }

    /// `outsetBoundsForAA()`.
    // Port of: src/gpu/graphite/DrawParams.h#L104-L114 (chrome/m156)
    pub fn outset_bounds_for_aa(&mut self) {
        // We use 1px to handle both subpixel/hairline approaches and the standard 1/2px outset
        // for shapes that cover multiple pixels.
        self.transformed_shape_bounds.outset(1.0);
        // This is a no-op for inverse fills (where draw_bounds was already equal to the scissor),
        // and equivalent to draw_bounds = transformed_shape_bounds.intersect(scissor) with the
        // outset shape bounds.
        let scissor = Rect::from_sk_irect(&self.scissor);
        self.draw_bounds.outset(1.0).intersect(scissor);
    }
}

/// The parameters of one draw, as handed to a `RenderStep` (`DrawParams`).
///
/// Skia stores the transform by reference. Here it is copied: `Transform` is a small `Copy`
/// value, and this keeps `DrawParams` free of lifetimes.
// Port of: src/gpu/graphite/DrawParams.h#L118-L172 (chrome/m156)
#[doc(alias = "skgpu::graphite::DrawParams")]
#[derive(Clone, Debug)]
pub struct DrawParams {
    transform: Transform,
    geometry: Geometry,
    draw_bounds: Rect,
    transformed_shape_bounds: Rect,
    scissor: IRect,
    order: DrawOrder,
    barrier_before_draws: BarrierType,
    stroke: Option<StrokeStyle>,
}

impl DrawParams {
    /// `DrawParams(transform, geometry, clip, drawOrder, stroke, barrierBeforeDraws)`.
    // Port of: src/gpu/graphite/DrawParams.h#L121-L135 (chrome/m156)
    #[must_use]
    pub fn new(
        transform: Transform,
        geometry: Geometry,
        clip: &Clip,
        draw_order: DrawOrder,
        stroke: Option<&StrokeStyle>,
        barrier_before_draws: BarrierType,
    ) -> Self {
        Self {
            transform,
            geometry,
            draw_bounds: clip.draw_bounds(),
            transformed_shape_bounds: clip.transformed_shape_bounds(),
            scissor: clip.scissor(),
            order: draw_order,
            barrier_before_draws,
            stroke: stroke.copied(),
        }
    }

    /// `transform()`.
    // Port of: src/gpu/graphite/DrawParams.h#L137 (chrome/m156)
    #[must_use]
    pub const fn transform(&self) -> &Transform {
        &self.transform
    }

    /// `geometry()`.
    // Port of: src/gpu/graphite/DrawParams.h#L138 (chrome/m156)
    #[must_use]
    pub const fn geometry(&self) -> &Geometry {
        &self.geometry
    }

    /// `order()`.
    // Port of: src/gpu/graphite/DrawParams.h#L139 (chrome/m156)
    #[must_use]
    pub const fn order(&self) -> DrawOrder {
        self.order
    }

    /// `drawBounds()`.
    // Port of: src/gpu/graphite/DrawParams.h#L143 (chrome/m156)
    #[must_use]
    pub const fn draw_bounds(&self) -> Rect {
        self.draw_bounds
    }

    /// `transformedShapeBounds()`.
    // Port of: src/gpu/graphite/DrawParams.h#L144 (chrome/m156)
    #[must_use]
    pub const fn transformed_shape_bounds(&self) -> Rect {
        self.transformed_shape_bounds
    }

    /// `barrierBeforeDraws()`.
    // Port of: src/gpu/graphite/DrawParams.h#L145 (chrome/m156)
    #[must_use]
    pub const fn barrier_before_draws(&self) -> BarrierType {
        self.barrier_before_draws
    }

    /// `scissor()`.
    // Port of: src/gpu/graphite/DrawParams.h#L147 (chrome/m156)
    #[must_use]
    pub const fn scissor(&self) -> IRect {
        self.scissor
    }

    /// `isStroke()`.
    // Port of: src/gpu/graphite/DrawParams.h#L150 (chrome/m156)
    #[must_use]
    pub const fn is_stroke(&self) -> bool {
        self.stroke.is_some()
    }

    /// Updates the order, draw bounds and scissor of a depth-only clip draw (Skia's
    /// `friend class ClipStack`).
    // Port of: src/gpu/graphite/DrawParams.h#L176 (chrome/m156), the ClipStack's writes in
    // `RawElement::drawClip()`
    pub(crate) fn update_clip_draw(&mut self, order: DrawOrder, draw_bounds: Rect, scissor: IRect) {
        self.order = order;
        self.draw_bounds = draw_bounds;
        self.scissor = scissor;
    }

    /// `strokeStyle()`. Skia asserts that the draw is stroked.
    ///
    /// # Panics
    /// If the draw is not stroked.
    // Port of: src/gpu/graphite/DrawParams.h#L152-L155 (chrome/m156)
    #[must_use]
    pub fn stroke_style(&self) -> &StrokeStyle {
        self.stroke
            .as_ref()
            .expect("strokeStyle() called on a filled draw")
    }
}
