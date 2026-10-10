// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGGeometryEffect.h, modules/sksg/src/SkSGGeometryEffect.cpp
// (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::path::Path as SkPath;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::stroke_rec::StrokeRec;
use skia_rust_effects::corner_path_effect;
use skia_rust_effects::dash_path_effect;
use skia_rust_effects::trim_path_effect;
use skia_rust_pathops::path_op::PathOp;

use crate::geometry_node::{GEOMETRY_TRAITS, GeometryNode};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::transform::Transform;
use crate::util::scalar_changed;

/// `SkScalarNearlyZero` with the default tolerance (`SK_ScalarNearlyZero`).
// Port of: include/private/base/SkFloatingPoint.h (chrome/m156) (`SkScalarNearlyZero`)
fn scalar_nearly_zero(x: f32) -> bool {
    // SK_ScalarNearlyZero = 1 / (1 << 12)
    x.abs() <= 1.0_f32 / 4096.0_f32
}

/// The state of a geometry effect: its child and the revalidated path (`GeometryEffect`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L14-L30 (chrome/m156) (`class GeometryEffect` fields)
#[derive(Debug)]
pub struct GeometryEffectState {
    child: Rc<dyn GeometryNode>,
    path: RefCell<SkPath>,
}

impl GeometryEffectState {
    /// The child geometry (`GeometryEffect::fChild`).
    #[must_use]
    pub fn child(&self) -> &Rc<dyn GeometryNode> {
        &self.child
    }

    fn new(child: Rc<dyn GeometryNode>) -> Self {
        Self {
            child,
            path: RefCell::new(SkPath::default()),
        }
    }
}

/// `GeometryEffect::onRevalidate`: revalidates the child, derives the path from it, and returns
/// the tight bounds of that path.
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L63-L70 (chrome/m156) (`GeometryEffect::onRevalidate`)
pub fn geometry_effect_revalidate(
    state: &GeometryEffectState,
    ic: Option<&mut InvalidationController>,
    ctm: &Matrix,
    revalidate_effect: impl FnOnce(&Rc<dyn GeometryNode>, &Matrix) -> SkPath,
) -> Rect {
    state.child.revalidate(ic, ctm);
    let path = revalidate_effect(&state.child, ctm);
    let bounds = path.compute_tight_bounds();
    *state.path.borrow_mut() = path;
    bounds
}

/// `GeometryEffect::onClip`.
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L47-L49 (chrome/m156) (`GeometryEffect::onClip`)
pub fn geometry_effect_clip(state: &GeometryEffectState, canvas: &Canvas, anti_alias: bool) {
    canvas.clip_path(&state.path.borrow(), ClipOp::Intersect, anti_alias);
}

/// `GeometryEffect::onDraw`.
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L51-L53 (chrome/m156) (`GeometryEffect::onDraw`)
pub fn geometry_effect_draw(state: &GeometryEffectState, canvas: &Canvas, paint: &Paint) {
    canvas.draw_path(&state.path.borrow(), paint);
}

/// `GeometryEffect::onContains`.
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L55-L57 (chrome/m156) (`GeometryEffect::onContains`)
pub fn geometry_effect_contains(state: &GeometryEffectState, p: Point) -> bool {
    state.path.borrow().contains(p)
}

/// `GeometryEffect::onAsPath`.
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L59-L61 (chrome/m156) (`GeometryEffect::onAsPath`)
pub fn geometry_effect_as_path(state: &GeometryEffectState) -> SkPath {
    state.path.borrow().clone()
}

/// Trims the path to a range of its length (`TrimEffect`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L37-L56 (chrome/m156) (`class TrimEffect`)
#[doc(alias = "sksg::TrimEffect")]
#[derive(Debug)]
pub struct TrimEffect {
    core: NodeCore,
    state: GeometryEffectState,
    start: Cell<f32>,
    stop: Cell<f32>,
    mode: Cell<trim_path_effect::Mode>,
}

impl TrimEffect {
    /// `TrimEffect::Make(child)`: `None` if there is no child.
    // Port of: modules/sksg/include/SkSGGeometryEffect.h#L40-L42 (chrome/m156) (`TrimEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn GeometryNode>>) -> Option<Rc<Self>> {
        Some(make_geometry_effect(&child?, |core, state| Self {
            core,
            state,
            start: Cell::new(0.0),
            stop: Cell::new(1.0),
            mode: Cell::new(trim_path_effect::Mode::Normal),
        }))
    }

    /// The start of the trimmed range (`getStart`).
    #[must_use]
    pub fn start(&self) -> f32 {
        self.start.get()
    }

    /// Sets the start, invalidating the node if it changed (`setStart`).
    pub fn set_start(&self, start: f32) {
        if scalar_changed(self.start.get(), start) {
            self.start.set(start);
            self.invalidate();
        }
    }

    /// The end of the trimmed range (`getStop`).
    #[must_use]
    pub fn stop(&self) -> f32 {
        self.stop.get()
    }

    /// Sets the stop, invalidating the node if it changed (`setStop`).
    pub fn set_stop(&self, stop: f32) {
        if scalar_changed(self.stop.get(), stop) {
            self.stop.set(stop);
            self.invalidate();
        }
    }

    /// The trim mode (`getMode`).
    #[must_use]
    pub fn mode(&self) -> trim_path_effect::Mode {
        self.mode.get()
    }

    /// Sets the trim mode, invalidating the node if it changed (`setMode`).
    pub fn set_mode(&self, mode: trim_path_effect::Mode) {
        if self.mode.get() != mode {
            self.mode.set(mode);
            self.invalidate();
        }
    }

    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L73-L85 (chrome/m156) (`TrimEffect::onRevalidateEffect`)
    fn revalidate_effect(&self, child: &Rc<dyn GeometryNode>) -> SkPath {
        let path = child.as_path();
        if let Some(trim) =
            trim_path_effect::new(self.start.get(), self.stop.get(), self.mode.get())
        {
            let mut rec = StrokeRec::new_hairline();
            let mut builder = PathBuilder::new();
            let filtered = trim.filter_path_inplace(&mut builder, &path, &mut rec, None::<&Rect>);
            debug_assert!(filtered);
            return builder.detach();
        }
        path
    }
}

impl Drop for TrimEffect {
    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L43-L45 (chrome/m156) (`GeometryEffect::~GeometryEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for TrimEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, _| {
            self.revalidate_effect(child)
        })
    }
}

impl GeometryNode for TrimEffect {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }
    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }
    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// Transforms the path of its child (`GeometryTransform`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L58-L70 (chrome/m156) (`class GeometryTransform`)
#[doc(alias = "sksg::GeometryTransform")]
#[derive(Debug)]
pub struct GeometryTransform {
    core: NodeCore,
    state: GeometryEffectState,
    transform: Rc<dyn Transform>,
}

impl GeometryTransform {
    /// `GeometryTransform::Make(child, transform)`: `None` if either is missing.
    // Port of: modules/sksg/include/SkSGGeometryEffect.h#L61-L65 (chrome/m156) (`GeometryTransform::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn GeometryNode>>,
        transform: Option<Rc<dyn Transform>>,
    ) -> Option<Rc<Self>> {
        let (child, transform) = (child?, transform?);
        let effect = make_geometry_effect(&child, |core, state| Self {
            core,
            state,
            transform: Rc::clone(&transform),
        });
        // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L87-L92 (chrome/m156) (`GeometryTransform::GeometryTransform`)
        effect.observe_inval(effect.transform.as_ref());
        Some(effect)
    }

    /// The transform (`getTransform`).
    #[doc(alias = "getTransform")]
    #[must_use]
    pub fn transform(&self) -> Rc<dyn Transform> {
        Rc::clone(&self.transform)
    }

    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L98-L103 (chrome/m156) (`GeometryTransform::onRevalidateEffect`)
    fn revalidate_effect(&self, child: &Rc<dyn GeometryNode>) -> SkPath {
        self.transform.revalidate(None, &Matrix::new_identity());
        let m = self.transform.as_matrix();
        child.as_path().make_transform(&m)
    }
}

impl Drop for GeometryTransform {
    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L94-L96 (chrome/m156) (`GeometryTransform::~GeometryTransform`)
    fn drop(&mut self) {
        self.unobserve_inval(self.transform.as_ref());
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for GeometryTransform {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, _| {
            self.revalidate_effect(child)
        })
    }
}

impl GeometryNode for GeometryTransform {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }
    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }
    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// Overrides the fill type of the path of its child (`FillTypeOverride`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L72-L87 (chrome/m156) (`class FillTypeOverride`)
#[doc(alias = "sksg::FillTypeOverride")]
#[derive(Debug)]
pub struct FillTypeOverride {
    core: NodeCore,
    state: GeometryEffectState,
    fill_type: Cell<PathFillType>,
}

impl FillTypeOverride {
    /// `FillTypeOverride::Make(child, ft)`: `None` if there is no child.
    // Port of: modules/sksg/include/SkSGGeometryEffect.h#L75-L78 (chrome/m156) (`FillTypeOverride::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn GeometryNode>>, fill_type: PathFillType) -> Option<Rc<Self>> {
        Some(make_geometry_effect(&child?, |core, state| Self {
            core,
            state,
            fill_type: Cell::new(fill_type),
        }))
    }

    /// The fill type (`getFillType`).
    #[must_use]
    pub fn fill_type(&self) -> PathFillType {
        self.fill_type.get()
    }

    /// Sets the fill type, invalidating the node if it changed (`setFillType`).
    pub fn set_fill_type(&self, fill_type: PathFillType) {
        if self.fill_type.get() != fill_type {
            self.fill_type.set(fill_type);
            self.invalidate();
        }
    }

    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L105-L110 (chrome/m156) (`FillTypeOverride::onRevalidateEffect`)
    fn revalidate_effect(&self, child: &Rc<dyn GeometryNode>) -> SkPath {
        let mut path = child.as_path();
        path.set_fill_type(self.fill_type.get());
        path
    }
}

impl Drop for FillTypeOverride {
    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L43-L45 (chrome/m156) (`GeometryEffect::~GeometryEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for FillTypeOverride {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, _| {
            self.revalidate_effect(child)
        })
    }
}

impl GeometryNode for FillTypeOverride {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }
    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }
    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// The dash path effect for `intervals` and `phase`, with an odd count of intervals doubled
/// (`make_dash`).
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L114-L135 (chrome/m156) (`make_dash`)
fn make_dash(intervals: &[f32], phase: f32) -> Option<PathEffect> {
    if intervals.is_empty() {
        return None;
    }
    // An odd count of intervals is repeated, so that the on/off pattern is preserved.
    let doubled: Vec<f32> = if intervals.len() % 2 == 1 {
        intervals.iter().chain(intervals.iter()).copied().collect()
    } else {
        intervals.to_vec()
    };
    dash_path_effect::new(&doubled, phase)
}

/// Dashes the path of its child (`DashEffect`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L89-L105 (chrome/m156) (`class DashEffect`)
#[doc(alias = "sksg::DashEffect")]
#[derive(Debug)]
pub struct DashEffect {
    core: NodeCore,
    state: GeometryEffectState,
    intervals: RefCell<Vec<f32>>,
    phase: Cell<f32>,
}

impl DashEffect {
    /// `DashEffect::Make(child)`: `None` if there is no child.
    // Port of: modules/sksg/include/SkSGGeometryEffect.h#L92-L94 (chrome/m156) (`DashEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn GeometryNode>>) -> Option<Rc<Self>> {
        Some(make_geometry_effect(&child?, |core, state| Self {
            core,
            state,
            intervals: RefCell::new(Vec::new()),
            phase: Cell::new(0.0),
        }))
    }

    /// The dash intervals (`getIntervals`).
    #[must_use]
    pub fn intervals(&self) -> Vec<f32> {
        self.intervals.borrow().clone()
    }

    /// Sets the dash intervals, invalidating the node (`setIntervals`).
    pub fn set_intervals(&self, intervals: Vec<f32>) {
        if *self.intervals.borrow() != intervals {
            *self.intervals.borrow_mut() = intervals;
            self.invalidate();
        }
    }

    /// The dash phase (`getPhase`).
    #[must_use]
    pub fn phase(&self) -> f32 {
        self.phase.get()
    }

    /// Sets the dash phase, invalidating the node if it changed (`setPhase`).
    pub fn set_phase(&self, phase: f32) {
        if scalar_changed(self.phase.get(), phase) {
            self.phase.set(phase);
            self.invalidate();
        }
    }

    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L137-L149 (chrome/m156) (`DashEffect::onRevalidateEffect`)
    fn revalidate_effect(&self, child: &Rc<dyn GeometryNode>) -> SkPath {
        let path = child.as_path();
        if let Some(dash) = make_dash(&self.intervals.borrow(), self.phase.get()) {
            let mut rec = StrokeRec::new_hairline();
            let mut builder = PathBuilder::new();
            dash.filter_path_inplace(&mut builder, &path, &mut rec, None::<&Rect>);
            return builder.detach();
        }
        path
    }
}

impl Drop for DashEffect {
    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L43-L45 (chrome/m156) (`GeometryEffect::~GeometryEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for DashEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, _| {
            self.revalidate_effect(child)
        })
    }
}

impl GeometryNode for DashEffect {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }
    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }
    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// Rounds the corners of the path of its child (`RoundEffect`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L107-L118 (chrome/m156) (`class RoundEffect`)
#[doc(alias = "sksg::RoundEffect")]
#[derive(Debug)]
pub struct RoundEffect {
    core: NodeCore,
    state: GeometryEffectState,
    radius: Cell<f32>,
}

impl RoundEffect {
    /// `RoundEffect::Make(child)`: `None` if there is no child.
    // Port of: modules/sksg/include/SkSGGeometryEffect.h#L110-L112 (chrome/m156) (`RoundEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn GeometryNode>>) -> Option<Rc<Self>> {
        Some(make_geometry_effect(&child?, |core, state| Self {
            core,
            state,
            radius: Cell::new(0.0),
        }))
    }

    /// The corner radius (`getRadius`).
    #[must_use]
    pub fn radius(&self) -> f32 {
        self.radius.get()
    }

    /// Sets the corner radius, invalidating the node if it changed (`setRadius`).
    pub fn set_radius(&self, radius: f32) {
        if scalar_changed(self.radius.get(), radius) {
            self.radius.set(radius);
            self.invalidate();
        }
    }

    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L151-L163 (chrome/m156) (`RoundEffect::onRevalidateEffect`)
    fn revalidate_effect(&self, child: &Rc<dyn GeometryNode>) -> SkPath {
        let path = child.as_path();
        if let Some(round) = corner_path_effect::new(self.radius.get()) {
            let mut rec = StrokeRec::new_hairline();
            let mut builder = PathBuilder::new();
            let filtered = round.filter_path_inplace(&mut builder, &path, &mut rec, None::<&Rect>);
            debug_assert!(filtered);
            return builder.detach();
        }
        path
    }
}

impl Drop for RoundEffect {
    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L43-L45 (chrome/m156) (`GeometryEffect::~GeometryEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for RoundEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, _| {
            self.revalidate_effect(child)
        })
    }
}

impl GeometryNode for RoundEffect {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }
    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }
    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// Offsets the outline of the path of its child, by growing or shrinking it (`OffsetEffect`).
// Port of: modules/sksg/include/SkSGGeometryEffect.h#L120-L140 (chrome/m156) (`class OffsetEffect`)
#[doc(alias = "sksg::OffsetEffect")]
#[derive(Debug)]
pub struct OffsetEffect {
    core: NodeCore,
    state: GeometryEffectState,
    offset: Cell<f32>,
    miter_limit: Cell<f32>,
    join: Cell<Join>,
}

impl OffsetEffect {
    /// `OffsetEffect::Make(child)`: `None` if there is no child.
    // Port of: modules/sksg/include/SkSGGeometryEffect.h#L123-L125 (chrome/m156) (`OffsetEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn GeometryNode>>) -> Option<Rc<Self>> {
        Some(make_geometry_effect(&child?, |core, state| Self {
            core,
            state,
            offset: Cell::new(0.0),
            miter_limit: Cell::new(4.0),
            join: Cell::new(Join::Miter),
        }))
    }

    /// The offset (`getOffset`).
    #[must_use]
    pub fn offset(&self) -> f32 {
        self.offset.get()
    }

    /// Sets the offset, invalidating the node if it changed (`setOffset`).
    pub fn set_offset(&self, offset: f32) {
        if scalar_changed(self.offset.get(), offset) {
            self.offset.set(offset);
            self.invalidate();
        }
    }

    /// The miter limit (`getMiterLimit`).
    #[must_use]
    pub fn miter_limit(&self) -> f32 {
        self.miter_limit.get()
    }

    /// Sets the miter limit, invalidating the node if it changed (`setMiterLimit`).
    pub fn set_miter_limit(&self, miter_limit: f32) {
        if scalar_changed(self.miter_limit.get(), miter_limit) {
            self.miter_limit.set(miter_limit);
            self.invalidate();
        }
    }

    /// The join (`getJoin`).
    #[must_use]
    pub fn join(&self) -> Join {
        self.join.get()
    }

    /// Sets the join, invalidating the node if it changed (`setJoin`).
    pub fn set_join(&self, join: Join) {
        if self.join.get() != join {
            self.join.set(join);
            self.invalidate();
        }
    }

    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L165-L194 (chrome/m156) (`OffsetEffect::onRevalidateEffect`)
    fn revalidate_effect(&self, child: &Rc<dyn GeometryNode>, ctm: &Matrix) -> SkPath {
        let mut path = child.as_path();
        let offset = self.offset.get();
        if !scalar_nearly_zero(offset) {
            // Clamp the offset value in device space, to avoid overwhelming pathops.
            const MAX_DEV_OFFSET: f32 = 100_000.0;
            let min_scale = ctm.min_scale();
            let max_abs_offset = if min_scale < 0.0 {
                MAX_DEV_OFFSET
            } else {
                MAX_DEV_OFFSET / min_scale
            };
            // std::min(max_abs_offset, std::abs(fOffset))
            let abs_offset = offset.abs();
            let abs_offset = if abs_offset < max_abs_offset {
                abs_offset
            } else {
                max_abs_offset
            };

            let mut paint = Paint::default();
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(abs_offset * 2.0);
            paint.set_stroke_miter(self.miter_limit.get());
            paint.set_stroke_join(self.join.get());
            let mut fill_builder = PathBuilder::new();
            skia_rust_core::path_utils::fill_path_with_paint(
                &path,
                &paint,
                &mut fill_builder,
                None::<&Rect>,
                None::<Matrix>,
            );
            let fill_path = fill_builder.detach();

            let op = if offset > 0.0 {
                PathOp::Union
            } else {
                PathOp::Difference
            };
            if let Some(result) = skia_rust_pathops::op(&path, &fill_path, op) {
                path = result;
            }
            // TODO: this seems to break path combining (winding mismatch?)
            // Simplify(path, &path);
        }
        path
    }
}

impl Drop for OffsetEffect {
    // Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L43-L45 (chrome/m156) (`GeometryEffect::~GeometryEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for OffsetEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        geometry_effect_revalidate(&self.state, ic, ctm, |child, ctm| {
            self.revalidate_effect(child, ctm)
        })
    }
}

impl GeometryNode for OffsetEffect {
    fn on_clip(&self, canvas: &Canvas, anti_alias: bool) {
        geometry_effect_clip(&self.state, canvas, anti_alias);
    }
    fn on_draw(&self, canvas: &Canvas, paint: &Paint) {
        geometry_effect_draw(&self.state, canvas, paint);
    }
    fn on_contains(&self, p: Point) -> bool {
        geometry_effect_contains(&self.state, p)
    }
    fn on_as_path(&self) -> SkPath {
        geometry_effect_as_path(&self.state)
    }
}

/// Builds a geometry effect node around `child`: the `GeometryEffect` base constructor observes
/// the child.
// Port of: modules/sksg/src/SkSGGeometryEffect.cpp#L36-L41 (chrome/m156) (`GeometryEffect::GeometryEffect`)
pub fn make_geometry_effect<T: GeometryNode + 'static>(
    child: &Rc<dyn GeometryNode>,
    build: impl FnOnce(NodeCore, GeometryEffectState) -> T,
) -> Rc<T> {
    let effect = Rc::new_cyclic(|weak: &Weak<T>| {
        build(
            NodeCore::new(GEOMETRY_TRAITS, weak.clone()),
            GeometryEffectState::new(Rc::clone(child)),
        )
    });
    effect.observe_inval(child.as_ref());
    effect
}
