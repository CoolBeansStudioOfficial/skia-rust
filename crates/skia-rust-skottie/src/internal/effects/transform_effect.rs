// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/TransformEffect.cpp (chrome/m156)
//
// The transform effect ("ADBE Geometry2"): a 2D transform of the layer with an opacity and a
// scale, applied as a transform node above the layer.

use std::rc::{Rc, Weak};

use skia_rust_core::point::Point;
use skia_rust_sksg::{OpacityEffect, RenderNode, Transform, TransformEffect};

use crate::impl_container_animator;
use crate::json::{ArrayValue, Value};
use crate::skottie_value::ScalarValue;

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, IntoJsonProp, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::super::transform::TransformAdapter2D;
use super::{EffectBuilder, attach_adapter_node};

/// Applies the opacity and the (uniform or per-axis) scale of the transform effect.
// Port of: modules/skottie/src/effects/TransformEffect.cpp#L14-L51 (chrome/m156) (`TransformEffectAdapter`)
struct TransformEffectAdapter {
    base: DiscardableAdapterBase<OpacityEffect>,
    transform_adapter: Rc<TransformAdapter2D>,
    opacity: Prop<ScalarValue>,
    uniform_scale: Prop<ScalarValue>,
    scale_width: Prop<ScalarValue>,
    scale_height: Prop<ScalarValue>,
}

impl TransformEffectAdapter {
    // Port of: modules/skottie/src/effects/TransformEffect.cpp#L23-L37 (chrome/m156) (`TransformEffectAdapter::TransformEffectAdapter`)
    fn make(
        abuilder: &AnimationBuilder<'_>,
        jopacity: &Value,
        juniform_scale: &Value,
        jscale_width: &Value,
        jscale_height: &Value,
        transform_adapter: Rc<TransformAdapter2D>,
        child: Rc<dyn RenderNode>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let node = OpacityEffect::make(Some(child), 1.0).expect("the child is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let opacity = Prop::new(100.0);
            let uniform_scale = Prop::new(0.0); // bool
            let scale_width = Prop::new(100.0);
            let scale_height = Prop::new(100.0);
            let container = base.container();
            container.bind(abuilder, jopacity.into_prop(), &opacity);
            container.bind(abuilder, juniform_scale.into_prop(), &uniform_scale);
            container.bind(abuilder, jscale_width.into_prop(), &scale_width);
            container.bind(abuilder, jscale_height.into_prop(), &scale_height);
            container.attach_discardable_adapter(Some(
                Rc::clone(&transform_adapter) as Rc<dyn AnimatablePropertyContainer>
            ));
            Self {
                base,
                transform_adapter,
                opacity,
                uniform_scale,
                scale_width,
                scale_height,
            }
        })
    }
}

impl AnimatablePropertyContainer for TransformEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/TransformEffect.cpp#L39-L47 (chrome/m156) (`TransformEffectAdapter::onSync`)
    fn on_sync(&self) {
        self.base.node().set_opacity(*self.opacity.borrow() * 0.01);
        let width =
            if skia_rust_core::scalar::scalar_round_to_int(*self.uniform_scale.borrow()) != 0 {
                *self.scale_height.borrow()
            } else {
                *self.scale_width.borrow()
            };
        self.transform_adapter
            .set_scale(Point::new(width, *self.scale_height.borrow()));
    }
}

impl_container_animator!(TransformEffectAdapter);

/// The transform effect (`ADBE Geometry2`).
// Port of: modules/skottie/src/effects/TransformEffect.cpp#L53-L121 (chrome/m156) (`EffectBuilder::attachTransformEffect`)
pub(super) fn attach_transform_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    const ANCHOR_POINT_INDEX: usize = 0;
    const POSITION_INDEX: usize = 1;
    const UNIFORM_SCALE_INDEX: usize = 2;
    const SCALE_HEIGHT_INDEX: usize = 3;
    const SCALE_WIDTH_INDEX: usize = 4;
    const SKEW_INDEX: usize = 5;
    const SKEW_AXIS_INDEX: usize = 6;
    const ROTATION_INDEX: usize = 7;
    const OPACITY_INDEX: usize = 8;

    let layer = layer?;
    let abuilder = eb.builder();
    let value = |index| EffectBuilder::get_prop_value(jprops, index);

    // The scale is handled by the effect adapter, not by the transform.
    let transform_adapter = TransformAdapter2D::make(
        abuilder,
        value(ANCHOR_POINT_INDEX),
        value(POSITION_INDEX),
        None::<&crate::json::ObjectValue>,
        value(ROTATION_INDEX),
        value(SKEW_INDEX),
        value(SKEW_AXIS_INDEX),
        false,
    );

    let transform_effect_node = TransformEffect::make(
        Some(layer),
        Some(Rc::clone(transform_adapter.node()) as Rc<dyn Transform>),
    )
    .expect("the layer and the transform are not null");

    let adapter = TransformEffectAdapter::make(
        abuilder,
        value(OPACITY_INDEX),
        value(UNIFORM_SCALE_INDEX),
        value(SCALE_WIDTH_INDEX),
        value(SCALE_HEIGHT_INDEX),
        transform_adapter,
        transform_effect_node as Rc<dyn RenderNode>,
    );
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(abuilder, &adapter, node))
}
