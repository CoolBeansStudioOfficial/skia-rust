// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGGradient.h, modules/sksg/src/SkSGGradient.cpp
// (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::Weak;

use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader as SkShader;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::shaders::{
    linear_gradient, radial_gradient, two_point_conical_gradient,
};
use skia_rust_effects::gradient::{Colors, Gradient as SkGradient, Interpolation};

use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::shader::{SHADER_TRAITS, ShaderNode, ShaderState, shader_on_revalidate};
use crate::util::scalar_changed;

/// `SkTPin(x, lo, hi)` for the color stop positions, with `SkTPin`'s NaN behaviour:
/// `std::max(lo, std::min(x, hi))`.
// Port of: include/private/base/SkTPin.h (chrome/m156) (`SkTPin`)
fn sk_tpin(x: f32, lo: f32, hi: f32) -> f32 {
    // std::min(x, hi) == (hi < x) ? hi : x
    let inner = if hi < x { hi } else { x };
    // std::max(lo, inner) == (lo < inner) ? inner : lo
    if lo < inner { inner } else { lo }
}

/// A color at a position of a gradient (`Gradient::ColorStop`).
// Port of: modules/sksg/include/SkSGGradient.h#L17-L24 (chrome/m156) (`Gradient::ColorStop`)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    /// The position of the stop, in `[0, 1]`.
    pub position: f32,
    /// The color of the stop.
    pub color: Color4f,
}

/// The state of a gradient: its shader, color stops and tile mode (`Gradient`).
// Port of: modules/sksg/include/SkSGGradient.h#L15-L49 (chrome/m156) (`class Gradient` fields)
#[derive(Debug)]
pub struct GradientBase {
    core: NodeCore,
    shader: ShaderState,
    color_stops: RefCell<Vec<ColorStop>>,
    tile_mode: Cell<TileMode>,
}

impl GradientBase {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L86-L88 (chrome/m156) (`Shader::Shader`)
    fn new(weak: Weak<dyn Node>) -> Self {
        Self {
            core: NodeCore::new(SHADER_TRAITS, weak),
            shader: ShaderState::default(),
            color_stops: RefCell::new(Vec::new()),
            tile_mode: Cell::new(TileMode::Clamp),
        }
    }
}

/// `Gradient::setColorStops`: sets the stops, invalidating `node` if they changed.
// Port of: modules/sksg/include/SkSGGradient.h#L27-L27 (chrome/m156) (`Gradient::setColorStops`)
fn set_color_stops(node: &dyn Node, base: &GradientBase, stops: Vec<ColorStop>) {
    if *base.color_stops.borrow() != stops {
        *base.color_stops.borrow_mut() = stops;
        node.invalidate();
    }
}

/// `Gradient::setTileMode`: sets the tile mode, invalidating `node` if it changed.
// Port of: modules/sksg/include/SkSGGradient.h#L28-L28 (chrome/m156) (`Gradient::setTileMode`)
fn set_tile_mode(node: &dyn Node, base: &GradientBase, mode: TileMode) {
    if base.tile_mode.get() != mode {
        base.tile_mode.set(mode);
        node.invalidate();
    }
}

/// `Gradient::onRevalidateShader`: splits the color stops into colors and positions, then makes the
/// shader with `make`. The positions are clamped to be non-decreasing and at most 1.
// Port of: modules/sksg/src/SkSGGradient.cpp#L19-L38 (chrome/m156) (`Gradient::onRevalidateShader`)
fn gradient_on_revalidate_shader(
    base: &GradientBase,
    make: impl FnOnce(&[Color4f], &[f32], TileMode) -> Option<SkShader>,
) -> Option<SkShader> {
    let stops = base.color_stops.borrow();
    if stops.is_empty() {
        return None;
    }
    let mut colors = Vec::with_capacity(stops.len());
    let mut positions = Vec::with_capacity(stops.len());
    let mut position = 0.0_f32;
    for stop in stops.iter() {
        colors.push(stop.color);
        position = sk_tpin(stop.position, position, 1.0);
        positions.push(position);
    }
    // TODO: detect even stop distributions, pass null for positions.
    make(&colors, &positions, base.tile_mode.get())
}

/// A gradient along a line from `start` to `end` (`LinearGradient`).
// Port of: modules/sksg/include/SkSGGradient.h#L51-L70 (chrome/m156) (`class LinearGradient`)
#[doc(alias = "sksg::LinearGradient")]
#[derive(Debug)]
pub struct LinearGradient {
    base: GradientBase,
    start_point: Cell<Point>,
    end_point: Cell<Point>,
}

impl LinearGradient {
    /// `LinearGradient::Make()`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> std::rc::Rc<Self> {
        std::rc::Rc::new_cyclic(|weak: &Weak<Self>| Self {
            base: GradientBase::new(weak.clone()),
            start_point: Cell::new(Point { x: 0.0, y: 0.0 }),
            end_point: Cell::new(Point { x: 0.0, y: 0.0 }),
        })
    }

    /// The color stops (`getColorStops`).
    #[must_use]
    pub fn color_stops(&self) -> Vec<ColorStop> {
        self.base.color_stops.borrow().clone()
    }

    /// Sets the color stops, invalidating the node if they changed (`setColorStops`).
    pub fn set_color_stops(&self, stops: Vec<ColorStop>) {
        set_color_stops(self, &self.base, stops);
    }

    /// The tile mode (`getTileMode`).
    #[must_use]
    pub fn tile_mode(&self) -> TileMode {
        self.base.tile_mode.get()
    }

    /// Sets the tile mode, invalidating the node if it changed (`setTileMode`).
    pub fn set_tile_mode(&self, mode: TileMode) {
        set_tile_mode(self, &self.base, mode);
    }

    /// The start point (`getStartPoint`).
    #[must_use]
    pub fn start_point(&self) -> Point {
        self.start_point.get()
    }

    /// Sets the start point, invalidating the node if it changed (`setStartPoint`).
    pub fn set_start_point(&self, p: Point) {
        if self.start_point.get() != p {
            self.start_point.set(p);
            self.invalidate();
        }
    }

    /// The end point (`getEndPoint`).
    #[must_use]
    pub fn end_point(&self) -> Point {
        self.end_point.get()
    }

    /// Sets the end point, invalidating the node if it changed (`setEndPoint`).
    pub fn set_end_point(&self, p: Point) {
        if self.end_point.get() != p {
            self.end_point.set(p);
            self.invalidate();
        }
    }
}

impl Node for LinearGradient {
    fn core(&self) -> &NodeCore {
        &self.base.core
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L90-L95 (chrome/m156) (`Shader::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        debug_assert!(self.base.core.has_inval());
        shader_on_revalidate(&self.base.shader, ShaderNode::on_revalidate_shader(self))
    }
}

impl ShaderNode for LinearGradient {
    fn shader_state(&self) -> &ShaderState {
        &self.base.shader
    }

    // Port of: modules/sksg/src/SkSGGradient.cpp#L40-L46 (chrome/m156) (`LinearGradient::onMakeShader`)
    fn on_revalidate_shader(&self) -> Option<SkShader> {
        gradient_on_revalidate_shader(&self.base, |colors, positions, tile_mode| {
            let pts = [self.start_point.get(), self.end_point.get()];
            let grad = SkGradient::new(
                Colors::new(colors, Some(positions), tile_mode, None::<ColorSpace>),
                Interpolation::default(),
            );
            linear_gradient((pts[0], pts[1]), &grad, None)
        })
    }
}

/// A gradient between two circles (`RadialGradient`).
// Port of: modules/sksg/include/SkSGGradient.h#L72-L96 (chrome/m156) (`class RadialGradient`)
#[doc(alias = "sksg::RadialGradient")]
#[derive(Debug)]
pub struct RadialGradient {
    base: GradientBase,
    start_center: Cell<Point>,
    end_center: Cell<Point>,
    start_radius: Cell<f32>,
    end_radius: Cell<f32>,
}

impl RadialGradient {
    /// `RadialGradient::Make()`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make() -> std::rc::Rc<Self> {
        std::rc::Rc::new_cyclic(|weak: &Weak<Self>| Self {
            base: GradientBase::new(weak.clone()),
            start_center: Cell::new(Point { x: 0.0, y: 0.0 }),
            end_center: Cell::new(Point { x: 0.0, y: 0.0 }),
            start_radius: Cell::new(0.0),
            end_radius: Cell::new(0.0),
        })
    }

    /// The color stops (`getColorStops`).
    #[must_use]
    pub fn color_stops(&self) -> Vec<ColorStop> {
        self.base.color_stops.borrow().clone()
    }

    /// Sets the color stops, invalidating the node if they changed (`setColorStops`).
    pub fn set_color_stops(&self, stops: Vec<ColorStop>) {
        set_color_stops(self, &self.base, stops);
    }

    /// The tile mode (`getTileMode`).
    #[must_use]
    pub fn tile_mode(&self) -> TileMode {
        self.base.tile_mode.get()
    }

    /// Sets the tile mode, invalidating the node if it changed (`setTileMode`).
    pub fn set_tile_mode(&self, mode: TileMode) {
        set_tile_mode(self, &self.base, mode);
    }

    /// The start center (`getStartCenter`).
    #[must_use]
    pub fn start_center(&self) -> Point {
        self.start_center.get()
    }

    /// Sets the start center, invalidating the node if it changed (`setStartCenter`).
    pub fn set_start_center(&self, p: Point) {
        if self.start_center.get() != p {
            self.start_center.set(p);
            self.invalidate();
        }
    }

    /// The end center (`getEndCenter`).
    #[must_use]
    pub fn end_center(&self) -> Point {
        self.end_center.get()
    }

    /// Sets the end center, invalidating the node if it changed (`setEndCenter`).
    pub fn set_end_center(&self, p: Point) {
        if self.end_center.get() != p {
            self.end_center.set(p);
            self.invalidate();
        }
    }

    /// The start radius (`getStartRadius`).
    #[must_use]
    pub fn start_radius(&self) -> f32 {
        self.start_radius.get()
    }

    /// Sets the start radius, invalidating the node if it changed (`setStartRadius`).
    pub fn set_start_radius(&self, r: f32) {
        if scalar_changed(self.start_radius.get(), r) {
            self.start_radius.set(r);
            self.invalidate();
        }
    }

    /// The end radius (`getEndRadius`).
    #[must_use]
    pub fn end_radius(&self) -> f32 {
        self.end_radius.get()
    }

    /// Sets the end radius, invalidating the node if it changed (`setEndRadius`).
    pub fn set_end_radius(&self, r: f32) {
        if scalar_changed(self.end_radius.get(), r) {
            self.end_radius.set(r);
            self.invalidate();
        }
    }
}

impl Node for RadialGradient {
    fn core(&self) -> &NodeCore {
        &self.base.core
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L90-L95 (chrome/m156) (`Shader::onRevalidate`)
    fn on_revalidate(&self, _ic: Option<&mut InvalidationController>, _ctm: &Matrix) -> Rect {
        debug_assert!(self.base.core.has_inval());
        shader_on_revalidate(&self.base.shader, ShaderNode::on_revalidate_shader(self))
    }
}

impl ShaderNode for RadialGradient {
    fn shader_state(&self) -> &ShaderState {
        &self.base.shader
    }

    // Port of: modules/sksg/src/SkSGGradient.cpp#L48-L57 (chrome/m156) (`RadialGradient::onMakeShader`)
    fn on_revalidate_shader(&self) -> Option<SkShader> {
        gradient_on_revalidate_shader(&self.base, |colors, positions, tile_mode| {
            let grad = SkGradient::new(
                Colors::new(colors, Some(positions), tile_mode, None::<ColorSpace>),
                Interpolation::default(),
            );
            let end_center = self.end_center.get();
            let end_radius = self.end_radius.get();
            if self.start_radius.get() <= 0.0 && self.start_center.get() == end_center {
                radial_gradient((end_center, end_radius), &grad, None)
            } else {
                two_point_conical_gradient(
                    (self.start_center.get(), self.start_radius.get()),
                    (end_center, end_radius),
                    &grad,
                    None,
                )
            }
        })
    }
}
