// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/Effects.h, modules/skottie/src/effects/Effects.cpp
// (chrome/m156)
//
// The effect builder boundary. The layer builder calls `EffectBuilder::attach_effects` and
// `attach_styles` for the layer effects ("ef") and layer styles ("sy"). The effects themselves
// are M21: this module has the builder, its lookup and its dispatch loops, and an empty builder
// table. Until M21 fills `BUILDER_INFO` (alphabetized by effect name, `mn`) and `STYLE_BUILDERS`,
// every effect is reported as unsupported and left out, exactly as Skia treats an effect it does
// not know.

use std::rc::Rc;

use skia_rust_core::size::Size;
use skia_rust_sksg::RenderNode;

use crate::json::{ArrayValue, ObjectValue, Value};
use crate::skottie::LoggerLevel;
use crate::skottie_json::{ValueExt, parse_default, string_text};
use crate::skottie_property::NodeType;

use super::composition::CompositionBuilder;
use super::skottie_priv::{AnimationBuilder, AutoPropertyTracker};

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
const BUILDER_INFO: &[(&str, EffectBuilderFn)] = &[];

/// The layer style builders, by style type (`ty`): `None` for the styles that are not supported.
/// M21 adds them.
// Port of: modules/skottie/src/effects/Effects.cpp#L199-L208 (chrome/m156) (`gStyleBuilders`)
const STYLE_BUILDERS: &[Option<StyleBuilderFn>] = &[];

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
            if let Some((n, builder)) = BUILDER_INFO.get(idx) {
                if *n == name {
                    return Some(*builder);
                }
            }
        }

        // Some legacy clients rely solely on the 'ty' field and generate (non-BM) JSON without a
        // valid 'mn' string. The effects they name (tint, fill, tritone, drop shadow, radial
        // wipe, gaussian blur) are M21's: they are not in the table yet.
        let _ = parse_default::<i32>(jeffect.get("ty"), -1);

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
