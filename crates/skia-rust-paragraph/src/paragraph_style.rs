// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skparagraph/include/ParagraphStyle.h,
// modules/skparagraph/src/ParagraphStyle.cpp

//! `StrutStyle` and `ParagraphStyle`.

use skia_rust_core::font_style::FontStyle;
use skia_rust_core::scalar::scalar;

use crate::dart_types::{TextAlign, TextDirection, TextHeightBehavior};
use crate::text_style::{TextStyle, nearly_equal_default};

// Port of: modules/skparagraph/include/ParagraphStyle.h#L26-L78 (chrome/m156)
/// `StrutStyle`: the minimum height of every line.
#[derive(Debug, Clone)]
pub struct StrutStyle {
    font_families: Vec<String>,
    font_style: FontStyle,
    font_size: scalar,
    height: scalar,
    leading: scalar,
    force_height: bool,
    enabled: bool,
    height_override: bool,
    // true: half leading.
    // false: scale ascent/descent with fHeight.
    half_leading: bool,
}

impl Default for StrutStyle {
    // Port of: modules/skparagraph/src/ParagraphStyle.cpp#L13-L22 (chrome/m156)
    fn default() -> Self {
        Self {
            font_families: Vec::new(),
            font_style: FontStyle::normal(),
            font_size: 14.0,
            height: 1.0,
            leading: -1.0,
            force_height: false,
            enabled: false,
            height_override: false,
            half_leading: false,
        }
    }
}

impl PartialEq for StrutStyle {
    // Port of: modules/skparagraph/include/ParagraphStyle.h#L62-L71 (chrome/m156)
    fn eq(&self, rhs: &Self) -> bool {
        self.enabled == rhs.enabled
            && self.height_override == rhs.height_override
            && self.force_height == rhs.force_height
            && self.half_leading == rhs.half_leading
            && nearly_equal_default(self.leading, rhs.leading)
            && nearly_equal_default(self.height, rhs.height)
            && nearly_equal_default(self.font_size, rhs.font_size)
            && self.font_style == rhs.font_style
            && self.font_families == rhs.font_families
    }
}

impl StrutStyle {
    /// `StrutStyle()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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

    /// `getFontSize()`.
    #[must_use]
    #[doc(alias = "getFontSize")]
    pub fn font_size(&self) -> scalar {
        self.font_size
    }

    /// `setFontSize(size)`.
    #[doc(alias = "setFontSize")]
    pub fn set_font_size(&mut self, font_size: scalar) -> &mut Self {
        self.font_size = font_size;
        self
    }

    /// `setHeight(height)`.
    #[doc(alias = "setHeight")]
    pub fn set_height(&mut self, height: scalar) -> &mut Self {
        self.height = height;
        self
    }

    /// `getHeight()`.
    #[must_use]
    #[doc(alias = "getHeight")]
    pub fn height(&self) -> scalar {
        self.height
    }

    /// `setLeading(Leading)`.
    #[doc(alias = "setLeading")]
    pub fn set_leading(&mut self, leading: scalar) -> &mut Self {
        self.leading = leading;
        self
    }

    /// `getLeading()`.
    #[must_use]
    #[doc(alias = "getLeading")]
    pub fn leading(&self) -> scalar {
        self.leading
    }

    /// `getStrutEnabled()`.
    #[must_use]
    #[doc(alias = "getStrutEnabled")]
    pub fn strut_enabled(&self) -> bool {
        self.enabled
    }

    /// `setStrutEnabled(v)`.
    #[doc(alias = "setStrutEnabled")]
    pub fn set_strut_enabled(&mut self, enabled: bool) -> &mut Self {
        self.enabled = enabled;
        self
    }

    /// `getForceStrutHeight()`.
    #[must_use]
    #[doc(alias = "getForceStrutHeight")]
    pub fn force_strut_height(&self) -> bool {
        self.force_height
    }

    /// `setForceStrutHeight(v)`.
    #[doc(alias = "setForceStrutHeight")]
    pub fn set_force_strut_height(&mut self, force_height: bool) -> &mut Self {
        self.force_height = force_height;
        self
    }

    /// `getHeightOverride()`.
    #[must_use]
    #[doc(alias = "getHeightOverride")]
    pub fn height_override(&self) -> bool {
        self.height_override
    }

    /// `setHeightOverride(v)`.
    #[doc(alias = "setHeightOverride")]
    pub fn set_height_override(&mut self, height_override: bool) -> &mut Self {
        self.height_override = height_override;
        self
    }

    /// `getHalfLeading()`.
    #[must_use]
    #[doc(alias = "getHalfLeading")]
    pub fn half_leading(&self) -> bool {
        self.half_leading
    }

    /// `setHalfLeading(halfLeading)`.
    #[doc(alias = "setHalfLeading")]
    pub fn set_half_leading(&mut self, half_leading: bool) -> &mut Self {
        self.half_leading = half_leading;
        self
    }
}

// Port of: modules/skparagraph/include/ParagraphStyle.h#L80-L156 (chrome/m156)
/// `ParagraphStyle`.
#[derive(Debug, Clone)]
pub struct ParagraphStyle {
    strut_style: StrutStyle,
    default_text_style: TextStyle,
    text_align: TextAlign,
    text_direction: TextDirection,
    lines_limit: usize,
    ellipsis_utf16: Vec<u16>,
    ellipsis: String,
    height: scalar,
    text_height_behavior: TextHeightBehavior,
    hinting_is_on: bool,
    replace_tab_characters: bool,
    fake_missing_font_styles: bool,
    apply_rounding_hack: bool,
    letter_spacing_by_css_spec: bool,
    render_soft_hyphens: bool,
}

impl Default for ParagraphStyle {
    // Port of: modules/skparagraph/src/ParagraphStyle.cpp#L24-L32 (chrome/m156), with the member
    // initializers of ParagraphStyle.h#L150-L152
    fn default() -> Self {
        Self {
            strut_style: StrutStyle::default(),
            default_text_style: TextStyle::default(),
            text_align: TextAlign::Start,
            text_direction: TextDirection::LTR,
            lines_limit: usize::MAX,
            ellipsis_utf16: Vec::new(),
            ellipsis: String::new(),
            height: 1.0,
            text_height_behavior: TextHeightBehavior::All,
            hinting_is_on: true,
            replace_tab_characters: false,
            fake_missing_font_styles: true,
            apply_rounding_hack: true,
            letter_spacing_by_css_spec: false,
            render_soft_hyphens: false,
        }
    }
}

impl PartialEq for ParagraphStyle {
    // Port of: modules/skparagraph/include/ParagraphStyle.h#L83-L91 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors `this->fHeight == rhs.fHeight`
    fn eq(&self, rhs: &Self) -> bool {
        self.height == rhs.height
            && self.ellipsis == rhs.ellipsis
            && self.ellipsis_utf16 == rhs.ellipsis_utf16
            && self.text_direction == rhs.text_direction
            && self.text_align == rhs.text_align
            && self.default_text_style == rhs.default_text_style
            && self.replace_tab_characters == rhs.replace_tab_characters
            && self.fake_missing_font_styles == rhs.fake_missing_font_styles
            && self.render_soft_hyphens == rhs.render_soft_hyphens
    }
}

impl ParagraphStyle {
    /// `ParagraphStyle()`.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `getStrutStyle()`.
    #[must_use]
    #[doc(alias = "getStrutStyle")]
    pub fn strut_style(&self) -> &StrutStyle {
        &self.strut_style
    }

    /// `setStrutStyle(strutStyle)`.
    #[doc(alias = "setStrutStyle")]
    pub fn set_strut_style(&mut self, strut_style: StrutStyle) -> &mut Self {
        self.strut_style = strut_style;
        self
    }

    /// `getTextStyle()`.
    #[must_use]
    #[doc(alias = "getTextStyle")]
    pub fn text_style(&self) -> &TextStyle {
        &self.default_text_style
    }

    /// `setTextStyle(textStyle)`.
    #[doc(alias = "setTextStyle")]
    pub fn set_text_style(&mut self, text_style: &TextStyle) -> &mut Self {
        self.default_text_style = text_style.clone();
        self
    }

    /// `getTextDirection()`.
    #[must_use]
    #[doc(alias = "getTextDirection")]
    pub fn text_direction(&self) -> TextDirection {
        self.text_direction
    }

    /// `setTextDirection(direction)`.
    #[doc(alias = "setTextDirection")]
    pub fn set_text_direction(&mut self, direction: TextDirection) -> &mut Self {
        self.text_direction = direction;
        self
    }

    /// `getTextAlign()`.
    #[must_use]
    #[doc(alias = "getTextAlign")]
    pub fn text_align(&self) -> TextAlign {
        self.text_align
    }

    /// `setTextAlign(align)`.
    #[doc(alias = "setTextAlign")]
    pub fn set_text_align(&mut self, align: TextAlign) -> &mut Self {
        self.text_align = align;
        self
    }

    /// `getMaxLines()`: `None` is the unlimited `std::numeric_limits<size_t>::max()`.
    #[must_use]
    #[doc(alias = "getMaxLines")]
    pub fn max_lines(&self) -> Option<usize> {
        if self.unlimited_lines() {
            None
        } else {
            Some(self.lines_limit)
        }
    }

    /// `getMaxLines()` as the raw `size_t`.
    #[must_use]
    pub fn max_lines_raw(&self) -> usize {
        self.lines_limit
    }

    /// `setMaxLines(maxLines)`: `None` is unlimited.
    #[doc(alias = "setMaxLines")]
    pub fn set_max_lines(&mut self, lines: impl Into<Option<usize>>) -> &mut Self {
        self.lines_limit = lines.into().unwrap_or(usize::MAX);
        self
    }

    /// `getEllipsis()`.
    #[must_use]
    #[doc(alias = "getEllipsis")]
    pub fn ellipsis(&self) -> &str {
        &self.ellipsis
    }

    /// `getEllipsisUtf16()`.
    #[must_use]
    #[doc(alias = "getEllipsisUtf16")]
    pub fn ellipsis_utf16(&self) -> &[u16] {
        &self.ellipsis_utf16
    }

    /// `setEllipsis(const SkString&)`.
    #[doc(alias = "setEllipsis")]
    pub fn set_ellipsis(&mut self, ellipsis: impl AsRef<str>) -> &mut Self {
        ellipsis.as_ref().clone_into(&mut self.ellipsis);
        self
    }

    /// `setEllipsis(const std::u16string&)`.
    #[doc(alias = "setEllipsis")]
    pub fn set_ellipsis_utf16(&mut self, ellipsis: &[u16]) -> &mut Self {
        self.ellipsis_utf16 = ellipsis.to_vec();
        self
    }

    /// `getHeight()`.
    #[must_use]
    #[doc(alias = "getHeight")]
    pub fn height(&self) -> scalar {
        self.height
    }

    /// `setHeight(height)`.
    #[doc(alias = "setHeight")]
    pub fn set_height(&mut self, height: scalar) -> &mut Self {
        self.height = height;
        self
    }

    /// `getTextHeightBehavior()`.
    #[must_use]
    #[doc(alias = "getTextHeightBehavior")]
    pub fn text_height_behavior(&self) -> TextHeightBehavior {
        self.text_height_behavior
    }

    /// `setTextHeightBehavior(v)`.
    #[doc(alias = "setTextHeightBehavior")]
    pub fn set_text_height_behavior(&mut self, v: TextHeightBehavior) -> &mut Self {
        self.text_height_behavior = v;
        self
    }

    /// `unlimited_lines()`.
    #[must_use]
    pub fn unlimited_lines(&self) -> bool {
        self.lines_limit == usize::MAX
    }

    /// `ellipsized()`.
    #[must_use]
    pub fn ellipsized(&self) -> bool {
        !self.ellipsis.is_empty() || !self.ellipsis_utf16.is_empty()
    }

    // Port of: modules/skparagraph/src/ParagraphStyle.cpp#L34-L42 (chrome/m156)
    /// `effective_align()`.
    #[must_use]
    pub fn effective_align(&self) -> TextAlign {
        if self.text_align == TextAlign::Start {
            if self.text_direction == TextDirection::LTR {
                TextAlign::Left
            } else {
                TextAlign::Right
            }
        } else if self.text_align == TextAlign::End {
            if self.text_direction == TextDirection::LTR {
                TextAlign::Right
            } else {
                TextAlign::Left
            }
        } else {
            self.text_align
        }
    }

    /// `hintingIsOn()`.
    #[must_use]
    #[doc(alias = "hintingIsOn")]
    pub fn hinting_is_on(&self) -> bool {
        self.hinting_is_on
    }

    /// `turnHintingOff()`.
    #[doc(alias = "turnHintingOff")]
    pub fn turn_hinting_off(&mut self) -> &mut Self {
        self.hinting_is_on = false;
        self
    }

    /// `fakeMissingFontStyles()`.
    #[must_use]
    #[doc(alias = "fakeMissingFontStyles")]
    pub fn fake_missing_font_styles(&self) -> bool {
        self.fake_missing_font_styles
    }

    /// `setFakeMissingFontStyles(value)`.
    #[doc(alias = "setFakeMissingFontStyles")]
    pub fn set_fake_missing_font_styles(&mut self, value: bool) -> &mut Self {
        self.fake_missing_font_styles = value;
        self
    }

    /// `getReplaceTabCharacters()`.
    #[must_use]
    #[doc(alias = "getReplaceTabCharacters")]
    pub fn replace_tab_characters(&self) -> bool {
        self.replace_tab_characters
    }

    /// `setReplaceTabCharacters(value)`.
    #[doc(alias = "setReplaceTabCharacters")]
    pub fn set_replace_tab_characters(&mut self, value: bool) -> &mut Self {
        self.replace_tab_characters = value;
        self
    }

    /// `getApplyRoundingHack()`.
    #[must_use]
    #[doc(alias = "getApplyRoundingHack")]
    pub fn apply_rounding_hack(&self) -> bool {
        self.apply_rounding_hack
    }

    /// `setApplyRoundingHack(value)`.
    #[doc(alias = "setApplyRoundingHack")]
    pub fn set_apply_rounding_hack(&mut self, value: bool) -> &mut Self {
        self.apply_rounding_hack = value;
        self
    }

    /// `getLetterSpacingByCSSSpec()`.
    #[must_use]
    #[doc(alias = "getLetterSpacingByCSSSpec")]
    pub fn letter_spacing_by_css_spec(&self) -> bool {
        self.letter_spacing_by_css_spec
    }

    /// `setLetterSpacingByCSSSpec(value)`.
    #[doc(alias = "setLetterSpacingByCSSSpec")]
    pub fn set_letter_spacing_by_css_spec(&mut self, value: bool) -> &mut Self {
        self.letter_spacing_by_css_spec = value;
        self
    }

    /// `getRenderSoftHyphens()`.
    #[must_use]
    #[doc(alias = "getRenderSoftHyphens")]
    pub fn render_soft_hyphens(&self) -> bool {
        self.render_soft_hyphens
    }

    /// `setRenderSoftHyphens(value)`.
    #[doc(alias = "setRenderSoftHyphens")]
    pub fn set_render_soft_hyphens(&mut self, value: bool) -> &mut Self {
        self.render_soft_hyphens = value;
        self
    }
}
