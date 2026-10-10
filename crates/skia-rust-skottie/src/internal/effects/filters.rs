// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/GaussianBlurEffect.cpp,
// modules/skottie/src/effects/DropShadowEffect.cpp (chrome/m156)
//
// The image filter effects: the gaussian blur and the drop shadow.

use std::rc::{Rc, Weak};

use skia_rust_core::floating_point::float_degrees_to_radians;
use skia_rust_core::scalar::{scalar_cos, scalar_round_to_int, scalar_sin};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_sksg::RenderNode;
use skia_rust_sksg::render_effect::{
    BlurImageFilter, Cropping, DropShadowImageFilter, DropShadowMode, ImageFilterEffect,
    ImageFilterNode,
};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ColorValue, ScalarValue};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::{AnimationBuilder, BLUR_SIZE_TO_SIGMA};
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The blur sizes of the dimension choices: 1 is horizontal and vertical, 2 horizontal, 3
/// vertical.
// Port of: modules/skottie/src/effects/GaussianBlurEffect.cpp#L63-L67 (chrome/m156) (`kDimensionsMap`)
const DIMENSIONS_MAP: [(f32, f32); 3] = [(1.0, 1.0), (1.0, 0.0), (0.0, 1.0)];

/// Blurs the layer with a gaussian of the given blurriness.
// Port of: modules/skottie/src/effects/GaussianBlurEffect.cpp#L14-L75 (chrome/m156) (`GaussianBlurEffectAdapter`)
struct GaussianBlurEffectAdapter {
    base: DiscardableAdapterBase<ImageFilterEffect>,
    blur: Rc<BlurImageFilter>,
    blurriness: Prop<ScalarValue>,
    dimensions: Prop<ScalarValue>,
    repeat_edge: Prop<ScalarValue>,
}

impl GaussianBlurEffectAdapter {
    // Port of: modules/skottie/src/effects/GaussianBlurEffect.cpp#L26-L40 (chrome/m156) (`GaussianBlurEffectAdapter::GaussianBlurEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: &Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let blur = BlurImageFilter::make();
            let filter = Rc::clone(&blur) as Rc<dyn ImageFilterNode>;
            let node = ImageFilterEffect::make_with_filter(layer, &filter);
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let blurriness = Prop::new(0.0); // Controls the blur sigma.
            // 1 -> horizontal & vertical, 2 -> horizontal, 3 -> vertical
            let dimensions = Prop::new(1.0);
            let repeat_edge = Prop::new(0.0); // 0 -> clamp, 1 -> repeat
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &blurriness)
                .bind(1, &dimensions)
                .bind(2, &repeat_edge);
            Self {
                base,
                blur,
                blurriness,
                dimensions,
                repeat_edge,
            }
        })
    }
}

impl AnimatablePropertyContainer for GaussianBlurEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/GaussianBlurEffect.cpp#L42-L61 (chrome/m156) (`GaussianBlurEffectAdapter::onSync`)
    fn on_sync(&self) {
        // Port of: static_cast<size_t>(fDimensions), clamped to [1, 3] and made zero-based.
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // the value is clamped
        let dim_index = (*self.dimensions.borrow() as usize).clamp(1, DIMENSIONS_MAP.len()) - 1;
        let sigma = *self.blurriness.borrow() * BLUR_SIZE_TO_SIGMA;
        let (dx, dy) = DIMENSIONS_MAP[dim_index];
        self.blur.set_sigma((sigma * dx, sigma * dy));

        let repeat_edge = *self.repeat_edge.borrow() != 0.0;
        self.blur.set_tile_mode(if repeat_edge {
            TileMode::Clamp
        } else {
            TileMode::Decal
        });
        self.base.node().set_cropping(if repeat_edge {
            Cropping::Content
        } else {
            Cropping::None
        });
    }
}

impl_container_animator!(GaussianBlurEffectAdapter);

/// The gaussian blur effect (`ADBE Gaussian Blur 2`, or the legacy effect type 29).
// Port of: modules/skottie/src/effects/GaussianBlurEffect.cpp#L77-L83 (chrome/m156) (`EffectBuilder::attachGaussianBlurEffect`)
pub(super) fn attach_gaussian_blur_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = GaussianBlurEffectAdapter::make(jprops, &layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

/// Casts a drop shadow of the layer, with a color, an offset and a blur.
// Port of: modules/skottie/src/effects/DropShadowEffect.cpp#L14-L82 (chrome/m156) (`DropShadowAdapter`)
struct DropShadowAdapter {
    base: DiscardableAdapterBase<ImageFilterEffect>,
    drop_shadow: Rc<DropShadowImageFilter>,
    color: Prop<ColorValue>,
    opacity: Prop<ScalarValue>,
    direction: Prop<ScalarValue>,
    distance: Prop<ScalarValue>,
    softness: Prop<ScalarValue>,
    shdw_only: Prop<ScalarValue>,
}

impl DropShadowAdapter {
    // Port of: modules/skottie/src/effects/DropShadowEffect.cpp#L24-L41 (chrome/m156) (`DropShadowAdapter::Make`)
    fn make(
        jprops: &ArrayValue,
        layer: &Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let drop_shadow = DropShadowImageFilter::make();
            let filter = Rc::clone(&drop_shadow) as Rc<dyn ImageFilterNode>;
            let node = ImageFilterEffect::make_with_filter(layer, &filter);
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let color = Prop::new(ColorValue::from_slice(&[0.0, 0.0, 0.0, 1.0]));
            let opacity = Prop::new(255.0);
            let direction = Prop::new(0.0);
            let distance = Prop::new(0.0);
            let softness = Prop::new(0.0);
            let shdw_only = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &color)
                .bind(1, &opacity)
                .bind(2, &direction)
                .bind(3, &distance)
                .bind(4, &softness)
                .bind(5, &shdw_only);
            Self {
                base,
                drop_shadow,
                color,
                opacity,
                direction,
                distance,
                softness,
                shdw_only,
            }
        })
    }
}

impl AnimatablePropertyContainer for DropShadowAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/DropShadowEffect.cpp#L43-L63 (chrome/m156) (`DropShadowAdapter::onSync`)
    fn on_sync(&self) {
        let color = self.color.borrow().to_color();
        // Port of: SkColorSetA(color, SkTPin(SkScalarRoundToInt(fOpacity), 0, 255)).
        #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped to [0, 255]
        let alpha = scalar_round_to_int(*self.opacity.borrow()).clamp(0, 255) as u8;
        self.drop_shadow.set_color(color.with_a(alpha));

        let direction = *self.direction.borrow();
        let distance = *self.distance.borrow();
        let rad = float_degrees_to_radians(90.0 - direction);
        self.drop_shadow
            .set_offset((distance * scalar_cos(rad), -distance * scalar_sin(rad)));

        let sigma = *self.softness.borrow() * BLUR_SIZE_TO_SIGMA;
        self.drop_shadow.set_sigma((sigma, sigma));
        self.drop_shadow
            .set_mode(if *self.shdw_only.borrow() == 0.0 {
                DropShadowMode::ShadowAndForeground
            } else {
                DropShadowMode::ShadowOnly
            });
    }
}

impl_container_animator!(DropShadowAdapter);

/// The drop shadow effect (`ADBE Drop Shadow`, or the legacy effect type 25).
// Port of: modules/skottie/src/effects/DropShadowEffect.cpp#L95-L101 (chrome/m156) (`EffectBuilder::attachDropShadowEffect`)
pub(super) fn attach_drop_shadow_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = DropShadowAdapter::make(jprops, &layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
