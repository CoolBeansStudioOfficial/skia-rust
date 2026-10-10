// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/ShiftChannelsEffect.cpp (chrome/m156)
//
// The shift channels effect: each output channel of the layer is taken from one selectable source
// (a channel, the luminance, a constant), through a color matrix.

use std::rc::{Rc, Weak};

use skia_rust_core::color_data::{
    ITU_BT709_LUM_COEFF_B, ITU_BT709_LUM_COEFF_G, ITU_BT709_LUM_COEFF_R,
};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::t_pin::t_pin;
use skia_rust_sksg::color_filter::Coverage;
use skia_rust_sksg::{ExternalColorFilter, RenderNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_value::ScalarValue;

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder};

/// The sources of a channel: the 1-based selector values of the effect.
// Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L27-L37 (chrome/m156) (`Source`)
const SOURCE_ALPHA: f32 = 1.0;
const SOURCE_RED: f32 = 2.0;
const SOURCE_GREEN: f32 = 3.0;
const SOURCE_BLUE: f32 = 4.0;
const SOURCE_FULL_ON: f32 = 9.0;
const SOURCE_MAX: f32 = 10.0;

/// The coefficients of each source: the row of the color matrix of a channel (`gSourceCoeffs`).
// Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L56-L69 (chrome/m156) (`gSourceCoeffs`)
const SOURCE_COEFFS: [[f32; 5]; 10] = [
    [0.0, 0.0, 0.0, 1.0, 0.0], // kAlpha
    [1.0, 0.0, 0.0, 0.0, 0.0], // kRed
    [0.0, 1.0, 0.0, 0.0, 0.0], // kGreen
    [0.0, 0.0, 1.0, 0.0, 0.0], // kBlue
    [
        ITU_BT709_LUM_COEFF_R,
        ITU_BT709_LUM_COEFF_G,
        ITU_BT709_LUM_COEFF_B,
        0.0,
        0.0,
    ], // kLuminance
    [0.0, 0.0, 0.0, 0.0, 0.0], // TODO: kHue
    [0.0, 0.0, 0.0, 0.0, 0.0], // TODO: kLightness
    [0.0, 0.0, 0.0, 0.0, 0.0], // TODO: kSaturation
    [0.0, 0.0, 0.0, 0.0, 1.0], // kFullOn
    [0.0, 0.0, 0.0, 0.0, 0.0], // kFullOff
];

/// The coefficients of the source `src`, clamped to the source range.
// Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L72-L76 (chrome/m156) (`coeffs`)
#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // clamped to [1, 10]
fn source_coeffs(src: f32) -> &'static [f32; 5] {
    let src = t_pin(src, 1.0, SOURCE_MAX);
    &SOURCE_COEFFS[(src as usize) - 1]
}

/// Shifts the channels of the layer, from their sources.
// Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L17-L97 (chrome/m156) (`ShiftChannelsEffectAdapter`)
struct ShiftChannelsEffectAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    r: Prop<ScalarValue>,
    g: Prop<ScalarValue>,
    b: Prop<ScalarValue>,
    a: Prop<ScalarValue>,
}

impl ShiftChannelsEffectAdapter {
    // Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L31-L49 (chrome/m156) (`ShiftChannelsEffectAdapter::ShiftChannelsEffectAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: &Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let node =
                ExternalColorFilter::make(Some(Rc::clone(layer))).expect("the layer is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let r = Prop::new(SOURCE_RED);
            let g = Prop::new(SOURCE_GREEN);
            let b = Prop::new(SOURCE_BLUE);
            let a = Prop::new(SOURCE_ALPHA);
            EffectBinder::new(jprops, abuilder, base.container())
                .bind(1, &r)
                .bind(2, &g)
                .bind(3, &b)
                .bind(0, &a);
            Self { base, r, g, b, a }
        })
    }
}

impl AnimatablePropertyContainer for ShiftChannelsEffectAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L51-L84 (chrome/m156) (`ShiftChannelsEffectAdapter::onSync`)
    fn on_sync(&self) {
        let (a_value, r_value, g_value, b_value) = (
            *self.a.borrow(),
            *self.r.borrow(),
            *self.g.borrow(),
            *self.b.borrow(),
        );
        let rc = source_coeffs(r_value);
        let gc = source_coeffs(g_value);
        let bc = source_coeffs(b_value);
        let ac = source_coeffs(a_value);
        let cm: [f32; 20] = [
            rc[0], rc[1], rc[2], rc[3], rc[4], //
            gc[0], gc[1], gc[2], gc[3], gc[4], //
            bc[0], bc[1], bc[2], bc[3], bc[4], //
            ac[0], ac[1], ac[2], ac[3], ac[4],
        ];
        self.base
            .node()
            .set_color_filter(color_filters::matrix_row_major(&cm, Clamp::Yes));
        self.base.node().set_coverage(if a_value == SOURCE_FULL_ON {
            Coverage::BoundingBox
        } else {
            Coverage::Normal
        });
    }
}

impl_container_animator!(ShiftChannelsEffectAdapter);

/// The shift channels effect (`ADBE Shift Channels`).
// Port of: modules/skottie/src/effects/ShiftChannelsEffect.cpp#L99-L103 (chrome/m156) (`EffectBuilder::attachShiftChannelsEffect`)
pub(super) fn attach_shift_channels_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = ShiftChannelsEffectAdapter::make(jprops, &layer, eb.builder());
    eb.builder().attach_discardable_adapter(&adapter);
    let node = Rc::clone(adapter.base.node());
    Some(node as Rc<dyn RenderNode>)
}
