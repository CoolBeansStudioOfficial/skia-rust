// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/GradientEffect.cpp (chrome/m156)
//
// The gradient ramp effect: a linear or radial gradient, chosen by the "shape" property, from the
// start color to the end color, shading the layer through a shader effect.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::point::Point;
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_sksg::{ColorStop, LinearGradient, RadialGradient, RenderNode, ShaderEffect};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ColorValue, ScalarValue, Vec2Value};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The gradient instance of the ramp: its type follows the "shape" property.
// Port of: modules/skottie/src/effects/GradientEffect.cpp#L49-L53 (chrome/m156) (`InstanceType`)
#[derive(Clone)]
enum GradientInstance {
    Linear(Rc<LinearGradient>),
    Radial(Rc<RadialGradient>),
}

/// The "shape" value of a linear gradient (`kLinearShapeValue`); any other value is radial.
// Port of: modules/skottie/src/effects/GradientEffect.cpp#L80 (chrome/m156) (`kLinearShapeValue`)
const LINEAR_SHAPE_VALUE: i32 = 1;

/// The shader effect of the ramp, with a linear or radial gradient shader.
// Port of: modules/skottie/src/effects/GradientEffect.cpp#L16-L108 (chrome/m156) (`GradientRampEffectAdapter`)
struct GradientRampEffectAdapter {
    base: DiscardableAdapterBase<ShaderEffect>,
    gradient: RefCell<Option<GradientInstance>>,
    start_color: Prop<ColorValue>,
    end_color: Prop<ColorValue>,
    start_point: Prop<Vec2Value>,
    end_point: Prop<Vec2Value>,
    // TODO in Skia: blend and scatter are bound but not applied.
    #[allow(dead_code)]
    blend: Prop<ScalarValue>,
    #[allow(dead_code)]
    scatter: Prop<ScalarValue>,
    shape: Prop<ScalarValue>,
}

impl GradientRampEffectAdapter {
    // Port of: modules/skottie/src/effects/GradientEffect.cpp#L23-L46 (chrome/m156) (`GradientRampEffectAdapter::GradientRampEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let shader_effect =
                ShaderEffect::make(Some(layer), None).expect("the layer is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), shader_effect);
            let start_color = Prop::new(ColorValue::new());
            let end_color = Prop::new(ColorValue::new());
            let start_point = Prop::new(Vec2Value::new(0.0, 0.0));
            let end_point = Prop::new(Vec2Value::new(0.0, 0.0));
            let shape = Prop::new(0.0);
            let scatter = Prop::new(0.0);
            let blend = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &start_point)
                .bind(1, &start_color)
                .bind(2, &end_point)
                .bind(3, &end_color)
                .bind(4, &shape)
                .bind(5, &scatter)
                .bind(6, &blend);
            Self {
                base,
                gradient: RefCell::new(None),
                start_color,
                end_color,
                start_point,
                end_point,
                blend,
                scatter,
                shape,
            }
        })
    }
}

impl AnimatablePropertyContainer for GradientRampEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/GradientEffect.cpp#L81-L121 (chrome/m156) (`GradientRampEffectAdapter::onSync`)
    fn on_sync(&self) {
        let stops = [
            ColorStop {
                position: 0.0,
                color: self.start_color.borrow().to_color4f(),
            },
            ColorStop {
                position: 1.0,
                color: self.end_color.borrow().to_color4f(),
            },
        ];
        let instance_is_linear = scalar_round_to_int(*self.shape.borrow()) == LINEAR_SHAPE_VALUE;

        // Sync the gradient shader instance if the type changed.
        let current = self.gradient.borrow().clone();
        let instance = match current {
            Some(GradientInstance::Linear(lg)) if instance_is_linear => {
                GradientInstance::Linear(lg)
            }
            Some(GradientInstance::Radial(rg)) if !instance_is_linear => {
                GradientInstance::Radial(rg)
            }
            _ if instance_is_linear => {
                let lg = LinearGradient::make();
                self.base.node().set_shader(Some(lg.clone()));
                GradientInstance::Linear(lg)
            }
            _ => {
                let rg = RadialGradient::make();
                self.base.node().set_shader(Some(rg.clone()));
                GradientInstance::Radial(rg)
            }
        };
        *self.gradient.borrow_mut() = Some(instance.clone());

        // Sync the instance-dependent gradient params.
        let start = *self.start_point.borrow();
        let end = *self.end_point.borrow();
        let start_point = Point::new(start.x, start.y);
        let end_point = Point::new(end.x, end.y);
        match instance {
            GradientInstance::Linear(lg) => {
                lg.set_color_stops(stops.to_vec());
                lg.set_start_point(start_point);
                lg.set_end_point(end_point);
            }
            GradientInstance::Radial(rg) => {
                rg.set_color_stops(stops.to_vec());
                rg.set_start_center(start_point);
                rg.set_end_center(start_point);
                rg.set_end_radius(Point::distance(start_point, end_point));
            }
        }
    }
}

impl_container_animator!(GradientRampEffectAdapter);

/// The gradient ramp effect (`ADBE Ramp`).
// Port of: modules/skottie/src/effects/GradientEffect.cpp#L124-L129 (chrome/m156) (`EffectBuilder::attachGradientEffect`)
pub(super) fn attach_gradient_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = GradientRampEffectAdapter::make(jprops, layer, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
