// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGColorFilter.h, modules/sksg/src/SkSGColorFilter.cpp
// (chrome/m156)

use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_data::{
    ITU_BT709_LUM_COEFF_B, ITU_BT709_LUM_COEFF_G, ITU_BT709_LUM_COEFF_R,
};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::effect_node::{effect_on_node_at, effect_on_render, effect_on_revalidate};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::paint_node::Color as ColorNode;
use crate::render_node::{Hit, RenderContext, RenderNode, ScopedRenderContext};
use crate::util::scalar_changed;

/// The state shared by the color filter nodes: the child and the revalidated filter
/// (`ColorFilter::fColorFilter`).
// Port of: modules/sksg/include/SkSGColorFilter.h#L15-L35 (chrome/m156) (`class ColorFilter` fields)
#[derive(Debug)]
pub struct ColorFilterState {
    child: Rc<dyn RenderNode>,
    color_filter: RefCell<Option<ColorFilter>>,
}

impl ColorFilterState {
    /// The child node.
    #[must_use]
    pub fn child(&self) -> &Rc<dyn RenderNode> {
        &self.child
    }
}

/// `ColorFilter::onRender`: renders the child with the color filter composed into the context.
// Port of: modules/sksg/src/SkSGColorFilter.cpp#L32-L36 (chrome/m156) (`ColorFilter::onRender`)
fn color_filter_on_render(state: &ColorFilterState, canvas: &Canvas, ctx: Option<&RenderContext>) {
    let filter = state.color_filter.borrow().clone();
    let scope = ScopedRenderContext::new(canvas, ctx).modulate_color_filter(filter);
    effect_on_render(&state.child, canvas, Some(scope.context()));
}

/// `ColorFilter::onRevalidate`: stores the revalidated filter, then revalidates the child.
// Port of: modules/sksg/src/SkSGColorFilter.cpp#L43-L49 (chrome/m156) (`ColorFilter::onRevalidate`)
fn color_filter_revalidate(
    state: &ColorFilterState,
    ic: Option<&mut InvalidationController>,
    ctm: &Matrix,
    filter: Option<ColorFilter>,
) -> Rect {
    *state.color_filter.borrow_mut() = filter;
    effect_on_revalidate(&state.child, ic, ctm)
}

/// `SK_LUM_COEFF_R/G/B` as `f32`s, in the order of the Skia constants.
const LUM_COEFF: [f32; 3] = [
    ITU_BT709_LUM_COEFF_R,
    ITU_BT709_LUM_COEFF_G,
    ITU_BT709_LUM_COEFF_B,
];

/// A 2-color gradient, as a color matrix: a luminance matrix, then a tint, composed into one.
// Port of: modules/sksg/src/SkSGColorFilter.cpp#L124-L168 (chrome/m156) (`Make2ColorGradient`)
fn make_2_color_gradient(color0: &ColorNode, color1: &ColorNode) -> Option<ColorFilter> {
    let c0 = Color4f::from(color0.color());
    let c1 = Color4f::from(color1.color());
    let d_r = c1.r - c0.r;
    let d_g = c1.g - c0.g;
    let d_b = c1.b - c0.b;
    let [lum_r, lum_g, lum_b] = LUM_COEFF;

    // The total tint matrix: the luminance L is stored in R, then interpolated component-wise.
    let tint_matrix: [f32; 20] = [
        d_r * lum_r,
        d_r * lum_g,
        d_r * lum_b,
        0.0,
        c0.r,
        d_g * lum_r,
        d_g * lum_g,
        d_g * lum_b,
        0.0,
        c0.g,
        d_b * lum_r,
        d_b * lum_g,
        d_b * lum_b,
        0.0,
        c0.b,
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ];
    color_filters::matrix_row_major(&tint_matrix, Clamp::Yes)
}

/// An N-color gradient (N > 2), as a color table composed with the luminance matrix.
// Port of: modules/sksg/src/SkSGColorFilter.cpp#L170-L220 (chrome/m156) (`MakeNColorGradient`)
fn make_n_color_gradient(colors: &[Rc<ColorNode>]) -> Option<ColorFilter> {
    // For N colors, we build a gradient color table.
    let mut r_table = [0_u8; 256];
    let mut g_table = [0_u8; 256];
    let mut b_table = [0_u8; 256];
    debug_assert!(colors.len() > 2);
    let span_count = colors.len() - 1;
    let mut span_start: usize = 0;
    for i in 0..span_count {
        // `std::round((i + 1) * 255.0f / span_count)`, in single precision.
        #[allow(clippy::cast_precision_loss, clippy::cast_sign_loss)] // indices are small
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // in [0, 255]
        let span_stop = (((i + 1) as f32 * 255.0_f32) / span_count as f32).round() as usize;
        if span_start > span_stop {
            // Degenerate case.
            continue;
        }
        let span_size = span_stop - span_start;
        debug_assert!(span_stop <= 255);

        // Fill the gradient in [span_start,span_stop] -> [c0,c1]
        let c0 = colors[i].color();
        let c1 = colors[i + 1].color();
        let mut r = f32::from(c0.r());
        let mut g = f32::from(c0.g());
        let mut b = f32::from(c0.b());
        #[allow(clippy::cast_precision_loss)] // span sizes are at most 255
        let span_size_f = span_size as f32;
        let d_r = (f32::from(c1.r()) - r) / span_size_f;
        let d_g = (f32::from(c1.g()) - g) / span_size_f;
        let d_b = (f32::from(c1.b()) - b) / span_size_f;
        for j in span_start..=span_stop {
            // The values are in [0, 255], so the casts do not truncate.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            {
                r_table[j] = r.round() as u8;
                g_table[j] = g.round() as u8;
                b_table[j] = b.round() as u8;
            }
            r += d_r;
            g += d_g;
            b += d_b;
        }
        // Ensure we always advance.
        span_start = span_stop + 1;
    }
    debug_assert_eq!(span_start, 256);

    let [lum_r, lum_g, lum_b] = LUM_COEFF;
    let luminance_matrix: [f32; 20] = [
        lum_r, lum_g, lum_b, 0.0, 0.0, // r' = L
        lum_r, lum_g, lum_b, 0.0, 0.0, // g' = L
        lum_r, lum_g, lum_b, 0.0, 0.0, // b' = L
        0.0, 0.0, 0.0, 1.0, 0.0, // a' = a
    ];
    // `TableARGB(...)->makeComposed(Matrix(...))`: result = table(matrix(x)).
    let table = color_filters::table_argb(None, Some(&r_table), Some(&g_table), Some(&b_table));
    let luminance = color_filters::matrix_row_major(&luminance_matrix, Clamp::Yes);
    color_filters::compose(table.as_ref(), luminance)
}

/// Applies a [`ColorFilter`] to the content of its child (`ExternalColorFilter` and the
/// `ColorFilter` base class).
// Port of: modules/sksg/include/SkSGColorFilter.h#L25-L45 (chrome/m156) (`class ExternalColorFilter`)
#[doc(alias = "sksg::ExternalColorFilter")]
#[derive(Debug)]
pub struct ExternalColorFilter {
    core: NodeCore,
    state: ColorFilterState,
    coverage: Cell<Coverage>,
}

/// What the color filter applies to (`ExternalColorFilter::Coverage`).
// Port of: modules/sksg/include/SkSGColorFilter.h#L36-L40 (chrome/m156) (`ExternalColorFilter::Coverage`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Coverage {
    /// The effect applies to the regular content coverage (`kNormal`).
    #[default]
    Normal,
    /// The effect applies to the full content bounding box (`kBoundingBox`).
    BoundingBox,
}

impl ExternalColorFilter {
    /// `ExternalColorFilter::Make(child)`: `None` if there is no child.
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L51-L54 (chrome/m156) (`ExternalColorFilter::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn RenderNode>>) -> Option<Rc<Self>> {
        let child = child?;
        let filter = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            state: ColorFilterState {
                child: Rc::clone(&child),
                color_filter: RefCell::new(None),
            },
            coverage: Cell::new(Coverage::Normal),
        });
        // The EffectNode base observes the child.
        filter.observe_inval(filter.state.child.as_ref());
        Some(filter)
    }

    /// The color filter (`getColorFilter`).
    #[must_use]
    pub fn color_filter(&self) -> Option<ColorFilter> {
        self.state.color_filter.borrow().clone()
    }

    /// Sets the color filter, invalidating the node if it changed (`setColorFilter`).
    pub fn set_color_filter(&self, filter: Option<ColorFilter>) {
        if *self.state.color_filter.borrow() != filter {
            *self.state.color_filter.borrow_mut() = filter;
            self.invalidate();
        }
    }

    /// The coverage (`getCoverage`).
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        self.coverage.get()
    }

    /// Sets the coverage, invalidating the node if it changed (`setCoverage`).
    pub fn set_coverage(&self, coverage: Coverage) {
        if self.coverage.get() != coverage {
            self.coverage.set(coverage);
            self.invalidate();
        }
    }
}

impl Drop for ExternalColorFilter {
    // Port of: modules/sksg/src/SkSGEffectNode.cpp (chrome/m156) (`EffectNode::~EffectNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for ExternalColorFilter {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // The external filter does not replace the filter from its child: it is an EffectNode.
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        effect_on_revalidate(&self.state.child, ic, ctm)
    }
}

impl RenderNode for ExternalColorFilter {
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L60-L71 (chrome/m156) (`ExternalColorFilter::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let filter = self.state.color_filter.borrow().clone();
        let mut local_ctx = ScopedRenderContext::new(canvas, ctx).modulate_color_filter(filter);
        if self.coverage.get() == Coverage::BoundingBox {
            // For bounding box coverage, use a layer clipped to the content bounding box.
            let bounds = self.core.bounds();
            canvas.save();
            canvas.clip_rect(bounds, ClipOp::Intersect, true);
            local_ctx = local_ctx.set_isolation(&bounds, &canvas.total_matrix(), true);
        }
        effect_on_render(&self.state.child, canvas, Some(local_ctx.context()));
    }

    // Port of: modules/sksg/src/SkSGEffectNode.cpp#L24-L26 (chrome/m156) (`EffectNode::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        effect_on_node_at(&self.state.child, p)
    }
}

/// A color filter that blends a paint color with the child content (`ModeColorFilter`).
// Port of: modules/sksg/include/SkSGColorFilter.h#L47-L60 (chrome/m156) (`class ModeColorFilter`)
#[doc(alias = "sksg::ModeColorFilter")]
#[derive(Debug)]
pub struct ModeColorFilter {
    core: NodeCore,
    state: ColorFilterState,
    color: Rc<ColorNode>,
    mode: BlendMode,
}

impl ModeColorFilter {
    /// `ModeColorFilter::Make(child, color, mode)`: `None` if either is missing.
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L73-L78 (chrome/m156) (`ModeColorFilter::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn RenderNode>>,
        color: Option<Rc<ColorNode>>,
        mode: BlendMode,
    ) -> Option<Rc<Self>> {
        let (child, color) = (child?, color?);
        let filter = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            state: ColorFilterState {
                child: Rc::clone(&child),
                color_filter: RefCell::new(None),
            },
            color: Rc::clone(&color),
            mode,
        });
        // The EffectNode base observes the child, then ModeColorFilter observes the color.
        filter.observe_inval(filter.state.child.as_ref());
        // Port of: modules/sksg/src/SkSGColorFilter.cpp#L80-L85 (chrome/m156) (`ModeColorFilter::ModeColorFilter`)
        filter.observe_inval(filter.color.as_ref());
        Some(filter)
    }

    /// `ModeColorFilter::onRevalidateFilter`.
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L91-L94 (chrome/m156) (`ModeColorFilter::onRevalidateFilter`)
    fn on_revalidate_filter(&self) -> Option<ColorFilter> {
        self.color.revalidate(None, &Matrix::new_identity());
        color_filters::blend_color(self.color.color(), self.mode)
    }
}

impl Drop for ModeColorFilter {
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L87-L89 (chrome/m156) (`ModeColorFilter::~ModeColorFilter`)
    fn drop(&mut self) {
        self.unobserve_inval(self.color.as_ref());
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for ModeColorFilter {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L43-L49 (chrome/m156) (`ColorFilter::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let filter = self.on_revalidate_filter();
        color_filter_revalidate(&self.state, ic, ctm, filter)
    }
}

impl RenderNode for ModeColorFilter {
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L32-L36 (chrome/m156) (`ColorFilter::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        color_filter_on_render(&self.state, canvas, ctx);
    }

    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L38-L41 (chrome/m156) (`ColorFilter::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        effect_on_node_at(&self.state.child, p)
    }
}

/// A color filter that interpolates the content through a gradient of colors, indexed by
/// luminance (`GradientColorFilter`).
// Port of: modules/sksg/include/SkSGColorFilter.h#L62-L80 (chrome/m156) (`class GradientColorFilter`)
#[doc(alias = "sksg::GradientColorFilter")]
#[derive(Debug)]
pub struct GradientColorFilter {
    core: NodeCore,
    state: ColorFilterState,
    colors: Vec<Rc<ColorNode>>,
    weight: Cell<f32>,
}

impl GradientColorFilter {
    /// `GradientColorFilter::Make(child, c0, c1)`: a two-color gradient.
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L96-L99 (chrome/m156) (`GradientColorFilter::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn RenderNode>>,
        c0: Option<Rc<ColorNode>>,
        c1: Option<Rc<ColorNode>>,
    ) -> Option<Rc<Self>> {
        Self::make_with_colors(child, &[c0?, c1?])
    }

    /// `GradientColorFilter::Make(child, colors)`: `None` unless there are at least two colors.
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L101-L106 (chrome/m156) (`GradientColorFilter::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make_with_colors(
        child: Option<Rc<dyn RenderNode>>,
        colors: &[Rc<ColorNode>],
    ) -> Option<Rc<Self>> {
        let child = child?;
        if colors.len() <= 1 {
            return None;
        }
        let filter = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            state: ColorFilterState {
                child: Rc::clone(&child),
                color_filter: RefCell::new(None),
            },
            colors: colors.to_vec(),
            weight: Cell::new(0.0),
        });
        // The EffectNode base observes the child, then each color.
        filter.observe_inval(filter.state.child.as_ref());
        // Port of: modules/sksg/src/SkSGColorFilter.cpp#L108-L114 (chrome/m156) (`GradientColorFilter::GradientColorFilter`)
        for color in &filter.colors {
            filter.observe_inval(color.as_ref());
        }
        Some(filter)
    }

    /// The weight of the gradient (`getWeight`).
    #[must_use]
    pub fn weight(&self) -> f32 {
        self.weight.get()
    }

    /// Sets the weight, invalidating the node if it changed (`setWeight`).
    pub fn set_weight(&self, weight: f32) {
        if scalar_changed(self.weight.get(), weight) {
            self.weight.set(weight);
            self.invalidate();
        }
    }

    /// `GradientColorFilter::onRevalidateFilter`.
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L224-L238 (chrome/m156) (`GradientColorFilter::onRevalidateFilter`)
    fn on_revalidate_filter(&self) -> Option<ColorFilter> {
        for color in &self.colors {
            color.revalidate(None, &Matrix::new_identity());
        }
        let weight = self.weight.get();
        if weight <= 0.0 {
            return None;
        }
        debug_assert!(self.colors.len() > 1);
        let gradient_cf = if self.colors.len() > 2 {
            make_n_color_gradient(&self.colors)
        } else {
            make_2_color_gradient(&self.colors[0], &self.colors[1])
        };
        color_filters::lerp(weight, None, gradient_cf)
    }
}

impl Drop for GradientColorFilter {
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L116-L120 (chrome/m156) (`GradientColorFilter::~GradientColorFilter`)
    fn drop(&mut self) {
        for color in &self.colors {
            self.unobserve_inval(color.as_ref());
        }
        self.unobserve_inval(self.state.child.as_ref());
    }
}

impl Node for GradientColorFilter {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L43-L49 (chrome/m156) (`ColorFilter::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let filter = self.on_revalidate_filter();
        color_filter_revalidate(&self.state, ic, ctm, filter)
    }
}

impl RenderNode for GradientColorFilter {
    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L32-L36 (chrome/m156) (`ColorFilter::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        color_filter_on_render(&self.state, canvas, ctx);
    }

    // Port of: modules/sksg/src/SkSGColorFilter.cpp#L38-L41 (chrome/m156) (`ColorFilter::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        effect_on_node_at(&self.state.child, p)
    }
}
