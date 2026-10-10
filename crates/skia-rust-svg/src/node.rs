// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/svg/include/SkSVGNode.h, modules/svg/src/SkSVGNode.cpp

//! The base of all SVG nodes (`SkSVGNode`).

use std::any::Any;
use std::sync::Arc;

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint as SkPaint;
use skia_rust_core::path::Path;
use skia_rust_core::rect::Rect;
use skia_rust_pathops::PathOpsExt;
use skia_rust_pathops::path_op::PathOp;

use crate::attribute::{Attribute, PresentationAttributes};
use crate::attribute_parser::AttributeParser;
use crate::render_context::{BboxContext, RenderContext};
use crate::types::{
    Align, ColorType, Colorspace, DashArray, Display, Fill, FillRule, FontFamily, FontSize,
    FontStyle, FontWeight, FuncIri, Length, LineCap, LineJoin, NumberType, Paint, Property,
    PreserveAspectRatio, Scale, TextAnchor, Visibility, VisibilityType,
};
use crate::value::Value;

/// A reference-counted SVG node (`sk_sp<SkSVGNode>`).
pub type Node = Arc<dyn SvgNode>;

/// `SkSVGTag`.
// Port of: modules/svg/include/SkSVGNode.h#L26-L72 (chrome/m156)
#[doc(alias = "SkSVGTag")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tag {
    Circle,
    ClipPath,
    Defs,
    Ellipse,
    FeBlend,
    FeColorMatrix,
    FeComponentTransfer,
    FeComposite,
    FeDiffuseLighting,
    FeDisplacementMap,
    FeDistantLight,
    FeFlood,
    FeFuncA,
    FeFuncR,
    FeFuncG,
    FeFuncB,
    FeGaussianBlur,
    FeImage,
    FeMerge,
    FeMergeNode,
    FeMorphology,
    FeOffset,
    FePointLight,
    FeSpecularLighting,
    FeSpotLight,
    FeTurbulence,
    Filter,
    G,
    Image,
    Line,
    LinearGradient,
    Mask,
    Path,
    Pattern,
    Polygon,
    Polyline,
    RadialGradient,
    Rect,
    Stop,
    Svg,
    Text,
    TextLiteral,
    TextPath,
    TSpan,
    Use,
}

/// What every node has: its tag and its presentation attributes (the data of `SkSVGNode`).
// Port of: modules/svg/include/SkSVGNode.h#L99-L206 (chrome/m156)
#[derive(Debug, Clone)]
pub struct NodeBase {
    tag: Tag,
    // FIXME: this should be sparse
    presentation_attributes: PresentationAttributes,
}

// SVG_PRES_ATTR: a getter, and a setter that stores `inherit` as the inherit state for
// inheritable attributes.
macro_rules! pres_attrs {
    ($(($get:ident, $set:ident, $field:ident, $ty:ty, $inherited:literal)),* $(,)?) => {
        impl NodeBase {
            $(
                #[must_use]
                pub fn $get(&self) -> &Property<$ty, $inherited> {
                    &self.presentation_attributes.$field
                }

                pub fn $set(&mut self, v: Property<$ty, $inherited>) {
                    let dest = &mut self.presentation_attributes.$field;
                    if !dest.is_inheritable() || v.is_value() {
                        // TODO: If dest is not inheritable, handle v == "inherit"
                        *dest = v;
                    } else {
                        dest.set_state(crate::types::PropertyState::Inherit);
                    }
                }
            )*
        }

        /// The presentation attribute accessors of `SkSVGNode`, on every node.
        pub trait PresentationAttributesExt {
            fn node_base(&self) -> &NodeBase;
            fn node_base_mut(&mut self) -> &mut NodeBase;
            $(
                fn $get(&self) -> &Property<$ty, $inherited> {
                    self.node_base().$get()
                }

                fn $set(&mut self, v: Property<$ty, $inherited>) {
                    self.node_base_mut().$set(v);
                }
            )*
        }

        impl<T: SvgNode + ?Sized> PresentationAttributesExt for T {
            fn node_base(&self) -> &NodeBase {
                self.base()
            }

            fn node_base_mut(&mut self) -> &mut NodeBase {
                self.base_mut()
            }
        }
    };
}

pres_attrs! {
    // inherited
    (clip_rule, set_clip_rule, clip_rule, FillRule, true),
    (color, set_color, color, ColorType, true),
    (color_interpolation, set_color_interpolation, color_interpolation, Colorspace, true),
    (color_interpolation_filters, set_color_interpolation_filters, color_interpolation_filters, Colorspace, true),
    (fill_rule, set_fill_rule, fill_rule, FillRule, true),
    (fill, set_fill, fill, Paint, true),
    (fill_opacity, set_fill_opacity, fill_opacity, NumberType, true),
    (font_family, set_font_family, font_family, FontFamily, true),
    (font_size, set_font_size, font_size, FontSize, true),
    (font_style, set_font_style, font_style, FontStyle, true),
    (font_weight, set_font_weight, font_weight, FontWeight, true),
    (stroke, set_stroke, stroke, Paint, true),
    (stroke_dash_array, set_stroke_dash_array, stroke_dash_array, DashArray, true),
    (stroke_dash_offset, set_stroke_dash_offset, stroke_dash_offset, Length, true),
    (stroke_line_cap, set_stroke_line_cap, stroke_line_cap, LineCap, true),
    (stroke_line_join, set_stroke_line_join, stroke_line_join, LineJoin, true),
    (stroke_miter_limit, set_stroke_miter_limit, stroke_miter_limit, NumberType, true),
    (stroke_opacity, set_stroke_opacity, stroke_opacity, NumberType, true),
    (stroke_width, set_stroke_width, stroke_width, Length, true),
    (text_anchor, set_text_anchor, text_anchor, TextAnchor, true),
    (visibility, set_visibility, visibility, Visibility, true),

    // not inherited
    (clip_path, set_clip_path, clip_path, FuncIri, false),
    (display, set_display, display, Display, false),
    (mask, set_mask, mask, FuncIri, false),
    (filter, set_filter, filter, FuncIri, false),
    (opacity, set_opacity, opacity, NumberType, false),
    (stop_color, set_stop_color, stop_color, Fill, false),
    (stop_opacity, set_stop_opacity, stop_opacity, NumberType, false),
    (flood_color, set_flood_color, flood_color, Fill, false),
    (flood_opacity, set_flood_opacity, flood_opacity, NumberType, false),
    (lighting_color, set_lighting_color, lighting_color, Fill, false),
}

impl NodeBase {
    // Port of: modules/svg/src/SkSVGNode.cpp#L21-L27 (chrome/m156)
    #[must_use]
    pub fn new(tag: Tag) -> Self {
        let mut presentation_attributes = PresentationAttributes::default();
        // Uninherited presentation attributes need a non-null default value.
        presentation_attributes
            .stop_color
            .set(Fill::new(skia_rust_core::color::Color::BLACK));
        presentation_attributes.stop_opacity.set(1.0);
        presentation_attributes
            .flood_color
            .set(Fill::new(skia_rust_core::color::Color::BLACK));
        presentation_attributes.flood_opacity.set(1.0);
        presentation_attributes
            .lighting_color
            .set(Fill::new(skia_rust_core::color::Color::WHITE));
        Self {
            tag,
            presentation_attributes,
        }
    }

    #[must_use]
    pub fn tag(&self) -> Tag {
        self.tag
    }

    /// An identity for the node that owns this base, for `==` on `const SkSVGNode*`.
    #[must_use]
    pub fn key(&self) -> usize {
        std::ptr::from_ref(self) as usize
    }

    /// The presentation attributes of the node.
    #[must_use]
    pub fn presentation_attributes(&self) -> &PresentationAttributes {
        &self.presentation_attributes
    }

    /// Called before `on_render()`, to apply local attributes to the context.
    ///
    /// Port of `SkSVGNode::onPrepareToRender`: returns whether rendering continues.
    // Port of: modules/svg/src/SkSVGNode.cpp#L69-L80 (chrome/m156)
    #[must_use]
    pub fn on_prepare_to_render(&self, has_children: bool, ctx: &mut RenderContext<'_>) -> bool {
        ctx.apply_presentation_attributes(
            &self.presentation_attributes,
            if has_children {
                0
            } else {
                RenderContext::LEAF
            },
        );

        // visibility:hidden and display:none disable rendering.
        // TODO: if display is not a value (true when display="inherit"), we currently
        //   ignore it. Eventually we should be able to add SkASSERT(display.isValue()).
        let visibility = ctx.presentation_context().inherited.visibility.ty();
        let display = &self.presentation_attributes.display; // display is uninherited
        visibility != VisibilityType::Hidden
            && (!display.is_value() || **display != Display::None)
    }

    // Port of: modules/svg/src/SkSVGNode.cpp#L96-L133 (chrome/m156)
    #[doc(alias = "parseAndSetAttribute")]
    pub fn parse_and_set_attribute(&mut self, n: &str, v: &str) -> bool {
        macro_rules! parse_and_set {
            ($svg_name:literal, $set:ident, $ty:ty, $inh:literal) => {
                if let Some(pr) = AttributeParser::parse_property::<$ty, $inh>($svg_name, n, v) {
                    self.$set(pr);
                    return true;
                }
            };
        }

        parse_and_set!("clip-path", set_clip_path, FuncIri, false);
        parse_and_set!("clip-rule", set_clip_rule, FillRule, true);
        parse_and_set!("color", set_color, ColorType, true);
        parse_and_set!("color-interpolation", set_color_interpolation, Colorspace, true);
        parse_and_set!(
            "color-interpolation-filters",
            set_color_interpolation_filters,
            Colorspace,
            true
        );
        parse_and_set!("display", set_display, Display, false);
        parse_and_set!("fill", set_fill, Paint, true);
        parse_and_set!("fill-opacity", set_fill_opacity, NumberType, true);
        parse_and_set!("fill-rule", set_fill_rule, FillRule, true);
        parse_and_set!("filter", set_filter, FuncIri, false);
        parse_and_set!("flood-color", set_flood_color, Fill, false);
        parse_and_set!("flood-opacity", set_flood_opacity, NumberType, false);
        parse_and_set!("font-family", set_font_family, FontFamily, true);
        parse_and_set!("font-size", set_font_size, FontSize, true);
        parse_and_set!("font-style", set_font_style, FontStyle, true);
        parse_and_set!("font-weight", set_font_weight, FontWeight, true);
        parse_and_set!("lighting-color", set_lighting_color, Fill, false);
        parse_and_set!("mask", set_mask, FuncIri, false);
        parse_and_set!("opacity", set_opacity, NumberType, false);
        parse_and_set!("stop-color", set_stop_color, Fill, false);
        parse_and_set!("stop-opacity", set_stop_opacity, NumberType, false);
        parse_and_set!("stroke", set_stroke, Paint, true);
        parse_and_set!("stroke-dasharray", set_stroke_dash_array, DashArray, true);
        parse_and_set!("stroke-dashoffset", set_stroke_dash_offset, Length, true);
        parse_and_set!("stroke-linecap", set_stroke_line_cap, LineCap, true);
        parse_and_set!("stroke-linejoin", set_stroke_line_join, LineJoin, true);
        parse_and_set!("stroke-miterlimit", set_stroke_miter_limit, NumberType, true);
        parse_and_set!("stroke-opacity", set_stroke_opacity, NumberType, true);
        parse_and_set!("stroke-width", set_stroke_width, Length, true);
        parse_and_set!("text-anchor", set_text_anchor, TextAnchor, true);
        parse_and_set!("visibility", set_visibility, Visibility, true);
        false
    }
}

/// A node of the SVG tree (`SkSVGNode`).
///
/// The protected virtuals of the C++ class (`onRender`, `onPrepareToRender`, ...) are the
/// `on_*` methods; call `render` and friends on the node, not those.
// Port of: modules/svg/include/SkSVGNode.h#L99-L206 (chrome/m156)
#[doc(alias = "SkSVGNode")]
pub trait SvgNode: Any + Send + Sync + std::fmt::Debug {
    /// The data every node has.
    fn base(&self) -> &NodeBase;
    fn base_mut(&mut self) -> &mut NodeBase;

    /// `self` as `Any`, to get from a [`Node`] to its concrete type.
    fn as_any(&self) -> &dyn Any;

    /// The `Arc` as `Any`, to get from a [`Node`] to its concrete type.
    fn into_any_arc(self: Arc<Self>) -> Arc<dyn Any + Send + Sync>;

    /// Adds a child. Nodes that cannot have children ignore the call, as in Skia.
    #[doc(alias = "appendChild")]
    fn append_child(&mut self, node: Node);

    // TODO: consolidate with existing setAttribute
    #[doc(alias = "parseAndSetAttribute")]
    fn parse_and_set_attribute(&mut self, name: &str, value: &str) -> bool {
        self.base_mut().parse_and_set_attribute(name, value)
    }

    /// Called before `on_render()`, to apply local attributes to the context. Unlike
    /// `on_render()`, it bubbles up the inheritance chain: overriders should always call the
    /// base version, unless they intend to short-circuit rendering (return false).
    #[doc(alias = "onPrepareToRender")]
    fn on_prepare_to_render(&self, ctx: &mut RenderContext<'_>) -> bool {
        self.base().on_prepare_to_render(self.has_children(), ctx)
    }

    #[doc(alias = "onRender")]
    fn on_render(&self, ctx: &RenderContext<'_>);

    #[doc(alias = "onAsPaint")]
    fn on_as_paint(&self, _ctx: &RenderContext<'_>, _paint: &mut SkPaint) -> bool {
        false
    }

    #[doc(alias = "onAsPath")]
    fn on_as_path(&self, ctx: &RenderContext<'_>) -> Path;

    #[doc(alias = "onSetAttribute")]
    fn on_set_attribute(&mut self, _attr: Attribute, _value: &Value<'_>) {}

    #[doc(alias = "hasChildren")]
    fn has_children(&self) -> bool {
        false
    }

    #[doc(alias = "onObjectBoundingBox")]
    fn on_object_bounding_box(&self, _ctx: &BboxContext<'_>) -> Rect {
        Rect::new_empty()
    }
}

impl dyn SvgNode {
    #[must_use]
    pub fn tag(&self) -> Tag {
        self.base().tag()
    }

    // Port of: modules/svg/src/SkSVGNode.cpp#L38-L44 (chrome/m156)
    pub fn render(&self, ctx: &RenderContext<'_>) {
        let mut local_context = RenderContext::with_node(ctx, self);

        if self.on_prepare_to_render(&mut local_context) {
            self.on_render(&local_context);
        }
    }

    // Port of: modules/svg/src/SkSVGNode.cpp#L46-L50 (chrome/m156)
    #[doc(alias = "asPaint")]
    pub fn as_paint(&self, ctx: &RenderContext<'_>, paint: &mut SkPaint) -> bool {
        let mut local_context = RenderContext::copy_of(ctx);

        self.on_prepare_to_render(&mut local_context) && self.on_as_paint(&local_context, paint)
    }

    // Port of: modules/svg/src/SkSVGNode.cpp#L52-L67 (chrome/m156)
    #[doc(alias = "asPath")]
    #[must_use]
    pub fn as_path(&self, ctx: &RenderContext<'_>) -> Path {
        let mut local_context = RenderContext::copy_of(ctx);
        if !self.on_prepare_to_render(&mut local_context) {
            return Path::default();
        }

        let mut path = self.on_as_path(&local_context);

        if let Some(clip_path) = local_context.clip_path() {
            // There is a clip-path present on the current node.
            if let Some(result) = path.op(clip_path, PathOp::Intersect) {
                path = result;
            }
        }

        path
    }

    // Port of: modules/svg/src/SkSVGNode.cpp#L69-L71 (chrome/m156)
    #[doc(alias = "objectBoundingBox")]
    #[must_use]
    pub fn object_bounding_box(&self, ctx: &RenderContext<'_>) -> Rect {
        self.on_object_bounding_box(&ctx.bbox_context())
    }

    // Port of: modules/svg/src/SkSVGNode.cpp#L82-L84 (chrome/m156)
    #[doc(alias = "setAttribute")]
    pub fn set_attribute(&mut self, attr: Attribute, value: &Value<'_>) {
        self.on_set_attribute(attr, value);
    }
}

// https://www.w3.org/TR/SVG11/coords.html#PreserveAspectRatioAttribute
// Port of: modules/svg/src/SkSVGNode.cpp#L135-L192 (chrome/m156)
#[doc(alias = "ComputeViewboxMatrix")]
#[must_use]
pub fn compute_viewbox_matrix(
    view_box: &Rect,
    view_port: &Rect,
    par: PreserveAspectRatio,
) -> Matrix {
    if view_box.is_empty() || view_port.is_empty() {
        return Matrix::scale((0.0, 0.0));
    }

    let compute_scale = || -> (f32, f32) {
        let sx = view_port.width() / view_box.width();
        let sy = view_port.height() / view_box.height();

        if par.align == Align::None {
            // none -> anisotropic scaling, regardless of fScale
            return (sx, sy);
        }

        // isotropic scaling
        // std::min(a, b) is `(b < a) ? b : a`; std::max(a, b) is `(a < b) ? b : a`
        let s = if par.scale == Scale::Meet {
            if sy < sx { sy } else { sx }
        } else if sx < sy {
            sy
        } else {
            sx
        };
        (s, s)
    };

    let compute_trans = |scale: (f32, f32)| -> (f32, f32) {
        const G_ALIGN_COEFFS: [f32; 3] = [
            0.0, // Min
            0.5, // Mid
            1.0, // Max
        ];

        let align = par.align as u8;
        let x_coeff = usize::from(align & 0x03);
        let y_coeff = usize::from((align >> 2) & 0x03);

        debug_assert!(x_coeff < G_ALIGN_COEFFS.len() && y_coeff < G_ALIGN_COEFFS.len());

        let tx = -view_box.x() * scale.0;
        let ty = -view_box.y() * scale.1;
        let dx = view_port.width() - view_box.width() * scale.0;
        let dy = view_port.height() - view_box.height() * scale.1;

        (
            tx + dx * G_ALIGN_COEFFS[x_coeff],
            ty + dy * G_ALIGN_COEFFS[y_coeff],
        )
    };

    let s = compute_scale();
    let t = compute_trans(s);

    Matrix::concat(&Matrix::translate(t), &Matrix::scale(s))
}

/// Implements the parts of [`SvgNode`] that every node type spells the same way, for a type
/// whose node data is the `NodeBase` found at `self.$($path).+`.
macro_rules! impl_svg_node_basics {
    ($($path:ident).+) => {
        fn base(&self) -> &$crate::node::NodeBase {
            &self.$($path).+
        }

        fn base_mut(&mut self) -> &mut $crate::node::NodeBase {
            &mut self.$($path).+
        }

        fn as_any(&self) -> &dyn ::std::any::Any {
            self
        }

        fn into_any_arc(
            self: ::std::sync::Arc<Self>,
        ) -> ::std::sync::Arc<dyn ::std::any::Any + Send + Sync> {
            self
        }
    };
}
pub(crate) use impl_svg_node_basics;

/// Implements all of [`SvgNode`] for a wrapper whose behaviour is that of the node in
/// `self.$field` (a class that adds nothing to its base, like `SkSVGG`).
macro_rules! impl_svg_node_by_delegation {
    ($field:ident) => {
        fn base(&self) -> &$crate::node::NodeBase {
            $crate::node::SvgNode::base(&self.$field)
        }

        fn base_mut(&mut self) -> &mut $crate::node::NodeBase {
            $crate::node::SvgNode::base_mut(&mut self.$field)
        }

        fn as_any(&self) -> &dyn ::std::any::Any {
            self
        }

        fn into_any_arc(
            self: ::std::sync::Arc<Self>,
        ) -> ::std::sync::Arc<dyn ::std::any::Any + Send + Sync> {
            self
        }

        fn append_child(&mut self, node: $crate::node::Node) {
            $crate::node::SvgNode::append_child(&mut self.$field, node);
        }

        fn parse_and_set_attribute(&mut self, name: &str, value: &str) -> bool {
            $crate::node::SvgNode::parse_and_set_attribute(&mut self.$field, name, value)
        }

        fn on_prepare_to_render(
            &self,
            ctx: &mut $crate::render_context::RenderContext<'_>,
        ) -> bool {
            $crate::node::SvgNode::on_prepare_to_render(&self.$field, ctx)
        }

        fn on_render(&self, ctx: &$crate::render_context::RenderContext<'_>) {
            $crate::node::SvgNode::on_render(&self.$field, ctx);
        }

        fn on_as_paint(
            &self,
            ctx: &$crate::render_context::RenderContext<'_>,
            paint: &mut ::skia_rust_core::paint::Paint,
        ) -> bool {
            $crate::node::SvgNode::on_as_paint(&self.$field, ctx, paint)
        }

        fn on_as_path(
            &self,
            ctx: &$crate::render_context::RenderContext<'_>,
        ) -> ::skia_rust_core::path::Path {
            $crate::node::SvgNode::on_as_path(&self.$field, ctx)
        }

        fn on_set_attribute(
            &mut self,
            attr: $crate::attribute::Attribute,
            value: &$crate::value::Value<'_>,
        ) {
            $crate::node::SvgNode::on_set_attribute(&mut self.$field, attr, value);
        }

        fn has_children(&self) -> bool {
            $crate::node::SvgNode::has_children(&self.$field)
        }

        fn on_object_bounding_box(
            &self,
            ctx: &$crate::render_context::BboxContext<'_>,
        ) -> ::skia_rust_core::rect::Rect {
            $crate::node::SvgNode::on_object_bounding_box(&self.$field, ctx)
        }
    };
}
pub(crate) use impl_svg_node_by_delegation;
