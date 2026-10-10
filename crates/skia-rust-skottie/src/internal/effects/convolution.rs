// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/SharpenEffect.cpp, DirectionalBlur.cpp (chrome/m156)
//
// The sharpen effect (a 3x3 matrix convolution) and the directional motion blur (a blur in a
// rotated frame).

use std::rc::{Rc, Weak};

use skia_rust_core::matrix::Matrix;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, matrix_convolution, matrix_transform};
use skia_rust_sksg::RenderNode;
use skia_rust_sksg::render_effect::{ExternalImageFilter, ImageFilterEffect, ImageFilterNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::ScalarValue;

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::{AnimationBuilder, BLUR_SIZE_TO_SIGMA};
use super::{EffectBinder, EffectBuilder};

/// Sharpens the layer with a 3x3 convolution of the given amount.
// Port of: modules/skottie/src/effects/SharpenEffect.cpp#L17-L50 (chrome/m156) (`SharpenAdapter`)
struct SharpenAdapter {
    base: DiscardableAdapterBase<ExternalImageFilter>,
    amount: Prop<ScalarValue>,
}

impl SharpenAdapter {
    // Port of: modules/skottie/src/effects/SharpenEffect.cpp#L22-L31 (chrome/m156) (`SharpenAdapter::SharpenAdapter`)
    fn make(jprops: &ArrayValue, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), ExternalImageFilter::make());
            let amount = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container()).bind(0, &amount);
            Self { base, amount }
        })
    }
}

impl AnimatablePropertyContainer for SharpenAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/SharpenEffect.cpp#L33-L45 (chrome/m156) (`SharpenAdapter::onSync`)
    fn on_sync(&self) {
        let intensity = 1.0 + (*self.amount.borrow() * 0.01);
        let discount = (1.0 - intensity) / 8.0;
        let kernel = [
            discount, discount, discount, //
            discount, intensity, discount, //
            discount, discount, discount,
        ];
        let filter = matrix_convolution(
            (3, 3),
            &kernel,
            1.0,
            0.0,
            (1, 1),
            TileMode::Repeat,
            true,
            None,
            None,
        );
        self.base.node().set_image_filter(filter);
    }
}

impl_container_animator!(SharpenAdapter);

/// The sharpen effect (`ADBE Sharpen`).
// Port of: modules/skottie/src/effects/SharpenEffect.cpp#L65-L75 (chrome/m156) (`EffectBuilder::attachSharpenEffect`)
pub(super) fn attach_sharpen_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = SharpenAdapter::make(jprops, eb.builder());
    eb.builder().attach_discardable_adapter(&adapter);
    let filter_node = Rc::clone(adapter.base.node());
    Some(ImageFilterEffect::make(
        layer,
        Some(filter_node as Rc<dyn ImageFilterNode>),
    ))
}

/// The linear sampling of the directional blur transforms (`SkSamplingOptions(kLinear)`).
// Port of: modules/skottie/src/effects/DirectionalBlur.cpp#L43 (chrome/m156)
fn linear_sampling() -> SamplingOptions {
    SamplingOptions::new(FilterMode::Linear, MipmapMode::None)
}

/// Blurs the layer along a direction, by a length.
// Port of: modules/skottie/src/effects/DirectionalBlur.cpp#L17-L58 (chrome/m156) (`DirectionalBlurAdapter`)
struct DirectionalBlurAdapter {
    base: DiscardableAdapterBase<ExternalImageFilter>,
    direction: Prop<ScalarValue>,
    blur_length: Prop<ScalarValue>,
}

impl DirectionalBlurAdapter {
    // Port of: modules/skottie/src/effects/DirectionalBlur.cpp#L22-L35 (chrome/m156) (`DirectionalBlurAdapter::DirectionalBlurAdapter`)
    fn make(jprops: &ArrayValue, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), ExternalImageFilter::make());
            let direction = Prop::new(0.0);
            let blur_length = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &direction)
                .bind(1, &blur_length);
            Self {
                base,
                direction,
                blur_length,
            }
        })
    }
}

impl AnimatablePropertyContainer for DirectionalBlurAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/DirectionalBlur.cpp#L37-L50 (chrome/m156) (`DirectionalBlurAdapter::onSync`)
    fn on_sync(&self) {
        let rot = *self.direction.borrow() - 90.0;
        let unrotated = matrix_transform(&Matrix::rotate_deg(-rot), linear_sampling(), None);
        let blurred = blur(
            *self.blur_length.borrow() * BLUR_SIZE_TO_SIGMA,
            0.0,
            TileMode::Decal,
            unrotated,
            None,
        );
        let filter = matrix_transform(&Matrix::rotate_deg(rot), linear_sampling(), blurred);
        self.base.node().set_image_filter(filter);
    }
}

impl_container_animator!(DirectionalBlurAdapter);

/// The directional blur effect (`ADBE Motion Blur`, or the legacy path of the motion blur).
// Port of: modules/skottie/src/effects/DirectionalBlur.cpp#L60-L67 (chrome/m156) (`EffectBuilder::attachDirectionalBlurEffect`)
pub(super) fn attach_directional_blur_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = DirectionalBlurAdapter::make(jprops, eb.builder());
    eb.builder().attach_discardable_adapter(&adapter);
    let filter_node = Rc::clone(adapter.base.node());
    Some(ImageFilterEffect::make(
        layer,
        Some(filter_node as Rc<dyn ImageFilterNode>),
    ))
}
