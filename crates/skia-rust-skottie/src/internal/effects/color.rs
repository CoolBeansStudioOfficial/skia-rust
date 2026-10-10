// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/FillEffect.cpp, TintEffect.cpp,
// TritoneEffect.cpp, InvertEffect.cpp, ThresholdEffect.cpp, HueSaturationEffect.cpp,
// LevelsEffect.cpp (chrome/m156)
//
// The color effects: fill, tint, tritone, invert, threshold, hue/saturation and the easy and pro
// levels. Each drives a color filter node from the layer's properties.

use std::rc::{Rc, Weak};

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color::Color;
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::data::Data;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::scalar::{
    SCALAR_NEARLY_ZERO, SCALAR_PI, Scalar, scalar_pow, scalar_round_to_int, scalar_trunc_to_int,
};
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::{
    Color as SgColor, ExternalColorFilter, GradientColorFilter, ModeColorFilter, RenderNode,
};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::{ColorValue, ScalarValue};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// `std::max(a, b)`: `b` only if `a < b`, so a NaN `a` is kept.
// Port of: <algorithm> std::max (chrome/m156)
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `std::min(a, b)`: `b` only if `b < a`, so a NaN `a` is kept.
// Port of: <algorithm> std::min (chrome/m156)
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// The `ExternalColorFilter` node that wraps the layer of an effect.
// Port of: modules/sksg/src/SkSGColorFilter.cpp#L19-L25 (chrome/m156) (`ExternalColorFilter::Make`)
fn external_color_filter(layer: Rc<dyn RenderNode>) -> Rc<ExternalColorFilter> {
    ExternalColorFilter::make(Some(layer)).expect("the layer is not null")
}

/// Builds a color filter from a runtime effect and its uniform bytes (`makeColorFilter`).
// Port of: src/effects/SkRuntimeEffect.cpp#L894-L909 (chrome/m156) (`SkRuntimeEffect::makeColorFilter`)
fn runtime_color_filter(effect: &RuntimeEffect, uniform: &[u8]) -> Option<ColorFilter> {
    effect.make_color_filter(Data::new_copy(uniform), &[])
}

// ---- Fill -------------------------------------------------------------------------------------

/// The index of the fill color and the opacity properties.
const FILL_COLOR_INDEX: usize = 2;
const FILL_OPACITY_INDEX: usize = 6;

/// Fills the layer with a solid color.
// Port of: modules/skottie/src/effects/FillEffect.cpp#L16-L63 (chrome/m156) (`FillAdapter`)
struct FillAdapter {
    base: DiscardableAdapterBase<ModeColorFilter>,
    color_node: Rc<SgColor>,
    color: Prop<ColorValue>,
    opacity: Prop<ScalarValue>,
}

impl FillAdapter {
    // Port of: modules/skottie/src/effects/FillEffect.cpp#L23-L61 (chrome/m156) (`FillAdapter::FillAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let color_node = SgColor::make(Color::BLACK);
            let filter =
                ModeColorFilter::make(Some(layer), Some(Rc::clone(&color_node)), BlendMode::SrcIn)
                    .expect("the layer is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), filter);
            let color = Prop::new(ColorValue::new());
            let opacity = Prop::new(1.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(FILL_COLOR_INDEX, &color)
                .bind(FILL_OPACITY_INDEX, &opacity);
            abuilder.dispatch_color_property(&color_node);
            Self {
                base,
                color_node,
                color,
                opacity,
            }
        })
    }
}

impl AnimatablePropertyContainer for FillAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/FillEffect.cpp#L49-L55 (chrome/m156) (`FillAdapter::onSync`)
    fn on_sync(&self) {
        let mut c = self.color.borrow().to_color4f();
        c.a = t_pin(*self.opacity.borrow(), 0.0, 1.0);
        self.color_node.set_color(c.to_color());
    }
}

impl_container_animator!(FillAdapter);

/// The fill effect: an `SrcIn` blend of the layer with a solid color.
// Port of: modules/skottie/src/effects/FillEffect.cpp#L65-L69 (chrome/m156) (`EffectBuilder::attachFillEffect`)
pub(super) fn attach_fill_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = FillAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Tint -------------------------------------------------------------------------------------

/// Maps the luminance of the layer between two colors.
// Port of: modules/skottie/src/effects/TintEffect.cpp#L15-L58 (chrome/m156) (`TintAdapter`)
struct TintAdapter {
    base: DiscardableAdapterBase<GradientColorFilter>,
    color_node0: Rc<SgColor>,
    color_node1: Rc<SgColor>,
    map_black_to: Prop<ColorValue>,
    map_white_to: Prop<ColorValue>,
    amount: Prop<ScalarValue>,
}

impl TintAdapter {
    // Port of: modules/skottie/src/effects/TintEffect.cpp#L20-L40 (chrome/m156) (`TintAdapter::TintAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let color_node0 = SgColor::make(Color::BLACK);
            let color_node1 = SgColor::make(Color::BLACK);
            let filter = GradientColorFilter::make(
                Some(layer),
                Some(Rc::clone(&color_node0)),
                Some(Rc::clone(&color_node1)),
            )
            .expect("the layer is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), filter);
            let map_black_to = Prop::new(ColorValue::new());
            let map_white_to = Prop::new(ColorValue::new());
            let amount = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &map_black_to)
                .bind(1, &map_white_to)
                .bind(2, &amount);
            Self {
                base,
                color_node0,
                color_node1,
                map_black_to,
                map_white_to,
                amount,
            }
        })
    }
}

impl AnimatablePropertyContainer for TintAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/TintEffect.cpp#L42-L46 (chrome/m156) (`TintAdapter::onSync`)
    fn on_sync(&self) {
        self.color_node0
            .set_color(self.map_black_to.borrow().to_color());
        self.color_node1
            .set_color(self.map_white_to.borrow().to_color());
        // 100-based
        self.base.node().set_weight(*self.amount.borrow() / 100.0);
    }
}

impl_container_animator!(TintAdapter);

/// The tint effect (`ADBE Tint`, or the legacy effect type 20).
// Port of: modules/skottie/src/effects/TintEffect.cpp#L60-L65 (chrome/m156) (`EffectBuilder::attachTintEffect`)
pub(super) fn attach_tint_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = TintAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Tritone ----------------------------------------------------------------------------------

/// Maps the luminance of the layer through three colors.
// Port of: modules/skottie/src/effects/TritoneEffect.cpp#L16-L62 (chrome/m156) (`TritoneAdapter`)
struct TritoneAdapter {
    base: DiscardableAdapterBase<GradientColorFilter>,
    lo_color_node: Rc<SgColor>,
    mi_color_node: Rc<SgColor>,
    hi_color_node: Rc<SgColor>,
    lo_color: Prop<ColorValue>,
    mi_color: Prop<ColorValue>,
    hi_color: Prop<ColorValue>,
    weight: Prop<ScalarValue>,
}

impl TritoneAdapter {
    // Port of: modules/skottie/src/effects/TritoneEffect.cpp#L21-L43 (chrome/m156) (`TritoneAdapter::TritoneAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let lo_color_node = SgColor::make(Color::BLACK);
            let mi_color_node = SgColor::make(Color::BLACK);
            let hi_color_node = SgColor::make(Color::BLACK);
            let filter = GradientColorFilter::make_with_colors(
                Some(layer),
                &[
                    Rc::clone(&lo_color_node),
                    Rc::clone(&mi_color_node),
                    Rc::clone(&hi_color_node),
                ],
            )
            .expect("the layer is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), filter);
            let hi_color = Prop::new(ColorValue::new());
            let mi_color = Prop::new(ColorValue::new());
            let lo_color = Prop::new(ColorValue::new());
            let weight = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &hi_color)
                .bind(1, &mi_color)
                .bind(2, &lo_color)
                .bind(3, &weight);
            Self {
                base,
                lo_color_node,
                mi_color_node,
                hi_color_node,
                lo_color,
                mi_color,
                hi_color,
                weight,
            }
        })
    }
}

impl AnimatablePropertyContainer for TritoneAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/TritoneEffect.cpp#L45-L53 (chrome/m156) (`TritoneAdapter::onSync`)
    fn on_sync(&self) {
        self.lo_color_node
            .set_color(self.lo_color.borrow().to_color());
        self.mi_color_node
            .set_color(self.mi_color.borrow().to_color());
        self.hi_color_node
            .set_color(self.hi_color.borrow().to_color());
        // 100-based, inverted
        self.base
            .node()
            .set_weight((100.0 - *self.weight.borrow()) / 100.0);
    }
}

impl_container_animator!(TritoneAdapter);

/// The tritone effect (`ADBE Tritone`, or the legacy effect type 23).
// Port of: modules/skottie/src/effects/TritoneEffect.cpp#L65-L70 (chrome/m156) (`EffectBuilder::attachTritoneEffect`)
pub(super) fn attach_tritone_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = TritoneAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Invert -----------------------------------------------------------------------------------

/// The color space an invert channel is computed in.
// Port of: modules/skottie/src/effects/InvertEffect.cpp#L40 (chrome/m156) (`enum class CS`)
#[derive(Clone, Copy, PartialEq, Eq)]
enum InvertSpace {
    Rgb,
    Hsl,
    Yiq,
}

/// The per-channel scale, translation and color space of an invert channel.
// Port of: modules/skottie/src/effects/InvertEffect.cpp#L41-L45 (chrome/m156) (`STColorMatrix`)
struct StColorMatrix {
    scale: [f32; 4],
    trans: [f32; 4],
    cs: InvertSpace,
}

/// The matrix of an invert channel selection (the lambda `stcm`).
// Port of: modules/skottie/src/effects/InvertEffect.cpp#L48-L84 (chrome/m156)
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors static_cast<uint8_t>
fn invert_st_color_matrix(channel: f32) -> StColorMatrix {
    use InvertSpace::{Hsl, Rgb, Yiq};

    // https://helpx.adobe.com/after-effects/using/channel-effects.html#invert_effect
    // NB: HLS vs. HSL
    const RGB_CHANNEL: u8 = 1;
    const R_CHANNEL: u8 = 2;
    const G_CHANNEL: u8 = 3;
    const B_CHANNEL: u8 = 4;
    const HLS_CHANNEL: u8 = 6;
    const H_CHANNEL: u8 = 7;
    const L_CHANNEL: u8 = 8;
    const S_CHANNEL: u8 = 9;
    const YIQ_CHANNEL: u8 = 11;
    const Y_CHANNEL: u8 = 12;
    const I_CHANNEL: u8 = 13;
    const Q_CHANNEL: u8 = 14;
    const A_CHANNEL: u8 = 16;

    let mk = |scale: [f32; 4], trans: [f32; 4], cs| StColorMatrix { scale, trans, cs };
    // Port of: static_cast<uint8_t>(fChannel); the selectors are small non-negative integers.
    match (channel as i32) as u8 {
        R_CHANNEL => mk([-1.0, 1.0, 1.0, 1.0], [1.0, 0.0, 0.0, 0.0], Rgb), // r' = 1 - r
        G_CHANNEL => mk([1.0, -1.0, 1.0, 1.0], [0.0, 1.0, 0.0, 0.0], Rgb), // g' = 1 - g
        B_CHANNEL => mk([1.0, 1.0, -1.0, 1.0], [0.0, 0.0, 1.0, 0.0], Rgb), // b' = 1 - b
        A_CHANNEL => mk([1.0, 1.0, 1.0, -1.0], [0.0, 0.0, 0.0, 1.0], Rgb), // a' = 1 - a
        RGB_CHANNEL => mk([-1.0, -1.0, -1.0, 1.0], [1.0, 1.0, 1.0, 0.0], Rgb),
        H_CHANNEL => mk([-1.0, 1.0, 1.0, 1.0], [0.5, 0.0, 0.0, 0.0], Hsl), // h' = .5 - h
        S_CHANNEL => mk([1.0, -1.0, 1.0, 1.0], [0.0, 1.0, 0.0, 0.0], Hsl), // s' = 1 - s
        L_CHANNEL => mk([1.0, 1.0, -1.0, 1.0], [0.0, 0.0, 1.0, 0.0], Hsl), // l' = 1 - l
        HLS_CHANNEL => mk([-1.0, -1.0, -1.0, 1.0], [0.5, 1.0, 1.0, 0.0], Hsl),
        Y_CHANNEL => mk([-1.0, 1.0, 1.0, 1.0], [1.0, 0.0, 0.0, 0.0], Yiq), // y' = 1 - y
        I_CHANNEL => mk([1.0, -1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0], Yiq), // i' = -i
        Q_CHANNEL => mk([1.0, 1.0, -1.0, 1.0], [0.0, 0.0, 0.0, 0.0], Yiq), // q' = -q
        YIQ_CHANNEL => mk([-1.0, -1.0, -1.0, 1.0], [1.0, 0.0, 0.0, 0.0], Yiq),
        _ => mk([1.0, 1.0, 1.0, 1.0], [0.0, 0.0, 0.0, 0.0], Rgb),
    }
}

/// Inverts a channel of the layer.
// Port of: modules/skottie/src/effects/InvertEffect.cpp#L23-L36 (chrome/m156) (`InvertEffectAdapter`)
struct InvertEffectAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    channel: Prop<ScalarValue>,
}

impl InvertEffectAdapter {
    // Port of: modules/skottie/src/effects/InvertEffect.cpp#L27-L35 (chrome/m156) (`InvertEffectAdapter::InvertEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), external_color_filter(layer));
            let channel = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container()).bind(0, &channel);
            Self { base, channel }
        })
    }
}

impl AnimatablePropertyContainer for InvertEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/InvertEffect.cpp#L38-L100 (chrome/m156) (`InvertEffectAdapter::onSync`)
    fn on_sync(&self) {
        let stcm = invert_st_color_matrix(*self.channel.borrow());
        let s = stcm.scale;
        let t = stcm.trans;
        let mut m = ColorMatrix::default();
        m.set_row_major(&[
            s[0], 0.0, 0.0, 0.0, t[0], //
            0.0, s[1], 0.0, 0.0, t[1], //
            0.0, 0.0, s[2], 0.0, t[2], //
            0.0, 0.0, 0.0, s[3], t[3],
        ]);

        if stcm.cs == InvertSpace::Yiq {
            // https://en.wikipedia.org/wiki/YIQ
            let mut rgb2yiq = ColorMatrix::default();
            rgb2yiq.set_row_major(&[
                0.2990, 0.5870, 0.1140, 0.0, 0.0, //
                0.5959, -0.2746, -0.3213, 0.0, 0.0, //
                0.2115, -0.5227, 0.3112, 0.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, 0.0,
            ]);
            let mut yiq2rgb = ColorMatrix::default();
            yiq2rgb.set_row_major(&[
                1.0, 0.9560, 0.6190, 0.0, 0.0, //
                1.0, -0.2720, -0.6470, 0.0, 0.0, //
                1.0, -1.1060, 1.7030, 0.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, 0.0,
            ]);
            m.pre_concat(&rgb2yiq);
            m.post_concat(&yiq2rgb);
        }

        let filter = if stcm.cs == InvertSpace::Hsl {
            color_filters::hsla_matrix_of_color_matrix(&m)
        } else {
            color_filters::matrix(&m, Clamp::Yes)
        };
        self.base.node().set_color_filter(filter);
    }
}

impl_container_animator!(InvertEffectAdapter);

/// The invert effect (`ADBE Invert`).
// Port of: modules/skottie/src/effects/InvertEffect.cpp#L144-L150 (chrome/m156) (`EffectBuilder::attachInvertEffect`)
pub(super) fn attach_invert_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = InvertEffectAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Threshold --------------------------------------------------------------------------------

/// Convert to black & white, based on input luminance and a threshold uniform.
// Port of: modules/skottie/src/effects/ThresholdEffect.cpp#L15-L24 (chrome/m156) (`gThresholdSkSL`)
const THRESHOLD_SKSL: &str = concat!(
    "uniform half t;",
    "half4 main(half4 color) {",
    "half4 c = unpremul(color);",
    "half lum = dot(c.rgb, half3(0.2126, 0.7152, 0.0722)),",
    "bw = step(t, lum);",
    "return bw.xxx1 * c.a;",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/ThresholdEffect.cpp#L44-L49 (chrome/m156) (`threshold_effect`)
    static THRESHOLD_EFFECT: RuntimeEffect =
        RuntimeEffect::make_for_color_filter(THRESHOLD_SKSL, None)
            .expect("the threshold effect compiles");
}

/// Converts the layer to black and white at a threshold.
// Port of: modules/skottie/src/effects/ThresholdEffect.cpp#L51-L81 (chrome/m156) (`ThresholdAdapter`)
struct ThresholdAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    level: Prop<ScalarValue>,
}

impl ThresholdAdapter {
    // Port of: modules/skottie/src/effects/ThresholdEffect.cpp#L54-L62 (chrome/m156) (`ThresholdAdapter::ThresholdAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), external_color_filter(layer));
            let level = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container()).bind(0, &level);
            Self { base, level }
        })
    }
}

impl AnimatablePropertyContainer for ThresholdAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/ThresholdEffect.cpp#L64-L69 (chrome/m156) (`ThresholdAdapter::onSync`)
    fn on_sync(&self) {
        let level = *self.level.borrow();
        let cf = THRESHOLD_EFFECT.with(|effect| runtime_color_filter(effect, &level.to_ne_bytes()));
        self.base.node().set_color_filter(cf);
    }
}

impl_container_animator!(ThresholdAdapter);

/// The threshold effect (`ADBE Threshold2`).
// Port of: modules/skottie/src/effects/ThresholdEffect.cpp#L83-L89 (chrome/m156) (`EffectBuilder::attachThresholdEffect`)
pub(super) fn attach_threshold_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = ThresholdAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Brightness/Contrast ----------------------------------------------------------------------

/// The brightness `SkSL`: a power curve on the inverted color.
// Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L76-L82 (chrome/m156) (`BRIGHTNESS_EFFECT`)
const BRIGHTNESS_SKSL: &str = concat!(
    "uniform half a;",
    "half4 main(half4 color) {",
    "color.rgb = 1 - pow(1 - color.rgb, half3(a));",
    "return color;",
    "}",
);

/// The contrast `SkSL`: a cubic polynomial in the color (the default, non-accurate approximation).
// Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L40-L53 (chrome/m156) (`CONTRAST_EFFECT`)
const CONTRAST_SKSL: &str = concat!(
    "uniform half a;",
    "uniform half b;",
    "uniform half c;",
    "half4 main(half4 color) {",
    "color.rgb = ((a*color.rgb + b)*color.rgb + c)*color.rgb;",
    "return color;",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L150-L151 (chrome/m156)
    static BRIGHTNESS_EFFECT: RuntimeEffect =
        RuntimeEffect::make_for_color_filter(BRIGHTNESS_SKSL, None)
            .expect("the brightness effect compiles");
    // Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L150-L151 (chrome/m156)
    static CONTRAST_EFFECT: RuntimeEffect =
        RuntimeEffect::make_for_color_filter(CONTRAST_SKSL, None)
            .expect("the contrast effect compiles");
}

/// The brightness uniform: the exponent of the power curve.
// Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L65-L69 (chrome/m156) (`make_brightness_coeffs`)
fn make_brightness_coeffs(brightness: f32) -> f32 {
    scalar_pow(2.0, brightness * 1.8)
}

/// The contrast uniforms `a`, `b` and `c` of the polynomial.
// Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L23-L33 (chrome/m156) (`make_contrast_coeffs`)
fn make_contrast_coeffs(contrast: f32) -> [u8; 12] {
    let b = SCALAR_PI * contrast;
    let a = -2.0 * b / 3.0;
    let c = 1.0 - b / 3.0;
    let mut bytes = [0_u8; 12];
    bytes[0..4].copy_from_slice(&a.to_ne_bytes());
    bytes[4..8].copy_from_slice(&b.to_ne_bytes());
    bytes[8..12].copy_from_slice(&c.to_ne_bytes());
    bytes
}

/// Adjusts the brightness and contrast of the layer, or applies the legacy matrix.
// Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L121-L186 (chrome/m156) (`BrightnessContrastAdapter`)
struct BrightnessContrastAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    brightness: Prop<ScalarValue>,
    contrast: Prop<ScalarValue>,
    use_legacy: Prop<ScalarValue>,
}

impl BrightnessContrastAdapter {
    // Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L144-L160 (chrome/m156) (`BrightnessContrastAdapter::BrightnessContrastAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), external_color_filter(layer));
            let brightness = Prop::new(0.0);
            let contrast = Prop::new(0.0);
            let use_legacy = Prop::new(0.0);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &brightness)
                .bind(1, &contrast)
                .bind(2, &use_legacy);
            Self {
                base,
                brightness,
                contrast,
                use_legacy,
            }
        })
    }

    // Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L162-L173 (chrome/m156) (`makeLegacyCF`)
    fn make_legacy_cf(&self) -> Option<ColorFilter> {
        let brightness = t_pin(*self.brightness.borrow(), -100.0, 100.0) / 255.0;
        let contrast = t_pin(*self.contrast.borrow(), -100.0, 100.0) / 100.0;
        let s = if contrast > 0.0 {
            1.0 / std_max(1.0 - contrast, SCALAR_NEARLY_ZERO)
        } else {
            1.0 + contrast
        };
        let b = 0.5 * (1.0 - s) + brightness * std_max(s, 1.0);
        let cm: [f32; 20] = [
            s, 0.0, 0.0, 0.0, b, //
            0.0, s, 0.0, 0.0, b, //
            0.0, 0.0, s, 0.0, b, //
            0.0, 0.0, 0.0, 1.0, 0.0,
        ];
        color_filters::matrix_row_major(&cm, Clamp::Yes)
    }

    // Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L175-L186 (chrome/m156) (`makeCF`)
    fn make_cf(&self) -> Option<ColorFilter> {
        let raw_contrast = *self.contrast.borrow();
        let brightness = t_pin(*self.brightness.borrow(), -150.0, 150.0) / 150.0;
        let contrast = t_pin(raw_contrast, -50.0, 100.0) / 100.0;

        let b_eff = if brightness.nearly_zero(None) {
            None
        } else {
            let coeff = make_brightness_coeffs(brightness);
            BRIGHTNESS_EFFECT.with(|effect| runtime_color_filter(effect, &coeff.to_ne_bytes()))
        };
        let c_eff = if raw_contrast.nearly_zero(None) {
            None
        } else {
            let coeffs = make_contrast_coeffs(contrast);
            CONTRAST_EFFECT.with(|effect| runtime_color_filter(effect, &coeffs))
        };
        color_filters::compose(c_eff.as_ref(), b_eff)
    }
}

impl AnimatablePropertyContainer for BrightnessContrastAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L134-L139 (chrome/m156) (`BrightnessContrastAdapter::onSync`)
    fn on_sync(&self) {
        let cf = if scalar_round_to_int(*self.use_legacy.borrow()) != 0 {
            self.make_legacy_cf()
        } else {
            self.make_cf()
        };
        self.base.node().set_color_filter(cf);
    }
}

impl_container_animator!(BrightnessContrastAdapter);

/// The brightness/contrast effect (`ADBE Brightness & Contrast 2`).
// Port of: modules/skottie/src/effects/BrightnessContrastEffect.cpp#L252-L258 (chrome/m156) (`EffectBuilder::attachBrightnessContrastEffect`)
pub(super) fn attach_brightness_contrast_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = BrightnessContrastAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Hue/Saturation ---------------------------------------------------------------------------

/// The saturation `SkSL`: AE saturation semantics, with a per-component chroma scale.
// Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L16-L47 (chrome/m156) (`gSaturateSkSL`)
const SATURATE_SKSL: &str = concat!(
    "uniform half u_scale;",
    "half4 main(half4 c) {",
    "half2 rg_srt = (c.r < c.g) ? c.rg : c.gr;",
    "half c_min = min(rg_srt.x, c.b),",
    "c_max = max(rg_srt.y, c.b),",
    "ch     = max(c_max - c_min, 0.0001),",
    "ch_mid = (c_min + c_max)*0.5,",
    "scale_max = min(ch_mid, c.a - ch_mid)/ch*2,",
    "scale = min(u_scale, scale_max);",
    "c.rgb = ch_mid + (c.rgb - ch_mid)*scale;",
    "return c;",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L49-L55 (chrome/m156)
    static SATURATE_EFFECT: RuntimeEffect =
        RuntimeEffect::make_for_color_filter(SATURATE_SKSL, None)
            .expect("the saturate effect compiles");
}

/// The saturation color filter for a chroma scale.
// Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L57-L61 (chrome/m156) (`make_saturate`)
fn make_saturate(chroma_scale: f32) -> Option<ColorFilter> {
    SATURATE_EFFECT.with(|effect| runtime_color_filter(effect, &chroma_scale.to_ne_bytes()))
}

/// Adjusts the master hue, saturation and lightness of the layer.
// Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L63-L130 (chrome/m156) (`HueSaturationEffectAdapter`)
struct HueSaturationEffectAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    chan_ctrl: Prop<f32>,
    master_hue: Prop<f32>,
    master_sat: Prop<f32>,
    master_light: Prop<f32>,
}

impl HueSaturationEffectAdapter {
    // Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L88-L104 (chrome/m156) (`HueSaturationEffectAdapter::HueSaturationEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), external_color_filter(layer));
            let chan_ctrl = Prop::new(0.0);
            let master_hue = Prop::new(0.0);
            let master_sat = Prop::new(0.0);
            let master_light = Prop::new(0.0);
            // TODO (upstream): colorize support?
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(0, &chan_ctrl)
                .bind(2, &master_hue)
                .bind(3, &master_sat)
                .bind(4, &master_light);
            Self {
                base,
                chan_ctrl,
                master_hue,
                master_sat,
                master_light,
            }
        })
    }

    // Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L106-L168 (chrome/m156) (`HueSaturationEffectAdapter::makeColorFilter`)
    fn make_color_filter(&self) -> Option<ColorFilter> {
        // Kept in the same order as the Skia enum: the master channel is 1.
        const MASTER_CHAN: i32 = 0x01;

        // We only support master channel controls at this point.
        if scalar_trunc_to_int(*self.chan_ctrl.borrow()) != MASTER_CHAN {
            return None;
        }

        let master_hue = *self.master_hue.borrow();
        let master_sat = *self.master_sat.borrow();
        let master_light = *self.master_light.borrow();

        let mut cf: Option<ColorFilter> = None;
        if !master_hue.nearly_zero(None) {
            // Linear control mapping hue(degrees) -> hue offset]
            let h = master_hue / 360.0;
            let cm: [f32; 20] = [
                1.0, 0.0, 0.0, 0.0, h, //
                0.0, 1.0, 0.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, 0.0,
            ];
            cf = color_filters::hsla_matrix(&cm);
        }
        if !master_sat.nearly_zero(None) {
            // AE clamps the max chroma scale to this value.
            const MAX_SCALE: f32 = 126.0;
            // Control mapping:
            //   * sat [-100 .. 0) -> scale [0 .. 1)   , linear
            //   * sat  [0 .. 100] -> scale [1 .. max] , nonlinear: 100/(100 - sat)
            let s = t_pin(master_sat / 100.0, -1.0, 1.0);
            let chroma_scale = if s < 0.0 {
                s + 1.0
            } else {
                std_min(1.0 / (1.0 - s), MAX_SCALE)
            };
            cf = color_filters::compose(cf.as_ref(), make_saturate(chroma_scale));
        }
        if !master_light.nearly_zero(None) {
            // AE implements Lightness as a component-wise interpolation to 0 (for L < 0),
            // or 1 (for L > 0).
            //
            // Control mapping:
            //   * lightness [-100 .. 0) -> lerp[0 .. 1) from 0, linear
            //   * lightness  [0 .. 100] -> lerp[1 .. 0] from 1, linear
            let l = t_pin(master_light / 100.0, -1.0, 1.0);
            let ls = 1.0 - l.abs(); // scale
            let lo = if l < 0.0 { 0.0 } else { 1.0 - ls }; // offset
            let cm: [f32; 20] = [
                ls, 0.0, 0.0, 0.0, lo, //
                0.0, ls, 0.0, 0.0, lo, //
                0.0, 0.0, ls, 0.0, lo, //
                0.0, 0.0, 0.0, 1.0, 0.0,
            ];
            let matrix = color_filters::matrix_row_major(&cm, Clamp::Yes);
            cf = color_filters::compose(cf.as_ref(), matrix);
        }
        cf
    }
}

impl AnimatablePropertyContainer for HueSaturationEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L120-L122 (chrome/m156) (`HueSaturationEffectAdapter::onSync`)
    fn on_sync(&self) {
        self.base.node().set_color_filter(self.make_color_filter());
    }
}

impl_container_animator!(HueSaturationEffectAdapter);

/// The hue/saturation effect (`ADBE HUE SATURATION`).
// Port of: modules/skottie/src/effects/HueSaturationEffect.cpp#L202-L207 (chrome/m156) (`EffectBuilder::attachHueSaturationEffect`)
pub(super) fn attach_hue_saturation_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = HueSaturationEffectAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- Levels -----------------------------------------------------------------------------------

/// The clip flags of a levels effect: 1 clips the output to the output range.
// Port of: modules/skottie/src/effects/LevelsEffect.cpp#L23-L26 (chrome/m156) (`ClipInfo`)
#[derive(Clone)]
struct ClipInfo {
    clip_black: Prop<ScalarValue>,
    clip_white: Prop<ScalarValue>,
}

impl ClipInfo {
    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L23-L26 (chrome/m156) (`ClipInfo` defaults)
    fn new() -> Self {
        Self {
            clip_black: Prop::new(1.0),
            clip_white: Prop::new(1.0),
        }
    }
}

/// The input and output range, and the gamma, of one channel.
// Port of: modules/skottie/src/effects/LevelsEffect.cpp#L28-L37 (chrome/m156) (`ChannelMapper`)
#[derive(Clone)]
struct ChannelMapper {
    in_black: Prop<ScalarValue>,
    in_white: Prop<ScalarValue>,
    out_black: Prop<ScalarValue>,
    out_white: Prop<ScalarValue>,
    gamma: Prop<ScalarValue>,
}

impl ChannelMapper {
    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L28-L37 (chrome/m156) (`ChannelMapper` defaults)
    fn new() -> Self {
        Self {
            in_black: Prop::new(0.0),
            in_white: Prop::new(1.0),
            out_black: Prop::new(0.0),
            out_white: Prop::new(1.0),
            gamma: Prop::new(1.0),
        }
    }

    /// Binds the five properties of the channel at `base`, `base + 1`, ... (in AE order).
    fn bind(&self, binder: &EffectBinder<'_, '_, '_>, base: usize) {
        binder
            .bind(base, &self.in_black)
            .bind(base + 1, &self.in_white)
            .bind(base + 2, &self.gamma)
            .bind(base + 3, &self.out_black)
            .bind(base + 4, &self.out_white);
    }

    /// The 256-entry lookup table of the channel, or `None` if it is the identity.
    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L39-L93 (chrome/m156) (`ChannelMapper::build_lut`)
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // the values are in [0, 255]
    fn build_lut(&self, clip_info: &ClipInfo) -> Option<[u8; 256]> {
        let mut in_0 = self.in_black.get();
        let in_1 = self.in_white.get();
        let out_0 = self.out_black.get();
        let out_1 = self.out_white.get();
        let g = 1.0_f32 / std_max(self.gamma.get(), 0.0);

        let mut clip = [0.0_f32, 1.0];
        // kLottieDoClip
        if scalar_trunc_to_int(clip_info.clip_black.get()) == 1 {
            // Port of: `fOutBlack <= fOutWhite ? 0 : 1`: the negation keeps NaN at index 1.
            #[allow(clippy::neg_cmp_op_on_partial_ord)]
            let idx = usize::from(!(out_0 <= out_1));
            clip[idx] = t_pin(out_0, 0.0, 1.0);
        }
        if scalar_trunc_to_int(clip_info.clip_white.get()) == 1 {
            let idx = usize::from(out_0 <= out_1);
            clip[idx] = t_pin(out_1, 0.0, 1.0);
        }

        if f32::nearly_equal(in_0, out_0, None)
            && f32::nearly_equal(in_1, out_1, None)
            && f32::nearly_equal(g, 1.0, None)
        {
            return None;
        }

        let mut d_in = in_1 - in_0;
        let d_out = out_1 - out_0;
        if d_in.nearly_zero(None) {
            let epsilon = 2.0 * SCALAR_NEARLY_ZERO;
            d_in += epsilon.copysign(d_in);
            in_0 += epsilon.copysign(0.5 - in_0);
        }

        let mut t = -in_0 / d_in;
        let d_t = 1.0 / 255.0 / d_in;
        let mut lut = [0_u8; 256];
        for entry in &mut lut {
            let out = out_0 + d_out * scalar_pow(std_max(t, 0.0), g);
            *entry = (t_pin(out, clip[0], clip[1]) * 255.0).round() as u8;
            t += d_t;
        }
        Some(lut)
    }
}

/// The easy levels effect: one channel selection with one mapper.
// Port of: modules/skottie/src/effects/LevelsEffect.cpp#L95-L163 (chrome/m156) (`EasyLevelsEffectAdapter`)
struct EasyLevelsEffectAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    mapper: ChannelMapper,
    clip: ClipInfo,
    channel: Prop<ScalarValue>,
}

impl EasyLevelsEffectAdapter {
    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L97-L122 (chrome/m156) (`EasyLevelsEffectAdapter::EasyLevelsEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), external_color_filter(layer));
            let mapper = ChannelMapper::new();
            let clip = ClipInfo::new();
            let channel = Prop::new(1.0); // 1: RGB, 2: R, 3: G, 4: B, 5: A
            let binder = EffectBinder::new(jprops, abuilder, base.container());
            binder.bind(0, &channel);
            mapper.bind(&binder, 2);
            binder.bind(7, &clip.clip_black).bind(8, &clip.clip_white);
            Self {
                base,
                mapper,
                clip,
                channel,
            }
        })
    }
}

impl AnimatablePropertyContainer for EasyLevelsEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L124-L149 (chrome/m156) (`EasyLevelsEffectAdapter::onSync`)
    fn on_sync(&self) {
        const RGB_CHANNEL: i32 = 1;
        const R_CHANNEL: i32 = 2;
        const G_CHANNEL: i32 = 3;
        const B_CHANNEL: i32 = 4;
        const A_CHANNEL: i32 = 5;

        let channel = scalar_trunc_to_int(*self.channel.borrow());
        let lut = if (RGB_CHANNEL..=A_CHANNEL).contains(&channel) {
            self.mapper.build_lut(&self.clip)
        } else {
            None
        };
        let Some(lut) = lut else {
            self.base.node().set_color_filter(None);
            return;
        };

        let pick = |selected: bool| if selected { Some(&lut) } else { None };
        let filter = color_filters::table_argb(
            pick(channel == A_CHANNEL),
            pick(channel == R_CHANNEL || channel == RGB_CHANNEL),
            pick(channel == G_CHANNEL || channel == RGB_CHANNEL),
            pick(channel == B_CHANNEL || channel == RGB_CHANNEL),
        );
        self.base.node().set_color_filter(filter);
    }
}

impl_container_animator!(EasyLevelsEffectAdapter);

/// The pro levels effect: a master mapper (RGB) plus one mapper per channel (R, G, B, A).
// Port of: modules/skottie/src/effects/LevelsEffect.cpp#L165-L263 (chrome/m156) (`ProLevelsEffectAdapter`)
struct ProLevelsEffectAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    rgb_mapper: ChannelMapper,
    r_mapper: ChannelMapper,
    g_mapper: ChannelMapper,
    b_mapper: ChannelMapper,
    a_mapper: ChannelMapper,
    clip: ClipInfo,
}

impl ProLevelsEffectAdapter {
    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L171-L219 (chrome/m156) (`ProLevelsEffectAdapter::ProLevelsEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), external_color_filter(layer));
            let rgb_mapper = ChannelMapper::new();
            let r_mapper = ChannelMapper::new();
            let g_mapper = ChannelMapper::new();
            let b_mapper = ChannelMapper::new();
            let a_mapper = ChannelMapper::new();
            let clip = ClipInfo::new();
            // NB: the clip flags (indices 37, 38) are not bound, as in Skia.
            let binder = EffectBinder::new(jprops, abuilder, base.container());
            rgb_mapper.bind(&binder, 3);
            r_mapper.bind(&binder, 10);
            g_mapper.bind(&binder, 17);
            b_mapper.bind(&binder, 24);
            a_mapper.bind(&binder, 31);
            Self {
                base,
                rgb_mapper,
                r_mapper,
                g_mapper,
                b_mapper,
                a_mapper,
                clip,
            }
        })
    }
}

impl AnimatablePropertyContainer for ProLevelsEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/LevelsEffect.cpp#L221-L240 (chrome/m156) (`ProLevelsEffectAdapter::onSync`)
    fn on_sync(&self) {
        let a = self.a_mapper.build_lut(&self.clip);
        let r = self.r_mapper.build_lut(&self.clip);
        let g = self.g_mapper.build_lut(&self.clip);
        let b = self.b_mapper.build_lut(&self.clip);
        let mut cf = color_filters::table_argb(a.as_ref(), r.as_ref(), g.as_ref(), b.as_ref());
        if let Some(rgb) = self.rgb_mapper.build_lut(&self.clip) {
            let rgb_table = color_filters::table_argb(None, Some(&rgb), Some(&rgb), Some(&rgb));
            cf = color_filters::compose(rgb_table.as_ref(), cf);
        }
        self.base.node().set_color_filter(cf);
    }
}

impl_container_animator!(ProLevelsEffectAdapter);

/// The easy levels effect (`ADBE Easy Levels2`).
// Port of: modules/skottie/src/effects/LevelsEffect.cpp#L305-L311 (chrome/m156) (`EffectBuilder::attachEasyLevelsEffect`)
pub(super) fn attach_easy_levels_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = EasyLevelsEffectAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

/// The pro levels effect (`ADBE Pro Levels2`).
// Port of: modules/skottie/src/effects/LevelsEffect.cpp#L313-L319 (chrome/m156) (`EffectBuilder::attachProLevelsEffect`)
pub(super) fn attach_pro_levels_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let adapter = ProLevelsEffectAdapter::make(jprops, layer?, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
