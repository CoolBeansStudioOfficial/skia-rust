// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGRenderEffect.h (the `Shader`, `ShaderEffect` and
// `MaskShaderEffect` parts), modules/sksg/src/SkSGRenderEffect.cpp (chrome/m156)

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use skia_rust_core::canvas::Canvas;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader as SkShader;

use crate::effect_node::{effect_on_node_at, effect_on_render, effect_on_revalidate};
use crate::invalidation_controller::InvalidationController;
use crate::node::{Node, NodeCore, inval_traits};
use crate::render_node::{Hit, RenderContext, RenderNode, ScopedRenderContext};

/// The state of a shader node: the revalidated shader (`Shader::fShader`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L39-L56 (chrome/m156) (`class Shader` fields)
#[derive(Debug, Default)]
pub struct ShaderState {
    shader: RefCell<Option<SkShader>>,
}

/// A node that produces a shader (`sksg::Shader`). The shader is recomputed on revalidation by
/// [`ShaderNode::on_revalidate_shader`].
// Port of: modules/sksg/include/SkSGRenderEffect.h#L39-L56 (chrome/m156) (`class Shader`)
#[doc(alias = "sksg::Shader")]
pub trait ShaderNode: Node {
    /// The state holding the revalidated shader.
    fn shader_state(&self) -> &ShaderState;

    /// Produces the shader from the node's current properties (`onRevalidateShader`).
    fn on_revalidate_shader(&self) -> Option<SkShader>;

    /// The revalidated shader (`getShader`).
    // Port of: modules/sksg/include/SkSGRenderEffect.h#L43-L47 (chrome/m156) (`Shader::getShader`)
    #[doc(alias = "getShader")]
    fn shader(&self) -> Option<SkShader> {
        debug_assert!(!self.core().has_inval());
        self.shader_state().shader.borrow().clone()
    }
}

/// `Shader::onRevalidate`: stores the revalidated shader. A shader has no bounds of its own.
// Port of: modules/sksg/src/SkSGRenderEffect.cpp#L90-L95 (chrome/m156) (`Shader::onRevalidate`)
pub fn shader_on_revalidate(state: &ShaderState, shader: Option<SkShader>) -> Rect {
    *state.shader.borrow_mut() = shader;
    Rect::new_empty()
}

/// The traits of shader nodes: their damage bubbles up to the paints that use them.
// Port of: modules/sksg/src/SkSGRenderEffect.cpp#L86-L86 (chrome/m156) (`Shader::Shader`)
pub const SHADER_TRAITS: u32 = inval_traits::BUBBLE_DAMAGE;

/// Overrides the shader of the paint of its child (`ShaderEffect`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L58-L70 (chrome/m156) (`class ShaderEffect`)
#[doc(alias = "sksg::ShaderEffect")]
#[derive(Debug)]
pub struct ShaderEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    shader: RefCell<Option<Rc<dyn ShaderNode>>>,
}

impl ShaderEffect {
    /// `ShaderEffect::Make(child, shader)`: `None` if there is no child.
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L41-L44 (chrome/m156) (`ShaderEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        child: Option<Rc<dyn RenderNode>>,
        shader: Option<Rc<dyn ShaderNode>>,
    ) -> Option<Rc<Self>> {
        let child = child?;
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            shader: RefCell::new(shader.clone()),
        });
        // The EffectNode base observes the child, then ShaderEffect observes the shader.
        effect.observe_inval(effect.child.as_ref());
        if let Some(shader) = shader {
            effect.observe_inval(shader.as_ref());
        }
        Some(effect)
    }

    /// Replaces the shader, moving the invalidation observers to the new one (`setShader`).
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L60-L69 (chrome/m156) (`ShaderEffect::setShader`)
    #[doc(alias = "setShader")]
    pub fn set_shader(&self, shader: Option<Rc<dyn ShaderNode>>) {
        if let Some(old) = self.shader.borrow().as_ref() {
            self.unobserve_inval(old.as_ref());
        }
        self.shader.borrow_mut().clone_from(&shader);
        if let Some(new) = shader {
            self.observe_inval(new.as_ref());
        }
    }
}

impl Drop for ShaderEffect {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L46-L52 (chrome/m156) (`ShaderEffect::~ShaderEffect`)
    fn drop(&mut self) {
        if let Some(shader) = self.shader.borrow().as_ref() {
            self.unobserve_inval(shader.as_ref());
        }
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for ShaderEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L71-L77 (chrome/m156) (`ShaderEffect::onRevalidate`)
    fn on_revalidate(&self, mut ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        debug_assert!(self.core.has_inval());
        if let Some(shader) = self.shader.borrow().clone() {
            shader.revalidate(ic.as_deref_mut(), ctm);
        }
        effect_on_revalidate(&self.child, ic, ctm)
    }
}

impl RenderNode for ShaderEffect {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L79-L84 (chrome/m156) (`ShaderEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let shader = self.shader.borrow().as_ref().and_then(|s| s.shader());
        let scope =
            ScopedRenderContext::new(canvas, ctx).modulate_shader(shader, &canvas.total_matrix());
        effect_on_render(&self.child, canvas, Some(scope.context()));
    }

    // Port of: modules/sksg/src/SkSGEffectNode.cpp#L24-L26 (chrome/m156) (`EffectNode::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        effect_on_node_at(&self.child, p)
    }
}

/// Overrides the mask shader of the content of its child (`MaskShaderEffect`).
// Port of: modules/sksg/include/SkSGRenderEffect.h#L72-L84 (chrome/m156) (`class MaskShaderEffect`)
#[doc(alias = "sksg::MaskShaderEffect")]
#[derive(Debug)]
pub struct MaskShaderEffect {
    core: NodeCore,
    child: Rc<dyn RenderNode>,
    shader: RefCell<Option<SkShader>>,
}

impl MaskShaderEffect {
    /// `MaskShaderEffect::Make(child, shader)`: `None` if there is no child.
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L24-L27 (chrome/m156) (`MaskShaderEffect::Make`)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(child: Option<Rc<dyn RenderNode>>, shader: Option<SkShader>) -> Option<Rc<Self>> {
        let child = child?;
        let effect = Rc::new_cyclic(|weak: &Weak<Self>| Self {
            core: NodeCore::new(0, weak.clone()),
            child: Rc::clone(&child),
            shader: RefCell::new(shader),
        });
        // The EffectNode base observes the child.
        effect.observe_inval(effect.child.as_ref());
        Some(effect)
    }

    /// The mask shader (`getShader`).
    #[must_use]
    pub fn shader(&self) -> Option<SkShader> {
        self.shader.borrow().clone()
    }

    /// Sets the mask shader, invalidating the node if it changed (`setShader`).
    pub fn set_shader(&self, shader: Option<SkShader>) {
        if *self.shader.borrow() != shader {
            *self.shader.borrow_mut() = shader;
            self.invalidate();
        }
    }
}

impl Drop for MaskShaderEffect {
    // Port of: modules/sksg/src/SkSGEffectNode.cpp (chrome/m156) (`EffectNode::~EffectNode`)
    fn drop(&mut self) {
        self.unobserve_inval(self.child.as_ref());
    }
}

impl Node for MaskShaderEffect {
    fn core(&self) -> &NodeCore {
        &self.core
    }

    // Port of: modules/sksg/src/SkSGEffectNode.cpp#L28-L32 (chrome/m156) (`EffectNode::onRevalidate`)
    fn on_revalidate(&self, ic: Option<&mut InvalidationController>, ctm: &Matrix) -> Rect {
        effect_on_revalidate(&self.child, ic, ctm)
    }
}

impl RenderNode for MaskShaderEffect {
    // Port of: modules/sksg/src/SkSGRenderEffect.cpp#L34-L39 (chrome/m156) (`MaskShaderEffect::onRender`)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        let shader = self.shader.borrow().clone();
        let scope = ScopedRenderContext::new(canvas, ctx)
            .modulate_mask_shader(shader, &canvas.total_matrix());
        effect_on_render(&self.child, canvas, Some(scope.context()));
    }

    // Port of: modules/sksg/src/SkSGEffectNode.cpp#L24-L26 (chrome/m156) (`EffectNode::onNodeAt`)
    fn on_node_at(&self, p: Point) -> Option<Hit> {
        effect_on_node_at(&self.child, p)
    }
}
