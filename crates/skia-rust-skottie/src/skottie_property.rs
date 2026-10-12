// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: modules/skottie/include/SkottieProperty.h, modules/skottie/src/SkottieProperty.cpp
// (chrome/m156)
//
// Property observers and handles, including the text property (`TextPropertyValue`,
// `TextPropertyHandle`, `PropertyObserver::onTextProperty`) of the text layers.

use std::marker::PhantomData;
use std::rc::Rc;

use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::Color;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Join;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::utils::text_utils::Align;
use skia_rust_sksg::{Color as SgColor, OpacityEffect};

use crate::internal::skottie_priv::SceneGraphRevalidator;
use crate::internal::text::text_adapter::TextAdapter;
use crate::internal::transform::TransformAdapter2D;
use crate::text_shaper::{Capitalization, Direction, LinebreakPolicy, ResizePolicy, VAlign};

/// The value of a color property.
// Port of: modules/skottie/include/SkottieProperty.h#L45 (chrome/m156) (`ColorPropertyValue`)
pub type ColorPropertyValue = Color;

/// The value of an opacity property: a percentage.
// Port of: modules/skottie/include/SkottieProperty.h#L46 (chrome/m156) (`OpacityPropertyValue`)
pub type OpacityPropertyValue = f32;

/// The order a text's fill and stroke are painted in.
// Port of: modules/skottie/include/SkottieProperty.h#L46-L49 (chrome/m156) (`TextPaintOrder`)
#[doc(alias = "skottie::TextPaintOrder")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextPaintOrder {
    /// The fill, then the stroke (`kFillStroke`).
    #[default]
    FillStroke,
    /// The stroke, then the fill (`kStrokeFill`).
    StrokeFill,
}

/// What a [`GlyphDecorator`] is told about a glyph.
// Port of: modules/skottie/include/SkottieProperty.h#L55-L60 (chrome/m156) (`GlyphDecorator::GlyphInfo`)
#[doc(alias = "skottie::GlyphDecorator::GlyphInfo")]
#[derive(Debug, Clone)]
pub struct GlyphInfo {
    /// The visual glyph bounds (`fBounds`).
    pub bounds: Rect,
    /// The glyph matrix (`fMatrix`).
    pub matrix: Matrix,
    /// The cluster index in the original text string (`fCluster`).
    pub cluster: usize,
    /// The horizontal glyph advance (`fAdvance`).
    pub advance: f32,
}

/// What a [`GlyphDecorator`] is told about the text.
// Port of: modules/skottie/include/SkottieProperty.h#L62-L65 (chrome/m156) (`GlyphDecorator::TextInfo`)
#[doc(alias = "skottie::GlyphDecorator::TextInfo")]
#[derive(Debug, Clone, Copy)]
pub struct TextInfo<'a> {
    /// The glyphs (`fGlyphs`).
    pub glyphs: &'a [GlyphInfo],
    /// Additional font scale applied by auto-sizing (`fScale`).
    pub scale: f32,
}

/// Optional callback invoked when drawing text layers. Allows clients to render custom text
/// decorations.
// Port of: modules/skottie/include/SkottieProperty.h#L51-L68 (chrome/m156) (`class GlyphDecorator`)
#[doc(alias = "skottie::GlyphDecorator")]
pub trait GlyphDecorator {
    /// Draws the decoration (`onDecorate`).
    #[doc(alias = "onDecorate")]
    fn on_decorate(&self, canvas: &Canvas, info: &TextInfo<'_>);
}

/// The value of a text property.
// Port of: modules/skottie/include/SkottieProperty.h#L70-L108 (chrome/m156) (`TextPropertyValue`)
#[doc(alias = "skottie::TextPropertyValue")]
#[derive(Clone)]
pub struct TextPropertyValue {
    /// The typeface (`fTypeface`).
    pub typeface: Option<Typeface>,
    /// The text (`fText`).
    pub text: String,
    /// The text size (`fTextSize`).
    pub text_size: f32,
    /// The minimum text size, when auto-sizing (`fMinTextSize`).
    pub min_text_size: f32,
    /// The maximum text size, when auto-sizing (`fMaxTextSize`).
    pub max_text_size: f32,
    /// The stroke width (`fStrokeWidth`).
    pub stroke_width: f32,
    /// The line height (`fLineHeight`).
    pub line_height: f32,
    /// The line shift (`fLineShift`).
    pub line_shift: f32,
    /// The ascent (`fAscent`).
    pub ascent: f32,
    /// The maximum number of lines, when auto-sizing (`fMaxLines`).
    pub max_lines: usize,
    /// The horizontal alignment (`fHAlign`).
    pub h_align: Align,
    /// The vertical alignment (`fVAlign`).
    pub v_align: VAlign,
    /// The resize policy (`fResize`).
    pub resize: ResizePolicy,
    /// The line break policy (`fLineBreak`).
    pub line_break: LinebreakPolicy,
    /// The text direction (`fDirection`).
    pub direction: Direction,
    /// The capitalization (`fCapitalization`).
    pub capitalization: Capitalization,
    /// The text box (`fBox`).
    pub box_: Rect,
    /// The fill color (`fFillColor`).
    pub fill_color: Color,
    /// The stroke color (`fStrokeColor`).
    pub stroke_color: Color,
    /// The paint order (`fPaintOrder`).
    pub paint_order: TextPaintOrder,
    /// The stroke join (`fStrokeJoin`).
    pub stroke_join: Join,
    /// True if the text has a fill (`fHasFill`).
    pub has_fill: bool,
    /// True if the text has a stroke (`fHasStroke`).
    pub has_stroke: bool,
    /// The glyph decorator (`fDecorator`).
    pub decorator: Option<Rc<dyn GlyphDecorator>>,
    /// The locale to be used for text shaping, in BCP47 form. This includes support for RFC6067
    /// extensions, so one can e.g. select strict line breaking rules for certain scripts:
    /// `ja-u-lb-strict`. Empty to use the system locale (`fLocale`).
    pub locale: String,
    /// Optional font family name, to be passed to the font manager for fallback
    /// (`fFontFamily`).
    pub font_family: String,
    /// Extra spacing between glyphs (letter tracking), in 1/1000 em units (`fTextTracking`).
    pub text_tracking: f32,
}

impl Default for TextPropertyValue {
    // Port of: modules/skottie/include/SkottieProperty.h#L70-L104 (chrome/m156) (member
    // initializers)
    fn default() -> Self {
        Self {
            typeface: None,
            text: String::new(),
            text_size: 0.0,
            min_text_size: 0.0,
            max_text_size: f32::MAX,
            stroke_width: 0.0,
            line_height: 0.0,
            line_shift: 0.0,
            ascent: 0.0,
            max_lines: 0,
            h_align: Align::Left,
            v_align: VAlign::Top,
            resize: ResizePolicy::None,
            line_break: LinebreakPolicy::Explicit,
            direction: Direction::LTR,
            capitalization: Capitalization::None,
            box_: Rect::new_empty(),
            fill_color: Color::TRANSPARENT,
            stroke_color: Color::TRANSPARENT,
            paint_order: TextPaintOrder::FillStroke,
            stroke_join: Join::Miter,
            has_fill: false,
            has_stroke: false,
            decorator: None,
            locale: String::new(),
            font_family: String::new(),
            text_tracking: 0.0,
        }
    }
}

impl std::fmt::Debug for TextPropertyValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TextPropertyValue")
            .field("text", &self.text)
            .field("text_size", &self.text_size)
            .finish_non_exhaustive()
    }
}

impl PartialEq for TextPropertyValue {
    // Port of: modules/skottie/src/SkottieProperty.cpp#L18-L44 (chrome/m156)
    #[allow(clippy::float_cmp)] // exact float comparison, as in Skia
    fn eq(&self, other: &Self) -> bool {
        let same_decorator = match (&self.decorator, &other.decorator) {
            (None, None) => true,
            (Some(a), Some(b)) => std::ptr::addr_eq(Rc::as_ptr(a), Rc::as_ptr(b)),
            _ => false,
        };
        self.typeface == other.typeface
            && self.text == other.text
            && self.text_size == other.text_size
            && self.stroke_width == other.stroke_width
            && self.line_height == other.line_height
            && self.line_shift == other.line_shift
            && self.ascent == other.ascent
            && self.max_lines == other.max_lines
            && self.h_align == other.h_align
            && self.v_align == other.v_align
            && self.resize == other.resize
            && self.line_break == other.line_break
            && self.direction == other.direction
            && self.capitalization == other.capitalization
            && self.box_ == other.box_
            && self.fill_color == other.fill_color
            && self.stroke_color == other.stroke_color
            && self.paint_order == other.paint_order
            && self.stroke_join == other.stroke_join
            && self.has_fill == other.has_fill
            && self.has_stroke == other.has_stroke
            && same_decorator
            && self.locale == other.locale
            && self.font_family == other.font_family
            && self.text_tracking == other.text_tracking
    }
}

/// The value of a transform property.
// Port of: modules/skottie/include/SkottieProperty.h#L121-L132 (chrome/m156) (`TransformPropertyValue`)
#[doc(alias = "skottie::TransformPropertyValue")]
#[derive(Debug, Clone, Copy, Default)]
pub struct TransformPropertyValue {
    /// The anchor point (`fAnchorPoint`).
    pub anchor_point: Point,
    /// The position (`fPosition`).
    pub position: Point,
    /// The scale, in percent (`fScale`).
    pub scale: Vector,
    /// The rotation, in degrees (`fRotation`).
    pub rotation: f32,
    /// The skew (`fSkew`).
    pub skew: f32,
    /// The skew axis (`fSkewAxis`).
    pub skew_axis: f32,
}

impl PartialEq for TransformPropertyValue {
    /// Like Skia's `operator==`, this does not compare the rotation.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L50-L57 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.anchor_point == other.anchor_point
            && self.position == other.position
            && self.scale == other.scale
            && self.skew == other.skew
            && self.skew_axis == other.skew_axis
    }
}

/// A handle that gets and sets the value of a property of the scene graph. Setting a value
/// revalidates the scene graph (when the handle came from an animation).
// Port of: modules/skottie/include/SkottieProperty.h#L134-L152 (chrome/m156) (`class PropertyHandle`)
#[doc(alias = "skottie::PropertyHandle")]
pub struct PropertyHandle<V, N> {
    node: Rc<N>,
    revalidator: Option<Rc<SceneGraphRevalidator>>,
    value: PhantomData<V>,
}

impl<V, N> Clone for PropertyHandle<V, N> {
    fn clone(&self) -> Self {
        Self {
            node: Rc::clone(&self.node),
            revalidator: self.revalidator.clone(),
            value: PhantomData,
        }
    }
}

impl<V, N> std::fmt::Debug for PropertyHandle<V, N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PropertyHandle").finish_non_exhaustive()
    }
}

impl<V, N> PropertyHandle<V, N> {
    /// A handle that does not revalidate the scene graph on set.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L60-L61 (chrome/m156) (`PropertyHandle(sk_sp<NodeT>)`)
    #[must_use]
    pub fn new(node: Rc<N>) -> Self {
        Self {
            node,
            revalidator: None,
            value: PhantomData,
        }
    }

    /// A handle that revalidates the scene graph on set.
    // Port of: modules/skottie/include/SkottieProperty.h#L141-L144 (chrome/m156)
    #[must_use]
    pub(crate) fn with_revalidator(node: Rc<N>, revalidator: Rc<SceneGraphRevalidator>) -> Self {
        Self {
            node,
            revalidator: Some(revalidator),
            value: PhantomData,
        }
    }

    fn revalidate(&self) {
        if let Some(revalidator) = &self.revalidator {
            revalidator.revalidate();
        }
    }
}

/// A handle to a color property.
// Port of: modules/skottie/include/SkottieProperty.h#L165-L166 (chrome/m156) (`ColorPropertyHandle`)
pub type ColorPropertyHandle = PropertyHandle<ColorPropertyValue, SgColor>;
/// A handle to an opacity property.
// Port of: modules/skottie/include/SkottieProperty.h#L167-L168 (chrome/m156) (`OpacityPropertyHandle`)
pub type OpacityPropertyHandle = PropertyHandle<OpacityPropertyValue, OpacityEffect>;
/// A handle to a text property.
// Port of: modules/skottie/include/SkottieProperty.h#L158-L159 (chrome/m156) (`TextPropertyHandle`)
pub type TextPropertyHandle = PropertyHandle<TextPropertyValue, TextAdapter>;
/// A handle to a transform property.
// Port of: modules/skottie/include/SkottieProperty.h#L171-L172 (chrome/m156) (`TransformPropertyHandle`)
pub type TransformPropertyHandle = PropertyHandle<TransformPropertyValue, TransformAdapter2D>;

impl ColorPropertyHandle {
    /// The color.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L79-L82 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> ColorPropertyValue {
        self.node.color()
    }

    /// Sets the color.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L84-L90 (chrome/m156)
    pub fn set(&self, c: ColorPropertyValue) {
        self.node.set_color(c);
        self.revalidate();
    }
}

impl OpacityPropertyHandle {
    /// The opacity, in percent.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L105-L108 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> OpacityPropertyValue {
        self.node.opacity() * 100.0
    }

    /// Sets the opacity, in percent.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L110-L116 (chrome/m156)
    pub fn set(&self, o: OpacityPropertyValue) {
        self.node.set_opacity(o / 100.0);
        self.revalidate();
    }
}

impl TextPropertyHandle {
    /// The text properties.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L125-L128 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> TextPropertyValue {
        self.node.get_text()
    }

    /// Sets the text properties.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L130-L137 (chrome/m156)
    pub fn set(&self, t: &TextPropertyValue) {
        self.node.set_text(t);
        self.revalidate();
    }
}

impl TransformPropertyHandle {
    /// The transform properties.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L151-L160 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> TransformPropertyValue {
        TransformPropertyValue {
            anchor_point: self.node.anchor_point(),
            position: self.node.position(),
            scale: self.node.scale(),
            rotation: self.node.rotation(),
            skew: self.node.skew(),
            skew_axis: self.node.skew_axis(),
        }
    }

    /// Sets the transform properties.
    // Port of: modules/skottie/src/SkottieProperty.cpp#L162-L172 (chrome/m156)
    pub fn set(&self, t: &TransformPropertyValue) {
        self.node.set_anchor_point(t.anchor_point);
        self.node.set_position(t.position);
        self.node.set_scale(t.scale);
        self.node.set_rotation(t.rotation);
        self.node.set_skew(t.skew);
        self.node.set_skew_axis(t.skew_axis);

        self.revalidate();
    }
}

/// The kinds of nodes a [`PropertyObserver`] is told about.
// Port of: modules/skottie/include/SkottieProperty.h#L174-L175 (chrome/m156) (`PropertyObserver::NodeType`)
#[doc(alias = "skottie::PropertyObserver::NodeType")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    /// A composition.
    Composition,
    /// A layer.
    Layer,
    /// An effect.
    Effect,
    /// Anything else.
    Other,
}

/// A lazily created property handle: the observer calls it if it wants the handle
/// (`LazyHandle`). Calling it marks the property as dispatched.
// Port of: modules/skottie/include/SkottieProperty.h#L177-L178 (chrome/m156) (`PropertyObserver::LazyHandle`)
pub type LazyHandle<'a, T> = &'a dyn Fn() -> Box<T>;

/// Receives callbacks during animation parsing. The default implementations ignore everything.
/// Node names are `None` for unnamed nodes.
// Port of: modules/skottie/include/SkottieProperty.h#L174-L205 (chrome/m156) (`class PropertyObserver`)
#[doc(alias = "skottie::PropertyObserver")]
pub trait PropertyObserver {
    /// A color property (`onColorProperty`).
    #[doc(alias = "onColorProperty")]
    fn on_color_property(
        &self,
        _node_name: Option<&str>,
        _handle: LazyHandle<'_, ColorPropertyHandle>,
    ) {
    }

    /// An opacity property (`onOpacityProperty`).
    #[doc(alias = "onOpacityProperty")]
    fn on_opacity_property(
        &self,
        _node_name: Option<&str>,
        _handle: LazyHandle<'_, OpacityPropertyHandle>,
    ) {
    }

    /// A text property (`onTextProperty`).
    #[doc(alias = "onTextProperty")]
    fn on_text_property(
        &self,
        _node_name: Option<&str>,
        _handle: LazyHandle<'_, TextPropertyHandle>,
    ) {
    }

    /// A transform property (`onTransformProperty`).
    #[doc(alias = "onTransformProperty")]
    fn on_transform_property(
        &self,
        _node_name: Option<&str>,
        _handle: LazyHandle<'_, TransformPropertyHandle>,
    ) {
    }

    /// Entering a node (`onEnterNode`).
    #[doc(alias = "onEnterNode")]
    fn on_enter_node(&self, _node_name: Option<&str>, _node_type: NodeType) {}

    /// Leaving a node (`onLeavingNode`).
    #[doc(alias = "onLeavingNode")]
    fn on_leaving_node(&self, _node_name: Option<&str>, _node_type: NodeType) {}
}
