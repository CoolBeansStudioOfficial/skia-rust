// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/BlackAndWhiteEffect.cpp, SkSLEffect.cpp (chrome/m156)
//
// The runtime effect based color effects: black & white, and the SkSL color filter. The SkSL
// effects compile the program of the layer's "sh" property with the SkSL runtime effects, and
// bind its uniforms to the effect properties.

use std::rc::{Rc, Weak};

use skia_rust_core::data::Data;
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_sksg::{ExternalColorFilter, RenderNode};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_json::{ValueExt, parse_default, string_text};
use crate::skottie_value::{ScalarValue, VectorValue};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::AnimationBuilder;
use super::{EffectBinder, EffectBuilder, attach_adapter_node};

/// The black and white `SkSL`: the luminance of the color, weighted per hue sector.
// Port of: modules/skottie/src/effects/BlackAndWhiteEffect.cpp#L15-L31 (chrome/m156) (`BLACK_AND_WHITE_EFFECT`)
const BLACK_AND_WHITE_SKSL: &str = concat!(
    "uniform half kR, kY, kG, kC, kB, kM;",
    "half4 main(half4 c) {",
    "half m = min(min(c.r, c.g), c.b),",
    "dr = c.r - m,",
    "dg = c.g - m,",
    "db = c.b - m,",
    "wy = min(dr,dg),",
    "wc = min(dg,db),",
    "wm = min(db,dr),",
    "wr = dr - wy - wm,",
    "wg = dg - wy - wc,",
    "wb = db - wc - wm,",
    "l = m + kR*wr + kY*wy + kG*wg + kC*wc + kB*wb + kM*wm;",
    "return half4(l, l, l, c.a);",
    "}",
);

thread_local! {
    // Port of: modules/skottie/src/effects/BlackAndWhiteEffect.cpp#L44-L58 (chrome/m156) (`make_effect`)
    static BLACK_AND_WHITE_EFFECT: RuntimeEffect =
        RuntimeEffect::make_for_color_filter(BLACK_AND_WHITE_SKSL, None)
            .expect("the black and white effect compiles");
}

/// Converts the layer to black and white, with a weight per color sector.
// Port of: modules/skottie/src/effects/BlackAndWhiteEffect.cpp#L60-L121 (chrome/m156) (`BlackAndWhiteAdapter`)
struct BlackAndWhiteAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    coeffs: [Prop<ScalarValue>; 6],
}

impl BlackAndWhiteAdapter {
    // Port of: modules/skottie/src/effects/BlackAndWhiteEffect.cpp#L74-L101 (chrome/m156) (`BlackAndWhiteAdapter::BlackAndWhiteAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: &Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(
                weak.clone(),
                ExternalColorFilter::make(Some(Rc::clone(layer))).expect("the layer is not null"),
            );
            let coeffs = [
                Prop::new(0.0),
                Prop::new(0.0),
                Prop::new(0.0),
                Prop::new(0.0),
                Prop::new(0.0),
                Prop::new(0.0),
            ];
            let binder = EffectBinder::new(jprops, abuilder, base.container());
            for (index, coeff) in coeffs.iter().enumerate() {
                binder.bind(index, coeff);
            }
            Self { base, coeffs }
        })
    }
}

impl AnimatablePropertyContainer for BlackAndWhiteAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/BlackAndWhiteEffect.cpp#L103-L115 (chrome/m156) (`BlackAndWhiteAdapter::onSync`)
    fn on_sync(&self) {
        // 100-based
        let mut normalized = [0.0_f32; 6];
        for (dst, coeff) in normalized.iter_mut().zip(&self.coeffs) {
            *dst = *coeff.borrow() / 100.0;
        }
        let mut bytes = [0_u8; 24];
        for (chunk, value) in bytes.as_chunks_mut::<4>().0.iter_mut().zip(normalized) {
            chunk.copy_from_slice(&value.to_ne_bytes());
        }
        let filter = BLACK_AND_WHITE_EFFECT
            .with(|effect| effect.make_color_filter(Data::new_copy(&bytes), &[]));
        self.base.node().set_color_filter(filter);
    }
}

impl_container_animator!(BlackAndWhiteAdapter);

/// The black and white effect (`ADBE Black&White`).
// Port of: modules/skottie/src/effects/BlackAndWhiteEffect.cpp#L123-L129 (chrome/m156) (`EffectBuilder::attachBlackAndWhiteEffect`)
pub(super) fn attach_black_and_white_effect(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = BlackAndWhiteAdapter::make(jprops, &layer, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}

// ---- SkSL -------------------------------------------------------------------------------------

/// The "ty" of an `SkSL` property: a uniform (the default), an image child, or the layer content.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L56-L60 (chrome/m156) (`kSkSLProp_*`)
const SKSL_PROP_UNIFORM: i32 = 0;
const SKSL_PROP_IMAGE: i32 = 98;
const SKSL_PROP_LAYER: i32 = 99;

/// The index of the `SkSL` program in the effect properties, and of the first uniform.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L49-L53 (chrome/m156) (`kSkSL_index`, `kFirstUniform_index`)
const SKSL_INDEX: usize = 0;
const FIRST_UNIFORM_INDEX: usize = 1;

/// The `SkSL` program of an effect, and the uniforms bound to its properties.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L120-L199 (chrome/m156) (`SkSLEffectBase`)
struct SkSlEffectBase {
    effect: Option<RuntimeEffect>,
    uniforms: Vec<(String, Prop<VectorValue>)>,
}

impl SkSlEffectBase {
    /// Compiles the program of `jprops` and binds its uniforms to `container`.
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L120-L143 (chrome/m156) (`SkSLEffectBase::SkSLEffectBase`)
    fn new(
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        container: &PropertyContainer,
    ) -> Self {
        let effect = Self::compile(jprops, abuilder);
        let mut this = Self {
            effect,
            uniforms: Vec::new(),
        };
        this.bind_uniforms(jprops, abuilder, container);
        this
    }

    /// The `SkSL` program of the "sh" property, or `None` if it is missing or does not compile.
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L126-L142 (chrome/m156)
    fn compile(jprops: &ArrayValue, abuilder: &AnimationBuilder<'_>) -> Option<RuntimeEffect> {
        if jprops.size() < 1 {
            return None;
        }
        let jsksl = jprops[SKSL_INDEX].as_object()?;
        let jshader = jsksl.get("sh").as_string()?;
        let shader = string_text(jshader);
        match RuntimeEffect::make_for_shader(&shader, None) {
            Ok(effect) => Some(effect),
            Err(error) => {
                abuilder.log(
                    crate::skottie::LoggerLevel::Error,
                    &format!("Failed to parse SkSL shader: {error}"),
                );
                None
            }
        }
    }

    /// Binds the uniform properties: a vector per uniform name (`bindUniforms`).
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L145-L199 (chrome/m156) (`SkSLEffectBase::bindUniforms`)
    fn bind_uniforms(
        &mut self,
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        container: &PropertyContainer,
    ) {
        for i in FIRST_UNIFORM_INDEX..jprops.size() {
            let Some(jprop) = jprops[i].as_object() else {
                continue;
            };
            let Some(uniform_name) = jprop.get("nm").as_string() else {
                continue;
            };
            let name = string_text(uniform_name);
            let ty = parse_default::<i32>(jprop.get("ty"), SKSL_PROP_UNIFORM);
            if (ty == SKSL_PROP_IMAGE || ty == SKSL_PROP_LAYER)
                && self
                    .effect
                    .as_ref()
                    .is_none_or(|effect| effect.find_child(&name).is_none())
            {
                // Ignoring an undeclared SkSL child.
                continue;
            }
            if ty == SKSL_PROP_UNIFORM {
                let value = Prop::new(VectorValue::new());
                container.bind(abuilder, jprop.get("v"), &value);
                self.uniforms.push((name, value));
            }
            // The image and layer children are bound by the shader effect (`buildChildrenData`);
            // the color filter has no children.
        }
    }

    /// The uniform bytes of the effect (`buildUniformData`): each uniform is copied to its offset
    /// when its count matches.
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L252-L269 (chrome/m156) (`buildUniformData`)
    fn build_uniform_data(&self, effect: &RuntimeEffect) -> Vec<u8> {
        let mut data = vec![0_u8; effect.uniform_size()];
        for (name, value) in &self.uniforms {
            let value = value.borrow();
            let Some(metadata) = effect.find_uniform(name) else {
                continue;
            };
            if usize::try_from(metadata.count()).ok() != Some(value.len()) {
                continue;
            }
            for (i, component) in value.iter().enumerate() {
                let start = metadata.offset() + i * std::mem::size_of::<f32>();
                data[start..start + std::mem::size_of::<f32>()]
                    .copy_from_slice(&component.to_ne_bytes());
            }
        }
        data
    }
}

/// The color filter of an `SkSL` program.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L271-L296 (chrome/m156) (`SkSLColorFilterAdapter`)
struct SkSlColorFilterAdapter {
    base: DiscardableAdapterBase<ExternalColorFilter>,
    sksl: SkSlEffectBase,
}

impl SkSlColorFilterAdapter {
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L275-L282 (chrome/m156) (`SkSLColorFilterAdapter::SkSLColorFilterAdapter`)
    fn make(
        jprops: &ArrayValue,
        layer: &Rc<dyn RenderNode>,
        abuilder: &AnimationBuilder<'_>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let node =
                ExternalColorFilter::make(Some(Rc::clone(layer))).expect("the layer is not null");
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let sksl = SkSlEffectBase::new(jprops, abuilder, base.container());
            Self { base, sksl }
        })
    }
}

impl AnimatablePropertyContainer for SkSlColorFilterAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L284-L292 (chrome/m156) (`SkSLColorFilterAdapter::onSync`)
    fn on_sync(&self) {
        let Some(effect) = &self.sksl.effect else {
            return;
        };
        let data = self.sksl.build_uniform_data(effect);
        let filter = effect.make_color_filter(Data::new_copy(&data), &[]);
        self.base.node().set_color_filter(filter);
    }
}

impl_container_animator!(SkSlColorFilterAdapter);

/// The `SkSL` color filter effect (`SkSL Color Filter`).
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L302-L309 (chrome/m156) (`EffectBuilder::attachSkSLColorFilter`)
pub(super) fn attach_sksl_color_filter(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let adapter = SkSlColorFilterAdapter::make(jprops, &layer, eb.builder());
    let node = Rc::clone(adapter.base.node());
    Some(attach_adapter_node(eb.builder(), &adapter, node))
}
