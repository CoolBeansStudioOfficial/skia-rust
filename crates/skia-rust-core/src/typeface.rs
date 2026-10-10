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
use std::sync::{Arc, OnceLock, RwLock};

use crate::advanced_typeface_metrics::{AdvancedTypefaceMetrics, FontFlags, FontType};
use crate::data::Data;
use crate::descriptor::Descriptor;
use crate::font::Font;
use crate::font_arguments::FontArguments;
use crate::font_arguments::variation_position::Coordinate;
use crate::font_descriptor::{FactoryId, FontDescriptor};
use crate::font_mgr::{FontMgr, TypefaceDecoder};
use crate::font_parameters::variation::Axis;
use crate::font_priv::count_text_elements;
use crate::font_style::{FontStyle, Slant, Weight};
use crate::font_types::{FourByteTag, GlyphId, TextEncoding, set_four_byte_tag};
use crate::matrix::Matrix;
use crate::paint::Paint;
use crate::rect::Rect;
use crate::scalar::scalar;
use crate::scaler_context::{
    ScalerContext, ScalerContextBuildFlags, ScalerContextEffects, ScalerContextRec,
};
use crate::stream::{DynamicMemoryWStream, Stream, StreamAsset, WStream};
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

/// A [`LocalizedStrings`] over names that were already read, in order. Backends that read the
/// names up front (the Fontations port does) return one of these.
#[derive(Debug, Default)]
pub struct VecLocalizedStrings(std::vec::IntoIter<LocalizedString>);

impl VecLocalizedStrings {
    /// Iterates over `names` in order.
    #[must_use]
    pub fn new(names: Vec<LocalizedString>) -> Self {
        Self(names.into_iter())
    }
}

impl LocalizedStrings for VecLocalizedStrings {
    // Port of: include/core/SkTypeface.h#L269 (chrome/m156)
    fn next(&mut self) -> Option<LocalizedString> {
        self.0.next()
    }
}

/// Copies `all` into `out` as `SkTypeface`'s buffer APIs do: an empty `out` is a count query,
/// a too-small `out` fails (C++ returns -1), and otherwise the values are copied and counted.
// Port of: src/core/SkTypeface.cpp#L290-L294 (chrome/m156), the buffer contract of the on*
// virtuals that these wrap
fn fill_span<T: Copy>(all: &[T], out: &mut [T]) -> Option<usize> {
    if out.is_empty() {
        return Some(all.len());
    }
    if out.len() < all.len() {
        return None;
    }
    out[..all.len()].copy_from_slice(all);
    Some(all.len())
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

    /// `SkTypeface::onGetVariationDesignParameters`: every axis of the typeface, or `None` if
    /// they cannot be read. C++ fills a caller's buffer; the port returns the axes.
    // Port of: include/core/SkTypeface.h#L409 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>>;

    /// `SkTypeface::onGetUPEM`: the units per em, or 0 if unknown.
    // Port of: include/core/SkTypeface.h#L419 (chrome/m156)
    fn on_get_upem(&self) -> i32;

    /// `SkTypeface::onGetPostScriptName`: `None` when the typeface has no PostScript name.
    // Port of: include/core/SkTypeface.h#L427 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String>;

    /// `SkTypeface::onCreateFamilyNameIterator`: the family names of the typeface.
    // Port of: include/core/SkTypeface.h#L431 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings>;

    /// `SkTypeface::onGetTableTags`: the tags of the font's tables, in directory order.
    // Port of: include/core/SkTypeface.h#L433 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag>;

    /// `SkTypeface::onGetTableData`: copies the table from `offset` into `data`, which is empty
    /// for a size query, and returns the bytes available up to `length` (C++ `min(copied,
    /// length)`). The caller passes `data` with `length` bytes, or none.
    // Port of: include/core/SkTypeface.h#L434-L435 (chrome/m156)
    fn on_get_table_data(
        &self,
        tag: FourByteTag,
        offset: usize,
        length: usize,
        data: &mut [u8],
    ) -> usize;

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

    /// `SkTypeface::onGetKerningPairAdjustments`: whether the typeface has kerning. The default
    /// is `false` (no kerning), as for `SkTypeface` itself and the test typefaces.
    // Port of: src/core/SkTypeface.cpp#L540-L542 (chrome/m156)
    #[doc(alias = "onGetKerningPairAdjustments")]
    fn on_get_kerning_pair_adjustments(
        &self,
        _glyphs: &[GlyphId],
        _adjustments: &mut [i32],
    ) -> bool {
        false
    }

    /// `SkTypeface::onGetGlyphToUnicodeMap`: the unichar of each glyph. The default is all zeros,
    /// as for the empty typeface.
    // Port of: include/core/SkTypeface.h (onGetGlyphToUnicodeMap, chrome/m156)
    #[doc(alias = "onGetGlyphToUnicodeMap")]
    fn on_get_glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        dst.fill(0);
    }

    /// `SkTypeface::onGetAdvancedMetrics`: what the PDF backend needs to embed the typeface.
    /// `None` (the default) for a typeface that has none, as `SkEmptyTypeface` and
    /// `SkUserTypeface` return null.
    // Port of: include/core/SkTypeface.h (onGetAdvancedMetrics, chrome/m156)
    #[doc(alias = "onGetAdvancedMetrics")]
    fn on_get_advanced_metrics(&self) -> Option<AdvancedTypefaceMetrics> {
        None
    }

    /// `SkTypeface::getPostScriptGlyphNames`: the PostScript name of each glyph, from the start
    /// of `dst`. The default leaves `dst` as it is (no names), as `SkEmptyTypeface` does.
    // Port of: include/core/SkTypeface.h#L393 (chrome/m156)
    #[doc(alias = "getPostScriptGlyphNames")]
    fn get_post_script_glyph_names(&self, _dst: &mut [String]) {}

    /// `SkTypeface::onComputeBounds`: the bounds the typeface gives itself. `None` (the default)
    /// makes [`Typeface::get_bounds`] measure the font.
    // Port of: include/core/SkTypeface.h (onComputeBounds, chrome/m156), overridden by SkUserTypeface
    #[doc(alias = "onComputeBounds")]
    fn on_compute_bounds(&self) -> Option<Rect> {
        None
    }

    /// `SkTypeface::onIsSyntheticBold`: false, unless a backend fake-bolds the typeface.
    // Port of: src/core/SkTypeface.cpp#L509 (chrome/m156)
    #[doc(alias = "onIsSyntheticBold")]
    fn on_is_synthetic_bold(&self) -> bool {
        false
    }

    /// `SkTypeface::onIsSyntheticOblique`: false, unless a backend fake-obliques the typeface.
    // Port of: src/core/SkTypeface.cpp#L510 (chrome/m156)
    #[doc(alias = "onIsSyntheticOblique")]
    fn on_is_synthetic_oblique(&self) -> bool {
        false
    }

    /// `SkTypeface::onGetResourceName`: the resource the typeface was loaded from, if the
    /// backend knows one. The default is none (C++ returns 0 and sets nothing).
    // Port of: src/core/SkTypeface.cpp#L479-L481 (chrome/m156)
    #[doc(alias = "onGetResourceName")]
    fn on_get_resource_name(&self) -> Option<String> {
        None
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

        if let Some(bounds) = self.0.on_compute_bounds() {
            return bounds;
        }
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
        let auto_descriptor = auto_descriptor_given_rec_and_effects(&rec, &no_effects);
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

    /// `SkTypeface::isSyntheticBold`: true if the typeface is internally being fake bolded.
    // Port of: src/core/SkTypeface.cpp#L507 (chrome/m156)
    #[doc(alias = "isSyntheticBold")]
    #[must_use]
    pub fn is_synthetic_bold(&self) -> bool {
        self.0.on_is_synthetic_bold()
    }

    /// `SkTypeface::isSyntheticOblique`: true if the typeface is internally being fake obliqued.
    // Port of: src/core/SkTypeface.cpp#L508 (chrome/m156)
    #[doc(alias = "isSyntheticOblique")]
    #[must_use]
    pub fn is_synthetic_oblique(&self) -> bool {
        self.0.on_is_synthetic_oblique()
    }

    /// `SkTypeface::getResourceName`: the resource the typeface was loaded from, or `None` (C++
    /// returns 0 and leaves the string alone).
    // Port of: src/core/SkTypeface.cpp#L475-L477 (chrome/m156)
    #[doc(alias = "getResourceName")]
    #[must_use]
    pub fn get_resource_name(&self) -> Option<String> {
        self.0.on_get_resource_name()
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

    /// `SkTypeface::getVariationDesignPosition(span)`: an empty `coordinates` queries the count;
    /// `None` is C++'s -1 (the buffer is too small, or the axes are unknown).
    // Port of: src/core/SkTypeface.cpp#L290-L294 (chrome/m156)
    #[doc(alias = "getVariationDesignPosition")]
    pub fn get_variation_design_position(&self, coordinates: &mut [Coordinate]) -> Option<usize> {
        fill_span(&self.0.on_get_variation_design_position()?, coordinates)
    }

    /// `SkTypeface::getVariationDesignParameters`: every axis of the typeface.
    // Port of: include/core/SkTypeface.h (getVariationDesignParameters, chrome/m156)
    #[doc(alias = "getVariationDesignParameters")]
    #[must_use]
    pub fn variation_design_parameters(&self) -> Option<Vec<Axis>> {
        self.0.on_get_variation_design_parameters()
    }

    /// `SkTypeface::getVariationDesignParameters(span)`: as
    /// [`get_variation_design_position`](Self::get_variation_design_position), for axes.
    // Port of: src/core/SkTypeface.cpp#L297-L300 (chrome/m156)
    #[doc(alias = "getVariationDesignParameters")]
    pub fn get_variation_design_parameters(&self, parameters: &mut [Axis]) -> Option<usize> {
        fill_span(&self.0.on_get_variation_design_parameters()?, parameters)
    }

    /// `SkTypeface::getUnitsPerEm`: the units per em, or `None` when the typeface reports 0.
    // Port of: src/core/SkTypeface.cpp#L444-L447 (chrome/m156)
    #[doc(alias = "getUnitsPerEm")]
    #[must_use]
    pub fn units_per_em(&self) -> Option<i32> {
        let units = self.0.on_get_upem();
        (units != 0).then_some(units)
    }

    /// `SkTypeface::getPostScriptName`: `None` when the typeface has none.
    // Port of: src/core/SkTypeface.cpp (getPostScriptName, chrome/m156)
    #[doc(alias = "getPostScriptName")]
    #[must_use]
    pub fn post_script_name(&self) -> Option<String> {
        self.0.on_get_postscript_name()
    }

    /// `SkTypeface::createFamilyNameIterator`: the family names, in the font's order.
    // Port of: src/core/SkTypeface.cpp (createFamilyNameIterator, chrome/m156)
    #[doc(alias = "createFamilyNameIterator")]
    pub fn new_family_name_iterator(&self) -> impl Iterator<Item = LocalizedString> {
        let mut names = self.0.on_create_family_name_iterator();
        std::iter::from_fn(move || names.next())
    }

    /// `SkTypeface::countTables`: the number of tables in the font.
    // Port of: src/core/SkTypeface.cpp#L302-L304 (chrome/m156)
    #[doc(alias = "countTables")]
    #[must_use]
    pub fn count_tables(&self) -> usize {
        self.0.on_get_table_tags().len()
    }

    /// `SkTypeface::readTableTags`: copies the table tags into `tags`, and returns how many
    /// tables the font has. An empty `tags` queries the count.
    // Port of: src/core/SkTypeface.cpp#L306-L308 (chrome/m156)
    #[doc(alias = "readTableTags")]
    pub fn read_table_tags(&self, tags: &mut [FourByteTag]) -> usize {
        let all = self.0.on_get_table_tags();
        let n = all.len().min(tags.len());
        tags[..n].copy_from_slice(&all[..n]);
        all.len()
    }

    /// `SkTypeface::getTableSize`: the size of a table, or `None` if there is none.
    // Port of: src/core/SkTypeface.cpp#L310-L312 (chrome/m156)
    #[doc(alias = "getTableSize")]
    #[must_use]
    pub fn get_table_size(&self, tag: FourByteTag) -> Option<usize> {
        let size = self.0.on_get_table_data(tag, 0, usize::MAX, &mut []);
        (size != 0).then_some(size)
    }

    /// `SkTypeface::copyTableData(tag)`: a copy of the table, or `None` when it is missing.
    // Port of: src/core/SkTypeface.cpp#L319-L331 (chrome/m156)
    #[doc(alias = "copyTableData")]
    #[must_use]
    pub fn copy_table_data(&self, tag: FourByteTag) -> Option<Data> {
        let size = self.get_table_size(tag)?;
        let mut bytes = vec![0; size];
        // `(void)this->getTableData(...)`: the size was just read, so the copy is complete.
        let _ = self.get_table_data(tag, 0, size, Some(&mut bytes));
        Some(Data::new_from_vec(bytes))
    }

    /// `SkTypeface::getTableData(tag, offset, length, data)`: copies at most `length` bytes of
    /// the table, from `offset`, into `data` (when given), and returns the number of bytes. With
    /// no `data` it returns the size the copy would have. `data` is never written past its end.
    // Port of: src/core/SkTypeface.cpp#L314-L316 (chrome/m156)
    #[doc(alias = "getTableData")]
    #[must_use]
    pub fn get_table_data(
        &self,
        tag: FourByteTag,
        offset: usize,
        length: usize,
        data: Option<&mut [u8]>,
    ) -> usize {
        match data {
            Some(data) => {
                let n = length.min(data.len());
                self.0
                    .on_get_table_data(tag, offset, length, &mut data[..n])
            }
            None => self.0.on_get_table_data(tag, offset, length, &mut []),
        }
    }

    /// `SkTypeface::Register(id, make)`: adds a decoder that [`Typeface::make_deserialize`] consults
    /// for descriptors with `factory_id`. The list is process-wide and append-only, as Skia's is:
    /// the first decoder with a matching id wins, and nothing can be removed.
    // Port of: src/core/SkTypeface.cpp#L195-L199 (chrome/m156), `SkTypeface::Register`
    #[doc(alias = "Register")]
    pub fn register_decoder(
        factory_id: FactoryId,
        make_from_stream: fn(Box<dyn StreamAsset>, &FontArguments<'_, '_>) -> Option<Typeface>,
    ) {
        REGISTERED_DECODERS
            .write()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(TypefaceDecoder {
                factory_id,
                make_from_stream,
            });
    }

    /// `SkTypeface::MakeDeserialize`: reads a descriptor written by [`Typeface::serialize`] and
    /// makes the typeface. The decoders are core's built-in one, then those added by
    /// [`Typeface::register_decoder`], then the ones `last_resort_mgr` lists (docs/design/text.md
    /// §5.2 and Q3). A stream without a decoder falls back to `last_resort_mgr` and then to the
    /// empty typeface.
    /// `sanitizer` may rewrite the font data first; returning `None` from it fails the read.
    // Port of: src/core/SkTypeface.cpp#L229-L270 (chrome/m156)
    #[doc(alias = "MakeDeserialize")]
    #[must_use]
    pub fn make_deserialize(
        stream: &mut dyn Stream,
        last_resort_mgr: Option<&FontMgr>,
        sanitizer: Option<&dyn Fn(Data) -> Option<Data>>,
    ) -> Option<Typeface> {
        let mut desc = FontDescriptor::deserialize(stream, sanitizer)?;
        if desc.has_stream() {
            let factory_id = desc.factory_id();
            let manager_decoders =
                last_resort_mgr.map_or_else(Vec::new, FontMgr::typeface_decoders);
            let decoder = builtin_decoders()
                .into_iter()
                .chain(registered_decoders())
                .chain(manager_decoders)
                .find(|decoder| decoder.factory_id == factory_id);
            if let Some(decoder) = decoder {
                let font_stream = desc.detach_stream()?;
                return (decoder.make_from_stream)(font_stream, &desc.font_arguments());
            }
            // C++ prints "Could not find factory" here (SkDEBUGF) and falls through.
        }
        if let Some(mgr) = last_resort_mgr {
            // The last ditch effort: the manager may know the right face by name or data.
            let font_stream = desc.detach_stream();
            let typeface = mgr.make_from_stream_args(font_stream, &desc.font_arguments());
            if typeface.is_some() {
                return typeface;
            }
            return mgr.legacy_make_typeface(Some(desc.family_name()), desc.style());
        }
        Some(Typeface::empty())
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

    /// `SkTypeface::getKerningPairAdjustments`: the kerning between each pair of `glyphs`, into
    /// `adjustments` (`glyphs.len() == adjustments.len() + 1` is expected; the shorter of the two
    /// is used). Returns whether the typeface has kerning at all, also when either is empty.
    // Port of: src/core/SkTypeface.cpp#L449-L460 (chrome/m156)
    #[doc(alias = "getKerningPairAdjustments")]
    pub fn get_kerning_pair_adjustments(
        &self,
        glyphs: &[GlyphId],
        adjustments: &mut [i32],
    ) -> bool {
        // We need glyphs.len() == adjustments.len() + 1 unless either is emptyish, in which case
        // the virtual is still called, just to get the boolean result.
        if glyphs.len() <= 1 || adjustments.is_empty() {
            return self.0.on_get_kerning_pair_adjustments(&[], &mut []);
        }
        let n = (glyphs.len() - 1).min(adjustments.len());
        self.0
            .on_get_kerning_pair_adjustments(&glyphs[..=n], &mut adjustments[..n])
    }

    /// `SkTypeface::getGlyphToUnicodeMap`: the unichar of each glyph, from the start of `dst`.
    // Port of: src/core/SkTypeface.cpp#L512-L514 (chrome/m156)
    #[doc(alias = "getGlyphToUnicodeMap")]
    pub fn glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        self.0.on_get_glyph_to_unicode_map(dst);
    }

    /// `SkTypeface::getPostScriptGlyphNames`: the PostScript name of each glyph, from the start
    /// of `dst`.
    // Port of: include/core/SkTypeface.h#L393 (chrome/m156)
    #[doc(alias = "getPostScriptGlyphNames")]
    pub fn post_script_glyph_names(&self, dst: &mut [String]) {
        self.0.get_post_script_glyph_names(dst);
    }

    /// `SkTypeface::getAdvancedMetrics`: what the PDF backend needs to embed the typeface, or
    /// `None` if the typeface cannot say. The PostScript name falls back on the family name, and
    /// the `OS/2` `fsType` of a TrueType or CFF font marks it not embeddable or not subsettable.
    // Port of: src/core/SkTypeface.cpp#L516-L536 (chrome/m156)
    #[doc(alias = "getAdvancedMetrics")]
    #[must_use]
    pub fn advanced_metrics(&self) -> Option<AdvancedTypefaceMetrics> {
        // The `SkOTTableOS2::Version::V2::Type::Raw` masks of `fsType`.
        const RESTRICTED: u16 = 1 << 1;
        const PREVIEW_PRINT: u16 = 1 << 2;
        const EDITABLE: u16 = 1 << 3;
        const NO_SUBSETTING: u16 = 1 << 8;
        const BITMAP: u16 = 1 << 9;
        let mut result = self.0.on_get_advanced_metrics()?;
        if result.post_script_name.is_empty() {
            result.post_script_name = self
                .post_script_name()
                .unwrap_or_else(|| self.family_name());
        }
        if result.font_type == FontType::TrueType || result.font_type == FontType::Cff {
            // SkOTTableOS2::Version::V2::Type::Field fsType, a big-endian `uint16_t` at offset 8.
            const OS2_TAG: FourByteTag = set_four_byte_tag(b'O', b'S', b'/', b'2');
            const FS_TYPE_OFFSET: usize = 8;
            let mut fs_type = [0u8; 2];
            if self.get_table_data(OS2_TAG, FS_TYPE_OFFSET, 2, Some(&mut fs_type)) == 2 {
                let fs_type = u16::from_be_bytes(fs_type);
                if fs_type & BITMAP != 0
                    || (fs_type & RESTRICTED != 0 && fs_type & (PREVIEW_PRINT | EDITABLE) == 0)
                {
                    result.flags |= FontFlags::NOT_EMBEDDABLE;
                }
                if fs_type & NO_SUBSETTING != 0 {
                    result.flags |= FontFlags::NOT_SUBSETTABLE;
                }
            }
        }
        Some(result)
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

/// The decoders added by [`Typeface::register_decoder`], in registration order (`SkTypeface`'s
/// `decoders()` list after its static entries).
///
/// This is a deliberate exception to "no global mutable state" (CLAUDE.md): Skia's decoder list is
/// process-wide and filled by `SkTypeface::Register`, and ported tests depend on that global
/// registration, as they do on `StrikeCache::global()`. The list is append-only and read under a
/// `RwLock`, so a read never observes a partly registered decoder.
static REGISTERED_DECODERS: RwLock<Vec<TypefaceDecoder>> = RwLock::new(Vec::new());

/// A snapshot of [`REGISTERED_DECODERS`]. A poisoned lock still yields its list: the list is only
/// appended to, so a panic in another thread cannot leave it half written.
fn registered_decoders() -> Vec<TypefaceDecoder> {
    REGISTERED_DECODERS
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// The decoders that core knows without a manager: `SkTypeface.cpp`'s static list, which has the
/// empty typeface only in core (custom, Fontations and the test typefaces are listed by the
/// managers that make them, docs/design/text.md §5.2).
// Port of: src/core/SkTypeface.cpp#L162-L178 (chrome/m156)
fn builtin_decoders() -> [TypefaceDecoder; 1] {
    [TypefaceDecoder {
        factory_id: EMPTY_FACTORY_ID,
        make_from_stream: |_stream, _args| Some(Typeface::empty()),
    }]
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

    /// `SkEmptyTypeface::onGetVariationDesignParameters`: no axes.
    // Port of: src/core/SkTypeface.cpp#L132-L135 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        Some(Vec::new())
    }

    /// `SkEmptyTypeface::onGetUPEM`: 0.
    // Port of: src/core/SkTypeface.cpp#L108 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        0
    }

    /// `SkEmptyTypeface::onGetPostScriptName`: none.
    // Port of: src/core/SkTypeface.cpp#L118-L120 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        None
    }

    /// `SkEmptyTypeface::onCreateFamilyNameIterator`: no names.
    // Port of: src/core/SkTypeface.cpp#L121-L123 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::default())
    }

    /// `SkEmptyTypeface::onGetTableTags`: no tables.
    // Port of: src/core/SkTypeface.cpp#L136 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        Vec::new()
    }

    /// `SkEmptyTypeface::onGetTableData`: no data.
    // Port of: src/core/SkTypeface.cpp#L137-L139 (chrome/m156)
    fn on_get_table_data(
        &self,
        _tag: FourByteTag,
        _offset: usize,
        _length: usize,
        _data: &mut [u8],
    ) -> usize {
        0
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
