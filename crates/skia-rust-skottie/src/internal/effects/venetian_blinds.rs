// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/VenetianBlindsEffect.cpp (chrome/m156)
//
// The venetian blinds effect: a repeating, feathered gradient mask along the blinds direction.

use std::rc::{Rc, Weak};

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::floating_point::float_degrees_to_radians;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{scalar_cos, scalar_sin};
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_sksg::{MaskShaderEffect, RenderNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::ScalarValue;

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{
    EffectBinder, EffectBuilder, MaskInfo, attach_adapter_node, make_mask_shader_node,
    sync_mask_shader,
};

/// `std::max(a, b)`: `b` only if `a < b`.
// Port of: <algorithm> std::max (chrome/m156)
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `b` only if `b < a`.
// Port of: <algorithm> std::min (chrome/m156)
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// The venetian blinds adapter: the completion, the direction, the width and the feather.
// Port of: modules/skottie/src/effects/VenetianBlindsEffect.cpp#L15-L40 (chrome/m156) (`VenetianBlindsAdapter`)
struct VenetianBlindsAdapter {
    base: DiscardableAdapterBase<MaskShaderEffect>,
    layer_size: Size,
    completion: Prop<ScalarValue>,
    direction: Prop<ScalarValue>,
    width: Prop<ScalarValue>,
    feather: Prop<ScalarValue>,
}

impl VenetianBlindsAdapter {
    // Port of: modules/skottie/src/effects/VenetianBlindsEffect.cpp#L20-L35 (chrome/m156) (`VenetianBlindsAdapter::VenetianBlindsAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        layer_size: Size,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), make_mask_shader_node(layer));
            let completion = Prop::new(0.0);
            let direction = Prop::new(0.0);
            let width = Prop::new(0.0);
            let feather = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &completion)
                .bind(1, &direction)
                .bind(2, &width)
                .bind(3, &feather);
            Self {
                base,
                layer_size,
                completion,
                direction,
                width,
                feather,
            }
        })
    }

    // Port of: modules/skottie/src/effects/VenetianBlindsEffect.cpp#L37-L140 (chrome/m156) (`VenetianBlindsAdapter::onMakeMask`)
    fn on_make_mask(&self) -> MaskInfo {
        let completion = *self.completion.borrow();
        if completion >= 100.0 {
            // The layer is fully disabled.
            // TODO: fix layer controller visibility clash and pass a null shader instead.
            return MaskInfo {
                shader: Some(skia_rust_core::shaders::color(Color::TRANSPARENT)),
                visible: false,
            };
        }
        if completion <= 0.0 {
            // The layer is fully visible (no mask).
            return MaskInfo {
                shader: None,
                visible: true,
            };
        }

        const FEATHER_SIGMA_FACTOR: f32 = 3.0;
        // for soft gradient edges
        const MIN_FEATHER: f32 = 0.5;

        let t = completion * 0.01_f32;
        let size = std_max(1.0_f32, *self.width.borrow());
        let angle = float_degrees_to_radians(-*self.direction.borrow());
        let feather = std_max(*self.feather.borrow() * FEATHER_SIGMA_FACTOR, MIN_FEATHER);
        // feather distance in normalized stop space
        let df = feather / size;
        let df0 = 0.5_f32 * std_min(df, t);
        let df1 = 0.5_f32 * std_min(df, 1.0_f32 - t);

        // In its simplest form, the Venetian Blinds effect is a single-step gradient repeating
        // along the direction vector. To avoid an expensive blur pass, we emulate the feather
        // property by softening the gradient edges (see the Skia source for the diagram).
        //
        // Gradient value at fp0/fp1, fp2/fp3.
        // Note: g01 > 0 iff fp0-fp1 is collapsed and g23 < 1 iff fp2-fp3 is collapsed
        let g01 = std_max(0.0_f32, 0.5_f32 * (1.0_f32 + ieee_float_divide(0.0_f32 - t, df)));
        let g23 = std_min(1.0_f32, 0.5_f32 * (1.0_f32 + ieee_float_divide(1.0_f32 - t, df)));

        let c01 = Color4f::new(1.0, 1.0, 1.0, g01);
        let c23 = Color4f::new(1.0, 1.0, 1.0, g23);
        let colors = [c01, c23, c23, c01];
        let pos = [
            // 0,              // fp0
            t - df0 - df0,     // fp1
            t + df1 - df0,     // fp2
            1.0 - df1 - df0,   // fp3
            1.0,
        ];

        let center = Point::new(
            0.5_f32 * self.layer_size.width,
            0.5_f32 * self.layer_size.height,
        );
        let grad_vec = Point::new(size * scalar_cos(angle), -size * scalar_sin(angle));
        let pts = [
            Point::new(
                center.x + grad_vec.x * (df0 + 0.0),
                center.y + grad_vec.y * (df0 + 0.0),
            ),
            Point::new(
                center.x + grad_vec.x * (df0 + 1.0),
                center.y + grad_vec.y * (df0 + 1.0),
            ),
        ];

        let gradient = Gradient::new(
            Colors::new(&colors[..], Some(&pos[..]), TileMode::Repeat, None),
            Interpolation::default(),
        );
        MaskInfo {
            shader: shaders::linear_gradient((pts[0], pts[1]), &gradient, None),
            visible: true,
        }
    }
}

/// `sk_ieee_float_divide(a, b)`: the IEEE float division, with no fast-math shortcuts.
// Port of: include/private/SkFloatingPoint.h (chrome/m156) (`sk_ieee_float_divide`)
fn ieee_float_divide(a: f32, b: f32) -> f32 {
    a / b
}

impl AnimatablePropertyContainer for VenetianBlindsAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/Effects.cpp#L211-L217 (chrome/m156) (`MaskShaderEffectBase::onSync`)
    fn on_sync(&self) {
        sync_mask_shader(self.base.node(), self.on_make_mask());
    }
}

impl_container_animator!(VenetianBlindsAdapter);

/// The venetian blinds effect (`ADBE Venetian Blinds`).
// Port of: modules/skottie/src/effects/VenetianBlindsEffect.cpp#L142-L150 (chrome/m156) (`EffectBuilder::attachVenetianBlindsEffect`)
pub(super) fn attach_venetian_blinds_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = VenetianBlindsAdapter::make(jprops, layer, eb.layer_size(), eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
