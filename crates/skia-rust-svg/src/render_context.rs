// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGRenderContext.h,
// modules/svg/src/SkSVGRenderContext.cpp

//! The state threaded through rendering (`SkSVGRenderContext`).

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;

use skia_rust_core::canvas::{Canvas, SaveLayerRec};
use skia_rust_core::color::Color;
use skia_rust_core::paint::{Cap, Join, Paint as SkPaint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::{SCALAR_SQRT2, scalar, scalar_sqrt};
use skia_rust_core::size::Size;
use skia_rust_core::t_pin::t_pin;
use skia_rust_effects::dash_path_effect;

use crate::attribute::PresentationAttributes;
use crate::id_mapper::IdMapper;
use crate::node::{Node, SvgNode, Tag};
use crate::types::{
    ColorKind, ColorType, DashArrayType, Fill, FuncIri, FuncIriType, Iri, IriType, Length,
    LengthUnit, LineCap, LineJoin, LineJoinType, ObjectBoundingBoxUnits,
    ObjectBoundingBoxUnitsType, Paint, PaintType,
};

/// What kind of length a percentage refers to (`SkSVGLengthContext::LengthType`).
// Port of: modules/svg/include/SkSVGRenderContext.h#L44-L48 (chrome/m156)
#[doc(alias = "SkSVGLengthContext::LengthType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LengthType {
    Horizontal,
    Vertical,
    Other,
}

// Port of: modules/svg/src/SkSVGRenderContext.cpp#L39-L56 (chrome/m156)
fn length_size_for_type(viewport: Size, t: LengthType) -> scalar {
    match t {
        LengthType::Horizontal => viewport.width,
        LengthType::Vertical => viewport.height,
        LengthType::Other => {
            // https://www.w3.org/TR/SVG11/coords.html#Units_viewport_percentage
            let rsqrt2: scalar = 1.0 / SCALAR_SQRT2;
            let (w, h) = (viewport.width, viewport.height);
            rsqrt2 * scalar_sqrt(w * w + h * h)
        }
    }
}

// Multipliers for DPI-relative units.
// Port of: modules/svg/src/SkSVGRenderContext.cpp#L58-L63 (chrome/m156)
const IN_MULTIPLIER: scalar = 1.00;
const PT_MULTIPLIER: scalar = IN_MULTIPLIER / 72.272;
const PC_MULTIPLIER: scalar = PT_MULTIPLIER * 12.0;
const MM_MULTIPLIER: scalar = IN_MULTIPLIER / 25.4;
const CM_MULTIPLIER: scalar = MM_MULTIPLIER * 10.0;

/// Resolves lengths to user units (`SkSVGLengthContext`).
// Port of: modules/svg/include/SkSVGRenderContext.h#L42-L68 (chrome/m156)
#[doc(alias = "SkSVGLengthContext")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LengthContext {
    viewport: Size,
    dpi: scalar,
}

impl LengthContext {
    /// `SkSVGLengthContext(viewport, dpi = 90)`.
    #[must_use]
    pub const fn new(viewport: Size) -> Self {
        Self {
            viewport,
            dpi: 90.0,
        }
    }

    #[must_use]
    pub const fn with_dpi(viewport: Size, dpi: scalar) -> Self {
        Self { viewport, dpi }
    }

    #[doc(alias = "viewPort")]
    #[must_use]
    pub const fn view_port(&self) -> &Size {
        &self.viewport
    }

    #[doc(alias = "setViewPort")]
    pub fn set_view_port(&mut self, viewport: Size) {
        self.viewport = viewport;
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L65-L89 (chrome/m156)
    #[must_use]
    pub fn resolve(&self, l: &Length, t: LengthType) -> scalar {
        match l.unit() {
            LengthUnit::Number | LengthUnit::PX => l.value(),
            LengthUnit::Percentage => l.value() * length_size_for_type(self.viewport, t) / 100.0,
            LengthUnit::CM => l.value() * self.dpi * CM_MULTIPLIER,
            LengthUnit::MM => l.value() * self.dpi * MM_MULTIPLIER,
            LengthUnit::IN => l.value() * self.dpi * IN_MULTIPLIER,
            LengthUnit::PT => l.value() * self.dpi * PT_MULTIPLIER,
            LengthUnit::PC => l.value() * self.dpi * PC_MULTIPLIER,
            _ => {
                // unsupported unit type
                0.0
            }
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L91-L98 (chrome/m156)
    #[doc(alias = "resolveRect")]
    #[must_use]
    pub fn resolve_rect(&self, x: &Length, y: &Length, w: &Length, h: &Length) -> Rect {
        Rect::from_xywh(
            self.resolve(x, LengthType::Horizontal),
            self.resolve(y, LengthType::Vertical),
            self.resolve(w, LengthType::Horizontal),
            self.resolve(h, LengthType::Vertical),
        )
    }
}

/// The presentation state inherited down the tree (`SkSVGPresentationContext`).
// Port of: modules/svg/include/SkSVGRenderContext.h#L70-L80 (chrome/m156)
#[doc(alias = "SkSVGPresentationContext")]
#[derive(Debug, Clone)]
pub struct PresentationContext {
    pub named_colors: Option<Arc<HashMap<String, ColorType>>>,

    /// Inherited presentation attributes, computed for the current node.
    pub inherited: PresentationAttributes,
}

impl Default for PresentationContext {
    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L137-L139 (chrome/m156)
    fn default() -> Self {
        Self {
            named_colors: None,
            inherited: PresentationAttributes::make_initial(),
        }
    }
}

/// What a bounding box computation needs of the render context at the node whose object
/// bounding box scope is active.
///
/// `SkSVGNode::objectBoundingBox` takes the `SkSVGRenderContext` that opened the scope; the
/// bounding box code only reads its length context and which node its scope belongs to, so
/// those are what a `BboxContext` carries.
#[derive(Debug, Clone, Copy)]
pub struct BboxContext<'a> {
    length_context: LengthContext,
    scope_node: Option<usize>,
    id_mapper: &'a IdMapper,
}

impl<'a> BboxContext<'a> {
    #[doc(alias = "lengthContext")]
    #[must_use]
    pub fn length_context(&self) -> &LengthContext {
        &self.length_context
    }

    /// Looks up a node by id, as [`RenderContext::find_node_by_id`] does.
    #[doc(alias = "findNodeById")]
    #[must_use]
    pub fn find_node_by_id(&self, iri: &Iri) -> BorrowedNode<'a> {
        find_node_by_id(self.id_mapper, iri)
    }

    /// Whether the node with data `base` is the node of the current object bounding box scope
    /// (`ctx.currentOBBScope().fNode == this`).
    #[must_use]
    pub fn is_scope_node(&self, base: &crate::node::NodeBase) -> bool {
        self.scope_node == Some(base.key())
    }
}

// Port of: modules/svg/src/SkSVGRenderContext.cpp#L199-L206 (chrome/m156)
fn find_node_by_id<'a>(id_mapper: &'a IdMapper, iri: &Iri) -> BorrowedNode<'a> {
    if iri.ty() != IriType::Local {
        // non-local iri references not currently supported
        return BorrowedNode::new(None);
    }
    BorrowedNode::new(id_mapper.find_slot(iri.iri()))
}

fn key_of(node: &dyn SvgNode) -> usize {
    node.base().key()
}

/// Captures data required for object bounding box resolution (`SkSVGRenderContext::OBBScope`).
// Port of: modules/svg/include/SkSVGRenderContext.h#L87-L90 (chrome/m156)
#[doc(alias = "SkSVGRenderContext::OBBScope")]
#[derive(Clone)]
pub struct ObbScope<'a> {
    node: Option<&'a dyn SvgNode>,
    // The length context of the render context that opened the scope (shared, since the node
    // updates it while preparing to render).
    ctx: Option<Rc<Cell<LengthContext>>>,
}

impl std::fmt::Debug for ObbScope<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObbScope")
            .field("has_node", &self.node.is_some())
            .finish_non_exhaustive()
    }
}

impl ObbScope<'_> {
    /// No scope (`{nullptr, nullptr}`).
    #[must_use]
    pub fn none() -> Self {
        ObbScope {
            node: None,
            ctx: None,
        }
    }

    #[doc(alias = "fNode")]
    #[must_use]
    pub fn node(&self) -> Option<&dyn SvgNode> {
        self.node
    }
}

/// A node taken out of the id map for as long as it is used, so that references that lead back
/// to it find nothing (`SkSVGRenderContext::BorrowedNode`).
///
/// The id to node association is cleared for the lifetime of the value (this breaks reference
/// cycles, assuming appropriate scoping of the value).
// Port of: modules/svg/include/SkSVGRenderContext.h#L106-L135 (chrome/m156)
#[doc(alias = "SkSVGRenderContext::BorrowedNode")]
#[derive(Debug)]
pub struct BorrowedNode<'a> {
    owner: Option<&'a std::sync::Mutex<Option<Node>>>,
    borrowed: Option<Node>,
}

impl<'a> BorrowedNode<'a> {
    // Port of: modules/svg/include/SkSVGRenderContext.h#L108-L114 (chrome/m156)
    fn new(owner: Option<&'a std::sync::Mutex<Option<Node>>>) -> Self {
        let borrowed = owner.and_then(|o| o.lock().ok().and_then(|mut n| n.take()));
        Self { owner, borrowed }
    }

    #[must_use]
    pub fn get(&self) -> Option<&dyn SvgNode> {
        self.borrowed.as_deref()
    }

    #[must_use]
    pub fn is_some(&self) -> bool {
        self.borrowed.is_some()
    }
}

impl Drop for BorrowedNode<'_> {
    // Port of: modules/svg/include/SkSVGRenderContext.h#L116-L120 (chrome/m156)
    fn drop(&mut self) {
        if let Some(owner) = self.owner
            && let Ok(mut slot) = owner.lock()
        {
            *slot = self.borrowed.take();
        }
    }
}

/// The translate/scale transformation required to map into the current OBB scope
/// (`SkSVGRenderContext::OBBTransform`).
#[doc(alias = "SkSVGRenderContext::OBBTransform")]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObbTransform {
    pub offset: (scalar, scalar),
    pub scale: (scalar, scalar),
}

// Port of: modules/svg/src/SkSVGRenderContext.cpp#L100-L113 (chrome/m156)
fn to_sk_cap(cap: LineCap) -> Cap {
    match cap {
        LineCap::Butt => Cap::Butt,
        LineCap::Round => Cap::Round,
        LineCap::Square => Cap::Square,
    }
}

// Port of: modules/svg/src/SkSVGRenderContext.cpp#L115-L128 (chrome/m156)
fn to_sk_join(join: LineJoin) -> Join {
    match join.ty() {
        LineJoinType::Miter => Join::Miter,
        LineJoinType::Round => Join::Round,
        LineJoinType::Bevel => Join::Bevel,
        LineJoinType::Inherit => {
            debug_assert!(false);
            Join::Miter
        }
    }
}

// Port of: modules/svg/src/SkSVGRenderContext.cpp#L130-L156 (chrome/m156)
fn dash_effect(props: &PresentationAttributes, lctx: &LengthContext) -> Option<PathEffect> {
    if props.stroke_dash_array.ty() != DashArrayType::DashArray {
        return None;
    }

    let da = &*props.stroke_dash_array;
    let count = da.dash_array().len();
    let mut intervals: Vec<scalar> = Vec::with_capacity(count);
    for dash in da.dash_array() {
        intervals.push(lctx.resolve(dash, LengthType::Other));
    }

    if count & 1 != 0 {
        // If an odd number of values is provided, then the list of values
        // is repeated to yield an even number of values.
        intervals.extend_from_within(..count);
    }

    debug_assert_eq!(intervals.len() & 1, 0);

    let phase = lctx.resolve(&props.stroke_dash_offset, LengthType::Other);

    dash_path_effect::new(&intervals, phase)
}

/// The state of a render pass over (part of) the tree (`SkSVGRenderContext`).
///
/// It restores the canvas to the save count it had at construction when it is dropped.
///
/// skia-rust: the font manager, resource provider and text shaping factory of Skia's context
/// arrive with the nodes that use them (images M14, text M15).
// Port of: modules/svg/include/SkSVGRenderContext.h#L82-L227 (chrome/m156)
#[doc(alias = "SkSVGRenderContext")]
pub struct RenderContext<'a> {
    id_mapper: &'a IdMapper,
    length_context: Rc<Cell<LengthContext>>,
    presentation_context: Rc<PresentationContext>,
    canvas: &'a Canvas,
    // The save count on 'canvas' at construction time.
    // A restoreToCount() will be issued on destruction.
    canvas_save_count: usize,

    // clipPath, if present for the current context (not inherited).
    clip_path: Option<Path>,

    // Deferred opacity optimization for leaf nodes.
    deferred_paint_opacity: f32,

    // Current object bounding box scope.
    obb_scope: ObbScope<'a>,
}

impl std::fmt::Debug for RenderContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderContext")
            .field("length_context", &self.length_context.get())
            .field("canvas_save_count", &self.canvas_save_count)
            .finish_non_exhaustive()
    }
}

impl<'a> RenderContext<'a> {
    /// `kLeaf`: the target node doesn't have descendants.
    pub const LEAF: u32 = 1 << 0;

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L141-L158 (chrome/m156)
    #[must_use]
    pub fn new(
        canvas: &'a Canvas,
        id_mapper: &'a IdMapper,
        lctx: &LengthContext,
        pctx: &PresentationContext,
        obbs: ObbScope<'a>,
    ) -> Self {
        Self {
            id_mapper,
            length_context: Rc::new(Cell::new(*lctx)),
            presentation_context: Rc::new(pctx.clone()),
            canvas,
            canvas_save_count: canvas.save_count(),
            clip_path: None,
            deferred_paint_opacity: 1.0,
            obb_scope: obbs,
        }
    }

    /// `SkSVGRenderContext(const SkSVGRenderContext&)`.
    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L160-L168 (chrome/m156)
    #[must_use]
    pub fn copy_of(other: &RenderContext<'a>) -> RenderContext<'a> {
        RenderContext {
            id_mapper: other.id_mapper,
            length_context: Rc::new(Cell::new(other.length_context.get())),
            presentation_context: Rc::clone(&other.presentation_context),
            canvas: other.canvas,
            canvas_save_count: other.canvas.save_count(),
            clip_path: None,
            deferred_paint_opacity: 1.0,
            obb_scope: other.obb_scope.clone(),
        }
    }

    /// `SkSVGRenderContext(const SkSVGRenderContext&, SkCanvas*)`.
    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L170-L178 (chrome/m156)
    #[must_use]
    pub fn with_canvas(other: &RenderContext<'a>, canvas: &'a Canvas) -> RenderContext<'a> {
        RenderContext {
            id_mapper: other.id_mapper,
            length_context: Rc::new(Cell::new(other.length_context.get())),
            presentation_context: Rc::clone(&other.presentation_context),
            canvas,
            canvas_save_count: canvas.save_count(),
            clip_path: None,
            deferred_paint_opacity: 1.0,
            obb_scope: other.obb_scope.clone(),
        }
    }

    /// `SkSVGRenderContext(const SkSVGRenderContext&, const SkSVGNode*)`: establishes a new OBB
    /// scope. Normally used when entering a node's render scope.
    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L180-L188 (chrome/m156)
    #[must_use]
    pub fn with_node<'b>(other: &'b RenderContext<'_>, node: &'b dyn SvgNode) -> RenderContext<'b> {
        let length_context = Rc::new(Cell::new(other.length_context.get()));
        RenderContext {
            id_mapper: other.id_mapper,
            length_context: Rc::clone(&length_context),
            presentation_context: Rc::clone(&other.presentation_context),
            canvas: other.canvas,
            canvas_save_count: other.canvas.save_count(),
            clip_path: None,
            deferred_paint_opacity: 1.0,
            obb_scope: ObbScope {
                node: Some(node),
                ctx: Some(length_context),
            },
        }
    }

    #[doc(alias = "lengthContext")]
    #[must_use]
    pub fn length_context(&self) -> LengthContext {
        self.length_context.get()
    }

    /// `writableLengthContext()`: updates the length context.
    #[doc(alias = "writableLengthContext")]
    pub fn update_length_context(&mut self, f: impl FnOnce(&mut LengthContext)) {
        let mut lctx = self.length_context.get();
        f(&mut lctx);
        self.length_context.set(lctx);
    }

    #[doc(alias = "presentationContext")]
    #[must_use]
    pub fn presentation_context(&self) -> &PresentationContext {
        &self.presentation_context
    }

    #[must_use]
    pub fn canvas(&self) -> &'a Canvas {
        self.canvas
    }

    /// The view of this context that bounding box computations use.
    #[must_use]
    pub fn bbox_context(&self) -> BboxContext<'a> {
        BboxContext {
            length_context: self.length_context.get(),
            scope_node: self.obb_scope.node.map(key_of),
            id_mapper: self.id_mapper,
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L277-L285 (chrome/m156)
    #[doc(alias = "saveOnce")]
    pub fn save_once(&mut self) {
        // The canvas only needs to be saved once, per local SkSVGRenderContext.
        if self.canvas.save_count() == self.canvas_save_count {
            self.canvas.save();
        }

        debug_assert!(self.canvas.save_count() > self.canvas_save_count);
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L190-L254 (chrome/m156)
    #[doc(alias = "applyPresentationAttributes")]
    #[allow(clippy::float_cmp)] // mirrors the `!=` of ApplyLazyInheritedAttribute
    pub fn apply_presentation_attributes(&mut self, attrs: &PresentationAttributes, flags: u32) {
        macro_rules! apply_lazy_inherited_attribute {
            ($f:ident) => {
                // All attributes should be defined on the inherited context.
                debug_assert!(self.presentation_context.inherited.$f.is_value());
                let attr = &attrs.$f;
                if attr.is_value() && **attr != *self.presentation_context.inherited.$f {
                    // Update the local attribute value
                    Rc::make_mut(&mut self.presentation_context)
                        .inherited
                        .$f
                        .set((**attr).clone());
                }
            };
        }

        apply_lazy_inherited_attribute!(fill);
        apply_lazy_inherited_attribute!(fill_opacity);
        apply_lazy_inherited_attribute!(fill_rule);
        apply_lazy_inherited_attribute!(font_family);
        apply_lazy_inherited_attribute!(font_size);
        apply_lazy_inherited_attribute!(font_style);
        apply_lazy_inherited_attribute!(font_weight);
        apply_lazy_inherited_attribute!(clip_rule);
        apply_lazy_inherited_attribute!(stroke);
        apply_lazy_inherited_attribute!(stroke_dash_offset);
        apply_lazy_inherited_attribute!(stroke_dash_array);
        apply_lazy_inherited_attribute!(stroke_line_cap);
        apply_lazy_inherited_attribute!(stroke_line_join);
        apply_lazy_inherited_attribute!(stroke_miter_limit);
        apply_lazy_inherited_attribute!(stroke_opacity);
        apply_lazy_inherited_attribute!(stroke_width);
        apply_lazy_inherited_attribute!(text_anchor);
        apply_lazy_inherited_attribute!(visibility);
        apply_lazy_inherited_attribute!(color);
        apply_lazy_inherited_attribute!(color_interpolation);
        apply_lazy_inherited_attribute!(color_interpolation_filters);

        // Uninherited attributes.  Only apply to the current context.

        let has_filter = attrs.filter.is_value();
        if attrs.opacity.is_value() {
            self.apply_opacity(*attrs.opacity, flags, has_filter);
        }

        if attrs.clip_path.is_value() {
            self.apply_clip(&attrs.clip_path);
        }

        if attrs.mask.is_value() {
            self.apply_mask(&attrs.mask);
        }

        // TODO: when both a filter and opacity are present, we can apply both with a single layer
        if has_filter {
            self.apply_filter(&attrs.filter);
        }

        // Remaining uninherited presentation attributes are accessed as SkSVGNode fields, not via
        // the render context.
        // TODO: resolve these in a pre-render styling pass and assert here that they are values.
        // - stop-color
        // - stop-opacity
        // - flood-color
        // - flood-opacity
        // - lighting-color
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L256-L282 (chrome/m156)
    fn apply_opacity(&mut self, opacity: scalar, flags: u32, has_filter: bool) {
        if opacity >= 1.0 {
            return;
        }

        let props = &self.presentation_context.inherited;
        let has_fill = props.fill.ty() != PaintType::None;
        let has_stroke = props.stroke.ty() != PaintType::None;

        // We can apply the opacity as paint alpha if it only affects one atomic draw.
        // For now, this means all of the following must be true:
        //   - the target node doesn't have any descendants;
        //   - it only has a stroke or a fill (but not both);
        //   - it does not have a filter.
        // Going forward, we may needto refine this heuristic (e.g. to accommodate markers).
        if (flags & Self::LEAF != 0) && (has_fill ^ has_stroke) && !has_filter {
            self.deferred_paint_opacity *= opacity;
        } else {
            // Expensive, layer-based fall back.
            let mut opacity_paint = SkPaint::default();
            opacity_paint.set_alpha_f(t_pin(opacity, 0.0, 1.0));
            // Balanced in the destructor, via restoreToCount().
            self.canvas
                .save_layer(&SaveLayerRec::default().paint(&opacity_paint));
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L284-L304 (chrome/m156)
    fn apply_filter(&mut self, filter: &FuncIri) {
        if filter.ty() != FuncIriType::IRI {
            return;
        }

        let node = self.find_node_by_id(filter.iri());
        match node.get() {
            Some(n) if n.tag() == Tag::Filter => {
                // skia-rust: `<filter>` nodes are ported with the filter effects (M14); until
                // then no node has this tag.
            }
            _ => {}
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L317-L340 (chrome/m156)
    fn apply_clip(&mut self, clip: &FuncIri) {
        if clip.ty() != FuncIriType::IRI {
            return;
        }

        let clip_node = self.find_node_by_id(clip.iri());
        match clip_node.get() {
            Some(n) if n.tag() == Tag::ClipPath => {
                // skia-rust: `<clipPath>` nodes are ported with the paint servers and clips
                // (M14); until then no node has this tag.
            }
            _ => {}
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L342-L372 (chrome/m156)
    fn apply_mask(&mut self, mask: &FuncIri) {
        if mask.ty() != FuncIriType::IRI {
            return;
        }

        let node = self.find_node_by_id(mask.iri());
        match node.get() {
            Some(n) if n.tag() == Tag::Mask => {
                // skia-rust: `<mask>` nodes are ported with the paint servers and clips (M14);
                // until then no node has this tag.
            }
            _ => {}
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L199-L212 (chrome/m156)
    #[doc(alias = "findNodeById")]
    #[must_use]
    pub fn find_node_by_id(&self, iri: &Iri) -> BorrowedNode<'a> {
        find_node_by_id(self.id_mapper, iri)
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L374-L429 (chrome/m156)
    fn common_paint(&self, paint_selector: &Paint, paint_opacity: f32) -> Option<SkPaint> {
        if paint_selector.ty() == PaintType::None {
            return None;
        }

        let mut p = SkPaint::default();

        match paint_selector.ty() {
            PaintType::Color => {
                p.set_color(self.resolve_svg_color(paint_selector.color()));
            }
            PaintType::IRI => {
                // Our property inheritance is borked as it follows the render path and not the
                // tree hierarchy.  To avoid gross transgressions like leaf node presentation
                // attributes leaking into the paint server context, use a pristine presentation
                // context when following hrefs.
                //
                // Preserve the OBB scope because some paints use object bounding box coords
                // (e.g. gradient control points), which requires access to the render context
                // and node being rendered.
                let mut pctx = PresentationContext::default();
                pctx.named_colors
                    .clone_from(&self.presentation_context.named_colors);
                let local_ctx = RenderContext::new(
                    self.canvas,
                    self.id_mapper,
                    &self.length_context.get(),
                    &pctx,
                    self.obb_scope.clone(),
                );

                let node = self.find_node_by_id(paint_selector.iri());
                let painted = node.get().is_some_and(|n| n.as_paint(&local_ctx, &mut p));
                if !painted {
                    // Use the fallback color.
                    p.set_color(self.resolve_svg_color(paint_selector.color()));
                }
            }
            PaintType::None => unreachable!(),
        }

        p.set_anti_alias(true); // TODO: shape-rendering support

        // We observe 3 opacity components:
        //   - initial paint server opacity (e.g. color stop opacity)
        //   - paint-specific opacity (e.g. 'fill-opacity', 'stroke-opacity')
        //   - deferred opacity override (optimization for leaf nodes 'opacity')
        p.set_alpha_f(t_pin(
            p.alpha_f() * paint_opacity * self.deferred_paint_opacity,
            0.0,
            1.0,
        ));

        Some(p)
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L431-L441 (chrome/m156)
    #[doc(alias = "fillPaint")]
    #[must_use]
    pub fn fill_paint(&self) -> Option<SkPaint> {
        let props = &self.presentation_context.inherited;
        let mut p = self.common_paint(&props.fill, *props.fill_opacity);

        if let Some(p) = p.as_mut() {
            p.set_style(Style::Fill);
        }

        p
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L443-L462 (chrome/m156)
    #[doc(alias = "strokePaint")]
    #[must_use]
    pub fn stroke_paint(&self) -> Option<SkPaint> {
        let props = &self.presentation_context.inherited;
        let mut p = self.common_paint(&props.stroke, *props.stroke_opacity);

        if let Some(p) = p.as_mut() {
            p.set_style(Style::Stroke);
            p.set_stroke_width(
                self.length_context
                    .get()
                    .resolve(&props.stroke_width, LengthType::Other),
            );
            p.set_stroke_cap(to_sk_cap(*props.stroke_line_cap));
            p.set_stroke_join(to_sk_join(*props.stroke_line_join));
            p.set_stroke_miter(*props.stroke_miter_limit);
            p.set_path_effect(dash_effect(props, &self.length_context.get()));
        }

        p
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L464-L484 (chrome/m156)
    #[doc(alias = "resolveSvgColor")]
    #[must_use]
    pub fn resolve_svg_color(&self, color: &Fill) -> ColorType {
        if let Some(named) = &self.presentation_context.named_colors {
            for ident in color.vars() {
                if let Some(c) = named.get(ident) {
                    return *c;
                }
            }
        }
        match color.ty() {
            ColorKind::Color => color.color(),
            ColorKind::CurrentColor => *self.presentation_context.inherited.color,
            ColorKind::ICCColor => {
                // ICC color unimplemented
                Color::BLACK
            }
        }
    }

    /// The local computed clip path (not inherited).
    #[doc(alias = "clipPath")]
    #[must_use]
    pub fn clip_path(&self) -> Option<&Path> {
        self.clip_path.as_ref()
    }

    #[doc(alias = "currentOBBScope")]
    #[must_use]
    pub fn current_obb_scope(&self) -> &ObbScope<'a> {
        &self.obb_scope
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L486-L497 (chrome/m156)
    #[doc(alias = "transformForCurrentOBB")]
    #[must_use]
    pub fn transform_for_current_obb(&self, u: ObjectBoundingBoxUnits) -> ObbTransform {
        let (Some(node), Some(ctx)) = (self.obb_scope.node, self.obb_scope.ctx.as_ref()) else {
            return ObbTransform {
                offset: (0.0, 0.0),
                scale: (1.0, 1.0),
            };
        };
        if u.ty() == ObjectBoundingBoxUnitsType::UserSpaceOnUse {
            return ObbTransform {
                offset: (0.0, 0.0),
                scale: (1.0, 1.0),
            };
        }

        let obb = node.on_object_bounding_box(&BboxContext {
            length_context: ctx.get(),
            scope_node: Some(key_of(node)),
            id_mapper: self.id_mapper,
        });
        ObbTransform {
            offset: (obb.x(), obb.y()),
            scale: (obb.width(), obb.height()),
        }
    }

    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L499-L520 (chrome/m156)
    #[doc(alias = "resolveOBBRect")]
    #[allow(clippy::many_single_char_names, clippy::similar_names)] // mirrors the C++ signature
    #[must_use]
    pub fn resolve_obb_rect(
        &self,
        x: &Length,
        y: &Length,
        w: &Length,
        h: &Length,
        obbu: ObjectBoundingBoxUnits,
    ) -> Rect {
        let mut lctx = self.length_context.get();

        if obbu.ty() == ObjectBoundingBoxUnitsType::ObjectBoundingBox {
            lctx = LengthContext::new(Size::new(1.0, 1.0));
        }

        let r = lctx.resolve_rect(x, y, w, h);
        let obbt = self.transform_for_current_obb(obbu);

        Rect::from_xywh(
            obbt.scale.0 * r.x() + obbt.offset.0,
            obbt.scale.1 * r.y() + obbt.offset.1,
            obbt.scale.0 * r.width(),
            obbt.scale.1 * r.height(),
        )
    }
}

impl Drop for RenderContext<'_> {
    // Port of: modules/svg/src/SkSVGRenderContext.cpp#L190-L192 (chrome/m156)
    fn drop(&mut self) {
        self.canvas.restore_to_count(self.canvas_save_count);
    }
}
