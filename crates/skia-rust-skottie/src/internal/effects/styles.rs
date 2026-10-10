// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/ShadowStyles.cpp, GlowStyles.cpp,
// ColorOverlayStyles.cpp (chrome/m156)
//
// The layer styles ("sy"): drop and inner shadow, inner and outer glow, and color overlay. Each
// one is an image filter of the layer, built from the style's properties.

use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::floating_point::float_degrees_to_radians;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::scalar::{Scalar, scalar_cos, scalar_pow, scalar_round_to_int, scalar_sin};
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blend, blur, color_filter, merge, offset};
use skia_rust_sksg::RenderNode;
use skia_rust_sksg::render_effect::{ExternalImageFilter, ImageFilterEffect, ImageFilterNode};

use crate::impl_container_animator;
use crate::json::ObjectValue;
use crate::skottie_value::{ColorValue, ScalarValue};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::{AnimationBuilder, BLUR_SIZE_TO_SIGMA};
use super::EffectBuilder;

/// The image filter of a style: the matrix color filter, followed by the blur and the offset.
// Port of: modules/skottie/src/effects/ShadowStyles.cpp (`SkImageFilters::ColorFilter`/`Blur`/`Offset`)
fn matrix_filter(cm: &ColorMatrix, input: Option<ImageFilter>) -> Option<ImageFilter> {
    color_filter(color_filters::matrix(cm, Clamp::Yes), input, None)
}

/// The image filter of a style, from the (row-major) matrix of its color filter.
// Port of: modules/skottie/src/effects/GlowStyles.cpp (`SkColorMatrix` from 20 row-major values)
fn matrix_from_row_major(values: &[f32; 20]) -> ColorMatrix {
    let mut cm = ColorMatrix::default();
    cm.set_row_major(values);
    cm
}

/// The `SkColorMatrix` of the edge of an inner style: `alpha' = 1 - alpha`.
// Port of: modules/skottie/src/effects/ShadowStyles.cpp (`cm.preConcat` for the inner shadow)
const INVERT_ALPHA: [f32; 20] = [
    1.0, 0.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 0.0, -1.0, 1.0,
];

// ---- Shadows ----------------------------------------------------------------------------------

/// The shadow styles.
// Port of: modules/skottie/src/effects/ShadowStyles.cpp#L17-L28 (chrome/m156) (`ShadowAdapter::Type`)
#[derive(Clone, Copy, PartialEq, Eq)]
enum ShadowType {
    DropShadow,
    InnerShadow,
}

/// Casts a shadow of the layer, offset by an angle and a distance.
// Port of: modules/skottie/src/effects/ShadowStyles.cpp#L17-L84 (chrome/m156) (`ShadowAdapter`)
struct ShadowAdapter {
    base: DiscardableAdapterBase<ExternalImageFilter>,
    kind: ShadowType,
    color: Prop<ColorValue>,
    opacity: Prop<ScalarValue>,
    angle: Prop<ScalarValue>,
    size: Prop<ScalarValue>,
    distance: Prop<ScalarValue>,
}

impl ShadowAdapter {
    // Port of: modules/skottie/src/effects/ShadowStyles.cpp#L27-L35 (chrome/m156) (`ShadowAdapter::ShadowAdapter`)
    fn make(jstyle: &ObjectValue, abuilder: &AnimationBuilder<'_>, kind: ShadowType) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), ExternalImageFilter::make());
            let color = Prop::new(ColorValue::new());
            let opacity = Prop::new(100.0); // percentage
            let angle = Prop::new(0.0); // degrees
            let size = Prop::new(0.0);
            let distance = Prop::new(0.0);
            let container = base.container();
            container.bind(abuilder, jstyle.get("c"), &color);
            container.bind(abuilder, jstyle.get("o"), &opacity);
            container.bind(abuilder, jstyle.get("a"), &angle);
            container.bind(abuilder, jstyle.get("s"), &size);
            container.bind(abuilder, jstyle.get("d"), &distance);
            Self {
                base,
                kind,
                color,
                opacity,
                angle,
                size,
                distance,
            }
        })
    }
}

impl AnimatablePropertyContainer for ShadowAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/ShadowStyles.cpp#L37-L77 (chrome/m156) (`ShadowAdapter::onSync`)
    fn on_sync(&self) {
        // 0deg -> left (style)
        let rad = float_degrees_to_radians(180.0 + *self.angle.borrow());
        let sigma = *self.size.borrow() * BLUR_SIZE_TO_SIGMA;
        let opacity = t_pin(*self.opacity.borrow() / 100.0, 0.0, 1.0);
        let distance = *self.distance.borrow();

        let color = self.color.borrow().to_color4f();
        let offset_x = distance * scalar_cos(rad);
        let offset_y = -distance * scalar_sin(rad);

        let mut cm = matrix_from_row_major(&[
            0.0,
            0.0,
            0.0,
            0.0,
            color.r, //
            0.0,
            0.0,
            0.0,
            0.0,
            color.g, //
            0.0,
            0.0,
            0.0,
            0.0,
            color.b, //
            0.0,
            0.0,
            0.0,
            opacity * color.a,
            0.0,
        ]);
        if self.kind == ShadowType::InnerShadow {
            cm.pre_concat(&matrix_from_row_major(&INVERT_ALPHA));
        }

        let mut f = matrix_filter(&cm, None);
        if sigma > 0.0 {
            f = blur(sigma, sigma, TileMode::Decal, f, None);
        }
        if !offset_x.nearly_zero(None) || !offset_y.nearly_zero(None) {
            f = offset((offset_x, offset_y), f, None);
        }

        let mut source: Option<ImageFilter> = None;
        if self.kind == ShadowType::InnerShadow {
            f = blend(BlendMode::DstIn, f, None, None);
            // std::swap(source, f)
            std::mem::swap(&mut source, &mut f);
        }
        self.base.node().set_image_filter(merge(&[f, source], None));
    }
}

impl_container_animator!(ShadowAdapter);

/// Applies a shadow style of the given kind to the layer.
// Port of: modules/skottie/src/effects/ShadowStyles.cpp#L86-L110 (chrome/m156) (`make_shadow_effect`)
fn make_shadow_effect(
    jstyle: &ObjectValue,
    abuilder: &AnimationBuilder<'_>,
    layer: Rc<dyn RenderNode>,
    kind: ShadowType,
) -> Rc<dyn RenderNode> {
    let adapter = ShadowAdapter::make(jstyle, abuilder, kind);
    abuilder.attach_discardable_adapter(&adapter);
    let filter_node = Rc::clone(adapter.base.node());
    ImageFilterEffect::make(layer, Some(filter_node as Rc<dyn ImageFilterNode>))
}

/// The drop shadow style (style type 1).
// Port of: modules/skottie/src/effects/ShadowStyles.cpp (`EffectBuilder::attachDropShadowStyle`)
pub(super) fn attach_drop_shadow_style(
    eb: &EffectBuilder<'_, '_>,
    jstyle: &ObjectValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    Some(make_shadow_effect(
        jstyle,
        eb.builder(),
        layer?,
        ShadowType::DropShadow,
    ))
}

/// The inner shadow style (style type 2).
// Port of: modules/skottie/src/effects/ShadowStyles.cpp (`EffectBuilder::attachInnerShadowStyle`)
pub(super) fn attach_inner_shadow_style(
    eb: &EffectBuilder<'_, '_>,
    jstyle: &ObjectValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    Some(make_shadow_effect(
        jstyle,
        eb.builder(),
        layer?,
        ShadowType::InnerShadow,
    ))
}

// ---- Glows ------------------------------------------------------------------------------------

/// The inner glow source: from the edges (1) or from the center (2).
// Port of: modules/skottie/src/effects/GlowStyles.cpp#L62-L66 (chrome/m156) (`InnerSource`)
const INNER_SOURCE_EDGE: i32 = 1;

/// The glow styles.
// Port of: modules/skottie/src/effects/GlowStyles.cpp#L17-L22 (chrome/m156) (`GlowAdapter::Type`)
#[derive(Clone, Copy, PartialEq, Eq)]
enum GlowType {
    OuterGlow,
    InnerGlow,
}

/// Glows the layer from its edge or its center.
// Port of: modules/skottie/src/effects/GlowStyles.cpp#L24-L104 (chrome/m156) (`GlowAdapter`)
struct GlowAdapter {
    base: DiscardableAdapterBase<ExternalImageFilter>,
    kind: GlowType,
    color: Prop<ColorValue>,
    opacity: Prop<ScalarValue>,
    size: Prop<ScalarValue>,
    inner_source: Prop<ScalarValue>,
    choke: Prop<ScalarValue>,
}

impl GlowAdapter {
    // Port of: modules/skottie/src/effects/GlowStyles.cpp#L30-L38 (chrome/m156) (`GlowAdapter::GlowAdapter`)
    fn make(jstyle: &ObjectValue, abuilder: &AnimationBuilder<'_>, kind: GlowType) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), ExternalImageFilter::make());
            let color = Prop::new(ColorValue::new());
            let opacity = Prop::new(100.0); // percentage
            let size = Prop::new(0.0);
            // The default is the edge source (`fInnerSource = kEdge`).
            let inner_source = Prop::new(1.0);
            let choke = Prop::new(0.0);
            let container = base.container();
            container.bind(abuilder, jstyle.get("c"), &color);
            container.bind(abuilder, jstyle.get("o"), &opacity);
            container.bind(abuilder, jstyle.get("s"), &size);
            container.bind(abuilder, jstyle.get("sr"), &inner_source);
            container.bind(abuilder, jstyle.get("ch"), &choke);
            Self {
                base,
                kind,
                color,
                opacity,
                size,
                inner_source,
                choke,
            }
        })
    }
}

impl AnimatablePropertyContainer for GlowAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/GlowStyles.cpp#L40-L100 (chrome/m156) (`GlowAdapter::onSync`)
    fn on_sync(&self) {
        const K_MAX_ALPHA_SCALE: f32 = 1e6;
        const K_CHOKE_GAMMA: f32 = 0.2;

        let sigma = *self.size.borrow() * BLUR_SIZE_TO_SIGMA;
        let opacity = t_pin(*self.opacity.borrow() / 100.0, 0.0, 1.0);
        let choke = t_pin(*self.choke.borrow() / 100.0, 0.0, 1.0);
        let color = self.color.borrow().to_color4f();

        let mut mask_cm = matrix_from_row_major(&[
            0.0, 0.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 0.0, 0.0, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ]);
        if self.kind == GlowType::InnerGlow
            && scalar_round_to_int(*self.inner_source.borrow()) == INNER_SOURCE_EDGE
        {
            mask_cm.pre_concat(&matrix_from_row_major(&INVERT_ALPHA));
        }

        let color_cm = matrix_from_row_major(&[
            0.0,
            0.0,
            0.0,
            0.0,
            color.r, //
            0.0,
            0.0,
            0.0,
            0.0,
            color.g, //
            0.0,
            0.0,
            0.0,
            0.0,
            color.b, //
            0.0,
            0.0,
            0.0,
            opacity * color.a,
            0.0,
        ]);

        let requires_alpha_choke = sigma > 0.0 && choke > 0.0;
        if !requires_alpha_choke {
            mask_cm.post_concat(&color_cm);
        }

        let mut f = matrix_filter(&mask_cm, None);
        if sigma > 0.0 {
            f = blur(sigma, sigma, TileMode::Decal, f, None);
        }
        if requires_alpha_choke {
            let alpha_scale = std_min(
                1.0 / (1.0 - scalar_pow(choke, K_CHOKE_GAMMA)),
                K_MAX_ALPHA_SCALE,
            );
            let choke_cm = matrix_from_row_major(&[
                1.0,
                0.0,
                0.0,
                0.0,
                0.0, //
                0.0,
                1.0,
                0.0,
                0.0,
                0.0, //
                0.0,
                0.0,
                1.0,
                0.0,
                0.0, //
                0.0,
                0.0,
                0.0,
                alpha_scale,
                0.0,
            ]);
            f = matrix_filter(&choke_cm, f);
            f = matrix_filter(&color_cm, f);
        }

        let mut source: Option<ImageFilter> = None;
        if self.kind == GlowType::InnerGlow {
            f = blend(BlendMode::DstIn, f, None, None);
            // std::swap(source, f)
            std::mem::swap(&mut source, &mut f);
        }
        self.base.node().set_image_filter(merge(&[f, source], None));
    }
}

/// `std::min(a, b)`: `b` only if `b < a`, so a NaN `a` is kept.
// Port of: <algorithm> std::min (chrome/m156)
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

impl_container_animator!(GlowAdapter);

/// Applies a glow style of the given kind to the layer.
// Port of: modules/skottie/src/effects/GlowStyles.cpp#L106-L115 (chrome/m156) (`make_glow_effect`)
fn make_glow_effect(
    jstyle: &ObjectValue,
    abuilder: &AnimationBuilder<'_>,
    layer: Rc<dyn RenderNode>,
    kind: GlowType,
) -> Rc<dyn RenderNode> {
    let adapter = GlowAdapter::make(jstyle, abuilder, kind);
    abuilder.attach_discardable_adapter(&adapter);
    let filter_node = Rc::clone(adapter.base.node());
    ImageFilterEffect::make(layer, Some(filter_node as Rc<dyn ImageFilterNode>))
}

/// The inner glow style (style type 4).
// Port of: modules/skottie/src/effects/GlowStyles.cpp#L117-L121 (chrome/m156) (`EffectBuilder::attachInnerGlowStyle`)
pub(super) fn attach_inner_glow_style(
    eb: &EffectBuilder<'_, '_>,
    jstyle: &ObjectValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    Some(make_glow_effect(
        jstyle,
        eb.builder(),
        layer?,
        GlowType::InnerGlow,
    ))
}

/// The outer glow style (style type 3).
// Port of: modules/skottie/src/effects/GlowStyles.cpp#L123-L128 (chrome/m156) (`EffectBuilder::attachOuterGlowStyle`)
pub(super) fn attach_outer_glow_style(
    eb: &EffectBuilder<'_, '_>,
    jstyle: &ObjectValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    Some(make_glow_effect(
        jstyle,
        eb.builder(),
        layer?,
        GlowType::OuterGlow,
    ))
}

// ---- Color overlay ----------------------------------------------------------------------------

/// Replaces the color of the layer, with an opacity.
// Port of: modules/skottie/src/effects/ColorOverlayStyles.cpp#L17-L51 (chrome/m156) (`ColorOverlayAdapter`)
struct ColorOverlayAdapter {
    base: DiscardableAdapterBase<ExternalImageFilter>,
    color: Prop<ColorValue>,
    opacity: Prop<ScalarValue>,
}

impl ColorOverlayAdapter {
    // Port of: modules/skottie/src/effects/ColorOverlayStyles.cpp#L22-L27 (chrome/m156) (`ColorOverlayAdapter::ColorOverlayAdapter`)
    fn make(jstyle: &ObjectValue, abuilder: &AnimationBuilder<'_>) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), ExternalImageFilter::make());
            let color = Prop::new(ColorValue::new());
            let opacity = Prop::new(100.0); // percentage
            let container = base.container();
            container.bind(abuilder, jstyle.get("c"), &color);
            container.bind(abuilder, jstyle.get("so"), &opacity);
            Self {
                base,
                color,
                opacity,
            }
        })
    }
}

impl AnimatablePropertyContainer for ColorOverlayAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/ColorOverlayStyles.cpp#L29-L43 (chrome/m156) (`ColorOverlayAdapter::onSync`)
    fn on_sync(&self) {
        let opacity = t_pin(*self.opacity.borrow() / 100.0, 0.0, 1.0);
        let color = self.color.borrow().to_color4f();
        let cm = matrix_from_row_major(&[
            1.0 - opacity,
            0.0,
            0.0,
            0.0,
            color.r * opacity, //
            0.0,
            1.0 - opacity,
            0.0,
            0.0,
            color.g * opacity, //
            0.0,
            0.0,
            1.0 - opacity,
            0.0,
            color.b * opacity, //
            0.0,
            0.0,
            0.0,
            color.a,
            0.0,
        ]);
        self.base.node().set_image_filter(matrix_filter(&cm, None));
    }
}

impl_container_animator!(ColorOverlayAdapter);

/// The color overlay style (style type 7).
// Port of: modules/skottie/src/effects/ColorOverlayStyles.cpp#L53-L62 (chrome/m156) (`EffectBuilder::attachColorOverlayStyle`)
pub(super) fn attach_color_overlay_style(
    eb: &EffectBuilder<'_, '_>,
    jstyle: &ObjectValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = ColorOverlayAdapter::make(jstyle, eb.builder());
    eb.builder().attach_discardable_adapter(&adapter);
    let filter_node = Rc::clone(adapter.base.node());
    Some(ImageFilterEffect::make(
        layer,
        Some(filter_node as Rc<dyn ImageFilterNode>),
    ))
}
