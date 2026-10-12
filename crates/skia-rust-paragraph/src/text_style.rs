// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/TextStyle.h,
// modules/skparagraph/src/TextStyle.cpp

//! `TextStyle`: the attributes of a run of text, with the blocks and placeholders that carry it.

use std::ops::Range;

use bitflags::bitflags;
use skia_rust_core::color::Color;
use skia_rust_core::floating_point::{is_finite, is_finite_all};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_arguments::FontArguments as CoreFontArguments;
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::paint::Paint;
use skia_rust_core::scalar::{SCALAR_NEARLY_ZERO, Scalar, scalar};
use skia_rust_core::typeface::Typeface;

use crate::dart_types::{EMPTY_INDEX, EMPTY_RANGE, RangeExtensions, TextBaseline};
use crate::font_arguments::FontArguments;
use crate::paragraph_painter::{PaintId, PaintOrId};
use crate::text_shadow::TextShadow;

// Port of: modules/skparagraph/include/TextStyle.h#L26 (chrome/m156)
/// `DEFAULT_FONT_FAMILY`.
pub const DEFAULT_FONT_FAMILY: &str = "sans-serif";

// Port of: modules/skparagraph/include/TextStyle.h#L33-L38 (chrome/m156)
/// `nearlyZero(x, tolerance)`: `SkScalarNearlyZero` for a finite `x`, false otherwise.
#[must_use]
#[doc(alias = "nearlyZero")]
pub fn nearly_zero(x: scalar, tolerance: scalar) -> bool {
    if is_finite(x) {
        return x.nearly_zero(tolerance);
    }
    false
}

// Port of: modules/skparagraph/include/TextStyle.h#L40-L47 (chrome/m156)
/// `nearlyEqual(x, y, tolerance)`: `SkScalarNearlyEqual` for finite values, else `x == y`.
#[must_use]
#[doc(alias = "nearlyEqual")]
#[allow(clippy::float_cmp)] // mirrors `return x == y` (Inf == Inf, anything else is false)
pub fn nearly_equal(x: scalar, y: scalar, tolerance: scalar) -> bool {
    if is_finite_all(x, &[y]) {
        return scalar::nearly_equal(x, y, tolerance);
    }
    // Inf == Inf, anything else is false
    x == y
}

/// `nearlyZero(x)` with the default tolerance `SK_ScalarNearlyZero`.
#[must_use]
pub fn nearly_zero_default(x: scalar) -> bool {
    nearly_zero(x, SCALAR_NEARLY_ZERO)
}

/// `nearlyEqual(x, y)` with the default tolerance `SK_ScalarNearlyZero`.
#[must_use]
pub fn nearly_equal_default(x: scalar, y: scalar) -> bool {
    nearly_equal(x, y, SCALAR_NEARLY_ZERO)
}

bitflags! {
    // Port of: modules/skparagraph/include/TextStyle.h#L51-L65 (chrome/m156)
    /// `TextDecoration`: multiple decorations can be applied at once. Ex: Underline and overline
    /// is (0x1 | 0x2).
    #[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct TextDecoration: u32 {
        /// `kNoDecoration`.
        const NO_DECORATION = 0x0;
        /// `kUnderline`.
        const UNDERLINE = 0x1;
        /// `kOverline`.
        const OVERLINE = 0x2;
        /// `kLineThrough`.
        const LINE_THROUGH = 0x4;
    }
}

impl Default for TextDecoration {
    fn default() -> Self {
        TextDecoration::NO_DECORATION
    }
}

impl TextDecoration {
    /// All the decorations.
    pub const ALL: TextDecoration = TextDecoration::all();
}

// Port of: modules/skparagraph/include/TextStyle.h#L59-L64 (chrome/m156)
/// `AllTextDecorations`.
#[doc(alias = "AllTextDecorations")]
pub const ALL_TEXT_DECORATIONS: [TextDecoration; 4] = [
    TextDecoration::NO_DECORATION,
    TextDecoration::UNDERLINE,
    TextDecoration::OVERLINE,
    TextDecoration::LINE_THROUGH,
];

// Port of: modules/skparagraph/include/TextStyle.h#L66 (chrome/m156)
/// `TextDecorationStyle`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TextDecorationStyle {
    /// `kSolid`.
    #[default]
    Solid,
    /// `kDouble`.
    Double,
    /// `kDotted`.
    Dotted,
    /// `kDashed`.
    Dashed,
    /// `kWavy`.
    Wavy,
}

// Port of: modules/skparagraph/include/TextStyle.h#L68 (chrome/m156)
/// `TextDecorationMode`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum TextDecorationMode {
    /// `kGaps`.
    #[default]
    Gaps,
    /// `kThrough`.
    Through,
}

// Port of: modules/skparagraph/include/TextStyle.h#L70-L80 (chrome/m156)
/// `StyleType`.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum StyleType {
    /// `kNone`.
    #[default]
    None,
    /// `kAllAttributes`.
    AllAttributes,
    /// `kFont`.
    Font,
    /// `kForeground`.
    Foreground,
    /// `kBackground`.
    Background,
    /// `kShadow`.
    Shadow,
    /// `kDecorations`.
    Decorations,
    /// `kLetterSpacing`.
    LetterSpacing,
    /// `kWordSpacing`.
    WordSpacing,
}

// Port of: modules/skparagraph/include/TextStyle.h#L82-L95 (chrome/m156)
/// `Decoration`: the decorations of a text style.
#[derive(Debug, Copy, Clone)]
pub struct Decoration {
    /// `fType`.
    pub ty: TextDecoration,
    /// `fMode`.
    pub mode: TextDecorationMode,
    /// `fColor`.
    pub color: Color,
    /// `fStyle`.
    pub style: TextDecorationStyle,
    /// `fThicknessMultiplier`.
    pub thickness_multiplier: scalar,
}

impl Default for Decoration {
    fn default() -> Self {
        Self {
            ty: TextDecoration::default(),
            mode: TextDecorationMode::default(),
            color: Color::TRANSPARENT,
            style: TextDecorationStyle::default(),
            thickness_multiplier: 1.0,
        }
    }
}

impl PartialEq for Decoration {
    // Port of: modules/skparagraph/include/TextStyle.h#L89-L94 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.mode == other.mode
            && self.color == other.color
            && self.style == other.style
            && self.thickness_multiplier == other.thickness_multiplier
    }
}

// Port of: modules/skparagraph/include/TextStyle.h#L98-L123 (chrome/m156)
/// `PlaceholderAlignment`: where to vertically align the placeholder relative to the surrounding
/// text.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, Default)]
#[repr(i32)]
pub enum PlaceholderAlignment {
    /// Match the baseline of the placeholder with the baseline.
    #[default]
    Baseline,
    /// Align the bottom edge of the placeholder with the baseline such that the placeholder sits
    /// on top of the baseline.
    AboveBaseline,
    /// Align the top edge of the placeholder with the baseline specified in such that the
    /// placeholder hangs below the baseline.
    BelowBaseline,
    /// Align the top edge of the placeholder with the top edge of the font. When the placeholder
    /// is very tall, the extra space will hang from the top and extend through the bottom of the
    /// line.
    Top,
    /// Align the bottom edge of the placeholder with the top edge of the font. When the
    /// placeholder is very tall, the extra space will rise from the bottom and extend through the
    /// top of the line.
    Bottom,
    /// Align the middle of the placeholder with the middle of the text. When the placeholder is
    /// very tall, the extra space will grow equally from the top and bottom of the line.
    Middle,
}

// Port of: modules/skparagraph/include/TextStyle.h#L125-L132 (chrome/m156)
/// `FontFeature`: an OpenType feature of a text style.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FontFeature {
    /// `fName`.
    pub name: String,
    /// `fValue`.
    pub value: i32,
}

impl FontFeature {
    /// `FontFeature(SkString name, int value)`.
    #[must_use]
    pub fn new(name: impl Into<String>, value: i32) -> Self {
        Self {
            name: name.into(),
            value,
        }
    }

    /// `fName`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// `fValue`.
    #[must_use]
    pub fn value(&self) -> i32 {
        self.value
    }
}

// Port of: modules/skparagraph/include/TextStyle.h#L134-L163 (chrome/m156)
/// `PlaceholderStyle`.
#[derive(Debug, Copy, Clone)]
pub struct PlaceholderStyle {
    /// `fWidth`.
    pub width: scalar,
    /// `fHeight`.
    pub height: scalar,
    /// `fAlignment`.
    pub alignment: PlaceholderAlignment,
    /// `fBaseline`.
    pub baseline: TextBaseline,
    /// Distance from the top edge of the rect to the baseline position. This baseline will be
    /// aligned against the alphabetic baseline of the surrounding text.
    ///
    /// Positive values drop the baseline lower (positions the rect higher) and small or negative
    /// values will cause the rect to be positioned underneath the line. When baseline == height,
    /// the bottom edge of the rect will rest on the alphabetic baseline.
    pub baseline_offset: scalar,
}

impl Default for PlaceholderStyle {
    // Port of: modules/skparagraph/include/TextStyle.h#L136-L137 and #L154-L162 (chrome/m156)
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            alignment: PlaceholderAlignment::Baseline,
            baseline: TextBaseline::Alphabetic,
            baseline_offset: 0.0,
        }
    }
}

impl PartialEq for PlaceholderStyle {
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl PlaceholderStyle {
    // Port of: modules/skparagraph/include/TextStyle.h#L137-L145 (chrome/m156)
    /// `PlaceholderStyle(width, height, alignment, baseline, offset)`.
    #[must_use]
    pub fn new(
        width: scalar,
        height: scalar,
        alignment: PlaceholderAlignment,
        baseline: TextBaseline,
        offset: scalar,
    ) -> Self {
        Self {
            width,
            height,
            alignment,
            baseline,
            baseline_offset: offset,
        }
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L198-L206 (chrome/m156)
    /// `equals(other)`.
    #[must_use]
    pub fn equals(&self, other: &PlaceholderStyle) -> bool {
        nearly_equal_default(self.width, other.width)
            && nearly_equal_default(self.height, other.height)
            && self.alignment == other.alignment
            && self.baseline == other.baseline
            && (self.alignment != PlaceholderAlignment::Baseline
                || nearly_equal_default(self.baseline_offset, other.baseline_offset))
    }
}

// Port of: modules/skparagraph/include/TextStyle.h#L165-L360 (chrome/m156)
/// `TextStyle`: the font, color, decorations, shadows and spacing of text.
#[derive(Debug, Clone)]
pub struct TextStyle {
    decoration: Decoration,
    font_style: FontStyle,
    font_families: Vec<String>,
    font_size: scalar,
    edging: Edging,
    subpixel: bool,
    hinting: FontHinting,
    height: scalar,
    height_override: bool,
    baseline_shift: scalar,
    // true: half leading.
    // false: scale ascent/descent with fHeight.
    half_leading: bool,
    locale: String,
    letter_spacing: scalar,
    word_spacing: scalar,
    text_baseline: TextBaseline,
    color: Color,
    has_background: bool,
    background: PaintOrId,
    has_foreground: bool,
    foreground: PaintOrId,
    text_shadows: Vec<TextShadow>,
    typeface: Option<Typeface>,
    is_placeholder: bool,
    font_features: Vec<FontFeature>,
    font_arguments: Option<FontArguments>,
}

impl Default for TextStyle {
    // Port of: modules/skparagraph/include/TextStyle.h#L165-L167 and the member initializers of
    // #L300-L347 (chrome/m156)
    fn default() -> Self {
        Self {
            decoration: Decoration {
                ty: TextDecoration::NO_DECORATION,
                // TODO: switch back to kGaps when (if) switching flutter to skparagraph
                mode: TextDecorationMode::Through,
                // It does not make sense to draw a transparent object, so we use this as a
                // default value to indicate no decoration color was set.
                color: Color::TRANSPARENT,
                style: TextDecorationStyle::Solid,
                // Thickness is applied as a multiplier to the default thickness of the font.
                thickness_multiplier: 1.0,
            },
            font_style: FontStyle::default(),
            font_families: vec![DEFAULT_FONT_FAMILY.to_owned()],
            font_size: 14.0,
            edging: Edging::AntiAlias,
            subpixel: true,
            hinting: FontHinting::Slight,
            height: 1.0,
            height_override: false,
            baseline_shift: 0.0,
            half_leading: false,
            locale: String::new(),
            letter_spacing: 0.0,
            word_spacing: 0.0,
            text_baseline: TextBaseline::Alphabetic,
            color: Color::WHITE,
            has_background: false,
            background: PaintOrId::default(),
            has_foreground: false,
            foreground: PaintOrId::default(),
            text_shadows: Vec::new(),
            typeface: None,
            is_placeholder: false,
            font_features: Vec::new(),
            font_arguments: None,
        }
    }
}

impl PartialEq for TextStyle {
    // Port of: modules/skparagraph/include/TextStyle.h#L176 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.equals(other)
    }
}

impl TextStyle {
    /// `TextStyle()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L18-L35 (chrome/m156)
    /// `cloneForPlaceholder()`.
    #[must_use]
    #[doc(alias = "cloneForPlaceholder")]
    pub fn clone_for_placeholder(&self) -> Self {
        let mut result = TextStyle::default();
        result.color = self.color;
        result.font_size = self.font_size;
        result.font_families.clone_from(&self.font_families);
        result.decoration = self.decoration;
        result.has_background = self.has_background;
        result.has_foreground = self.has_foreground;
        result.background = self.background.clone();
        result.foreground = self.foreground.clone();
        result.height_override = self.height_override;
        result.is_placeholder = true;
        result.font_features.clone_from(&self.font_features);
        result.half_leading = self.half_leading;
        result.baseline_shift = self.baseline_shift;
        result.font_arguments.clone_from(&self.font_arguments);
        result
    }

    /// `cloneForPlaceholder()`, named as `skia_safe::textlayout::TextStyle::to_placeholder`.
    #[must_use]
    pub fn to_placeholder(&self) -> Self {
        self.clone_for_placeholder()
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L37-L94 (chrome/m156)
    /// `equals(other)`.
    #[must_use]
    #[allow(clippy::float_cmp)] // mirrors the float `!=` comparisons of TextStyle::equals
    pub fn equals(&self, other: &TextStyle) -> bool {
        if self.is_placeholder || other.is_placeholder {
            return false;
        }
        if self.color != other.color {
            return false;
        }
        if self.decoration != other.decoration {
            return false;
        }
        if self.font_style != other.font_style {
            return false;
        }
        if self.font_families != other.font_families {
            return false;
        }
        if self.letter_spacing != other.letter_spacing {
            return false;
        }
        if self.word_spacing != other.word_spacing {
            return false;
        }
        if self.height != other.height {
            return false;
        }
        if self.height_override != other.height_override {
            return false;
        }
        if self.half_leading != other.half_leading {
            return false;
        }
        if self.baseline_shift != other.baseline_shift {
            return false;
        }
        if self.font_size != other.font_size {
            return false;
        }
        if self.locale != other.locale {
            return false;
        }
        if self.has_foreground != other.has_foreground || self.foreground != other.foreground {
            return false;
        }
        if self.has_background != other.has_background || self.background != other.background {
            return false;
        }
        if self.text_shadows.len() != other.text_shadows.len() {
            return false;
        }
        for i in 0..self.text_shadows.len() {
            if self.text_shadows[i] != other.text_shadows[i] {
                return false;
            }
        }
        if self.font_features.len() != other.font_features.len() {
            return false;
        }
        for i in 0..self.font_features.len() {
            if self.font_features[i] != other.font_features[i] {
                return false;
            }
        }
        if self.font_arguments != other.font_arguments {
            return false;
        }
        true
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L96-L110 (chrome/m156)
    /// `equalsByFonts(that)`.
    #[must_use]
    #[doc(alias = "equalsByFonts")]
    pub fn equals_by_fonts(&self, that: &TextStyle) -> bool {
        !self.is_placeholder
            && !that.is_placeholder
            && self.font_style == that.font_style
            && self.font_families == that.font_families
            && self.font_features == that.font_features
            && self.font_arguments == that.font_arguments
            && nearly_equal_default(self.letter_spacing, that.letter_spacing)
            && nearly_equal_default(self.word_spacing, that.word_spacing)
            && nearly_equal_default(self.height, that.height)
            && nearly_equal_default(self.baseline_shift, that.baseline_shift)
            && nearly_equal_default(self.font_size, that.font_size)
            && self.locale == that.locale
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L112-L156 (chrome/m156)
    /// `matchOneAttribute(styleType, other)`.
    #[must_use]
    #[doc(alias = "matchOneAttribute")]
    #[allow(clippy::float_cmp)] // mirrors the float `==` comparisons of matchOneAttribute
    pub fn match_one_attribute(&self, style_type: StyleType, other: &TextStyle) -> bool {
        match style_type {
            StyleType::Foreground => {
                (!self.has_foreground && !other.has_foreground && self.color == other.color)
                    || (self.has_foreground
                        && other.has_foreground
                        && self.foreground == other.foreground)
            }
            StyleType::Background => {
                (!self.has_background && !other.has_background)
                    || (self.has_background
                        && other.has_background
                        && self.background == other.background)
            }
            StyleType::Shadow => {
                if self.text_shadows.len() != other.text_shadows.len() {
                    return false;
                }
                for i in 0..self.text_shadows.len() {
                    if self.text_shadows[i] != other.text_shadows[i] {
                        return false;
                    }
                }
                true
            }
            StyleType::Decorations => self.decoration == other.decoration,
            StyleType::LetterSpacing => self.letter_spacing == other.letter_spacing,
            StyleType::WordSpacing => self.word_spacing == other.word_spacing,
            StyleType::AllAttributes => self.equals(other),
            StyleType::Font => {
                // TODO: should not we take typefaces in account?
                self.font_style == other.font_style
                    && self.locale == other.locale
                    && self.font_families == other.font_families
                    && self.font_size == other.font_size
                    && self.height == other.height
                    && self.half_leading == other.half_leading
                    && self.baseline_shift == other.baseline_shift
                    && self.font_arguments == other.font_arguments
            }
            StyleType::None => {
                debug_assert!(false);
                false
            }
        }
    }

    // Colors

    /// `getColor()`.
    #[must_use]
    #[doc(alias = "getColor")]
    pub fn color(&self) -> Color {
        self.color
    }

    /// `setColor(color)`.
    #[doc(alias = "setColor")]
    pub fn set_color(&mut self, color: impl Into<Color>) -> &mut Self {
        self.color = color.into();
        self
    }

    /// `hasForeground()`.
    #[must_use]
    #[doc(alias = "hasForeground")]
    pub fn has_foreground(&self) -> bool {
        self.has_foreground
    }

    /// `getForeground()`: the paint, or a default paint for a paint id.
    #[must_use]
    #[doc(alias = "getForeground")]
    pub fn foreground(&self) -> Paint {
        match &self.foreground {
            PaintOrId::Paint(paint) => paint.clone(),
            PaintOrId::Id(_) => Paint::default(),
        }
    }

    /// `getForegroundPaintOrID()`.
    #[must_use]
    #[doc(alias = "getForegroundPaintOrID")]
    pub fn foreground_paint_or_id(&self) -> PaintOrId {
        self.foreground.clone()
    }

    /// `setForegroundPaint(paint)`.
    #[doc(alias = "setForegroundPaint")]
    pub fn set_foreground_paint(&mut self, paint: &Paint) -> &mut Self {
        self.has_foreground = true;
        self.foreground = PaintOrId::Paint(paint.clone());
        self
    }

    /// `setForegroundColor(paint)`: deprecated, prefer [`TextStyle::set_foreground_paint`].
    #[doc(alias = "setForegroundColor")]
    pub fn set_foreground_color(&mut self, paint: &Paint) -> &mut Self {
        self.set_foreground_paint(paint)
    }

    /// `setForegroundPaintID(paintID)`: sets the foreground to a paint ID, for clients that
    /// implement a custom `ParagraphPainter` that can not accept a paint.
    #[doc(alias = "setForegroundPaintID")]
    pub fn set_foreground_paint_id(&mut self, paint_id: PaintId) -> &mut Self {
        self.has_foreground = true;
        self.foreground = PaintOrId::Id(paint_id);
        self
    }

    /// `clearForegroundColor()`.
    #[doc(alias = "clearForegroundColor")]
    pub fn clear_foreground_color(&mut self) -> &mut Self {
        self.has_foreground = false;
        self
    }

    /// `hasBackground()`.
    #[must_use]
    #[doc(alias = "hasBackground")]
    pub fn has_background(&self) -> bool {
        self.has_background
    }

    /// `getBackground()`: the paint, or a default paint for a paint id.
    #[must_use]
    #[doc(alias = "getBackground")]
    pub fn background(&self) -> Paint {
        match &self.background {
            PaintOrId::Paint(paint) => paint.clone(),
            PaintOrId::Id(_) => Paint::default(),
        }
    }

    /// `getBackgroundPaintOrID()`.
    #[must_use]
    #[doc(alias = "getBackgroundPaintOrID")]
    pub fn background_paint_or_id(&self) -> PaintOrId {
        self.background.clone()
    }

    /// `setBackgroundPaint(paint)`.
    #[doc(alias = "setBackgroundPaint")]
    pub fn set_background_paint(&mut self, paint: &Paint) -> &mut Self {
        self.has_background = true;
        self.background = PaintOrId::Paint(paint.clone());
        self
    }

    /// `setBackgroundColor(paint)`: deprecated, prefer [`TextStyle::set_background_paint`].
    #[doc(alias = "setBackgroundColor")]
    pub fn set_background_color(&mut self, paint: &Paint) -> &mut Self {
        self.set_background_paint(paint)
    }

    /// `setBackgroundPaintID(paintID)`.
    #[doc(alias = "setBackgroundPaintID")]
    pub fn set_background_paint_id(&mut self, paint_id: PaintId) -> &mut Self {
        self.has_background = true;
        self.background = PaintOrId::Id(paint_id);
        self
    }

    /// `clearBackgroundColor()`.
    #[doc(alias = "clearBackgroundColor")]
    pub fn clear_background_color(&mut self) -> &mut Self {
        self.has_background = false;
        self
    }

    // Decorations

    /// `getDecoration()`.
    #[must_use]
    #[doc(alias = "getDecoration")]
    pub fn decoration(&self) -> &Decoration {
        &self.decoration
    }

    /// `getDecorationType()`.
    #[must_use]
    #[doc(alias = "getDecorationType")]
    pub fn decoration_type(&self) -> TextDecoration {
        self.decoration.ty
    }

    /// `getDecorationMode()`.
    #[must_use]
    #[doc(alias = "getDecorationMode")]
    pub fn decoration_mode(&self) -> TextDecorationMode {
        self.decoration.mode
    }

    /// `getDecorationColor()`.
    #[must_use]
    #[doc(alias = "getDecorationColor")]
    pub fn decoration_color(&self) -> Color {
        self.decoration.color
    }

    /// `getDecorationStyle()`.
    #[must_use]
    #[doc(alias = "getDecorationStyle")]
    pub fn decoration_style(&self) -> TextDecorationStyle {
        self.decoration.style
    }

    /// `getDecorationThicknessMultiplier()`.
    #[must_use]
    #[doc(alias = "getDecorationThicknessMultiplier")]
    pub fn decoration_thickness_multiplier(&self) -> scalar {
        self.decoration.thickness_multiplier
    }

    /// The whole decoration (`fDecoration = decoration`).
    pub fn set_decoration(&mut self, decoration: &Decoration) {
        self.decoration = *decoration;
    }

    /// `setDecoration(decoration)`: the type of the decoration.
    #[doc(alias = "setDecoration")]
    pub fn set_decoration_type(&mut self, decoration: TextDecoration) {
        self.decoration.ty = decoration;
    }

    /// `setDecorationMode(mode)`.
    #[doc(alias = "setDecorationMode")]
    pub fn set_decoration_mode(&mut self, mode: TextDecorationMode) {
        self.decoration.mode = mode;
    }

    /// `setDecorationStyle(style)`.
    #[doc(alias = "setDecorationStyle")]
    pub fn set_decoration_style(&mut self, style: TextDecorationStyle) {
        self.decoration.style = style;
    }

    /// `setDecorationColor(color)`.
    #[doc(alias = "setDecorationColor")]
    pub fn set_decoration_color(&mut self, color: impl Into<Color>) {
        self.decoration.color = color.into();
    }

    /// `setDecorationThicknessMultiplier(m)`.
    #[doc(alias = "setDecorationThicknessMultiplier")]
    pub fn set_decoration_thickness_multiplier(&mut self, multiplier: scalar) {
        self.decoration.thickness_multiplier = multiplier;
    }

    /// The decoration, mutably.
    pub fn decoration_mut(&mut self) -> &mut Decoration {
        &mut self.decoration
    }

    // Weight/Width/Slant

    /// `getFontStyle()`.
    #[must_use]
    #[doc(alias = "getFontStyle")]
    pub fn font_style(&self) -> FontStyle {
        self.font_style
    }

    /// `setFontStyle(fontStyle)`.
    #[doc(alias = "setFontStyle")]
    pub fn set_font_style(&mut self, font_style: FontStyle) -> &mut Self {
        self.font_style = font_style;
        self
    }

    // Shadows

    /// `getShadowNumber()`.
    #[must_use]
    #[doc(alias = "getShadowNumber")]
    pub fn shadow_number(&self) -> usize {
        self.text_shadows.len()
    }

    /// `getShadows()`.
    #[must_use]
    #[doc(alias = "getShadows")]
    pub fn shadows(&self) -> &[TextShadow] {
        &self.text_shadows
    }

    /// `addShadow(shadow)`.
    #[doc(alias = "addShadow")]
    pub fn add_shadow(&mut self, shadow: TextShadow) -> &mut Self {
        self.text_shadows.push(shadow);
        self
    }

    /// `resetShadows()`.
    #[doc(alias = "resetShadows")]
    pub fn reset_shadows(&mut self) -> &mut Self {
        self.text_shadows.clear();
        self
    }

    // Font features

    /// `getFontFeatureNumber()`.
    #[must_use]
    #[doc(alias = "getFontFeatureNumber")]
    pub fn font_feature_number(&self) -> usize {
        self.font_features.len()
    }

    /// `getFontFeatures()`.
    #[must_use]
    #[doc(alias = "getFontFeatures")]
    pub fn font_features(&self) -> &[FontFeature] {
        &self.font_features
    }

    /// `addFontFeature(fontFeature, value)`.
    #[doc(alias = "addFontFeature")]
    pub fn add_font_feature(&mut self, font_feature: impl AsRef<str>, value: i32) {
        self.font_features
            .push(FontFeature::new(font_feature.as_ref(), value));
    }

    /// `resetFontFeatures()`.
    #[doc(alias = "resetFontFeatures")]
    pub fn reset_font_features(&mut self) {
        self.font_features.clear();
    }

    // Font arguments

    /// `getFontArguments()`.
    #[must_use]
    #[doc(alias = "getFontArguments")]
    pub fn font_arguments(&self) -> Option<&FontArguments> {
        self.font_arguments.as_ref()
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L184-L190 (chrome/m156)
    /// `setFontArguments(args)`: the contents of the arguments are copied into the style.
    #[doc(alias = "setFontArguments")]
    pub fn set_font_arguments<'fa>(
        &mut self,
        arguments: impl Into<Option<&'fa CoreFontArguments<'fa, 'fa>>>,
    ) {
        match arguments.into() {
            None => self.font_arguments = None,
            Some(args) => self.font_arguments = Some(FontArguments::from(args)),
        }
    }

    /// `getFontSize()`.
    #[must_use]
    #[doc(alias = "getFontSize")]
    pub fn font_size(&self) -> scalar {
        self.font_size
    }

    /// `setFontSize(size)`.
    #[doc(alias = "setFontSize")]
    pub fn set_font_size(&mut self, size: scalar) -> &mut Self {
        self.font_size = size;
        self
    }

    /// `getFontFamilies()`.
    #[must_use]
    #[doc(alias = "getFontFamilies")]
    pub fn font_families(&self) -> &[String] {
        &self.font_families
    }

    /// `setFontFamilies(families)`.
    #[doc(alias = "setFontFamilies")]
    pub fn set_font_families(&mut self, families: &[impl AsRef<str>]) -> &mut Self {
        self.font_families = families.iter().map(|f| f.as_ref().to_owned()).collect();
        self
    }

    /// `getBaselineShift()`.
    #[must_use]
    #[doc(alias = "getBaselineShift")]
    pub fn baseline_shift(&self) -> scalar {
        self.baseline_shift
    }

    /// `setBaselineShift(baselineShift)`.
    #[doc(alias = "setBaselineShift")]
    pub fn set_baseline_shift(&mut self, baseline_shift: scalar) -> &mut Self {
        self.baseline_shift = baseline_shift;
        self
    }

    /// `setHeight(height)`.
    #[doc(alias = "setHeight")]
    pub fn set_height(&mut self, height: scalar) -> &mut Self {
        self.height = height;
        self
    }

    /// `getHeight()`: zero unless the height is overridden.
    #[must_use]
    #[doc(alias = "getHeight")]
    pub fn height(&self) -> scalar {
        if self.height_override {
            self.height
        } else {
            0.0
        }
    }

    /// `setHeightOverride(heightOverride)`.
    #[doc(alias = "setHeightOverride")]
    pub fn set_height_override(&mut self, height_override: bool) -> &mut Self {
        self.height_override = height_override;
        self
    }

    /// `getHeightOverride()`.
    #[must_use]
    #[doc(alias = "getHeightOverride")]
    pub fn height_override(&self) -> bool {
        self.height_override
    }

    /// `setHalfLeading(halfLeading)`.
    #[doc(alias = "setHalfLeading")]
    pub fn set_half_leading(&mut self, half_leading: bool) -> &mut Self {
        self.half_leading = half_leading;
        self
    }

    /// `getHalfLeading()`.
    #[must_use]
    #[doc(alias = "getHalfLeading")]
    pub fn half_leading(&self) -> bool {
        self.half_leading
    }

    /// `setLetterSpacing(letterSpacing)`.
    #[doc(alias = "setLetterSpacing")]
    pub fn set_letter_spacing(&mut self, letter_spacing: scalar) -> &mut Self {
        self.letter_spacing = letter_spacing;
        self
    }

    /// `getLetterSpacing()`.
    #[must_use]
    #[doc(alias = "getLetterSpacing")]
    pub fn letter_spacing(&self) -> scalar {
        self.letter_spacing
    }

    /// `setWordSpacing(wordSpacing)`.
    #[doc(alias = "setWordSpacing")]
    pub fn set_word_spacing(&mut self, word_spacing: scalar) -> &mut Self {
        self.word_spacing = word_spacing;
        self
    }

    /// `getWordSpacing()`.
    #[must_use]
    #[doc(alias = "getWordSpacing")]
    pub fn word_spacing(&self) -> scalar {
        self.word_spacing
    }

    /// `refTypeface()`.
    #[must_use]
    #[doc(alias = "refTypeface")]
    pub fn typeface(&self) -> Option<Typeface> {
        self.typeface.clone()
    }

    /// `setTypeface(typeface)`.
    #[doc(alias = "setTypeface")]
    pub fn set_typeface(&mut self, typeface: impl Into<Option<Typeface>>) -> &mut Self {
        self.typeface = typeface.into();
        self
    }

    /// `getLocale()`.
    #[must_use]
    #[doc(alias = "getLocale")]
    pub fn locale(&self) -> &str {
        &self.locale
    }

    /// `setLocale(locale)`.
    #[doc(alias = "setLocale")]
    pub fn set_locale(&mut self, locale: impl AsRef<str>) -> &mut Self {
        locale.as_ref().clone_into(&mut self.locale);
        self
    }

    /// `getTextBaseline()`.
    #[must_use]
    #[doc(alias = "getTextBaseline")]
    pub fn text_baseline(&self) -> TextBaseline {
        self.text_baseline
    }

    /// `setTextBaseline(baseline)`.
    #[doc(alias = "setTextBaseline")]
    pub fn set_text_baseline(&mut self, baseline: TextBaseline) -> &mut Self {
        self.text_baseline = baseline;
        self
    }

    // Port of: modules/skparagraph/src/TextStyle.cpp#L158-L182 (chrome/m156)
    /// `getFontMetrics(&metrics)`.
    #[must_use]
    #[doc(alias = "getFontMetrics")]
    pub fn font_metrics(&self) -> FontMetrics {
        let mut font = Font::from_typeface(self.typeface.clone());
        font.set_size(self.font_size);
        font.set_edging(self.edging);
        font.set_subpixel(self.subpixel);
        font.set_hinting(self.hinting);
        let (_, mut metrics) = font.metrics();
        if self.height_override {
            let multiplier = self.height * self.font_size;
            let height = metrics.descent - metrics.ascent + metrics.leading;
            metrics.ascent = (metrics.ascent - metrics.leading / 2.0) * multiplier / height;
            metrics.descent = (metrics.descent + metrics.leading / 2.0) * multiplier / height;
        } else {
            metrics.ascent -= metrics.leading / 2.0;
            metrics.descent += metrics.leading / 2.0;
        }
        // If we shift the baseline we need to make sure the shifted text fits the line
        metrics.ascent += self.baseline_shift;
        metrics.descent += self.baseline_shift;
        metrics
    }

    /// `isPlaceholder()`.
    #[must_use]
    #[doc(alias = "isPlaceholder")]
    pub fn is_placeholder(&self) -> bool {
        self.is_placeholder
    }

    /// `setPlaceholder()`.
    #[doc(alias = "setPlaceholder")]
    pub fn set_placeholder(&mut self) -> &mut Self {
        self.is_placeholder = true;
        self
    }

    /// `setFontEdging(edging)`.
    #[doc(alias = "setFontEdging")]
    pub fn set_font_edging(&mut self, edging: Edging) -> &mut Self {
        self.edging = edging;
        self
    }

    /// `getFontEdging()`.
    #[must_use]
    #[doc(alias = "getFontEdging")]
    pub fn font_edging(&self) -> Edging {
        self.edging
    }

    /// `setSubpixel(subpixel)`.
    #[doc(alias = "setSubpixel")]
    pub fn set_subpixel(&mut self, subpixel: bool) -> &mut Self {
        self.subpixel = subpixel;
        self
    }

    /// `getSubpixel()`.
    #[must_use]
    #[doc(alias = "getSubpixel")]
    pub fn subpixel(&self) -> bool {
        self.subpixel
    }

    /// `setFontHinting(hinting)`.
    #[doc(alias = "setFontHinting")]
    pub fn set_font_hinting(&mut self, hinting: FontHinting) -> &mut Self {
        self.hinting = hinting;
        self
    }

    /// `getFontHinting()`.
    #[must_use]
    #[doc(alias = "getFontHinting")]
    pub fn font_hinting(&self) -> FontHinting {
        self.hinting
    }
}

// Port of: modules/skparagraph/include/TextStyle.h#L362 (chrome/m156)
/// `TextIndex`.
pub type TextIndex = usize;
// Port of: modules/skparagraph/include/TextStyle.h#L363 (chrome/m156)
/// `TextRange`.
pub type TextRange = Range<usize>;
// Port of: modules/skparagraph/include/TextStyle.h#L364 (chrome/m156)
/// `EMPTY_TEXT`.
pub const EMPTY_TEXT: TextRange = EMPTY_RANGE;

// Port of: modules/skparagraph/include/TextStyle.h#L366-L380 (chrome/m156)
/// `Block`: a text style and the text it applies to.
#[derive(Debug, Clone)]
pub struct Block {
    /// `fRange`.
    pub range: TextRange,
    /// `fStyle`.
    pub style: TextStyle,
}

impl Default for Block {
    fn default() -> Self {
        Self {
            range: EMPTY_RANGE,
            style: TextStyle::default(),
        }
    }
}

impl Block {
    /// `Block(TextRange textRange, const TextStyle& style)`.
    #[must_use]
    pub fn new(range: TextRange, style: TextStyle) -> Self {
        Self { range, style }
    }

    // Port of: modules/skparagraph/include/TextStyle.h#L371-L374 (chrome/m156)
    /// `add(tail)`.
    pub fn add(&mut self, tail: TextRange) -> &mut Self {
        debug_assert!(self.range.end == tail.start);
        self.range = self.range.start..self.range.start + self.range.width() + tail.width();
        self
    }
}

// Port of: modules/skparagraph/include/TextStyle.h#L382 (chrome/m156)
/// `BlockIndex`.
pub type BlockIndex = usize;
// Port of: modules/skparagraph/include/TextStyle.h#L383 (chrome/m156)
/// `BlockRange`.
pub type BlockRange = Range<usize>;
// Port of: modules/skparagraph/include/TextStyle.h#L385 (chrome/m156)
/// `EMPTY_BLOCK`.
pub const EMPTY_BLOCK: usize = EMPTY_INDEX;
// Port of: modules/skparagraph/include/TextStyle.h#L386 (chrome/m156)
/// `EMPTY_BLOCKS`.
pub const EMPTY_BLOCKS: Range<usize> = EMPTY_RANGE;

// Port of: modules/skparagraph/include/TextStyle.h#L388-L410 (chrome/m156)
/// `Placeholder`.
#[derive(Debug, Clone)]
pub struct Placeholder {
    /// `fRange`.
    pub range: TextRange,
    /// `fStyle`.
    pub style: PlaceholderStyle,
    /// `fTextStyle`.
    pub text_style: TextStyle,
    /// `fBlocksBefore`.
    pub blocks_before: BlockRange,
    /// `fTextBefore`.
    pub text_before: TextRange,
}

impl Default for Placeholder {
    fn default() -> Self {
        Self {
            range: EMPTY_RANGE,
            style: PlaceholderStyle::default(),
            text_style: TextStyle::default(),
            blocks_before: 0..0,
            text_before: 0..0,
        }
    }
}

impl Placeholder {
    /// `Placeholder(start, end, style, textStyle, blocksBefore, textBefore)`.
    #[must_use]
    pub fn new(
        range: Range<usize>,
        style: PlaceholderStyle,
        text_style: TextStyle,
        blocks_before: BlockRange,
        text_before: TextRange,
    ) -> Self {
        Self {
            range,
            style,
            text_style,
            blocks_before,
            text_before,
        }
    }
}
