// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/CornerPinEffect.cpp (chrome/m156)
//
// The corner pin effect: maps the four corners of the layer to four points, with a perspective-free
// (poly to poly) transform.

use std::rc::{Rc, Weak};

use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::size::Size;
use skia_rust_sksg::transform::Matrix as SgMatrix;
use skia_rust_sksg::{RenderNode, Transform, TransformEffect};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::Vec2Value;

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder};

/// Pins the corners of the layer to the upper-left, upper-right, lower-left and lower-right
/// points of the effect.
// Port of: modules/skottie/src/effects/CornerPinEffect.cpp#L13-L72 (chrome/m156) (`CornerPinAdapter`)
struct CornerPinAdapter {
    base: DiscardableAdapterBase<SgMatrix<Matrix>>,
    layer_size: Size,
    ul: Prop<Vec2Value>,
    ll: Prop<Vec2Value>,
    ur: Prop<Vec2Value>,
    lr: Prop<Vec2Value>,
}

impl CornerPinAdapter {
    // Port of: modules/skottie/src/effects/CornerPinEffect.cpp#L18-L35 (chrome/m156) (`CornerPinAdapter::CornerPinAdapter`)
    fn make(jprops: &ArrayValue, abuilder: &AnimationBuilder<'_>, layer_size: Size) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                SgMatrix::<Matrix>::make(Matrix::new_identity()),
            );
            let ul = Prop::new(Vec2Value::new(0.0, 0.0));
            let ur = Prop::new(Vec2Value::new(0.0, 0.0));
            let ll = Prop::new(Vec2Value::new(0.0, 0.0));
            let lr = Prop::new(Vec2Value::new(0.0, 0.0));
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &ul)
                .bind(1, &ur)
                .bind(2, &ll)
                .bind(3, &lr);
            Self {
                base,
                layer_size,
                ul,
                ll,
                ur,
                lr,
            }
        })
    }
}

impl AnimatablePropertyContainer for CornerPinAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/CornerPinEffect.cpp#L37-L55 (chrome/m156) (`CornerPinAdapter::onSync`)
    fn on_sync(&self) {
        let (w, h) = (self.layer_size.width, self.layer_size.height);
        let src = [
            Point::new(0.0, 0.0),
            Point::new(w, 0.0),
            Point::new(w, h),
            Point::new(0.0, h),
        ];
        let (ul, ur, lr, ll) = (
            *self.ul.borrow(),
            *self.ur.borrow(),
            *self.lr.borrow(),
            *self.ll.borrow(),
        );
        let dst = [
            Point::new(ul.x, ul.y),
            Point::new(ur.x, ur.y),
            Point::new(lr.x, lr.y),
            Point::new(ll.x, ll.y),
        ];
        if let Some(m) = Matrix::poly_to_poly(&src, &dst) {
            self.base.node().set_matrix(m);
        }
    }
}

impl_container_animator!(CornerPinAdapter);

/// The corner pin effect (`ADBE Corner Pin`).
// Port of: modules/skottie/src/effects/CornerPinEffect.cpp#L75-L81 (chrome/m156) (`EffectBuilder::attachCornerPinEffect`)
pub(super) fn attach_corner_pin_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = CornerPinAdapter::make(jprops, eb.builder(), eb.layer_size());
    eb.builder().attach_discardable_adapter(&adapter);
    let matrix_node = Rc::clone(adapter.base.node());
    TransformEffect::make(Some(layer), Some(matrix_node as Rc<dyn Transform>))
        .map(|node| node as Rc<dyn RenderNode>)
}
