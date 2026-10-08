// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkTypeface.h, src/core/SkTypeface.cpp

//! [`Typeface`]: the font face a glyph comes from (`SkTypeface`).
//!
//! `Typeface` is a cheap-clone handle over `Arc<dyn TypefaceBase>`, as `Shader` is. The state
//! every typeface has (id, style, fixed pitch) lives in [`TypefaceCore`], and each backend
//! implements [`TypefaceBase`] for its own `on*` methods.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, OnceLock};

use crate::data::Data;
use crate::descriptor::Descriptor;
use crate::font::Font;
use crate::font_arguments::FontArguments;
use crate::font_arguments::variation_position::Coordinate;
use crate::font_descriptor::{FactoryId, FontDescriptor};
use crate::font_priv::count_text_elements;
use crate::font_style::{FontStyle, Slant, Weight};
use crate::font_types::{GlyphId, TextEncoding, set_four_byte_tag};
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::rect::Rect;
use crate::scalar::scalar;
use crate::scaler_context::{
    ScalerContext, ScalerContextBuildFlags, ScalerContextEffects, ScalerContextRec,
};
use crate::stream::{DynamicMemoryWStream, StreamAsset, WStream};
use crate::strike_spec::auto_descriptor_given_rec_and_effects;
use crate::surface_props::SurfaceProps;
use crate::typeface_cache::new_typeface_id;
use crate::utf::{Unichar, next_utf8, next_utf16};

/// A unique id of a typeface (`SkTypefaceID`).
// Port of: include/core/SkTypeface.h#L40 (chrome/m156)
#[doc(alias = "SkTypefaceID")]
pub type TypefaceId = u32;

/// How much of a typeface [`Typeface::serialize`] writes (`SkTypeface::SerializeBehavior`).
// Port of: include/core/SkTypeface.h#L127-L131 (chrome/m156)
#[doc(alias = "SkTypeface::SerializeBehavior")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub enum SerializeBehavior {
    /// Always include the font data.
    DoIncludeData,
    /// Never include the font data; write the descriptor only.
    DontIncludeData,
    /// Include the data if the typeface reports that it is local to the process.
    IncludeDataIfLocal,
}

/// One localized name of a typeface (`SkTypeface::LocalizedString`).
// Port of: include/core/SkTypeface.h#L261-L264 (chrome/m156)
#[doc(alias = "SkTypeface::LocalizedString")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LocalizedString {
    /// The name, in UTF-8.
    pub string: String,
    /// The BCP 47 language tag of the name.
    pub language: String,
}

/// An iterator over the localized names of a typeface (`SkTypeface::LocalizedStrings`).
// Port of: include/core/SkTypeface.h#L265-L271 (chrome/m156)
#[doc(alias = "SkTypeface::LocalizedStrings")]
pub trait LocalizedStrings {
    /// Returns the next name, or `None` at the end (`next` returning false in C++).
    // Port of: include/core/SkTypeface.h#L269 (chrome/m156)
    fn next(&mut self) -> Option<LocalizedString>;
}

/// The state every typeface has (`SkTypeface`'s data members).
// Port of: include/core/SkTypeface.h#L460-L464 (chrome/m156)
#[derive(Debug)]
pub struct TypefaceCore {
    unique_id: TypefaceId,
    style: FontStyle,
    is_fixed_pitch: bool,
    /// `fBounds` with `fBoundsOnce`: computed on first use by [`Typeface::get_bounds`].
    bounds: OnceLock<Rect>,
}

impl TypefaceCore {
    /// `SkTypeface::SkTypeface(style, isFixedPitch)`: assigns the next unique id.
    // Port of: src/core/SkTypeface.cpp#L57-L58 (chrome/m156)
    #[must_use]
    pub fn new(style: FontStyle, is_fixed_pitch: bool) -> Self {
        Self {
            unique_id: new_typeface_id(),
            style,
            is_fixed_pitch,
            bounds: OnceLock::new(),
        }
    }
}

/// The per-backend half of a typeface: the `on*` virtuals of `SkTypeface`.
///
/// This carries the methods the first text slice needs (serialization, the empty typeface and
/// the cache). The rest of `SkTypeface`'s virtuals (glyph lookup, metrics, scaler contexts,
/// tables) are added with the phases that need them: the scaler context with T6, glyph mapping
/// and tables with T9.
// Port of: include/core/SkTypeface.h#L365-L440 (chrome/m156), the subset named above
#[doc(alias = "SkTypeface")]
pub trait TypefaceBase: Any + Send + Sync + fmt::Debug {
    /// The common state of the typeface.
    fn core(&self) -> &TypefaceCore;

    /// `SkTypeface::onGetFontDescriptor`: the descriptor, and whether the data is local to this
    /// process (`isLocal`).
    // Port of: include/core/SkTypeface.h#L414 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool);

    /// `SkTypeface::onOpenStream`: the font data and its collection index, or `None`.
    // Port of: include/core/SkTypeface.h#L400 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)>;

    /// `SkTypeface::onGetFamilyName`.
    // Port of: include/core/SkTypeface.h#L426 (chrome/m156)
    fn on_get_family_name(&self) -> String;

    /// `SkTypeface::onGetVariationDesignPosition`: the axes of the typeface, or `None` if the
    /// number of axes is unknown (C++ returns -1).
    // Port of: include/core/SkTypeface.h#L406-L408 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>>;

    /// `SkTypeface::onFilterRec`: lets the typeface adjust a scaler context record.
    // Port of: include/core/SkTypeface.h#L385 (chrome/m156)
    fn on_filter_rec(&self, rec: &mut ScalerContextRec);

    /// `SkTypeface::onGlyphMaskNeedsCurrentColor`.
    // Port of: include/core/SkTypeface.h#L404 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool;

    /// `SkTypeface::onMakeClone`. `this` is the handle of this typeface, which a clone may
    /// return as is (C++ returns `sk_ref_sp(this)`).
    // Port of: include/core/SkTypeface.h#L369 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, args: &FontArguments<'_, '_>) -> Typeface;

    /// `SkTypeface::onCreateScalerContext`: a valid scaler context, never null. `this` is the
    /// handle of this typeface, which the context keeps.
    // Port of: include/core/SkTypeface.h#L381-L382 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext;

    /// `SkTypeface::onCharsToGlyphs`: maps unichars to glyph ids. The default maps everything to
    /// glyph 0, as `SkEmptyTypeface` does; a typeface with glyphs overrides it.
    // Port of: include/core/SkTypeface.h (onCharsToGlyphs, chrome/m156)
    #[doc(alias = "onCharsToGlyphs")]
    fn on_chars_to_glyphs(&self, _unichars: &[Unichar], glyphs: &mut [GlyphId]) {
        glyphs.fill(0);
    }

    /// `SkTypeface::onCountGlyphs`: the number of glyphs. The default is 0, as for the empty
    /// typeface.
    // Port of: include/core/SkTypeface.h (onCountGlyphs, chrome/m156)
    #[doc(alias = "onCountGlyphs")]
    fn on_count_glyphs(&self) -> i32 {
        0
    }

    /// `SkTypeface::onGetGlyphToUnicodeMap`: the unichar of each glyph. The default is all zeros,
    /// as for the empty typeface.
    // Port of: include/core/SkTypeface.h (onGetGlyphToUnicodeMap, chrome/m156)
    #[doc(alias = "onGetGlyphToUnicodeMap")]
    fn on_get_glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        dst.fill(0);
    }
}

/// A typeface handle (`sk_sp<SkTypeface>`). Cloning it shares the typeface.
// Port of: include/core/SkTypeface.h#L54 (chrome/m156)
#[doc(alias = "SkTypeface")]
#[derive(Clone)]
pub struct Typeface(Arc<dyn TypefaceBase>);

impl Typeface {
    /// Wraps a backend in a handle.
    #[must_use]
    pub fn new(base: Arc<dyn TypefaceBase>) -> Self {
        Self(base)
    }

    /// `SkTypeface::MakeEmpty()`: the typeface that draws nothing and has no glyphs. It is the
    /// same instance every time.
    // Port of: src/core/SkTypeface.cpp#L144-L146 (chrome/m156)
    #[doc(alias = "MakeEmpty")]
    #[must_use]
    pub fn empty() -> Self {
        static EMPTY: OnceLock<Typeface> = OnceLock::new();
        EMPTY
            .get_or_init(|| Self::new(Arc::new(EmptyTypeface::new())))
            .clone()
    }

    /// `SkTypeface::getBounds`: the bounds of the font, in font units scaled to one point.
    /// Computed once per typeface.
    // Port of: src/core/SkTypeface.cpp#L550-L557 (chrome/m156)
    #[doc(alias = "getBounds")]
    #[must_use]
    pub fn get_bounds(&self) -> Rect {
        *self.0.core().bounds.get_or_init(|| self.compute_bounds())
    }

    /// `SkTypeface::onComputeBounds`: the font's extremes, measured at 2048 points with linear
    /// metrics and scaled back down. Empty when the font has no bounds.
    // Port of: src/core/SkTypeface.cpp#L559-L586 (chrome/m156)
    fn compute_bounds(&self) -> Rect {
        // we use a big size to ensure lots of significant bits from the scalercontext.
        // then we scale back down to return our final answer (at 1-pt)
        const TEXT_SIZE: scalar = 2048.0;
        const INV_TEXT_SIZE: scalar = 1.0 / TEXT_SIZE;

        let mut font = Font::from_size(self.clone(), TEXT_SIZE);
        font.set_linear_metrics(true);

        // SkScalerContext::MakeRecAndEffectsFromFont: an empty paint, no flags.
        let (rec, _effects) = ScalerContext::make_rec_and_effects(
            &font,
            &Paint::default(),
            &SurfaceProps::default(),
            ScalerContextBuildFlags::NONE,
            Matrix::i(),
        );
        // SkScalerContext::AutoDescriptorGivenRecAndEffects with no effects.
        let no_effects = ScalerContextEffects::default();
        let Some(auto_descriptor) = auto_descriptor_given_rec_and_effects(&rec, &no_effects) else {
            return Rect::default();
        };
        let mut ctx = self.create_scaler_context(&no_effects, auto_descriptor.get_desc());
        let fm = ctx.get_font_metrics();
        if !fm.has_bounds() {
            return Rect::default();
        }
        Rect::from_ltrb(
            fm.x_min * INV_TEXT_SIZE,
            fm.top * INV_TEXT_SIZE,
            fm.x_max * INV_TEXT_SIZE,
            fm.bottom * INV_TEXT_SIZE,
        )
    }

    /// `SkTypeface::uniqueID`.
    // Port of: include/core/SkTypeface.h#L104 (chrome/m156)
    #[doc(alias = "uniqueID")]
    #[must_use]
    pub fn unique_id(&self) -> TypefaceId {
        self.0.core().unique_id
    }

    /// `sk_sp` identity: whether two handles are the same typeface object (`SkFont::operator==`
    /// compares the pointers).
    // Port of: src/core/SkFont.cpp#L73 (fTypeface.get() ==, chrome/m156)
    #[must_use]
    pub fn ptr_eq(&self, other: &Typeface) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// `SkTypeface::makeClone`: a typeface with the given arguments applied.
    // Port of: src/core/SkTypeface.cpp#L186-L188 (chrome/m156)
    #[must_use]
    pub fn make_clone(&self, args: &FontArguments<'_, '_>) -> Typeface {
        self.0.on_make_clone(self.clone(), args)
    }

    /// `SkTypeface::createScalerContext` (`onCreateScalerContext`): a scaler context for the
    /// descriptor and effects.
    // Port of: include/core/SkTypeface.h#L381 (chrome/m156)
    #[must_use]
    pub fn create_scaler_context(
        &self,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        self.0.on_create_scaler_context(self.clone(), effects, desc)
    }

    /// `SkTypeface::filterRec`: lets the typeface adjust a record before a scaler context uses it.
    // Port of: include/core/SkTypeface.h#L343-L346 (chrome/m156)
    pub fn filter_rec(&self, rec: &mut ScalerContextRec) {
        self.0.on_filter_rec(rec);
    }

    /// `SkTypeface::onGlyphMaskNeedsCurrentColor`: whether glyph masks depend on the paint color.
    // Port of: include/core/SkTypeface.h#L404 (chrome/m156)
    #[must_use]
    pub fn glyph_mask_needs_current_color(&self) -> bool {
        self.0.on_glyph_mask_needs_current_color()
    }

    /// `SkTypeface::fontStyle`.
    // Port of: src/core/SkTypeface.cpp#L483-L485 (chrome/m156)
    #[doc(alias = "fontStyle")]
    #[must_use]
    pub fn font_style(&self) -> FontStyle {
        self.0.core().style
    }

    /// `SkTypeface::isFixedPitch`.
    // Port of: src/core/SkTypeface.cpp#L499-L501 (chrome/m156)
    #[doc(alias = "isFixedPitch")]
    #[must_use]
    pub fn is_fixed_pitch(&self) -> bool {
        self.0.core().is_fixed_pitch
    }

    /// `SkTypeface::isBold`: the weight is at least semi-bold.
    // Port of: src/core/SkTypeface.cpp#L491-L493 (chrome/m156)
    #[doc(alias = "isBold")]
    #[must_use]
    pub fn is_bold(&self) -> bool {
        *self.font_style().weight() >= *Weight::SEMI_BOLD
    }

    /// `SkTypeface::isItalic`: the slant is not upright.
    // Port of: src/core/SkTypeface.cpp#L495-L497 (chrome/m156)
    #[doc(alias = "isItalic")]
    #[must_use]
    pub fn is_italic(&self) -> bool {
        self.font_style().slant() != Slant::Upright
    }

    /// True if this handle is the only reference to the typeface (`SkRefCnt::unique`). The
    /// typeface cache purges only such typefaces.
    #[doc(alias = "unique")]
    #[must_use]
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }

    /// `SkTypeface::getFontDescriptor`: the descriptor, and whether the data is local.
    // Port of: include/core/SkTypeface.h#L348-L350 (chrome/m156)
    #[doc(alias = "getFontDescriptor")]
    #[must_use]
    pub fn font_descriptor(&self) -> (FontDescriptor, bool) {
        self.0.on_get_font_descriptor()
    }

    /// `SkTypeface::getFamilyName`.
    // Port of: src/core/SkTypeface.cpp#L466-L469 (chrome/m156)
    #[doc(alias = "getFamilyName")]
    #[must_use]
    pub fn family_name(&self) -> String {
        self.0.on_get_family_name()
    }

    /// `SkTypeface::openStream`: the font data and its collection index, or `None`.
    // Port of: src/core/SkTypeface.cpp#L333-L340 (chrome/m156)
    #[doc(alias = "openStream")]
    #[must_use]
    pub fn open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        self.0.on_open_stream()
    }

    /// `SkTypeface::getVariationDesignPosition`: the axes, or `None` when unknown.
    // Port of: src/core/SkTypeface.cpp#L290-L294 (chrome/m156)
    #[doc(alias = "getVariationDesignPosition")]
    #[must_use]
    pub fn variation_design_position(&self) -> Option<Vec<Coordinate>> {
        self.0.on_get_variation_design_position()
    }

    /// `SkTypeface::serialize(SkWStream*, behavior)`: writes the descriptor, and the font data
    /// when `behavior` asks for it. Returns false if a write fails.
    // Port of: src/core/SkTypeface.cpp#L201-L233 (chrome/m156)
    #[doc(alias = "serialize")]
    pub fn serialize_to(&self, stream: &mut dyn WStream, behavior: SerializeBehavior) -> bool {
        let (mut desc, is_local_data) = self.0.on_get_font_descriptor();
        let should_serialize_data = match behavior {
            SerializeBehavior::DoIncludeData => true,
            SerializeBehavior::DontIncludeData => false,
            SerializeBehavior::IncludeDataIfLocal => is_local_data,
        };
        if should_serialize_data {
            let opened = self.open_stream();
            let has_stream = opened.is_some();
            let (font_stream, index) = match opened {
                Some((font_stream, index)) => (Some(font_stream), index),
                None => (None, 0),
            };
            desc.set_stream(font_stream);
            if has_stream {
                desc.set_collection_index(index);
            }
            if let Some(coordinates) = self.variation_design_position()
                && !coordinates.is_empty()
            {
                desc.set_variation_coordinates(coordinates.len())
                    .copy_from_slice(&coordinates);
            }
        }
        desc.serialize(stream)
    }

    /// `SkTypeface::serialize(SerializeBehavior)`: the serialized form as data, or `None` if
    /// writing fails.
    // Port of: src/core/SkTypeface.cpp#L235-L238 (chrome/m156)
    #[must_use]
    pub fn serialize(&self, behavior: SerializeBehavior) -> Option<Data> {
        let mut stream = DynamicMemoryWStream::new();
        if self.serialize_to(&mut stream, behavior) {
            Some(stream.detach_as_data())
        } else {
            None
        }
    }
}

impl Typeface {
    /// `SkTypeface::unicharToGlyph`: the glyph for one unichar, 0 if there is none.
    // Port of: src/core/SkTypeface.cpp#L371-L375 (chrome/m156)
    #[doc(alias = "unicharToGlyph")]
    #[must_use]
    pub fn unichar_to_glyph(&self, uni: Unichar) -> GlyphId {
        let mut glyphs = [0];
        self.0.on_chars_to_glyphs(&[uni], &mut glyphs);
        glyphs[0]
    }

    /// `SkTypeface::unicharsToGlyphs`: the glyphs for the unichars, up to the shorter of the two
    /// slices.
    // Port of: src/core/SkTypeface.cpp#L365-L369 (chrome/m156)
    #[doc(alias = "unicharsToGlyphs")]
    pub fn unichars_to_glyphs(&self, unis: &[Unichar], glyphs: &mut [GlyphId]) {
        let n = unis.len().min(glyphs.len());
        if n > 0 {
            self.0.on_chars_to_glyphs(&unis[..n], &mut glyphs[..n]);
        }
    }

    /// `SkTypeface::textToGlyphs`: the glyphs for `text` in `encoding`. Returns the number of
    /// glyphs the text has. If `glyphs` is too short for them, nothing is written.
    // Port of: src/core/SkTypeface.cpp#L415-L438 (chrome/m156)
    #[doc(alias = "textToGlyphs")]
    pub fn text_to_glyphs(
        &self,
        text: &[u8],
        encoding: TextEncoding,
        glyphs: &mut [GlyphId],
    ) -> usize {
        if text.is_empty() {
            return 0;
        }
        let count = count_text_elements(text, encoding);
        if count > glyphs.len() {
            return count;
        }
        if encoding == TextEncoding::GlyphId {
            for (glyph, &[a, b]) in glyphs.iter_mut().zip(text.as_chunks::<2>().0) {
                *glyph = GlyphId::from_ne_bytes([a, b]);
            }
            return count;
        }
        let unis = convert_to_utf32(text, encoding);
        self.unichars_to_glyphs(&unis, glyphs);
        count
    }

    /// `SkTypeface::countGlyphs`: the number of glyphs in the typeface.
    // Port of: src/core/SkTypeface.cpp#L440-L442 (chrome/m156)
    #[doc(alias = "countGlyphs")]
    #[must_use]
    pub fn count_glyphs(&self) -> i32 {
        self.0.on_count_glyphs()
    }

    /// `SkTypeface::getGlyphToUnicodeMap`: the unichar of each glyph, from the start of `dst`.
    // Port of: src/core/SkTypeface.cpp#L512-L514 (chrome/m156)
    #[doc(alias = "getGlyphToUnicodeMap")]
    pub fn glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        self.0.on_get_glyph_to_unicode_map(dst);
    }
}

/// `SkConvertToUTF32::convert`: decodes `text` in `encoding` to unichars. The bytes are read as
/// native-endian code units; a UTF-32 text is only reinterpreted in C++, so it is decoded here.
// Port of: src/core/SkTypeface.cpp#L378-L409 (chrome/m156)
fn convert_to_utf32(text: &[u8], encoding: TextEncoding) -> Vec<Unichar> {
    match encoding {
        TextEncoding::UTF8 => {
            let mut ptr = text;
            let mut out = Vec::new();
            while !ptr.is_empty() {
                out.push(next_utf8(&mut ptr));
            }
            out
        }
        TextEncoding::UTF16 => {
            let units: Vec<u16> = text
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[a, b]| u16::from_ne_bytes([a, b]))
                .collect();
            let mut ptr = units.as_slice();
            let mut out = Vec::new();
            while !ptr.is_empty() {
                out.push(next_utf16(&mut ptr));
            }
            out
        }
        TextEncoding::UTF32 => text
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&bytes| Unichar::from_ne_bytes(bytes))
            .collect(),
        TextEncoding::GlyphId => Vec::new(),
    }
}

impl PartialEq for Typeface {
    /// `SkTypeface::Equal`: the same handle, or the same unique id.
    // Port of: src/core/SkTypeface.cpp#L148-L156 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0) || self.unique_id() == other.unique_id()
    }
}

impl Eq for Typeface {}

impl fmt::Debug for Typeface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Typeface")
            .field("unique_id", &self.unique_id())
            .field("style", &self.font_style())
            .finish()
    }
}

/// `SkEmptyTypeface`: no glyphs, no data, a fixed pitch, and the normal style.
// Port of: src/core/SkTypeface.cpp#L66-L140 (chrome/m156)
#[derive(Debug)]
struct EmptyTypeface {
    core: TypefaceCore,
}

impl EmptyTypeface {
    /// `SkEmptyTypeface() : SkTypeface(SkFontStyle(), true)`.
    // Port of: src/core/SkTypeface.cpp#L83 (chrome/m156)
    fn new() -> Self {
        Self {
            core: TypefaceCore::new(FontStyle::default(), true),
        }
    }
}

/// `SkEmptyTypeface::FactoryId`.
// Port of: src/core/SkTypeface.cpp#L73 (chrome/m156)
const EMPTY_FACTORY_ID: FactoryId = set_four_byte_tag(b'e', b'm', b't', b'y');

impl TypefaceBase for EmptyTypeface {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `SkEmptyTypeface::onGetFontDescriptor`: the factory id, and not serialized as data.
    // Port of: src/core/SkTypeface.cpp#L98-L101 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        let mut desc = FontDescriptor::new();
        desc.set_factory_id(EMPTY_FACTORY_ID);
        (desc, false)
    }

    /// `SkEmptyTypeface::onOpenStream`: there is no data.
    // Port of: src/core/SkTypeface.cpp#L85 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        None
    }

    /// `SkEmptyTypeface::onGetFamilyName`: the empty name.
    // Port of: src/core/SkTypeface.cpp#L115-L117 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        String::new()
    }

    /// `SkEmptyTypeface::onGetVariationDesignPosition`: no axes.
    // Port of: src/core/SkTypeface.cpp#L127-L130 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        Some(Vec::new())
    }

    /// `SkEmptyTypeface::onMakeClone`: the same object.
    // Port of: src/core/SkTypeface.cpp#L86-L88 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, _args: &FontArguments<'_, '_>) -> Typeface {
        this
    }

    /// `SkEmptyTypeface::onCreateScalerContext`: `SkScalerContext::MakeEmpty`.
    // Port of: src/core/SkTypeface.cpp#L89-L93 (chrome/m156)
    fn on_create_scaler_context(
        &self,
        this: Typeface,
        effects: &ScalerContextEffects,
        desc: &Descriptor,
    ) -> ScalerContext {
        ScalerContext::make_empty(this, effects, desc)
    }

    /// `SkEmptyTypeface::onFilterRec`: no change.
    // Port of: src/core/SkTypeface.cpp#L94 (chrome/m156)
    fn on_filter_rec(&self, _rec: &mut ScalerContextRec) {}

    /// `SkEmptyTypeface::onGlyphMaskNeedsCurrentColor`: false.
    // Port of: src/core/SkTypeface.cpp#L124-L126 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        false
    }
}
