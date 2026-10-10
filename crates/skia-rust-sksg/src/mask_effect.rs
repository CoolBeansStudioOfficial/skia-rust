// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGMaskEffect.h, modules/sksg/src/SkSGMaskEffect.cpp
// (chrome/m156)

use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::data::Data;
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

use crate::effect_node::{effect_on_node_at, effect_on_render, effect_on_revalidate};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore};
use crate::render_node::{Hit, RenderContext, RenderNode, node_at};

/// How a mask is generated and applied (`MaskEffect::Mode`). The low bit inverts the mask, and
/// the second bit selects luma instead of alpha as the mask source.
// Port of: modules/sksg/include/SkSGMaskEffect.h#L17-L22 (chrome/m156) (`MaskEffect::Mode`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MaskMode {
    /// Coverage is the mask alpha (`kAlphaNormal`).
    #[default]
    AlphaNormal = 0,
    /// Coverage is one minus the mask alpha (`kAlphaInvert`).
    AlphaInvert = 1,
    /// Coverage is the mask luma (`kLumaNormal`).
    LumaNormal = 2,
    /// Coverage is one minus the mask luma (`kLumaInvert`).
    LumaInvert = 3,
}

// Port of: modules/sksg/src/SkSGMaskEffect.cpp#L24-L26 (chrome/m156) (`is_inverted`)
fn is_inverted(mode: MaskMode) -> bool {
    (mode as u32) & 1 != 0
}

// Port of: modules/sksg/src/SkSGMaskEffect.cpp#L28-L30 (chrome/m156) (`is_luma`)
fn is_luma(mode: MaskMode) -> bool {
    (mode as u32) & 2 != 0
}

/// `SkLumaColorFilter::Make()`: a known runtime effect, built with empty uniforms.
// Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L162-L168 (chrome/m156) (`SkLumaColorFilter::Make`)
pub(crate) fn make_luma_color_filter() -> Option<ColorFilter> {
    get_known_runtime_effect(StableKey::Luma)?.make_color_filter(Data::new_empty(), &[])
}

/// Masks its child with the alpha or luma of another node (`MaskEffect`).
// Port of: modules/sksg/include/SkSGMaskEffect.h#L24-L51 (chrome/m156) (`class MaskEffect`)
#[doc(alias = "sksg::MaskEffect")]
#[derive(Debug)]
pub struct MaskEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    mask_node: Rc<dyn RenderNode>,
    mask_mode: MaskMode,
}

impl MaskEffect {
    /// `MaskEffect::Make(child, mask, mode)`: `None` if either is missing.
    // Port of: modules/sksg/include/SkSGMaskEffect.h#L33-L37 (chrome/m156) (`MaskEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn RenderNode>>,
        mask: Option<Rc<dyn RenderNode>>,
        mode: MaskMode,
    ) -> Option<Rc<Self>> {
        let (child, mask) = (child?, mask?);
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            mask_node: Rc::clone(&mask),
            mask_mode: mode,
        });
        // The EffectNode base observes the child, then MaskEffect observes the mask.
        effect.observe_inval(effect.child.as_ref());
        // Port of: modules/sksg/src/SkSGMaskEffect.cpp#L32-L37 (chrome/m156) (`MaskEffect::MaskEffect`)
        effect.observe_inval(effect.mask_node.as_ref());
        Some(effect)
    }

    /// The mask mode.
    #[must_use]
    pub fn mask_mode(&self) -> MaskMode {
        self.mask_mode
    }
}

impl Drop for MaskEffect {
    // Port of: modules/sksg/src/SkSGMaskEffect.cpp#L39-L41 (chrome/m156) (`MaskEffect::~MaskEffect`)
    fn drop(&mut self) {
        self.unobserve_inval(self.mask_node.as_ref());
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for MaskEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGMaskEffect.cpp#L95-L104 (chrome/m156) (`MaskEffect::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        let mask_bounds = self.mask_node.revalidate(ic.as_deref_mut(), ctm);
        let mut child_bounds = effect_on_revalidate(&self.child, ic, ctm);
        if is_inverted(self.mask_mode) || child_bounds.intersect(mask_bounds) {
            child_bounds
        } else {
            Rect::new_empty()
        }
    }
}

impl RenderNode for MaskEffect {
    // Port of: modules/sksg/src/SkSGMaskEffect.cpp#L43-L84 (chrome/m156) (`MaskEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        // SkAutoCanvasRestore(canvas, false): the save count is restored on every exit path.
        let save_count = canvas.save_count();
        let bounds = self.core().bounds();

        // The mask layer: the mask coverage is stored in the alpha channel.
        let mut mask_layer_paint = Paint::default();
        if let Some(ctx) = ctx {
            // Apply all optional context overrides upfront.
            ctx.modulate_paint(&canvas.total_matrix(), &mut mask_layer_paint, false);
        }
        let mut mask_render_context = RenderContext::default();
        if is_luma(self.mask_mode) {
            mask_render_context.color_filter = make_luma_color_filter();
        }
        canvas.save_layer(
            &SaveLayerRec::default()
                .bounds(&bounds)
                .paint(&mask_layer_paint),
        );
        self.mask_node.render(canvas, Some(&mask_render_context));

        // The inner layer: the masked content.
        let mut content_layer_paint = Paint::default();
        content_layer_paint.set_blend_mode(if is_inverted(self.mask_mode) {
            BlendMode::SrcOut
        } else {
            BlendMode::SrcIn
        });
        canvas.save_layer(
            &SaveLayerRec::default()
                .bounds(&bounds)
                .paint(&content_layer_paint),
        );
        effect_on_render(&self.child, canvas, None);

        canvas.restore_to_count(save_count);
    }

    // Port of: modules/sksg/src/SkSGMaskEffect.cpp#L85-L93 (chrome/m156) (`MaskEffect::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        let mask_hit = node_at(&self.mask_node, p).is_some() != is_inverted(self.mask_mode);
        if !mask_hit {
            return None;
        }
        effect_on_node_at(&self.child, p)
    }
}
