// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkFont.h, src/core/SkFont.cpp (value type and setters)

//! [`Font`]: the typeface, size, scale, skew, edging, hinting and the flags that control how
//! the typeface's glyphs are rasterized. A plain value type. Measuring and glyph lookup are
//! added with the text engine.

use crate::font_types::FontHinting;
use crate::scalar::scalar;
use crate::typeface::Typeface;

/// `SkPaintDefaults_TextSize`: the default text size, in pixels.
// Port of: src/core/SkPaintDefaults.h (SkPaintDefaults_TextSize = 12, chrome/m156)
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

    /// Reads one private flag bit.
    fn flag(&self, bit: u8) -> bool {
        self.flags & bit != 0
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
