// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/Effects.h, modules/skottie/src/effects/Effects.cpp
// (chrome/m156)
//
// The effect builder. The layer builder calls `EffectBuilder::attach_effects` and
// `attach_styles` for the layer effects ("ef") and layer styles ("sy"). The effect builders are
// in the submodules: `color` (fill, tint, tritone, invert, threshold, hue/saturation, levels).
// Each effect not yet ported is left out of `BUILDER_INFO` and reported as unsupported, exactly
// as Skia treats an effect it does not know.

use std::rc::Rc;

use skia_rust_core::matrix::Matrix;
use skia_rust_core::picture::Picture;
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::{MaskShaderEffect, RenderNode};

use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie::LoggerLevel;
use crate::skottie_json::{ValueExt, parse_default, string_text};
use crate::skottie_property::NodeType;

use super::animator::{
    AnimatablePropertyContainer, Bindable, IntoJsonProp, Prop, PropertyContainer,
};
use super::composition::CompositionBuilder;
use super::skottie_priv::{AnimationBuilder, AutoPropertyTracker};

mod cc_toner;
mod color;
mod displacement_map;
mod fractal_noise;
mod convolution;
mod corner_pin;
mod filters;
mod gradient_ramp;
mod linear_wipe;
mod radial_wipe;
mod runtime;
mod shift_channels;
mod styles;
mod transform_effect;
mod venetian_blinds;

/// Attaches an adapter (`attachDiscardableAdapter`) and returns its node.
// Port of: modules/skottie/src/SkottiePriv.h#L168-L181 (chrome/m156) (`attachDiscardableAdapter<T>`)
fn attach_adapter_node<A, N>(
    abuilder: &AnimationBuilder<'_>,
    adapter: &Rc<A>,
    node: Rc<N>,
) -> Rc<dyn RenderNode>
where
    A: AnimatablePropertyContainer + 'static,
    N: RenderNode + 'static,
{
    abuilder.attach_discardable_adapter(adapter);
    node as Rc<dyn RenderNode>
}

/// The mask of a mask-shader effect: its shader, and whether the layer is visible
/// (`MaskShaderEffectBase::MaskInfo`).
// Port of: modules/skottie/src/effects/Effects.h#L162-L165 (chrome/m156) (`MaskShaderEffectBase::MaskInfo`)
pub(super) struct MaskInfo {
    /// The mask shader, or `None` for no mask.
    pub(super) shader: Option<Shader>,
    /// False if the layer is fully hidden.
    pub(super) visible: bool,
}

/// The mask-shader node that masks the layer: the `MaskShaderEffect` of the base class.
// Port of: modules/skottie/src/effects/Effects.cpp#L207-L209 (chrome/m156) (`MaskShaderEffectBase::MaskShaderEffectBase`)
pub(super) fn make_mask_shader_node(layer: Rc<dyn RenderNode>) -> Rc<MaskShaderEffect> {
    MaskShaderEffect::make(Some(layer), None).expect("the layer is not null")
}

/// Records the content of a node into a picture (`get_content_picture` of the displacement and
/// bulge effects): the node is revalidated, and rendered into a recording of its bounds.
// Port of: modules/skottie/src/effects/DisplacementMapEffect.cpp#L144-L153 (chrome/m156) (`get_content_picture`)
pub(super) fn get_content_picture(
    node: Option<&Rc<dyn RenderNode>>,
    ic: Option<&mut InvalidationController>,
    ctm: &Matrix,
) -> Option<Picture> {
    let node = node?;
    let bounds = node.revalidate(ic, ctm);
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(bounds, false);
    node.render(canvas, None);
    recorder.finish_recording_as_picture(None)
}

/// Pushes a mask to its node (`MaskShaderEffectBase::onSync`).
// Port of: modules/skottie/src/effects/Effects.cpp#L211-L217 (chrome/m156) (`MaskShaderEffectBase::onSync`)
pub(super) fn sync_mask_shader(node: &MaskShaderEffect, info: MaskInfo) {
    node.set_visible(info.visible);
    node.set_shader(info.shader);
}

/// The syntactic helper that binds the properties of an effect by index (`EffectBinder`).
// Port of: modules/skottie/src/effects/Effects.h#L87-L108 (chrome/m156) (`class EffectBinder`)
pub(super) struct EffectBinder<'a, 'j, 'c> {
    jprops: &'a ArrayValue,
    abuilder: &'a AnimationBuilder<'j>,
    container: &'c PropertyContainer,
}

impl<'a, 'j, 'c> EffectBinder<'a, 'j, 'c> {
    /// A binder of the properties `jprops`, for the container `container`.
    // Port of: modules/skottie/src/effects/Effects.h#L87-L96 (chrome/m156) (`EffectBinder::EffectBinder`)
    pub(super) fn new(
        jprops: &'a ArrayValue,
        abuilder: &'a AnimationBuilder<'j>,
        container: &'c PropertyContainer,
    ) -> Self {
        Self {
            jprops,
            abuilder,
            container,
        }
    }

    /// Binds the property at `prop_index` to `target`.
    // Port of: modules/skottie/src/effects/Effects.h#L98-L104 (chrome/m156) (`EffectBinder::bind`)
    pub(super) fn bind<T: Bindable>(&self, prop_index: usize, target: &Prop<T>) -> &Self {
        let jprop = EffectBuilder::get_prop_value(self.jprops, prop_index);
        self.container
            .bind(self.abuilder, jprop.into_prop(), target);
        self
    }
}

/// The function that attaches one effect: the effect properties and the layer to apply it to.
// Port of: modules/skottie/src/effects/Effects.h#L61-L62 (chrome/m156) (`EffectBuilder::EffectBuilderT`)
pub type EffectBuilderFn = for<'a, 'j> fn(
    &EffectBuilder<'a, 'j>,
    &ArrayValue,
    Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>>;

/// The function that attaches one layer style.
// Port of: modules/skottie/src/effects/Effects.cpp#L196-L198 (chrome/m156) (`StyleBuilder`)
pub type StyleBuilderFn = for<'a, 'j> fn(
    &EffectBuilder<'a, 'j>,
    &ObjectValue,
    Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>>;

/// The supported effects, by name (`mn`), alphabetized for binary search lookup. M21 adds them.
// Port of: modules/skottie/src/effects/Effects.cpp#L31-L63 (chrome/m156) (`gBuilderInfo`)
const BUILDER_INFO: &[(&str, EffectBuilderFn)] = &[
    // alphabetized for binary search lookup
    ("ADBE Black&White", runtime::attach_black_and_white_effect),
    (
        "ADBE Brightness & Contrast 2",
        color::attach_brightness_contrast_effect,
    ),
    ("ADBE Corner Pin", corner_pin::attach_corner_pin_effect),
    (
        "ADBE Displacement Map",
        displacement_map::attach_displacement_map_effect,
    ),
    ("ADBE Drop Shadow", filters::attach_drop_shadow_effect),
    ("ADBE Easy Levels2", color::attach_easy_levels_effect),
    ("ADBE Fill", color::attach_fill_effect),
    ("ADBE Fractal Noise", fractal_noise::attach_fractal_noise_effect),
    ("ADBE Gaussian Blur 2", filters::attach_gaussian_blur_effect),
    ("ADBE Geometry2", transform_effect::attach_transform_effect),
    ("ADBE HUE SATURATION", color::attach_hue_saturation_effect),
    ("ADBE Invert", color::attach_invert_effect),
    ("ADBE Linear Wipe", linear_wipe::attach_linear_wipe_effect),
    (
        "ADBE Motion Blur",
        convolution::attach_directional_blur_effect,
    ),
    ("ADBE Pro Levels2", color::attach_pro_levels_effect),
    ("ADBE Radial Wipe", radial_wipe::attach_radial_wipe_effect),
    ("ADBE Ramp", gradient_ramp::attach_gradient_effect),
    ("ADBE Sharpen", convolution::attach_sharpen_effect),
    (
        "ADBE Shift Channels",
        shift_channels::attach_shift_channels_effect,
    ),
    ("ADBE Threshold2", color::attach_threshold_effect),
    ("ADBE Tint", color::attach_tint_effect),
    ("ADBE Tritone", color::attach_tritone_effect),
    (
        "ADBE Venetian Blinds",
        venetian_blinds::attach_venetian_blinds_effect,
    ),
    ("CC Toner", cc_toner::attach_cc_toner_effect),
    ("SkSL Color Filter", runtime::attach_sksl_color_filter),
];

/// The legacy effect types (`ty`) of the clients that do not name the effect (`mn`).
// Port of: modules/skottie/src/effects/Effects.cpp#L50-L60 (chrome/m156) (`kTint_Effect` etc.)
const LEGACY_TINT_EFFECT: i32 = 20;
const LEGACY_FILL_EFFECT: i32 = 21;
const LEGACY_TRITONE_EFFECT: i32 = 23;
const LEGACY_RADIAL_WIPE_EFFECT: i32 = 26;
const LEGACY_DROP_SHADOW_EFFECT: i32 = 25;
const LEGACY_GAUSSIAN_BLUR_EFFECT: i32 = 29;

/// The layer style builders, by style type (`ty`): `None` for the styles that are not supported.
// Port of: modules/skottie/src/effects/Effects.cpp#L199-L208 (chrome/m156) (`gStyleBuilders`)
const STYLE_BUILDERS: &[Option<StyleBuilderFn>] = &[
    None,                                     // 'ty': 0 -> stroke
    Some(styles::attach_drop_shadow_style),   // 'ty': 1 -> drop shadow
    Some(styles::attach_inner_shadow_style),  // 'ty': 2 -> inner shadow
    Some(styles::attach_outer_glow_style),    // 'ty': 3 -> outer glow
    Some(styles::attach_inner_glow_style),    // 'ty': 4 -> inner glow
    None,                                     // 'ty': 5 -> bevel/emboss
    None,                                     // 'ty': 6 -> satin
    Some(styles::attach_color_overlay_style), // 'ty': 7 -> color overlay
];

/// A layer content tree and its size.
// Port of: modules/skottie/src/effects/Effects.h#L50-L53 (chrome/m156) (`EffectBuilder::LayerContent`)
#[derive(Clone)]
pub struct LayerContent {
    /// The content tree.
    pub content: Option<Rc<dyn RenderNode>>,
    /// The size of the layer.
    pub size: Size,
}

/// Attaches the effects and styles of a layer.
// Port of: modules/skottie/src/effects/Effects.h#L27-L59 (chrome/m156) (`class EffectBuilder`)
#[doc(alias = "skottie::internal::EffectBuilder")]
pub struct EffectBuilder<'a, 'j> {
    builder: &'a AnimationBuilder<'j>,
    comp_builder: &'a CompositionBuilder<'j>,
    layer_size: Size,
}

impl<'a, 'j> EffectBuilder<'a, 'j> {
    /// A builder of the effects of a layer of the given size.
    // Port of: modules/skottie/src/effects/Effects.cpp#L20-L26 (chrome/m156)
    #[must_use]
    pub fn new(
        abuilder: &'a AnimationBuilder<'j>,
        layer_size: Size,
        cbuilder: &'a CompositionBuilder<'j>,
    ) -> Self {
        Self {
            builder: abuilder,
            comp_builder: cbuilder,
            layer_size,
        }
    }

    /// The animation builder.
    #[must_use]
    pub fn builder(&self) -> &'a AnimationBuilder<'j> {
        self.builder
    }

    /// The size of the layer the effects apply to.
    #[must_use]
    pub fn layer_size(&self) -> Size {
        self.layer_size
    }

    /// The builder of the effect `jeffect`, or `None` (and a warning) if it is not supported.
    // Port of: modules/skottie/src/effects/Effects.cpp#L28-L103 (chrome/m156) (`findBuilder`)
    fn find_builder(&self, jeffect: &ObjectValue) -> Option<EffectBuilderFn> {
        let mn = jeffect.get("mn").as_string();
        if let Some(mn) = mn {
            let name = string_text(mn);
            // lower_bound over the alphabetized table.
            let idx = BUILDER_INFO.partition_point(|(n, _)| n.as_bytes() < name.as_bytes());
            if let Some((n, builder)) = BUILDER_INFO.get(idx)
                && *n == name
            {
                return Some(*builder);
            }
        }

        // Some legacy clients rely solely on the 'ty' field and generate (non-BM) JSON without a
        // valid 'mn' string.
        let legacy: Option<EffectBuilderFn> = match parse_default::<i32>(jeffect.get("ty"), -1) {
            LEGACY_TINT_EFFECT => Some(color::attach_tint_effect),
            LEGACY_FILL_EFFECT => Some(color::attach_fill_effect),
            LEGACY_TRITONE_EFFECT => Some(color::attach_tritone_effect),
            LEGACY_RADIAL_WIPE_EFFECT => Some(radial_wipe::attach_radial_wipe_effect),
            LEGACY_DROP_SHADOW_EFFECT => Some(filters::attach_drop_shadow_effect),
            LEGACY_GAUSSIAN_BLUR_EFFECT => Some(filters::attach_gaussian_blur_effect),
            _ => None,
        };
        if legacy.is_some() {
            return legacy;
        }

        self.builder.log_json(
            LoggerLevel::Warning,
            jeffect,
            &format!(
                "Unsupported layer effect: {}",
                mn.map_or_else(|| "(unknown)".to_string(), string_text)
            ),
        );

        None
    }

    /// Attaches the effects of `jeffects` to the layer.
    // Port of: modules/skottie/src/effects/Effects.cpp#L105-L128 (chrome/m156) (`attachEffects`)
    #[must_use]
    pub fn attach_effects(
        &self,
        jeffects: &ArrayValue,
        layer: Option<Rc<dyn RenderNode>>,
    ) -> Option<Rc<dyn RenderNode>> {
        let mut layer = layer?;

        for i in 0..jeffects.size() {
            let Some(jeffect) = jeffects[i].as_object() else {
                continue;
            };

            let builder = self.find_builder(jeffect);
            let jprops = jeffect.get("ef").as_array();
            let (Some(builder), Some(jprops)) = (builder, jprops) else {
                continue;
            };

            let _apt = AutoPropertyTracker::new(self.builder, jeffect, NodeType::Effect);
            let Some(attached) = builder(self, jprops, Some(layer)) else {
                self.builder
                    .log_json(LoggerLevel::Error, jeffect, "Invalid layer effect.");
                return None;
            };
            layer = attached;
        }

        Some(layer)
    }

    /// Attaches the layer styles of `jstyles` to the layer.
    // Port of: modules/skottie/src/effects/Effects.cpp#L130-L168 (chrome/m156) (`attachStyles`)
    #[must_use]
    pub fn attach_styles(
        &self,
        jstyles: &ArrayValue,
        layer: Option<Rc<dyn RenderNode>>,
    ) -> Option<Rc<dyn RenderNode>> {
        let mut layer = layer?;

        for i in 0..jstyles.size() {
            let Some(jstyle) = jstyles[i].as_object() else {
                continue;
            };

            let style_type = parse_default::<usize>(jstyle.get("ty"), usize::MAX);
            let builder = STYLE_BUILDERS.get(style_type).copied().flatten();

            let Some(builder) = builder else {
                self.builder
                    .log_json(LoggerLevel::Warning, jstyle, "Unsupported layer style.");
                continue;
            };

            let Some(attached) = builder(self, jstyle, Some(Rc::clone(&layer))) else {
                continue;
            };
            layer = attached;
        }

        Some(layer)
    }

    /// The value of the property at `prop_index` of an effect (`GetPropValue`), or null.
    // Port of: modules/skottie/src/effects/Effects.cpp#L170-L180 (chrome/m156)
    #[must_use]
    pub fn get_prop_value(jprops: &ArrayValue, prop_index: usize) -> &Value {
        static NULL: Value = Value::Null(crate::json::NullValue);

        if prop_index >= jprops.size() {
            return &NULL;
        }

        match jprops[prop_index].as_object() {
            Some(jprop) => jprop.get("v"),
            None => &NULL,
        }
    }

    /// The content tree and size of the layer at `layer_index` of the composition.
    // Port of: modules/skottie/src/effects/Effects.cpp#L182-L188 (chrome/m156) (`getLayerContent`)
    #[must_use]
    pub fn get_layer_content(&self, layer_index: i32) -> LayerContent {
        if let Some(lbuilder) = self.comp_builder.layer_builder(layer_index) {
            return LayerContent {
                content: lbuilder.get_content_tree(self.builder, self.comp_builder),
                size: lbuilder.size(),
            };
        }

        LayerContent {
            content: None,
            size: Size::new(0.0, 0.0),
        }
    }
}

opaque_debug!(EffectBuilder<'a, 'j>, LayerContent);
