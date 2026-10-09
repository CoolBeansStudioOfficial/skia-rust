// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFont.h, src/core/SkFont.cpp (value type and setters)

//! [`Font`]: the typeface, size, scale, skew, edging, hinting and the flags that control how
//! the typeface's glyphs are rasterized. A plain value type. Measuring and glyph lookup are
//! added with the text engine.

use crate::font_metrics::FontMetrics;
use crate::font_priv::{count_text_elements, scale_font_metrics};
use crate::font_types::{FontHinting, GlyphId, TextEncoding};
use crate::glyph_intercepts::glyph_run_intercepts;
use crate::glyph_run::GlyphRun;
use crate::matrix::Matrix;
use crate::paint::{Paint, Style};
use crate::path::Path;
use crate::point::Point;
use crate::read_buffer::ReadBuffer;
use crate::rect::Rect;
use crate::scalar::scalar;
use crate::scaler_context::ScalerContextBuildFlags;
use crate::strike::StrikeRef;
use crate::strike_spec::{BulkGlyphMetrics, BulkGlyphMetricsAndPaths, StrikeSpec};
use crate::typeface::Typeface;
use crate::utf::Unichar;
use crate::write_buffer::BinaryWriteBuffer;

// The packed word of `SkFont_serial.cpp`: control bits, the size as a byte, flags, edging and
// hinting.
// Port of: src/core/SkFont_serial.cpp#L19-L36 (chrome/m156)
const SIZE_IS_BYTE_BIT: u32 = 1 << 31;
const HAS_SCALE_X_BIT: u32 = 1 << 30;
const HAS_SKEW_X_BIT: u32 = 1 << 29;
const HAS_TYPEFACE_BIT: u32 = 1 << 28;
const SHIFT_FOR_SIZE: u32 = 16;
const MASK_FOR_SIZE: u32 = 0xFF;
const SHIFT_FOR_FLAGS: u32 = 4;
const SHIFT_FOR_EDGING: u32 = 2;
const MASK_FOR_EDGING: u32 = 0x3;
const SHIFT_FOR_HINTING: u32 = 0;
const MASK_FOR_HINTING: u32 = 0x3;
/// `SkFont::kAllFlags`: every private flag bit.
const ALL_FLAGS: u8 =
    FORCE_AUTO_HINTING | EMBEDDED_BITMAPS | SUBPIXEL | LINEAR_METRICS | EMBOLDEN | BASELINE_SNAP;

/// `scalar_is_byte`: whether a size is an integer in `0..=255`.
// Port of: src/core/SkFont_serial.cpp#L38-L41 (chrome/m156)
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::float_cmp
)] // C++ `(int)x` and `ix == x`
fn scalar_is_byte(x: scalar) -> bool {
    let ix = x as i32;
    ix as scalar == x && (0..=0xFF).contains(&ix)
}

/// The bits of an edging, as `SkFont::Edging` is stored in the packed word.
fn edging_bits(edging: Edging) -> u32 {
    match edging {
        Edging::Alias => 0,
        Edging::AntiAlias => 1,
        Edging::SubpixelAntiAlias => 2,
    }
}

fn edging_from_bits(bits: u32) -> Edging {
    match bits {
        1 => Edging::AntiAlias,
        2 => Edging::SubpixelAntiAlias,
        _ => Edging::Alias,
    }
}

/// The bits of a hinting, as `SkFontHinting` is stored in the packed word.
fn hinting_bits(hinting: FontHinting) -> u32 {
    match hinting {
        FontHinting::None => 0,
        FontHinting::Slight => 1,
        FontHinting::Normal => 2,
        FontHinting::Full => 3,
    }
}

fn hinting_from_bits(bits: u32) -> FontHinting {
    match bits {
        1 => FontHinting::Slight,
        2 => FontHinting::Normal,
        3 => FontHinting::Full,
        _ => FontHinting::None,
    }
}

/// `scale_rect` of `SkFont.cpp`: each side times `s`.
// Port of: src/core/SkFont.cpp#L224-L228 (chrome/m156)
fn scale_rect(r: Rect, s: scalar) -> Rect {
    Rect::from_ltrb(r.left * s, r.top * s, r.right * s, r.bottom * s)
}

/// `SkPaintDefaults_TextSize`: the default text size, in pixels.
// Port of: src/core/SkPaintDefaults.h (SkPaintDefaults_TextSize = 12, chrome/m156)
/// `SkFontPriv::kCanonicalTextSizeForPaths`: the size glyphs are outlined at when drawn as paths.
// Port of: src/core/SkFontPriv.h#L36 (chrome/m156)
const CANONICAL_TEXT_SIZE_FOR_PATHS: scalar = 64.0;

const DEFAULT_SIZE: scalar = 12.0;

/// `SkPaintDefaults_Hinting`: the default hinting.
// Port of: src/core/SkPaintDefaults.h (SkPaintDefaults_Hinting = kNormal, chrome/m156)
const DEFAULT_HINTING: FontHinting = FontHinting::Normal;

/// `kForceAutoHinting_PrivFlag`.
// Port of: include/core/SkFont.h#L502 (chrome/m156)
const FORCE_AUTO_HINTING: u8 = 1 << 0;
/// `kEmbeddedBitmaps_PrivFlag`.
// Port of: include/core/SkFont.h#L503 (chrome/m156)
const EMBEDDED_BITMAPS: u8 = 1 << 1;
/// `kSubpixel_PrivFlag`.
// Port of: include/core/SkFont.h#L504 (chrome/m156)
const SUBPIXEL: u8 = 1 << 2;
/// `kLinearMetrics_PrivFlag`.
// Port of: include/core/SkFont.h#L505 (chrome/m156)
const LINEAR_METRICS: u8 = 1 << 3;
/// `kEmbolden_PrivFlag`.
// Port of: include/core/SkFont.h#L506 (chrome/m156)
const EMBOLDEN: u8 = 1 << 4;
/// `kBaselineSnap_PrivFlag`.
// Port of: include/core/SkFont.h#L507 (chrome/m156)
const BASELINE_SNAP: u8 = 1 << 5;

/// `kDefault_Flags`: only baseline snap is set by default.
// Port of: src/core/SkFont.cpp#L44 (chrome/m156)
const DEFAULT_FLAGS: u8 = BASELINE_SNAP;

/// Negative sizes are clamped to zero (`valid_size`).
// Port of: src/core/SkFont.cpp#L48-L50 (chrome/m156)
fn valid_size(size: scalar) -> scalar {
    size.max(0.0)
}

/// The edging of a glyph's mask (`SkFont::Edging`).
#[doc(alias = "SkFont::Edging")]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Edging {
    /// No transparent pixels on glyph edges.
    // Port of: include/core/SkFont.h#L44 (chrome/m156)
    Alias,
    /// May have transparent pixels on glyph edges.
    // Port of: include/core/SkFont.h#L45 (chrome/m156)
    #[default]
    AntiAlias,
    /// Glyph positioned in pixel using transparency.
    // Port of: include/core/SkFont.h#L46 (chrome/m156)
    SubpixelAntiAlias,
}

/// A font: a typeface plus the size, scale, skew and rendering options (`SkFont`).
///
/// `Font::default()` is `SkFont()`: the empty typeface at 12 pixels, anti-aliased, normal
/// hinting, with baseline snap on. The typeface is never null; a `None` typeface becomes the
/// empty typeface, as in C++.
// Port of: include/core/SkFont.h#L36-L515 (chrome/m156)
#[doc(alias = "SkFont")]
#[derive(Clone, Debug)]
pub struct Font {
    typeface: Typeface,
    size: scalar,
    scale_x: scalar,
    skew_x: scalar,
    flags: u8,
    edging: Edging,
    hinting: FontHinting,
}

impl Default for Font {
    /// `SkFont()`: the default typeface (empty), size and hinting.
    // Port of: src/core/SkFont.cpp#L69 (chrome/m156)
    fn default() -> Self {
        Self::with_typeface(None, DEFAULT_SIZE, 1.0, 0.0)
    }
}

impl PartialEq for Font {
    /// `SkFont::operator==`: the typeface object (by identity), size, scale, skew, flags, edging
    /// and hinting.
    // Port of: src/core/SkFont.cpp#L71-L78 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.typeface.ptr_eq(&other.typeface)
            && self.size == other.size
            && self.scale_x == other.scale_x
            && self.skew_x == other.skew_x
            && self.flags == other.flags
            && self.edging == other.edging
            && self.hinting == other.hinting
    }
}

impl Font {
    /// `SkFont(sk_sp<SkTypeface>, SkScalar size, SkScalar scaleX, SkScalar skewX)`.
    // Port of: src/core/SkFont.cpp#L52-L63 (chrome/m156)
    #[must_use]
    pub fn new(typeface: Typeface, size: scalar, scale_x: scalar, skew_x: scalar) -> Self {
        Self::with_typeface(Some(typeface), size, scale_x, skew_x)
    }

    /// `SkFont(sk_sp<SkTypeface>, SkScalar size)`: scale 1, skew 0.
    // Port of: src/core/SkFont.cpp#L65 (chrome/m156)
    #[must_use]
    pub fn from_size(typeface: Typeface, size: scalar) -> Self {
        Self::new(typeface, size, 1.0, 0.0)
    }

    /// `SkFont(sk_sp<SkTypeface>)`: the default size, scale 1, skew 0. `None` gives the empty
    /// typeface.
    // Port of: src/core/SkFont.cpp#L67 (chrome/m156)
    #[must_use]
    pub fn from_typeface(typeface: Option<Typeface>) -> Self {
        Self::with_typeface(typeface, DEFAULT_SIZE, 1.0, 0.0)
    }

    /// The constructor body shared by the C++ overloads: a `None` typeface becomes the empty
    /// typeface.
    // Port of: src/core/SkFont.cpp#L52-L63 (chrome/m156)
    fn with_typeface(
        typeface: Option<Typeface>,
        size: scalar,
        scale_x: scalar,
        skew_x: scalar,
    ) -> Self {
        Self {
            typeface: typeface.unwrap_or_else(Typeface::empty),
            size: valid_size(size),
            scale_x,
            skew_x,
            flags: DEFAULT_FLAGS,
            edging: Edging::AntiAlias,
            hinting: DEFAULT_HINTING,
        }
    }

    /// `SkFont::getTypeface`: the typeface (never null).
    // Port of: include/core/SkFont.h#L212 (chrome/m156)
    #[must_use]
    pub fn typeface(&self) -> &Typeface {
        &self.typeface
    }

    /// `SkFont::setTypeface`: `None` sets the empty typeface.
    // Port of: src/core/SkFont.cpp#L91-L95 (chrome/m156)
    pub fn set_typeface(&mut self, typeface: Option<Typeface>) {
        self.typeface = typeface.unwrap_or_else(Typeface::empty);
    }

    /// `SkFont::getSize`.
    // Port of: include/core/SkFont.h#L222 (chrome/m156)
    #[must_use]
    pub fn size(&self) -> scalar {
        self.size
    }

    /// `SkFont::setSize`: negative sizes become zero.
    // Port of: src/core/SkFont.cpp#L130-L132 (chrome/m156)
    pub fn set_size(&mut self, size: scalar) {
        self.size = valid_size(size);
    }

    /// `SkFont::getScaleX`.
    // Port of: include/core/SkFont.h#L229 (chrome/m156)
    #[must_use]
    pub fn scale_x(&self) -> scalar {
        self.scale_x
    }

    /// `SkFont::setScaleX`.
    // Port of: src/core/SkFont.cpp#L133-L135 (chrome/m156)
    pub fn set_scale_x(&mut self, scale: scalar) {
        self.scale_x = scale;
    }

    /// `SkFont::getSkewX`.
    // Port of: include/core/SkFont.h#L236 (chrome/m156)
    #[must_use]
    pub fn skew_x(&self) -> scalar {
        self.skew_x
    }

    /// `SkFont::setSkewX`.
    // Port of: src/core/SkFont.cpp#L136-L138 (chrome/m156)
    pub fn set_skew_x(&mut self, skew: scalar) {
        self.skew_x = skew;
    }

    /// `SkFont::getEdging`.
    // Port of: include/core/SkFont.h#L184 (chrome/m156)
    #[must_use]
    pub fn edging(&self) -> Edging {
        self.edging
    }

    /// `SkFont::setEdging`.
    // Port of: src/core/SkFont.cpp#L122-L124 (chrome/m156)
    pub fn set_edging(&mut self, edging: Edging) {
        self.edging = edging;
    }

    /// `SkFont::getHinting`.
    // Port of: include/core/SkFont.h#L198 (chrome/m156)
    #[must_use]
    pub fn hinting(&self) -> FontHinting {
        self.hinting
    }

    /// `SkFont::setHinting`.
    // Port of: src/core/SkFont.cpp#L126-L128 (chrome/m156)
    pub fn set_hinting(&mut self, hinting: FontHinting) {
        self.hinting = hinting;
    }

    /// `SkFont::isForceAutoHinting`.
    // Port of: include/core/SkFont.h#L105 (chrome/m156)
    #[must_use]
    pub fn is_force_auto_hinting(&self) -> bool {
        self.flag(FORCE_AUTO_HINTING)
    }

    /// `SkFont::setForceAutoHinting`.
    // Port of: src/core/SkFont.cpp#L104-L106 (chrome/m156)
    pub fn set_force_auto_hinting(&mut self, value: bool) {
        self.set_flag(FORCE_AUTO_HINTING, value);
    }

    /// `SkFont::isEmbeddedBitmaps`.
    // Port of: include/core/SkFont.h#L111 (chrome/m156)
    #[must_use]
    pub fn is_embedded_bitmaps(&self) -> bool {
        self.flag(EMBEDDED_BITMAPS)
    }

    /// `SkFont::setEmbeddedBitmaps`.
    // Port of: src/core/SkFont.cpp#L107-L109 (chrome/m156)
    pub fn set_embedded_bitmaps(&mut self, value: bool) {
        self.set_flag(EMBEDDED_BITMAPS, value);
    }

    /// `SkFont::isSubpixel`.
    // Port of: include/core/SkFont.h#L117 (chrome/m156)
    #[must_use]
    pub fn is_subpixel(&self) -> bool {
        self.flag(SUBPIXEL)
    }

    /// `SkFont::setSubpixel`.
    // Port of: src/core/SkFont.cpp#L110-L112 (chrome/m156)
    pub fn set_subpixel(&mut self, value: bool) {
        self.set_flag(SUBPIXEL, value);
    }

    /// `SkFont::isLinearMetrics`.
    // Port of: include/core/SkFont.h#L123 (chrome/m156)
    #[must_use]
    pub fn is_linear_metrics(&self) -> bool {
        self.flag(LINEAR_METRICS)
    }

    /// `SkFont::setLinearMetrics`.
    // Port of: src/core/SkFont.cpp#L113-L115 (chrome/m156)
    pub fn set_linear_metrics(&mut self, value: bool) {
        self.set_flag(LINEAR_METRICS, value);
    }

    /// `SkFont::isEmbolden`.
    // Port of: include/core/SkFont.h#L130 (chrome/m156)
    #[must_use]
    pub fn is_embolden(&self) -> bool {
        self.flag(EMBOLDEN)
    }

    /// `SkFont::setEmbolden`.
    // Port of: src/core/SkFont.cpp#L116-L118 (chrome/m156)
    pub fn set_embolden(&mut self, value: bool) {
        self.set_flag(EMBOLDEN, value);
    }

    /// `SkFont::isBaselineSnap`.
    // Port of: include/core/SkFont.h#L137 (chrome/m156)
    #[must_use]
    pub fn is_baseline_snap(&self) -> bool {
        self.flag(BASELINE_SNAP)
    }

    /// `SkFont::setBaselineSnap`.
    // Port of: src/core/SkFont.cpp#L119-L121 (chrome/m156)
    pub fn set_baseline_snap(&mut self, value: bool) {
        self.set_flag(BASELINE_SNAP, value);
    }

    /// `SkFont::setupForAsPaths`: prepares the font to be drawn as paths. Sets the canonical size
    /// (`kCanonicalTextSizeForPaths`), turns off hinting and bitmaps, and, if a paint is given,
    /// makes it fill with no path effect. Returns the scale from the canonical size back to the
    /// font's own size.
    // Port of: src/core/SkFont.cpp#L148-L165 (chrome/m156)
    #[doc(alias = "setupForAsPaths")]
    pub fn setup_for_as_paths(&mut self, paint: Option<&mut Paint>) -> scalar {
        // `kEmbeddedBitmaps_PrivFlag | kForceAutoHinting_PrivFlag`
        const FLAGS_TO_IGNORE: u8 = EMBEDDED_BITMAPS | FORCE_AUTO_HINTING;
        self.flags = (self.flags & !FLAGS_TO_IGNORE) | SUBPIXEL;
        self.set_hinting(FontHinting::None);
        if self.edging() == Edging::SubpixelAntiAlias {
            self.set_edging(Edging::AntiAlias);
        }
        if let Some(paint) = paint {
            paint.set_style(Style::Fill);
            paint.set_path_effect(None);
        }
        let text_size = self.size;
        self.set_size(CANONICAL_TEXT_SIZE_FOR_PATHS);
        text_size / CANONICAL_TEXT_SIZE_FOR_PATHS
    }

    /// Reads one private flag bit.
    fn flag(&self, bit: u8) -> bool {
        self.flags & bit != 0
    }

    /// `SkFont::makeWithSize`: a copy of this font with another size.
    // Port of: src/core/SkFont.cpp#L140-L146 (chrome/m156)
    #[doc(alias = "makeWithSize")]
    #[must_use]
    pub fn make_with_size(&self, size: scalar) -> Self {
        let mut font = self.clone();
        font.set_size(size);
        font
    }

    /// `SkFont::hasSomeAntiAliasing`.
    // Port of: src/core/SkFont.cpp#L168-L172 (chrome/m156)
    #[doc(alias = "hasSomeAntiAliasing")]
    #[must_use]
    pub fn has_some_anti_aliasing(&self) -> bool {
        matches!(self.edging(), Edging::AntiAlias | Edging::SubpixelAntiAlias)
    }

    /// `SkFont::unicharToGlyph`.
    // Port of: src/core/SkFont.cpp#L174-L176 (chrome/m156)
    #[doc(alias = "unicharToGlyph")]
    #[must_use]
    pub fn unichar_to_glyph(&self, uni: Unichar) -> GlyphId {
        self.typeface().unichar_to_glyph(uni)
    }

    /// `SkFont::unicharsToGlyphs`.
    // Port of: src/core/SkFont.cpp#L178-L180 (chrome/m156)
    #[doc(alias = "unicharsToGlyphs")]
    pub fn unichars_to_glyphs(&self, unis: &[Unichar], glyphs: &mut [GlyphId]) {
        self.typeface().unichars_to_glyphs(unis, glyphs);
    }

    /// `SkFont::textToGlyphs`: the glyphs of `text`, returning their count.
    // Port of: src/core/SkFont.cpp#L182-L185 (chrome/m156)
    #[doc(alias = "textToGlyphs")]
    pub fn text_to_glyphs(
        &self,
        text: &[u8],
        encoding: TextEncoding,
        glyphs: &mut [GlyphId],
    ) -> usize {
        self.typeface().text_to_glyphs(text, encoding, glyphs)
    }

    /// `SkFont::countText`: the number of characters (or glyphs) in `text`.
    // Port of: src/core/SkFontPriv.h (CountTextElements via SkFont::countText, chrome/m156)
    #[doc(alias = "countText")]
    #[must_use]
    pub fn count_text(&self, text: &[u8], encoding: TextEncoding) -> usize {
        count_text_elements(text, encoding)
    }

    /// `SkFont::measureText`: the advance of `text` and the bounding rectangle of its glyphs
    /// (empty when there are no glyphs). `paint` gives the strike's effects.
    // Port of: src/core/SkFont.cpp#L187-L238 (chrome/m156)
    #[doc(alias = "measureText")]
    #[must_use]
    pub fn measure_text(
        &self,
        text: &[u8],
        encoding: TextEncoding,
        paint: Option<&Paint>,
    ) -> (scalar, Rect) {
        let glyph_ids = self.glyph_ids_of(text, encoding);
        if glyph_ids.is_empty() {
            return (0.0, Rect::from_ltrb(0.0, 0.0, 0.0, 0.0));
        }
        let (spec, scale) = self.canonicalized_spec(paint);
        let glyphs = BulkGlyphMetrics::new(&spec).glyphs(&glyph_ids);

        let mut bounds = glyphs[0].rect();
        let mut width = glyphs[0].advance_x();
        for glyph in &glyphs[1..] {
            let mut r = glyph.rect();
            r.offset((width, 0.0));
            bounds.join(r);
            width += glyph.advance_x();
        }
        #[allow(clippy::float_cmp)] // C++ `if (strikeToSourceScale != 1)`
        if scale != 1.0 {
            width *= scale;
            bounds = scale_rect(bounds, scale);
        }
        (width, bounds)
    }

    /// `SkFont::getWidths`: the advance of each glyph (`getWidthsBounds` without the bounds).
    // Port of: src/core/SkFont.cpp#L240-L243 (chrome/m156)
    #[doc(alias = "getWidths")]
    pub fn get_widths(&self, glyph_ids: &[GlyphId], widths: &mut [scalar]) {
        self.get_widths_bounds(glyph_ids, widths, &mut [], None);
    }

    /// `SkFont::getWidthsBounds`: the advance and bounds of each glyph. An empty `widths` or
    /// `bounds` is not written.
    // Port of: src/core/SkFont.cpp#L245-L266 (chrome/m156)
    #[doc(alias = "getWidthsBounds")]
    pub fn get_widths_bounds(
        &self,
        glyph_ids: &[GlyphId],
        widths: &mut [scalar],
        bounds: &mut [Rect],
        paint: Option<&Paint>,
    ) {
        let (spec, scale) = self.canonicalized_spec(paint);
        let glyphs = BulkGlyphMetrics::new(&spec).glyphs(glyph_ids);
        if !bounds.is_empty() {
            let n = bounds.len().min(glyphs.len());
            for (bound, glyph) in bounds[..n].iter_mut().zip(&glyphs[..n]) {
                *bound = scale_rect(glyph.rect(), scale);
            }
        }
        if !widths.is_empty() {
            let n = widths.len().min(glyphs.len());
            for (width, glyph) in widths[..n].iter_mut().zip(&glyphs[..n]) {
                *width = glyph.advance_x() * scale;
            }
        }
    }

    /// `SkFont::getPos`: the origin of each glyph when the glyphs are laid out from `origin`.
    // Port of: src/core/SkFont.cpp#L268-L279 (chrome/m156)
    #[doc(alias = "getPos")]
    pub fn get_pos(&self, glyph_ids: &[GlyphId], pos: &mut [Point], origin: Point) {
        let (spec, scale) = self.canonicalized_spec(None);
        let glyphs = BulkGlyphMetrics::new(&spec).glyphs(glyph_ids);
        let mut sum = origin;
        let n = pos.len().min(glyphs.len());
        for (position, glyph) in pos[..n].iter_mut().zip(&glyphs[..n]) {
            *position = sum;
            sum += glyph.advance_vector() * scale;
        }
    }

    /// `SkFont::getXPos`: the x origin of each glyph when the glyphs are laid out from `origin`.
    // Port of: src/core/SkFont.cpp#L281-L292 (chrome/m156)
    #[doc(alias = "getXPos")]
    pub fn get_x_pos(&self, glyph_ids: &[GlyphId], xpos: &mut [scalar], origin: scalar) {
        let (spec, scale) = self.canonicalized_spec(None);
        let glyphs = BulkGlyphMetrics::new(&spec).glyphs(glyph_ids);
        let mut loc = origin;
        let n = xpos.len().min(glyphs.len());
        for (xposition, glyph) in xpos[..n].iter_mut().zip(&glyphs[..n]) {
            *xposition = loc;
            loc += glyph.advance_x() * scale;
        }
    }

    /// `SkFont::getIntercepts`: the x intervals, as pairs `[start, end]`, where the band between
    /// `top` and `bottom` crosses the outlines of the glyphs placed at `positions`. `paint` gives
    /// the stroke and path effect. Empty if there are no glyphs.
    // Port of: src/core/SkTextBlob.cpp#L956-L974 (chrome/m156)
    #[doc(alias = "getIntercepts")]
    #[must_use]
    pub fn get_intercepts<'a>(
        &self,
        glyphs: &[GlyphId],
        positions: &[Point],
        (top, bottom): (scalar, scalar),
        paint: impl Into<Option<&'a Paint>>,
    ) -> Vec<scalar> {
        let count = glyphs.len().min(positions.len());
        if count == 0 {
            return Vec::new();
        }
        let paint = paint.into().cloned().unwrap_or_default();
        let run = GlyphRun::new(
            self.clone(),
            positions[..count].to_vec(),
            glyphs[..count].to_vec(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        );
        let mut intervals = Vec::new();
        glyph_run_intercepts(&run, &paint, [top, bottom], &mut intervals);
        intervals
    }

    /// `SkFont::getPaths`: calls `f` with each glyph's path and the matrix that scales it to this
    /// font's size. A glyph without a path gives `None`.
    // Port of: src/core/SkFont.cpp#L294-L307 (chrome/m156)
    #[doc(alias = "getPaths")]
    pub fn get_paths(&self, glyph_ids: &[GlyphId], mut f: impl FnMut(Option<&Path>, &Matrix)) {
        let mut font = self.clone();
        let scale = font.setup_for_as_paths(None);
        let mx = Matrix::scale((scale, scale));
        let spec = StrikeSpec::make_with_no_device(&font, None, ScalerContextBuildFlags::NONE);
        let glyphs = BulkGlyphMetricsAndPaths::new(&spec).glyphs(glyph_ids);
        for glyph in &glyphs {
            f(glyph.path(), &mx);
        }
    }

    /// `SkFont::getPath`: the path of one glyph at this font's size, if it has one.
    // Port of: src/core/SkFont.cpp#L309-L320 (chrome/m156)
    #[doc(alias = "getPath")]
    #[must_use]
    pub fn get_path(&self, glyph_id: GlyphId) -> Option<Path> {
        let mut result = None;
        self.get_paths(&[glyph_id], |path, mx| {
            if let Some(path) = path {
                result = path.try_make_transform(mx);
            }
        });
        result
    }

    /// `SkFont::getMetrics`: the font metrics at this font's size, and the line spacing
    /// (`descent - ascent + leading`).
    // Port of: src/core/SkFont.cpp#L322-L340 (chrome/m156)
    #[doc(alias = "getMetrics")]
    #[must_use]
    pub fn metrics(&self) -> (scalar, FontMetrics) {
        let (spec, scale) = self.canonicalized_spec(None);
        let cache = spec.find_or_create_strike();
        let mut metrics = *cache.font_metrics();
        #[allow(clippy::float_cmp)] // C++ `if (strikeToSourceScale != 1)`
        if scale != 1.0 {
            scale_font_metrics(&mut metrics, scale);
        }
        (metrics.descent - metrics.ascent + metrics.leading, metrics)
    }

    /// `SkFont::makeStrikeRef`: the strike of this font, with the scale back to this font's size.
    // Port of: src/core/SkFont.cpp#L240-L243 (chrome/m156)
    #[doc(alias = "makeStrikeRef")]
    #[must_use]
    pub fn make_strike_ref(&self) -> StrikeRef {
        let (spec, scale) = self.canonicalized_spec(None);
        StrikeRef::new(spec.find_or_create_strike(), scale)
    }

    /// The glyph ids of `text`, as `SkAutoToGlyphs` makes them.
    // Port of: src/core/SkFontPriv.h#L93-L106 (chrome/m156), SkAutoToGlyphs
    fn glyph_ids_of(&self, text: &[u8], encoding: TextEncoding) -> Vec<GlyphId> {
        if encoding == TextEncoding::GlyphId || text.is_empty() {
            return text
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[a, b]| GlyphId::from_ne_bytes([a, b]))
                .collect();
        }
        let mut glyphs = vec![0; self.count_text(text, encoding)];
        // The buffer holds exactly the count of the text, so every glyph is written.
        let _ = self.text_to_glyphs(text, encoding, &mut glyphs);
        glyphs
    }

    /// The canonicalized strike spec of this font and `paint`.
    fn canonicalized_spec(&self, paint: Option<&Paint>) -> (StrikeSpec, scalar) {
        StrikeSpec::make_canonicalized(self, paint)
    }

    /// `SkFontPriv::Flatten`: writes the font. The packed word holds the flags, edging and
    /// hinting, and says whether the size, scale, skew and typeface follow.
    // Port of: src/core/SkFont_serial.cpp#L48-L86 (chrome/m156)
    #[doc(alias = "Flatten")]
    pub fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        let mut packed: u32 = 0;
        packed |= u32::from(self.flags) << SHIFT_FOR_FLAGS;
        packed |= edging_bits(self.edging) << SHIFT_FOR_EDGING;
        packed |= hinting_bits(self.hinting) << SHIFT_FOR_HINTING;
        if scalar_is_byte(self.size) {
            packed |= SIZE_IS_BYTE_BIT;
            // `(int)fSize`: the size is a byte here, checked by `scalar_is_byte`.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            let size_byte = self.size as u32;
            packed |= size_byte << SHIFT_FOR_SIZE;
        }
        // Exact comparisons, as `SkFont_serial.cpp` makes them.
        #[allow(clippy::float_cmp)]
        if self.scale_x != 1.0 {
            packed |= HAS_SCALE_X_BIT;
        }
        #[allow(clippy::float_cmp)]
        if self.skew_x != 0.0 {
            packed |= HAS_SKEW_X_BIT;
        }
        // A font always has a typeface (the empty one at least), so the bit is always set.
        packed |= HAS_TYPEFACE_BIT;

        buffer.write_uint(packed);
        if packed & SIZE_IS_BYTE_BIT == 0 {
            buffer.write_scalar(self.size);
        }
        if packed & HAS_SCALE_X_BIT != 0 {
            buffer.write_scalar(self.scale_x);
        }
        if packed & HAS_SKEW_X_BIT != 0 {
            buffer.write_scalar(self.skew_x);
        }
        if packed & HAS_TYPEFACE_BIT != 0 {
            buffer.write_typeface(Some(&self.typeface));
        }
    }

    /// `SkFontPriv::Unflatten`: reads a font written by [`Font::flatten`]. Unknown flag bits are
    /// dropped, and out-of-range edging or hinting become the defaults. Returns whether the
    /// buffer is still valid.
    // Port of: src/core/SkFont_serial.cpp#L88-L123 (chrome/m156)
    #[doc(alias = "Unflatten")]
    pub fn unflatten(&mut self, buffer: &mut ReadBuffer<'_>) -> bool {
        let packed = buffer.read_uint();
        self.size = if packed & SIZE_IS_BYTE_BIT != 0 {
            // A byte, so exact in a scalar.
            #[allow(clippy::cast_precision_loss)]
            {
                ((packed >> SHIFT_FOR_SIZE) & MASK_FOR_SIZE) as scalar
            }
        } else {
            buffer.read_scalar()
        };
        if packed & HAS_SCALE_X_BIT != 0 {
            self.scale_x = buffer.read_scalar();
        }
        if packed & HAS_SKEW_X_BIT != 0 {
            self.skew_x = buffer.read_scalar();
        }
        if packed & HAS_TYPEFACE_BIT != 0 {
            // `setTypeface(nullptr)` makes the empty typeface.
            self.typeface = buffer.read_typeface().unwrap_or_else(Typeface::empty);
        }
        // Keep only the flag bits that exist (`& kAllFlags`).
        // The mask keeps the value within the 8 bits of `flags`.
        #[allow(clippy::cast_possible_truncation)]
        {
            self.flags = ((packed >> SHIFT_FOR_FLAGS) & u32::from(ALL_FLAGS)) as u8;
        }
        let mut edging = (packed >> SHIFT_FOR_EDGING) & MASK_FOR_EDGING;
        if edging > 2 {
            edging = 0;
        }
        self.edging = edging_from_bits(edging);
        let mut hinting = (packed >> SHIFT_FOR_HINTING) & MASK_FOR_HINTING;
        if hinting > 3 {
            hinting = 0;
        }
        self.hinting = hinting_from_bits(hinting);
        buffer.is_valid()
    }

    /// Sets or clears one private flag bit (`set_clear_mask`).
    // Port of: src/core/SkFont.cpp#L100-L102 (chrome/m156)
    fn set_flag(&mut self, bit: u8, value: bool) {
        if value {
            self.flags |= bit;
        } else {
            self.flags &= !bit;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::write_buffer::BinaryWriteBuffer;

    /// `FontTest::Font_flatten`'s round trip, for the empty typeface (the test proper uses a
    /// user typeface, which arrives with T17 and typeface serialization with T16).
    #[test]
    fn flatten_and_unflatten_round_trip_without_a_typeface_table() {
        let mut font = Font::from_size(Typeface::empty(), 10.0);
        font.set_scale_x(5.0);
        font.set_skew_x(-5.0);
        font.set_edging(Edging::SubpixelAntiAlias);
        font.set_hinting(FontHinting::Full);
        font.set_subpixel(true);
        font.set_baseline_snap(false);

        let mut buffer = BinaryWriteBuffer::new();
        font.flatten(&mut buffer);
        let mut data = vec![0; buffer.bytes_written()];
        buffer.write_to_memory(&mut data);

        let mut read = Font::default();
        let mut reader = ReadBuffer::new(&data);
        assert!(read.unflatten(&mut reader));
        assert_eq!(read.size(), 10.0);
        assert_eq!(read.scale_x(), 5.0);
        assert_eq!(read.skew_x(), -5.0);
        assert_eq!(read.edging(), Edging::SubpixelAntiAlias);
        assert_eq!(read.hinting(), FontHinting::Full);
        assert!(read.is_subpixel());
        assert!(!read.is_baseline_snap());
        // Without the typeface table the typeface reads back as the empty one.
        assert_eq!(read.typeface().count_glyphs(), 0);
    }

    #[test]
    fn measuring_empty_glyphs_gives_zero_advance() {
        let font = Font::from_typeface(Some(Typeface::empty()));
        // The empty typeface maps every character to glyph 0, which has no advance.
        assert_eq!(
            font.measure_text(b"ab", TextEncoding::UTF8, None),
            (0.0, Rect::from_ltrb(0.0, 0.0, 0.0, 0.0))
        );
        assert_eq!(font.measure_text(b"", TextEncoding::UTF8, None).0, 0.0);
    }
}
