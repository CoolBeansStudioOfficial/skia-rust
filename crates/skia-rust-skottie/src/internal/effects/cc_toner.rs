// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/CCTonerEffect.cpp (chrome/m156)
//
// The CC Toner effect: a gradient color filter whose five stops are the tone colors (duotone,
// tritone, pentone or solid), blended with the layer by the blend amount.

use std::rc::{Rc, Weak};

use skia_rust_core::color::Color;
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_sksg::{Color as SgColor, GradientColorFilter, RenderNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ColorValue, ScalarValue};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// Loads the four channels of a packed color as floats, in memory (little-endian) byte order
/// (`Sk4f_fromL32`): each channel is scaled by `1/255`.
// Port of: src/core/SkSwizzlePriv.h#L49-L51 (chrome/m156) (`Sk4f_fromL32`)
fn sk4f_from_l32(c: Color) -> [f32; 4] {
    [c.b(), c.g(), c.r(), c.a()].map(|channel| f32::from(channel) * (1.0_f32 / 255.0_f32))
}

/// Stores the four channels as a packed color, rounding each to the nearest byte and pinning it
/// to `[0, 255]` (`Sk4f_toL32`).
// Port of: src/core/SkSwizzlePriv.h#L53-L60 (chrome/m156) (`Sk4f_toL32`)
fn sk4f_to_l32(px: [f32; 4]) -> Color {
    // For the expected positive color values, the +0.5 before the pin and cast effectively rounds
    // to the nearest int without having to call round() or lrint().
    let [b, g, r, a] = px.map(|channel| {
        let pinned = (channel * 255.0_f32 + 0.5_f32).clamp(0.0_f32, 255.0_f32);
        // `pinned` is within [0, 255], so the truncating cast cannot overflow.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // mirrors the truncating float -> u8 cast of Sk4f_toL32
        let byte = pinned as u8;
        byte
    });
    Color::from_argb(a, r, g, b)
}

/// The color that is `t` of the way from `c0` to `c1` (`lerpColor`).
// Port of: modules/skottie/src/effects/CCTonerEffect.cpp#L39-L46 (chrome/m156) (`CCTonerAdapter::lerpColor`)
fn lerp_color(c0: Color, c1: Color, t: f32) -> Color {
    let c0_4f = sk4f_from_l32(c0);
    let c1_4f = sk4f_from_l32(c1);
    let c_4f: [f32; 4] = std::array::from_fn(|i| c0_4f[i] + (c1_4f[i] - c0_4f[i]) * t);
    sk4f_to_l32(c_4f)
}

/// The CC Toner adapter: the gradient stops are the tone colors, the weight is the blend amount.
// Port of: modules/skottie/src/effects/CCTonerEffect.cpp#L20-L40 (chrome/m156) (`CCTonerAdapter`)
struct CcTonerAdapter {
    base: DiscardableAdapterBase<GradientColorFilter>,
    color_nodes: Vec<Rc<SgColor>>,
    tone: Prop<ScalarValue>,
    highlights: Prop<ColorValue>,
    brights: Prop<ColorValue>,
    midtones: Prop<ColorValue>,
    darktones: Prop<ColorValue>,
    shadows: Prop<ColorValue>,
    blend: Prop<ScalarValue>,
}

impl CcTonerAdapter {
    // Port of: modules/skottie/src/effects/CCTonerEffect.cpp#L20-L40 (chrome/m156) (`CCTonerAdapter::CCTonerAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
        color_nodes: Vec<Rc<SgColor>>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let filter = GradientColorFilter::make_with_colors(Some(layer), &color_nodes)
                .expect("the layer is not null and there are five colors");
            let base = DiscardableAdapterBase::new(weak.clone(), filter);
            let tone = Prop::new(0.0);
            let highlights = Prop::new(ColorValue::new());
            let brights = Prop::new(ColorValue::new());
            let midtones = Prop::new(ColorValue::new());
            let darktones = Prop::new(ColorValue::new());
            let shadows = Prop::new(ColorValue::new());
            let blend = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &tone)
                .bind(1, &highlights)
                .bind(2, &brights)
                .bind(3, &midtones)
                .bind(4, &darktones)
                .bind(5, &shadows)
                .bind(6, &blend);
            Self {
                base,
                color_nodes,
                tone,
                highlights,
                brights,
                midtones,
                darktones,
                shadows,
                blend,
            }
        })
    }
}

impl AnimatablePropertyContainer for CcTonerAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/CCTonerEffect.cpp#L58-L100 (chrome/m156) (`CCTonerAdapter::onSync`)
    fn on_sync(&self) {
        let highlights = self.highlights.borrow().to_color();
        let brights = self.brights.borrow().to_color();
        let midtones = self.midtones.borrow().to_color();
        let darktones = self.darktones.borrow().to_color();
        let shadows = self.shadows.borrow().to_color();
        let colors = &self.color_nodes;

        match scalar_round_to_int(*self.tone.borrow()) {
            // duotone
            1 => {
                colors[0].set_color(shadows);
                colors[1].set_color(lerp_color(shadows, highlights, 0.25));
                colors[2].set_color(lerp_color(shadows, highlights, 0.5));
                colors[3].set_color(lerp_color(shadows, highlights, 0.75));
                colors[4].set_color(highlights);
            }
            // tritone
            2 => {
                colors[0].set_color(shadows);
                colors[1].set_color(lerp_color(shadows, midtones, 0.5));
                colors[2].set_color(midtones);
                colors[3].set_color(lerp_color(midtones, highlights, 0.5));
                colors[4].set_color(highlights);
            }
            // pentone
            3 => {
                colors[0].set_color(shadows);
                colors[1].set_color(darktones);
                colors[2].set_color(midtones);
                colors[3].set_color(brights);
                colors[4].set_color(highlights);
            }
            // solid
            _ => {
                for color in colors {
                    color.set_color(midtones);
                }
            }
        }

        self.base
            .node()
            .set_weight((100.0 - *self.blend.borrow()) / 100.0);
    }
}

impl_container_animator!(CcTonerAdapter);

/// The CC Toner effect (`CC Toner`).
// Port of: modules/skottie/src/effects/CCTonerEffect.cpp#L102-L118 (chrome/m156) (`EffectBuilder::attachCCTonerEffect`)
pub(super) fn attach_cc_toner_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let color_nodes: Vec<Rc<SgColor>> = (0..5).map(|_| SgColor::make(Color::RED)).collect();
    let adapter = CcTonerAdapter::make(jprops, layer, eb.builder(), color_nodes);
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
