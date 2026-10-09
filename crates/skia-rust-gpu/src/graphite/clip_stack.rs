// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ClipStack.h (the interface Device uses), and the
// wide-open / device-rect part of src/gpu/graphite/ClipStack.cpp

//! The seam between `Device` and the clip stack.
//!
//! `ClipStack` (element tree, analytic/depth-only/atlas clips, `visitClipStackForDraw`) is G10b.
//! [`ClipStack`] lists exactly the calls `Device` makes on it, and [`ClipDrawHooks`] the calls it
//! makes back on the `Device` (the `friend class ClipStack` of `Device`). [`BasicClipStack`] is
//! what a device uses until G10b replaces it: it is exact for a wide-open clip and for clips that
//! are a single pixel-aligned device rectangle (a scissor); any other clip element is dropped
//! with a warning, so draws are clipped only by the bounds of the clip.

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::paint::Cap;
use skia_rust_core::shader::Shader;
use skia_rust_core::stroke_rec::StrokeRec;

use crate::gpu::sk_log::skia_log_w;
use crate::graphite::draw_list_types::{DrawParamsId, LayerId};
use crate::graphite::draw_order::{CompressedPaintersOrder, DrawOrder, PaintersDepth};
use crate::graphite::draw_params::Clip;
use crate::graphite::geom::bounds_manager::BoundsManager;
use crate::graphite::geom::geometry::Geometry;
use crate::graphite::geom::non_msaa_clip::AnalyticClip;
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::geom::transform::{Transform, Type as TransformType};

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
/// (`ClipStack::ElementList`).
pub type ElementList = Vec<ClipElement>;

/// What the clip stack calls on its device (`Device::drawClipShape()`,
/// `Device::drawClipShapeImmediate()` and `Device::updateNextDepthForClipping()`).
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
}

/// The calls `Device` makes on the clip stack.
// Port of: src/gpu/graphite/ClipStack.h#L28-L160 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClipStack")]
pub trait ClipStack: std::fmt::Debug {
    /// `clipState()`.
    #[doc(alias = "clipState")]
    fn clip_state(&self) -> ClipState;

    /// `conservativeBounds()`.
    #[doc(alias = "conservativeBounds")]
    fn conservative_bounds(&self) -> Rect;

    /// The active elements, from most recent to oldest (`begin()`/`end()`).
    fn elements(&self) -> Vec<ClipElement>;

    /// `save()`.
    fn save(&mut self);

    /// `restore()`: pending clip draws of the removed elements are recorded through `hooks`.
    fn restore(&mut self, hooks: &mut dyn ClipDrawHooks);

    /// `clipShape(localToDevice, shape, op, snapping)`.
    #[doc(alias = "clipShape")]
    fn clip_shape(
        &mut self,
        hooks: &mut dyn ClipDrawHooks,
        local_to_device: &Transform,
        shape: &Shape,
        op: ClipOp,
        snapping: PixelSnapping,
    );

    /// `clipShader(shader)`.
    #[doc(alias = "clipShader")]
    fn clip_shader(&mut self, shader: Shader);

    /// `visitClipStackForDraw(localToDevice, geometry, style, outEffectiveElements)`: computes
    /// the bounds of the clipped draw and the elements that affect it. `geometry` may be
    /// modified (intersected with elements).
    #[doc(alias = "visitClipStackForDraw")]
    fn visit_clip_stack_for_draw(
        &self,
        local_to_device: &Transform,
        geometry: &mut Geometry,
        style: &StrokeRec,
        out_effective_elements: &mut ElementList,
    ) -> Clip;

    /// The shader of the active clip (`Clip::shader()`), if any.
    fn clip_shader_ref(&self) -> Option<&Shader>;

    /// `updateClipStateForDraw(clip, effectiveElements, boundsManager, z)`: returns the largest
    /// order that will be used by any of the clip elements that affect the draw and the latest
    /// layer a depth-only clip draw was inserted into.
    #[doc(alias = "updateClipStateForDraw")]
    fn update_clip_state_for_draw(
        &mut self,
        hooks: &mut dyn ClipDrawHooks,
        clip: &Clip,
        effective_elements: &ElementList,
        bounds_manager: &dyn BoundsManager,
        z: PaintersDepth,
    ) -> (CompressedPaintersOrder, Option<LayerId>);

    /// `recordDeferredClipDraws()`.
    #[doc(alias = "recordDeferredClipDraws")]
    fn record_deferred_clip_draws(&mut self, hooks: &mut dyn ClipDrawHooks);
}

// Port of: src/gpu/graphite/ClipStack.cpp#L308-L318 (chrome/m156)
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

/// One save level of the [`BasicClipStack`].
#[derive(Clone, Debug)]
struct Save {
    state: ClipState,
    // The clip, in device pixels (the device bounds when wide open).
    bounds: Rect,
    shader: Option<Shader>,
}

/// The clip stack of a device until G10b (see the module docs).
#[derive(Debug)]
pub struct BasicClipStack {
    device_bounds: Rect,
    saves: Vec<Save>,
}

impl BasicClipStack {
    /// A wide-open clip for a device of `width` x `height` pixels.
    #[must_use]
    pub fn new(width: i32, height: i32) -> Self {
        #[allow(clippy::cast_precision_loss)] // device sizes are small
        let device_bounds = Rect::wh(width as f32, height as f32);
        Self {
            device_bounds,
            saves: vec![Save {
                state: ClipState::WideOpen,
                bounds: device_bounds,
                shader: None,
            }],
        }
    }

    fn current(&self) -> &Save {
        self.saves
            .last()
            .expect("the base save record is never popped")
    }

    fn current_mut(&mut self) -> &mut Save {
        self.saves
            .last_mut()
            .expect("the base save record is never popped")
    }
}

impl ClipStack for BasicClipStack {
    fn clip_state(&self) -> ClipState {
        self.current().state
    }

    fn conservative_bounds(&self) -> Rect {
        self.current().bounds
    }

    fn elements(&self) -> Vec<ClipElement> {
        let current = self.current();
        if current.state == ClipState::DeviceRect {
            vec![ClipElement {
                shape: Shape::from_rect(current.bounds),
                local_to_device: Transform::identity(),
                op: ClipOp::Intersect,
            }]
        } else {
            Vec::new()
        }
    }

    fn save(&mut self) {
        let current = self.current().clone();
        self.saves.push(current);
    }

    fn restore(&mut self, _hooks: &mut dyn ClipDrawHooks) {
        debug_assert!(self.saves.len() > 1, "restore() without a save()");
        if self.saves.len() > 1 {
            self.saves.pop();
        }
    }

    fn clip_shape(
        &mut self,
        _hooks: &mut dyn ClipDrawHooks,
        local_to_device: &Transform,
        shape: &Shape,
        op: ClipOp,
        snapping: PixelSnapping,
    ) {
        if self.current().state == ClipState::Empty {
            return;
        }
        let device_rect = if shape.is_rect()
            && !shape.inverted()
            && local_to_device.type_() <= TransformType::RectStaysRect
        {
            Some(local_to_device.map_rect(shape.rect()))
        } else {
            None
        };
        match (op, device_rect) {
            (ClipOp::Intersect, Some(mut rect)) if snapping == PixelSnapping::Yes => {
                rect.round();
                let mut bounds = self.current().bounds;
                bounds.intersect(rect);
                let current = self.current_mut();
                if bounds.is_empty_negative_or_nan() {
                    current.state = ClipState::Empty;
                    current.bounds = Rect::infinite_inverted();
                } else if bounds != current.bounds {
                    current.state = ClipState::DeviceRect;
                    current.bounds = bounds;
                }
            }
            (ClipOp::Intersect, Some(rect)) if rect == rect.make_round() => {
                let mut bounds = self.current().bounds;
                bounds.intersect(rect);
                let current = self.current_mut();
                if bounds.is_empty_negative_or_nan() {
                    current.state = ClipState::Empty;
                    current.bounds = Rect::infinite_inverted();
                } else if bounds != current.bounds {
                    current.state = ClipState::DeviceRect;
                    current.bounds = bounds;
                }
            }
            _ => {
                skia_log_w!("ClipStack (G10b) is not ported: the clip element is ignored.");
            }
        }
    }

    fn clip_shader(&mut self, shader: Shader) {
        self.current_mut().shader = Some(shader);
    }

    // Port of: src/gpu/graphite/ClipStack.cpp#L1816-L1936 (chrome/m156), the cases where the
    // clip stack is wide open or a device rect
    fn visit_clip_stack_for_draw(
        &self,
        local_to_device: &Transform,
        geometry: &mut Geometry,
        style: &StrokeRec,
        _out_effective_elements: &mut ElementList,
    ) -> Clip {
        let clipped_out = Clip::new(
            Rect::infinite_inverted(),
            Rect::infinite_inverted(),
            skia_rust_core::rect::IRect::new_empty(),
            AnalyticClip::default(),
            false,
        );

        let cs = self.current();
        if cs.state == ClipState::Empty {
            // We know the draw is clipped out so don't bother computing the base draw bounds.
            return clipped_out;
        }
        // Compute draw bounds, clipped only to our device bounds since we need to return that
        // even if the clip stack is known to be wide-open.
        let device_bounds = self.device_bounds;

        // `DrawShape::applyStyle()`
        let shape = match geometry {
            Geometry::Shape(shape) => shape.clone(),
            other => Shape::from_rect(other.bounds()),
        };
        let mut transformed_shape_bounds = shape.bounds();
        let orig_size = transformed_shape_bounds.size();
        if !orig_size.x().is_finite() || !orig_size.y().is_finite() {
            // Discard all non-finite geometry as if it were clipped out
            return clipped_out;
        }

        // Discard fills and strokes that cannot produce any coverage: an empty fill, or a
        // zero-length stroke that has butt caps.
        if !shape.inverted() && (shape.is_line() || orig_size.x() == 0.0 || orig_size.y() == 0.0) {
            #[allow(clippy::float_cmp)] // exact, as in the C++
            if style.is_fill_style()
                || (style.cap() == Cap::Butt && orig_size.x() == 0.0 && orig_size.y() == 0.0)
            {
                return clipped_out;
            }
        }

        // Anti-aliasing makes shapes larger than their original coordinates.
        let local_aa_outset = local_to_device.local_aa_radius(&transformed_shape_bounds);
        if local_aa_outset.is_finite() {
            // SkStrokeRec::GetInflationRadius() returns a device-space inflation for hairlines.
            let mut local_outset = 0.0;
            if !style.is_fill_style() && !style.is_hairline_style() {
                // Rectangles, rounded rectangles, and lines do not produce miters so don't count
                // the pessimistic limit against their draw bounds.
                let effective_miter_limit = if shape.is_path() { style.miter() } else { 1.0 };
                // Rectangles and rounded rectangles don't have caps, so don't count that against
                // their draw bounds.
                let effective_cap = if shape.is_rect() || shape.is_rrect() {
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
                    && !shape.inverted()
                    && (orig_size.x() < local_aa_outset || orig_size.y() < local_aa_outset))
            {
                // The geometry is a hairline or projects to a subpixel shape, so rendering will
                // not follow the typical 1/2px outset anti-aliasing that is compatible with
                // clipping.
                local_outset += local_aa_outset;
            }

            if local_outset > 0.0 {
                transformed_shape_bounds.outset(local_outset);
            }
            transformed_shape_bounds = local_to_device.map_rect(&transformed_shape_bounds);
        } else {
            // We cannot calculate an accurate local shape bounds, and transformedShapeBounds is
            // meant to be unclipped.
            transformed_shape_bounds = device_bounds;
        }
        let mut outer_bounds = transformed_shape_bounds;

        // The scissor: for intersect clips the snapped outer bounds of the clip; a device rect
        // that cuts the draw is a tight scissor (the element handling of C++ tightens it).
        let mut scissor = snap_scissor(&cs.bounds, &device_bounds);
        if cs.state == ClipState::DeviceRect && !cs.bounds.contains(outer_bounds) {
            scissor = cs.bounds.make_intersect(device_bounds);
        }
        outer_bounds.intersect(scissor);

        let draw_bounds = if shape.inverted() {
            scissor
        } else {
            outer_bounds
        };
        let _ = geometry;
        Clip::new(
            draw_bounds,
            transformed_shape_bounds,
            scissor.as_sk_irect(),
            AnalyticClip::default(),
            cs.shader.is_some(),
        )
    }

    fn clip_shader_ref(&self) -> Option<&Shader> {
        self.current().shader.as_ref()
    }

    fn update_clip_state_for_draw(
        &mut self,
        _hooks: &mut dyn ClipDrawHooks,
        clip: &Clip,
        effective_elements: &ElementList,
        _bounds_manager: &dyn BoundsManager,
        _z: PaintersDepth,
    ) -> (CompressedPaintersOrder, Option<LayerId>) {
        // The basic clip never has deferred elements.
        debug_assert!(effective_elements.is_empty() || clip.is_clipped_out());
        (DrawOrder::K_NO_INTERSECTION, None)
    }

    fn record_deferred_clip_draws(&mut self, _hooks: &mut dyn ClipDrawHooks) {}
}
