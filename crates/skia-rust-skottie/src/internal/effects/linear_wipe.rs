// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/LinearWipeEffect.cpp (chrome/m156)
//
// The linear wipe effect: a mask shader, a linear gradient across the layer at the wipe angle,
// that reveals the layer as the completion advances.

use std::rc::{Rc, Weak};

use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::floating_point::float_degrees_to_radians;
use skia_rust_core::point::Point;
use skia_rust_core::scalar::{scalar_cos, scalar_sin};
use skia_rust_core::size::Size;
use skia_rust_core::t_pin::t_pin;
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

/// The linear wipe adapter: the completion, the wipe angle and the feather of the mask.
// Port of: modules/skottie/src/effects/LinearWipeEffect.cpp#L15-L118 (chrome/m156) (`LinearWipeAdapter`)
struct LinearWipeAdapter {
    base: DiscardableAdapterBase<MaskShaderEffect>,
    layer_size: Size,
    completion: Prop<ScalarValue>,
    angle: Prop<ScalarValue>,
    feather: Prop<ScalarValue>,
}

impl LinearWipeAdapter {
    // Port of: modules/skottie/src/effects/LinearWipeEffect.cpp#L23-L43 (chrome/m156) (`LinearWipeAdapter::LinearWipeAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        layer_size: Size,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), make_mask_shader_node(layer));
            let completion = Prop::new(0.0);
            let angle = Prop::new(0.0);
            let feather = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &completion)
                .bind(1, &angle)
                .bind(2, &feather);
            Self {
                base,
                layer_size,
                completion,
                angle,
                feather,
            }
        })
    }

    // Port of: modules/skottie/src/effects/LinearWipeEffect.cpp#L44-L115 (chrome/m156) (`LinearWipeAdapter::onMakeMask`)
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

        let t = t_pin(completion * 0.01_f32, 0.0_f32, 1.0_f32);
        // std::max(fFeather, 0.0f)
        let feather_value = *self.feather.borrow();
        let feather = if feather_value < 0.0 {
            0.0
        } else {
            feather_value
        };
        let angle = float_degrees_to_radians(90.0_f32 - *self.angle.borrow());
        let cos_ = scalar_cos(angle);
        let sin_ = scalar_sin(angle);

        // Select the correct diagonal vector depending on quadrant.
        let angle_v = Point::new(cos_, sin_);
        let diag_v = Point::new(
            self.layer_size.width.copysign(cos_),
            self.layer_size.height.copysign(sin_),
        );
        // The transition length is the projection of the diagonal onto the angle vector.
        let len = diag_v.x * angle_v.x + diag_v.y * angle_v.y;

        // Pad the gradient segment to accommodate optional feather ramps at both extremities.
        let grad_len = len + feather * 2.0;
        let grad_v = Point::new(angle_v.x * grad_len, angle_v.y * grad_len);
        let adjusted_grad_v = Point::new(grad_v.x, -grad_v.y); // Y flipped for drawing space.
        let center_v = Point::new(
            0.5_f32 * self.layer_size.width,
            0.5_f32 * self.layer_size.height,
        );

        // Gradient start/end points.
        let half = Point::new(adjusted_grad_v.x * 0.5_f32, adjusted_grad_v.y * 0.5_f32);
        let pts = [
            Point::new(center_v.x - half.x, center_v.y - half.y),
            Point::new(center_v.x + half.x, center_v.y + half.y),
        ];

        let colors = [
            Color4f::new(0.0, 0.0, 0.0, 0.0),
            Color4f::new(1.0, 1.0, 1.0, 1.0),
        ];

        // To emulate the feather effect, we distance the color stops to generate a linear
        // transition/ramp. For t == 0 the ramp should be completely outside/before the transition
        // domain, and for t == 1 it should be completely outside/after.
        let adjusted_t = t * (len + feather) / grad_len;
        let pos = [adjusted_t, adjusted_t + feather / grad_len];

        let gradient = Gradient::new(
            Colors::new(
                &colors[..],
                Some(&pos[..]),
                skia_rust_core::tile_mode::TileMode::Clamp,
                None,
            ),
            Interpolation::default(),
        );
        MaskInfo {
            shader: shaders::linear_gradient((pts[0], pts[1]), &gradient, None),
            visible: true,
        }
    }
}

impl AnimatablePropertyContainer for LinearWipeAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/Effects.cpp#L211-L217 (chrome/m156) (`MaskShaderEffectBase::onSync`)
    fn on_sync(&self) {
        sync_mask_shader(self.base.node(), self.on_make_mask());
    }
}

impl_container_animator!(LinearWipeAdapter);

/// The linear wipe effect (`ADBE Linear Wipe`).
// Port of: modules/skottie/src/effects/LinearWipeEffect.cpp#L119-L126 (chrome/m156) (`EffectBuilder::attachLinearWipeEffect`)
pub(super) fn attach_linear_wipe_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = LinearWipeAdapter::make(jprops, layer, eb.layer_size(), eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
