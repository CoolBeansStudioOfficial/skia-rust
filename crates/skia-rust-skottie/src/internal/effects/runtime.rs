// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/src/effects/BlackAndWhiteEffect.cpp, SkSLEffect.cpp (chrome/m156)
//
// The runtime effect based color effects: black & white, and the SkSL color filter. The SkSL
// effects compile the program of the layer's "sh" property with the SkSL runtime effects, and
// bind its uniforms to the effect properties.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::data::Data;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeEffect};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_sksg::invalidation_controller::InvalidationController;
use skia_rust_sksg::render_node::{Hit, has_children_inval};
use skia_rust_sksg::{
    ExternalColorFilter, Node, NodeCore, RenderContext, RenderNode, ScopedRenderContext,
};

use crate::impl_container_animator;
use crate::json::ArrayValue;
use crate::skottie_json::{ValueExt, parse_default, string_text};
use crate::skottie_value::{ScalarValue, VectorValue};

use super::super::animator::{
    AnimatablePropertyContainer, DiscardableAdapterBase, Prop, PropertyContainer,
};
use super::super::skottie_priv::{AnimationBuilder, ScopedAssetRef};
use super::{EffectBinder, EffectBuilder, attach_adapter_node, repeating_content_shader};
use crate::skottie::LoggerLevel;

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
    children: Vec<ChildData>,
}

/// A child of the `SkSL` program bound to a property: an image (its shader), or the layer content.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L80-L86 (chrome/m156) (`SkSLEffectBase::ChildData`)
struct ChildData {
    ty: i32,
    name: String,
    child: ChildPtr,
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
            children: Vec::new(),
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
                self.uniforms.push((name.clone(), value));
            }
            if ty == SKSL_PROP_IMAGE {
                self.bind_image(jprop, abuilder, name);
            } else if ty == SKSL_PROP_LAYER {
                self.children.push(ChildData {
                    ty,
                    name,
                    child: ChildPtr::Empty,
                });
            }
        }
    }

    /// Binds an image child to the first frame of its footage asset, as a linear shader.
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L165-L182 (chrome/m156) (`SkSLEffectBase::bindUniforms`, image children)
    fn bind_image(
        &mut self,
        jprop: &crate::json::ObjectValue,
        abuilder: &AnimationBuilder<'_>,
        name: String,
    ) {
        let Some(jimage_ref) = jprop.get("v").as_object() else {
            return;
        };
        let footage_asset = ScopedAssetRef::new(abuilder, jimage_ref);
        let asset_info = footage_asset
            .asset()
            .and_then(|jasset| abuilder.load_footage_asset(jasset));
        match asset_info {
            Some(info) => {
                let frame_data = info.asset.get_frame_data(0.0);
                let sampling = SamplingOptions::from(FilterMode::Linear);
                let child = frame_data
                    .image
                    .and_then(|image| {
                        image.to_shader(None::<(TileMode, TileMode)>, sampling, None::<&Matrix>)
                    })
                    .map_or(ChildPtr::Empty, ChildPtr::from);
                self.children.push(ChildData {
                    ty: SKSL_PROP_IMAGE,
                    name,
                    child,
                });
            }
            None => abuilder.log(
                LoggerLevel::Warning,
                "cannot find asset for custom shader effect",
            ),
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

/// The `SkSL` shader node: it fills its layer with the shader of the effect.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L56-L114 (chrome/m156) (`SkSLShaderNode`)
#[derive(Debug)]
pub(super) struct SkSlShaderNode {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    content_size: Size,
    // Cached shaders.
    effect_shader: RefCell<Option<Shader>>,
    content_shader: RefCell<Option<Shader>>,
}

impl SkSlShaderNode {
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L60-L62 (chrome/m156) (`SkSLShaderNode::SkSLShaderNode`)
    fn make(child: &Rc<dyn RenderNode>, content_size: Size) -> Rc<Self> {
        let node = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(child),
            content_size,
            effect_shader: RefCell::new(None),
            content_shader: RefCell::new(None),
        });
        // The custom node observes its child.
        node.observe_inval(node.child.as_ref());
        node
    }

    /// Sets the effect shader, invalidating the node if it changed (`setShader`).
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L70-L72 (chrome/m156) (`SG_ATTRIBUTE(Shader)`)
    pub(super) fn set_shader(&self, shader: Option<Shader>) {
        if *self.effect_shader.borrow() != shader {
            *self.effect_shader.borrow_mut() = shader;
            self.invalidate();
        }
    }

    /// The layer content as a repeating picture shader (`contentShader`).
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L60-L75 (chrome/m156) (`SkSLShaderNode::contentShader`)
    pub(super) fn content_shader(&self) -> Option<Shader> {
        if self.content_shader.borrow().is_none()
            || has_children_inval(std::slice::from_ref(&self.child))
        {
            *self.content_shader.borrow_mut() =
                repeating_content_shader(&self.child, self.content_size);
        }
        self.content_shader.borrow().clone()
    }
}

impl Drop for SkSlShaderNode {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for SkSlShaderNode {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L84-L87 (chrome/m156) (`SkSLShaderNode::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        self.child.revalidate(ic, ctm)
    }
}

impl RenderNode for SkSlShaderNode {
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L88-L100 (chrome/m156) (`SkSLShaderNode::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let bounds = self.core().bounds();
        let scope = ScopedRenderContext::new(canvas, ctx).set_isolation(
            &bounds,
            &canvas.total_matrix(),
            true,
        );
        canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
        self.child.render(canvas, Some(scope.context()));

        let mut effect_paint = Paint::default();
        effect_paint.set_shader(self.effect_shader.borrow().clone());
        effect_paint.set_blend_mode(skia_rust_core::blend_mode::BlendMode::SrcIn);
        canvas.draw_paint(&effect_paint);
    }

    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L101 (chrome/m156) (`SkSLShaderNode::onNodeAt`)
    fn on_node_at(&self, _p: Point) -> Option<Hit> {
        // no hit-testing
        None
    }
}

/// The `SkSL` shader adapter: the uniforms and children of the program make the shader.
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L232-L262 (chrome/m156) (`SkSLShaderAdapter`)
struct SkSlShaderAdapter {
    base: DiscardableAdapterBase<SkSlShaderNode>,
    sksl: SkSlEffectBase,
}

impl SkSlShaderAdapter {
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L234-L245 (chrome/m156) (`SkSLShaderAdapter::SkSLShaderAdapter`)
    fn make(
        jprops: &ArrayValue,
        abuilder: &AnimationBuilder<'_>,
        node: Rc<SkSlShaderNode>,
    ) -> Rc<Self> {
        Rc::new_cyclic(|weak: &Weak<Self>| {
            let base = DiscardableAdapterBase::new(weak.clone(), node);
            let sksl = SkSlEffectBase::new(jprops, abuilder, base.container());
            Self { base, sksl }
        })
    }

    /// The children of the program (`buildChildrenData`): the layer content, or the image shader.
    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L213-L229 (chrome/m156) (`SkSLEffectBase::buildChildrenData`)
    fn build_children_data(
        effect: &RuntimeEffect,
        children: &[ChildData],
        node: &SkSlShaderNode,
    ) -> Vec<ChildPtr> {
        let mut children_data = vec![ChildPtr::Empty; effect.children().len()];
        for child_data in children {
            // Undeclared children are skipped (Skia logs them).
            let Some(metadata) = effect.find_child(&child_data.name) else {
                continue;
            };
            if child_data.ty == SKSL_PROP_LAYER {
                children_data[metadata.index()] = node
                    .content_shader()
                    .map_or(ChildPtr::Empty, ChildPtr::from);
            } else if child_data.ty == SKSL_PROP_IMAGE {
                children_data[metadata.index()] = child_data.child.clone();
            }
        }
        children_data
    }
}

impl AnimatablePropertyContainer for SkSlShaderAdapter {
    fn container(&self) -> &PropertyContainer {
        self.base.container()
    }

    // Port of: modules/skottie/src/effects/SkSLEffect.cpp#L247-L258 (chrome/m156) (`SkSLShaderAdapter::onSync`)
    fn on_sync(&self) {
        let Some(effect) = &self.sksl.effect else {
            return;
        };
        let uniforms = Data::new_copy(&self.sksl.build_uniform_data(effect));
        let children = Self::build_children_data(effect, &self.sksl.children, self.base.node());
        let shader = effect.make_shader(uniforms, &children, None::<&Matrix>);
        self.base.node().set_shader(shader);
    }
}

impl_container_animator!(SkSlShaderAdapter);

/// The `SkSL` shader effect (`SkSL Shader`).
// Port of: modules/skottie/src/effects/SkSLEffect.cpp#L290-L296 (chrome/m156) (`EffectBuilder::attachSkSLShader`)
pub(super) fn attach_sksl_shader(
    eb: &EffectBuilder<'_, '_>,
    jprops: &ArrayValue,
    layer: Option<Rc<dyn RenderNode>>,
) -> Option<Rc<dyn RenderNode>> {
    let layer = layer?;
    let shader_node = SkSlShaderNode::make(&layer, eb.layer_size());
    let adapter = SkSlShaderAdapter::make(jprops, eb.builder(), Rc::clone(&shader_node));
    Some(attach_adapter_node(eb.builder(), &adapter, shader_node))
}
