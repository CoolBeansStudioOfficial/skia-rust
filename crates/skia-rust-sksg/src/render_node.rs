// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/sksg/include/SkSGRenderNode.h, modules/sksg/src/SkSGRenderNode.cpp
// (chrome/m156)

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::blender::Blender;
use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::color_filter::ColorFilter;
use skia_rust_core::color_filters;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;

use std::rc::Rc;

use crate::node::Node;
use crate::util::{float_round_to_int, rect_contains};

/// Scales an alpha value by an opacity.
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L23-L25 (chrome/m156) (`ScaleAlpha`)
fn scale_alpha(alpha: u8, opacity: f32) -> u8 {
    // SkToU8 of the rounded product; the product is in [0, 255] for opacities in [0, 1].
    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // 0..=255 by construction
    let scaled = float_round_to_int(f32::from(alpha) * opacity) as u8;
    scaled
}

/// Makes `shader` take `ctm` into account: undoes the transforms pushed since `base`.
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L27-L60 (chrome/m156) (`LocalShader`)
fn local_shader(shader: &Shader, base: &Matrix, ctm: &Matrix) -> Shader {
    // Mask filters / shaders are declared to operate under a specific transform, but due to the
    // deferral mechanism, other transformations might have been pushed to the state. We want to
    // undo these transforms (T): baseCTM x T = ctm, so Inv(T) = Inv(ctm) x baseCTM.
    let lm = match ctm.invert() {
        Some(mut lm) if base != ctm => {
            lm.pre_concat(base);
            lm
        }
        _ => Matrix::new_identity(),
    };
    // Note: this doesn't compose with existing shader local matrices (see the C++ comment).
    shader.with_local_matrix(&lm)
}

/// `SkShaders::Blend` of two optional shaders: `nullptr` when either is missing.
// Port of: src/shaders/SkBlendShader.cpp#L119-L122 (chrome/m156) (`SkShaders::Blend`, null case)
fn blend_shaders(mode: BlendMode, dst: Option<Shader>, src: Option<Shader>) -> Option<Shader> {
    match (dst, src) {
        (Some(dst), Some(src)) => Some(shaders::blend(mode, dst, src)),
        _ => None,
    }
}

/// The paint overrides a render node sub-DAG inherits from its ancestors.
// Port of: modules/sksg/include/SkSGRenderNode.h#L51-L77 (chrome/m156) (`RenderNode::RenderContext`)
#[doc(alias = "sksg::RenderNode::RenderContext")]
#[derive(Debug, Clone)]
pub struct RenderContext {
    pub color_filter: Option<ColorFilter>,
    pub shader: Option<Shader>,
    pub mask_shader: Option<Shader>,
    pub blender: Option<Blender>,
    pub shader_ctm: Matrix,
    pub mask_ctm: Matrix,
    pub opacity: f32,
}

impl Default for RenderContext {
    fn default() -> Self {
        Self {
            color_filter: None,
            shader: None,
            mask_shader: None,
            blender: None,
            shader_ctm: Matrix::new_identity(),
            mask_ctm: Matrix::new_identity(),
            opacity: 1.0,
        }
    }
}

impl RenderContext {
    /// True if the paint overrides need a layer when applied to non-atomic draws.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L62-L67 (chrome/m156) (`requiresIsolation`)
    #[must_use]
    pub fn requires_isolation(&self) -> bool {
        // Note: `shader` is never applied on isolation layers.
        scale_alpha(255, self.opacity) != 255
            || self.color_filter.is_some()
            || self.mask_shader.is_some()
            || self.blender.is_some()
    }

    /// Applies the overrides to `paint`, for a draw under `ctm`.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L69-L92 (chrome/m156) (`modulatePaint`)
    pub fn modulate_paint(&self, ctm: &Matrix, paint: &mut Paint, is_layer_paint: bool) {
        paint.set_alpha(scale_alpha(paint.alpha(), self.opacity));
        let inner = paint.color_filter();
        paint.set_color_filter(color_filters::compose(self.color_filter.as_ref(), inner));
        if let Some(shader) = &self.shader {
            paint.set_shader(local_shader(shader, &self.shader_ctm, ctm));
        }
        if let Some(blender) = &self.blender {
            paint.set_blender(blender.clone());
        }
        // Only apply the shader mask for regular paints. Isolation layers require special
        // handling on restore.
        if let Some(mask_shader) = self.mask_shader.as_ref().filter(|_| !is_layer_paint) {
            let mask = local_shader(mask_shader, &self.mask_ctm, ctm);
            let paint_shader = paint.shader();
            paint.set_shader(blend_shaders(BlendMode::SrcIn, Some(mask), paint_shader));
        }
    }
}

/// Scopes paint overrides on a canvas: the layers they open are restored on drop.
// Port of: modules/sksg/include/SkSGRenderNode.h#L79-L137 (chrome/m156) (`ScopedRenderContext`)
#[doc(alias = "sksg::RenderNode::ScopedRenderContext")]
#[derive(Debug)]
pub struct ScopedRenderContext<'a> {
    canvas: &'a Canvas,
    ctx: RenderContext,
    /// To be applied at isolation layer restore time.
    mask_shader: Option<Shader>,
    /// `None` once the scope has been moved from (`fRestoreCount == -1`).
    restore_count: Option<usize>,
}

impl<'a> ScopedRenderContext<'a> {
    /// Opens a scope on `canvas`, starting from `ctx` (or the default context).
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L156-L160 (chrome/m156) (`ScopedRenderContext`)
    #[must_use]
    pub fn new(canvas: &'a Canvas, ctx: Option<&RenderContext>) -> Self {
        Self {
            canvas,
            ctx: ctx.cloned().unwrap_or_default(),
            mask_shader: None,
            restore_count: Some(canvas.save_count()),
        }
    }

    /// The overrides of this scope.
    #[must_use]
    pub fn context(&self) -> &RenderContext {
        &self.ctx
    }

    /// Multiplies the opacity override.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L173-L177 (chrome/m156) (`modulateOpacity`)
    #[must_use]
    pub fn modulate_opacity(mut self, opacity: f32) -> Self {
        debug_assert!((0.0..=1.0).contains(&opacity));
        self.ctx.opacity *= opacity;
        self
    }

    /// Composes a color filter override.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L179-L183 (chrome/m156) (`modulateColorFilter`)
    #[must_use]
    pub fn modulate_color_filter(mut self, cf: Option<ColorFilter>) -> Self {
        self.ctx.color_filter = color_filters::compose(self.ctx.color_filter.as_ref(), cf);
        self
    }

    /// Sets the shader override, unless a higher one is set already.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L185-L192 (chrome/m156) (`modulateShader`)
    #[must_use]
    pub fn modulate_shader(mut self, sh: Option<Shader>, shader_ctm: &Matrix) -> Self {
        // Topmost shader takes precedence.
        if self.ctx.shader.is_none() {
            self.ctx.shader = sh;
            self.ctx.shader_ctm = shader_ctm.clone();
        }
        self
    }

    /// Adds a mask shader override, composed with any mask already set.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L194-L215 (chrome/m156) (`modulateMaskShader`)
    #[must_use]
    pub fn modulate_mask_shader(mut self, ms: Option<Shader>, ctm: &Matrix) -> Self {
        if let Some(existing) = self.ctx.mask_shader.take() {
            // As we compose mask filters, use the relative transform T for the inner mask:
            // maskCTM x T = ctm => T = Inv(maskCTM) x ctm.
            if let (Some(ms), Some(inv_mask_ctm)) = (ms, self.ctx.mask_ctm.invert()) {
                let mut relative_transform = inv_mask_ctm;
                relative_transform.pre_concat(ctm);
                let inner = ms.with_local_matrix(&relative_transform);
                self.ctx.mask_shader = blend_shaders(BlendMode::SrcIn, Some(existing), Some(inner));
            } else {
                self.ctx.mask_shader = Some(existing);
            }
        } else {
            self.ctx.mask_shader = ms;
            self.ctx.mask_ctm = ctm.clone();
        }
        self
    }

    /// Sets the blender override.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L217-L220 (chrome/m156) (`modulateBlender`)
    #[must_use]
    pub fn modulate_blender(mut self, blender: Option<Blender>) -> Self {
        self.ctx.blender = blender;
        self
    }

    /// Forces content isolation for the sub-DAG: the overrides are applied by a layer.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L222-L242 (chrome/m156) (`setIsolation`)
    #[must_use]
    pub fn set_isolation(mut self, bounds: &Rect, ctm: &Matrix, isolation: bool) -> Self {
        if isolation && self.ctx.requires_isolation() {
            let mut layer_paint = Paint::default();
            self.ctx.modulate_paint(ctm, &mut layer_paint, true);
            self.canvas
                .save_layer(&SaveLayerRec::default().bounds(bounds).paint(&layer_paint));
            // Fetch the mask shader for restore.
            if let Some(mask) = &self.ctx.mask_shader {
                self.mask_shader = Some(local_shader(mask, &self.ctx.mask_ctm, ctm));
            }
            // Reset only the props applied via isolation layers.
            self.ctx.color_filter = None;
            self.ctx.mask_shader = None;
            self.ctx.blender = None;
            self.ctx.opacity = 1.0;
        }
        self
    }

    /// Forces content isolation for the sub-DAG by applying the overrides and an image filter
    /// via one layer.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L244-L262 (chrome/m156) (`setFilterIsolation`)
    #[must_use]
    pub fn set_filter_isolation(
        mut self,
        bounds: &Rect,
        ctm: &Matrix,
        filter: Option<skia_rust_core::image_filter::ImageFilter>,
    ) -> Self {
        if let Some(filter) = filter {
            let mut layer_paint = Paint::default();
            self.ctx.modulate_paint(ctm, &mut layer_paint, false);
            // shaders and image filters are not composable, so we convert the shader to an image
            // filter and blend them together
            let filter = match layer_paint.shader() {
                Some(shader) => skia_rust_effects::image_filters::blend_filter::blend(
                    BlendMode::SrcIn,
                    Some(filter),
                    skia_rust_effects::image_filters::shader_filter::shader(
                        Some(shader),
                        skia_rust_effects::image_filters::shader_filter::Dither::default(),
                        None,
                    ),
                    None,
                ),
                None => Some(filter),
            };
            layer_paint.set_image_filter(filter);
            self.canvas
                .save_layer(&SaveLayerRec::default().bounds(bounds).paint(&layer_paint));
            self.ctx = RenderContext::default();
        }
        self
    }
}

impl Drop for ScopedRenderContext<'_> {
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L162-L171 (chrome/m156) (`~ScopedRenderContext`)
    fn drop(&mut self) {
        if let Some(restore_count) = self.restore_count {
            if let Some(mask_shader) = self.mask_shader.take() {
                let mut mask_paint = Paint::default();
                mask_paint.set_blend_mode(BlendMode::DstIn);
                mask_paint.set_shader(mask_shader);
                self.canvas.draw_paint(&mask_paint);
            }
            self.canvas.restore_to_count(restore_count);
        }
    }
}

/// A node that renders to a canvas and hit-tests points.
// Port of: modules/sksg/include/SkSGRenderNode.h#L22-L50 (chrome/m156) (`class RenderNode`)
#[doc(alias = "sksg::RenderNode")]
pub trait RenderNode: Node {
    /// Renders the node and its descendants (`onRender`).
    // Port of: modules/sksg/include/SkSGRenderNode.h#L37-L37 (chrome/m156)
    fn on_render(&self, canvas: &Canvas, ctx: Option<&RenderContext>);

    /// Front-to-back hit-test of `p` (`onNodeAt`): `This` if this node is hit, or the hit
    /// descendant. `None` if nothing is hit.
    // Port of: modules/sksg/include/SkSGRenderNode.h#L38-L39 (chrome/m156)
    fn on_node_at(&self, p: Point) -> Option<Hit>;

    /// Renders the node and its descendants to the canvas (`render`). Invisible nodes are not
    /// rendered, but they still participate in revalidation.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L30-L38 (chrome/m156)
    fn render(&self, canvas: &Canvas, ctx: Option<&RenderContext>) {
        debug_assert!(!self.core().has_inval());
        if self.is_visible() && !self.core().bounds().is_empty() {
            self.on_render(canvas, ctx);
        }
        debug_assert!(!self.core().has_inval());
    }

    /// Controls the visibility: invisible nodes are not rendered.
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L24-L26 (chrome/m156) (`isVisible`)
    #[doc(alias = "isVisible")]
    fn is_visible(&self) -> bool {
        self.core().node_flags() & INVISIBLE_FLAG == 0
    }

    /// Sets the visibility (`setVisible`).
    // Port of: modules/sksg/src/SkSGRenderNode.cpp#L28-L37 (chrome/m156)
    #[doc(alias = "setVisible")]
    fn set_visible(&self, visible: bool) {
        if visible == self.is_visible() {
            return;
        }
        self.invalidate();
        let flags = self.core().node_flags();
        self.core().set_node_flags(if visible {
            flags & !INVISIBLE_FLAG
        } else {
            flags | INVISIBLE_FLAG
        });
    }
}

/// The result of a node's hit-test hook: the node itself, or one of its descendants.
#[derive(Debug, Clone)]
pub enum Hit {
    /// The node hit is the node whose hook returned this.
    This,
    /// A descendant hit.
    Child(Rc<dyn RenderNode>),
}

/// The node located at `p`, or `None`. Normally, hit-testing stops at leaf Draw nodes
/// (`RenderNode::nodeAt`).
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L49-L51 (chrome/m156) (`RenderNode::nodeAt`)
#[doc(alias = "nodeAt")]
#[must_use]
pub fn node_at(node: &Rc<dyn RenderNode>, p: Point) -> Option<Rc<dyn RenderNode>> {
    if !rect_contains(&node.core().bounds(), p) {
        return None;
    }
    match node.on_node_at(p)? {
        Hit::This => Some(Rc::clone(node)),
        Hit::Child(child) => Some(child),
    }
}

/// `kInvisible_Flag` of `RenderNode`.
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L11-L15 (chrome/m156)
const INVISIBLE_FLAG: u8 = 1 << 0;

/// Observes the children of a custom render node (`CustomRenderNode`'s constructor). Such nodes
/// cannot make assumptions about their children's damage, so they use
/// [`inval_traits::OVERRIDE_DAMAGE`].
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L252-L256 (chrome/m156) (`CustomRenderNode::CustomRenderNode`)
pub fn observe_children(observer: &dyn Node, children: &[Rc<dyn RenderNode>]) {
    for child in children {
        observer.observe_inval(child.as_ref());
    }
}

/// Unobserves the children of a custom render node (`~CustomRenderNode`).
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L258-L262 (chrome/m156) (`CustomRenderNode::~CustomRenderNode`)
pub fn unobserve_children(observer: &dyn Node, children: &[Rc<dyn RenderNode>]) {
    for child in children {
        observer.unobserve_inval(child.as_ref());
    }
}

/// True if any child needs revalidation (`CustomRenderNode::hasChildrenInval`).
// Port of: modules/sksg/src/SkSGRenderNode.cpp#L264-L272 (chrome/m156)
#[must_use]
pub fn has_children_inval(children: &[Rc<dyn RenderNode>]) -> bool {
    children.iter().any(|child| child.core().has_inval())
}
