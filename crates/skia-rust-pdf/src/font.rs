// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFFont.{h,cpp} (chrome/m156)

//! `SkPDFFont`: the fonts of a PDF document.
//!
//! A [`PdfStrike`] is what a document draws a font and paint with (the canonicalized path strike
//! and image strike, de-duplicated by the document), and it owns the [`PdfFont`] resources that
//! are made for it: a font with single-byte glyph codes covers at most 255 glyphs, so a strike
//! has one font per run of 255 glyph ids, or a single font with two-byte codes. Which kind of
//! font a glyph needs follows [`font_type`]: TrueType and CID-keyed Type1 fonts are embedded as
//! Type0 fonts, Type1 fonts as Type1 fonts, and anything else (variable fonts, fonts that may not
//! be embedded, CFF, glyphs with modified paths, bitmaps, drawables, mask filters) as Type3 fonts
//! made of the glyphs' paths and images.
//!
//! The fonts are written when the document closes ([`PdfFont::emit_subset`]), once the glyphs
//! they need are known.
//!
//! skia-rust: a font keeps a weak handle to its strike (C++ keeps a raw pointer, and the strike
//! keeps the fonts in `fFontMap`), and the glyph usage of a font is shared with the device that
//! draws with it, so it is behind a `RefCell`. The document is single-threaded.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

use skia_rust_core::advanced_typeface_metrics::{AdvancedTypefaceMetrics, FontFlags, FontType};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::{FontHinting, GlyphId};
use skia_rust_core::glyph::Glyph;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::scalar::{scalar, scalar_round_to_int};
use skia_rust_core::scaler_context::ScalerContextBuildFlags;
use skia_rust_core::stream::{DynamicMemoryWStream, StreamAsset, WStream};
use skia_rust_core::strike_spec::{
    BulkGlyphMetricsAndDrawables, BulkGlyphMetricsAndImages, BulkGlyphMetricsAndPaths, StrikeSpec,
};
use skia_rust_core::t_pin::t_pin;
use skia_rust_core::typeface::{Typeface, TypefaceId};
use skia_rust_core::utf::Unichar;

use crate::bitmap::serialize_image_xobject;
use crate::cid_glyph_widths::make_cid_glyph_widths_array;
use crate::device::PdfDevice;
use crate::document::DocHandle;
use crate::form_xobject::make_form_x_object;
use crate::glyph_use::PdfGlyphUse;
use crate::graphic_state::{SMaskMode, get_smask_graphic_state};
use crate::subset_font::{pdf_can_subset_table_based_fonts, pdf_subset_font};
use crate::to_unicode_cmap::{GlyphToUnicodeEx, make_to_unicode_cmap};
use crate::type1_font::emit_type1_font;
use crate::types::{PdfArray, PdfDict, PdfIndirectReference, PdfParentTreeKey};
use crate::utils::{
    EmptyArea, EmptyPath, EmptyVerb, append_rectangle, append_scalar, apply_graphic_state,
    emit_path, make_int_array, matrix_to_array, paint_path,
};

/// `SK_PDF_MASK_QUALITY`: the JPEG quality of the masks.
// Port of: src/pdf/SkPDFTypes.h#L28-L31 (chrome/m156)
const PDF_MASK_QUALITY: i32 = 50;

/// PDF's notion of symbolic vs non-symbolic is related to the character set, not symbols vs.
/// characters. Rarely is a font the right character set to call it non-symbolic, so always call
/// it symbolic. (PDF 1.4 spec, section 5.7.1)
// Port of: src/pdf/SkPDFFont.cpp#L77-L79 (kPdfSymbolic, chrome/m156)
const PDF_SYMBOLIC: i32 = 4;

/// Scale from em-units to base-1000, returning as a scalar.
// Port of: src/pdf/SkPDFFont.cpp#L81-L84 (from_font_units, chrome/m156)
fn from_font_units(scaled: scalar, em_size: u16) -> scalar {
    if em_size == 1000 {
        scaled
    } else {
        scaled * 1000.0 / scalar::from(em_size)
    }
}

// Port of: src/pdf/SkPDFFont.cpp#L86-L88 (scaleFromFontUnits, chrome/m156)
fn scale_from_font_units(val: i16, em_size: u16) -> scalar {
    from_font_units(scalar::from(val), em_size)
}

/// Specify width and bounding box for the glyph.
// Port of: src/pdf/SkPDFFont.cpp#L90-L103 (setGlyphWidthAndBoundingBox, chrome/m156)
fn set_glyph_width_and_bounding_box(
    width: scalar,
    bbox: &IRect,
    content: &mut DynamicMemoryWStream,
) {
    append_scalar(width, content);
    content.write_text(" 0 ");
    content.write_dec_as_text(bbox.left);
    content.write_text(" ");
    content.write_dec_as_text(bbox.top);
    content.write_text(" ");
    content.write_dec_as_text(bbox.right);
    content.write_text(" ");
    content.write_dec_as_text(bbox.bottom);
    content.write_text(" d1\n");
}

/// What a strike scales a paint with: a mask filter that is a blur and a dash path effect can be
/// scaled, others cannot.
// Port of: src/pdf/SkPDFFont.cpp#L107-L145 (scale_paint, chrome/m156)
fn scale_paint(paint: &mut Paint, font_to_em_scale: scalar) -> bool {
    // What we really want here is a way ask the path effect or mask filter for a scaled
    // version of itself (if it is linearly scalable).

    if let Some(mask_filter) = paint.mask_filter() {
        if let Some(mut blur_rec) = mask_filter.as_base().as_a_blur() {
            // asABlur returns false if ignoring the CTM
            blur_rec.sigma *= font_to_em_scale;
            paint.set_mask_filter(skia_rust_core::mask_filter::MaskFilter::blur(
                blur_rec.style,
                blur_rec.sigma,
                true,
            ));
        } else {
            return false;
        }
    }
    if let Some(path_effect) = paint.path_effect() {
        if let Some(dash_info) = path_effect.as_a_dash() {
            let dst: Vec<scalar> = dash_info
                .intervals
                .iter()
                .map(|&interval| interval * font_to_em_scale)
                .collect();
            paint.set_path_effect(skia_rust_effects::dash_path_effect::new(
                &dst,
                dash_info.phase * font_to_em_scale,
            ));
        } else {
            return false;
        }
    }

    if paint.style() != Style::Fill && paint.stroke_width() > 0.0 {
        paint.set_stroke_width(paint.stroke_width() * font_to_em_scale);
    }

    true
}

/// `SkPDFStrikeSpec`: a strike spec and the units per em of its font.
// Port of: src/pdf/SkPDFFont.h#L30-L36 (chrome/m156)
#[doc(alias = "SkPDFStrikeSpec")]
#[derive(Clone, Debug)]
pub struct PdfStrikeSpec {
    /// `fStrikeSpec`.
    pub strike_spec: StrikeSpec,
    /// `fUnitsPerEM`.
    pub units_per_em: scalar,
}

/// The state of a [`PdfStrike`].
#[derive(Debug)]
struct StrikeData {
    path: PdfStrikeSpec,
    image: PdfStrikeSpec,
    has_mask_filter: bool,
    /// `fFontMap`: the font of each run of glyphs, by the first glyph id of the run (0 for the
    /// font with two-byte codes).
    font_map: RefCell<HashMap<GlyphId, PdfFont>>,
}

/// `SkPDFStrike`: what a document draws a font and paint with, and the fonts it has made for it.
/// Cloning gives another handle to the same strike.
// Port of: src/pdf/SkPDFFont.h#L38-L67 (chrome/m156)
#[doc(alias = "SkPDFStrike")]
#[derive(Clone, Debug)]
pub struct PdfStrike(Rc<StrikeData>);

impl PdfStrike {
    /// `SkPDFStrike::Make`: makes or returns an existing strike, canonicalizing for resource
    /// de-duplication. The document owns it. `None` if the typeface has no units per em or no
    /// glyphs.
    // Port of: src/pdf/SkPDFFont.cpp#L152-L205 (chrome/m156)
    #[must_use]
    pub fn make(doc: &DocHandle, font: &Font, paint: &Paint) -> Option<PdfStrike> {
        // `SK_PDF_BITMAP_GLYPH_RASTER_SIZE` is not defined: the default is 64.
        const BITMAP_FONT_SIZE: f32 = 64.0;

        let units_per_em = font.typeface().units_per_em().map_or(0.0, |u| u as scalar);
        let glyph_count = font.typeface().count_glyphs();
        if units_per_em <= 0.0 || glyph_count <= 0 {
            return None;
        }

        let mut canon_font = font.clone();
        canon_font.set_baseline_snap(false); // canonicalize
        canon_font.set_edging(Edging::AntiAlias); // canonicalize
        canon_font.set_embedded_bitmaps(false); // canonicalize
        // canonFont.setEmbolden(); // applied by scaler context, sets glyph path to modified
        canon_font.set_force_auto_hinting(false); // canonicalize
        canon_font.set_hinting(FontHinting::None); // canonicalize
        canon_font.set_linear_metrics(true); // canonicalize
        canon_font.set_scale_x(1.0); // original value applied by SkPDFDevice
        // canonFont.setSize(unitsPerEm);  // canonicalize below, adjusted by SkPDFDevice
        canon_font.set_skew_x(0.0); // original value applied by SkPDFDevice
        canon_font.set_subpixel(false); // canonicalize
        // canonFont.setTypeface();

        let mut path_paint = paint.clone();
        if scale_paint(&mut path_paint, units_per_em / font.size()) {
            canon_font.set_size(units_per_em);
        } else {
            canon_font.set_size(font.size());
        }
        let path_strike_em = canon_font.size();
        let path_strike_spec = StrikeSpec::make_with_no_device(
            &canon_font,
            Some(&path_paint),
            ScalerContextBuildFlags::NONE,
        );

        if let Some(strike) = doc.with(|d| d.strikes.get(path_strike_spec.descriptor()).cloned()) {
            return Some(strike);
        }

        // `if (kBitmapFontSize <= 0)` (old code path compatibility) is compiled out: it is 64.
        let mut image_paint = paint.clone();
        if scale_paint(&mut image_paint, BITMAP_FONT_SIZE / font.size()) {
            canon_font.set_size(BITMAP_FONT_SIZE);
        } else {
            canon_font.set_size(font.size());
        }
        let image_strike_em = canon_font.size();
        let image_strike_spec = StrikeSpec::make_with_no_device(
            &canon_font,
            Some(&image_paint),
            ScalerContextBuildFlags::NONE,
        );

        let descriptor = path_strike_spec.descriptor().clone();
        let strike = PdfStrike(Rc::new(StrikeData {
            path: PdfStrikeSpec {
                strike_spec: path_strike_spec,
                units_per_em: path_strike_em,
            },
            image: PdfStrikeSpec {
                strike_spec: image_strike_spec,
                units_per_em: image_strike_em,
            },
            has_mask_filter: path_paint.mask_filter().is_some(),
            font_map: RefCell::new(HashMap::new()),
        }));
        doc.with(|d| d.strikes.insert(descriptor, strike.clone()));
        Some(strike)
    }

    /// `fPath`.
    #[must_use]
    pub fn path(&self) -> &PdfStrikeSpec {
        &self.0.path
    }

    /// `fImage`.
    #[must_use]
    pub fn image(&self) -> &PdfStrikeSpec {
        &self.0.image
    }

    /// `fHasMaskFilter`.
    #[must_use]
    pub fn has_mask_filter(&self) -> bool {
        self.0.has_mask_filter
    }

    /// The fonts of the strike, in the order of their references.
    pub(crate) fn fonts(&self) -> Vec<PdfFont> {
        self.0.font_map.borrow().values().cloned().collect()
    }

    /// `SkPDFStrike::getFontResource`: the font resource for the glyph. The strike owns it.
    ///
    /// # Panics
    ///
    /// Panics where C++ asserts: the typeface must be one that [`get_metrics`] knows.
    // Port of: src/pdf/SkPDFFont.cpp#L342-L398 (chrome/m156)
    #[must_use]
    pub fn get_font_resource(&self, doc: &DocHandle, glyph: &Glyph) -> PdfFont {
        let typeface = self.0.path.strike_spec.typeface();
        // SkPDFDevice::internalDrawText ensures the typeface is good.
        // GetMetrics only returns null to signify a bad typeface.
        let font_metrics = get_metrics(typeface, doc).expect("a typeface with metrics");
        let metrics: &AdvancedTypefaceMetrics = &font_metrics;

        // Determine the FontType.
        // 1. Can the "original" font data be used directly
        // (simple OpenType, no non-default variations, not WOFF, etc).
        // 2. Is the glyph to be drawn unmodified from the font data
        // (no path effect, stroking, fake bolding, extra matrix, mask filter).
        // 3. Will PDF viewers draw this glyph the way we want
        // (at the moment this means an unmodified glyph path).
        let mut font_type = font_type(self, metrics);
        // Keep the type (and original data) if the glyph is empty or the glyph has an unmodified
        // path. Otherwise, fall back to Type3.
        if !(glyph.is_empty() || (glyph.path().is_some() && !glyph.path_is_modified())) {
            font_type = FontType::Other;
        }

        let multibyte = is_multi_byte(font_type);
        let subset_code = if multibyte {
            0
        } else {
            first_nonzero_glyph_for_single_byte_encoding(glyph.glyph_id())
        };
        if let Some(font) = self.0.font_map.borrow().get(&subset_code) {
            debug_assert_eq!(multibyte, font.multi_byte_glyphs());
            return font.clone();
        }

        let mut last_glyph = GlyphId::try_from(typeface.count_glyphs() - 1).expect("SkToU16");
        // should be caught by SkPDFDevice::internalDrawText
        debug_assert!(glyph.glyph_id() <= last_glyph);

        let first_non_zero_glyph;
        if multibyte {
            first_non_zero_glyph = 1;
        } else {
            first_non_zero_glyph = subset_code;
            last_glyph = GlyphId::try_from(i32::from(last_glyph).min(254 + i32::from(subset_code)))
                .expect("SkToU16");
        }
        let reference = doc.reserve_ref();
        let font = PdfFont::new(self, first_non_zero_glyph, last_glyph, font_type, reference);
        self.0
            .font_map
            .borrow_mut()
            .insert(subset_code, font.clone());
        font
    }
}

// Port of: src/pdf/SkPDFFont.cpp#L336-L340 (first_nonzero_glyph_for_single_byte_encoding,
// chrome/m156)
fn first_nonzero_glyph_for_single_byte_encoding(gid: GlyphId) -> GlyphId {
    if gid == 0 { 1 } else { gid - (gid - 1) % 255 }
}

/// The state of a [`PdfFont`].
#[derive(Debug)]
struct FontData {
    strike: Weak<StrikeData>,
    glyph_usage: RefCell<PdfGlyphUse>,
    indirect_reference: PdfIndirectReference,
    font_type: FontType,
}

/// `SkPDFFont`: a PDF font resource, owned by a [`PdfStrike`]. Cloning gives another handle to
/// the same font.
// Port of: src/pdf/SkPDFFont.h#L69-L166 (chrome/m156)
#[doc(alias = "SkPDFFont")]
#[derive(Clone, Debug)]
pub struct PdfFont(Rc<FontData>);

/// `SkPDFFont::IsMultiByte`.
// Port of: src/pdf/SkPDFFont.h#L88-L92 (chrome/m156)
#[must_use]
pub fn is_multi_byte(font_type: FontType) -> bool {
    matches!(
        font_type,
        FontType::Type1Cid | FontType::TrueType | FontType::Cff
    )
}

/// `SkPDFFont::FontType`: the type of font the strike needs for a typeface with these metrics.
// Port of: src/pdf/SkPDFFont.cpp#L316-L334 (chrome/m156)
#[doc(alias = "FontType")]
#[must_use]
pub fn font_type(pdf_strike: &PdfStrike, metrics: &AdvancedTypefaceMetrics) -> FontType {
    if metrics.flags.contains(FontFlags::VARIABLE)
        // PDF is actually interested in the encoding of the data, not just the logical format.
        // If the TrueType is actually wOFF or wOF2 then it should not be directly embedded in PDF.
        // Export these as Type3 if the subsetter cannot handle table based fonts.
        || (metrics.flags.contains(FontFlags::ALT_DATA_FORMAT)
            && !pdf_can_subset_table_based_fonts())
        || metrics.flags.contains(FontFlags::NOT_EMBEDDABLE)
        // Something like 45eeeddb00741493 and 7c86e7641b348ca7b0 to output OpenType should work,
        // but requires PDF 1.6 which is still not supported by all printers. One could fix this by
        // using bare CFF like 31a170226c22244cbd00497b67f6ae181f0f3e76 which is only PDF 1.3,
        // but this only works when the CFF CIDs == CFF index == GlyphID as PDF bare CFF prefers
        // CFF CIDs instead of GlyphIDs and Skia doesn't know the CIDs.
        || metrics.font_type == FontType::Cff
        || pdf_strike.has_mask_filter()
    {
        // force Type3 fallback.
        return FontType::Other;
    }
    metrics.font_type
}

impl PdfFont {
    /// The private constructor of `SkPDFFont`.
    // Port of: src/pdf/SkPDFFont.cpp#L402-L414 (chrome/m156)
    fn new(
        strike: &PdfStrike,
        first_glyph_id: GlyphId,
        last_glyph_id: GlyphId,
        font_type: FontType,
        indirect_reference: PdfIndirectReference,
    ) -> Self {
        let font = PdfFont(Rc::new(FontData {
            strike: Rc::downgrade(&strike.0),
            glyph_usage: RefCell::new(PdfGlyphUse::new(first_glyph_id, last_glyph_id)),
            indirect_reference,
            font_type,
        }));
        // Always include glyph 0
        font.note_glyph_usage(0);
        font
    }

    /// `getType`: the font type represented in this font. For Type0 fonts, the type of the
    /// descendant font.
    #[must_use]
    pub fn font_type(&self) -> FontType {
        self.0.font_type
    }

    /// `multiByteGlyphs`: true if this font encoding supports glyph IDs above 255.
    #[must_use]
    pub fn multi_byte_glyphs(&self) -> bool {
        is_multi_byte(self.font_type())
    }

    /// `firstGlyphID`.
    #[must_use]
    pub fn first_glyph_id(&self) -> GlyphId {
        self.0.glyph_usage.borrow().first_non_zero()
    }

    /// `lastGlyphID`.
    #[must_use]
    pub fn last_glyph_id(&self) -> GlyphId {
        self.0.glyph_usage.borrow().last_glyph()
    }

    /// `hasGlyph`: true if this font has an encoding for the passed glyph id.
    #[must_use]
    pub fn has_glyph(&self, gid: GlyphId) -> bool {
        (gid >= self.first_glyph_id() && gid <= self.last_glyph_id()) || gid == 0
    }

    /// `glyphToPDFFontEncoding`: converts the input glyph ID into the font encoding.
    #[must_use]
    pub fn glyph_to_pdf_font_encoding(&self, gid: GlyphId) -> GlyphId {
        if self.multi_byte_glyphs() || gid == 0 {
            return gid;
        }
        debug_assert!(gid >= self.first_glyph_id() && gid <= self.last_glyph_id());
        debug_assert!(self.first_glyph_id() > 0);
        gid - self.first_glyph_id() + 1
    }

    /// `noteGlyphUsage`.
    pub fn note_glyph_usage(&self, glyph: GlyphId) {
        debug_assert!(self.has_glyph(glyph));
        self.0.glyph_usage.borrow_mut().set(glyph);
    }

    /// `indirectReference`.
    #[must_use]
    pub fn indirect_reference(&self) -> PdfIndirectReference {
        self.0.indirect_reference
    }

    /// `glyphUsage`.
    #[must_use]
    pub fn glyph_usage(&self) -> PdfGlyphUse {
        self.0.glyph_usage.borrow().clone()
    }

    /// `strike`.
    ///
    /// # Panics
    ///
    /// Panics if the strike is gone, which it is not while the document is.
    #[must_use]
    pub fn strike(&self) -> PdfStrike {
        PdfStrike(
            self.0
                .strike
                .upgrade()
                .expect("the document owns the strike"),
        )
    }

    /// `SkPDFFont::emitSubset`: writes the font.
    // Port of: src/pdf/SkPDFFont.cpp#L898-L911 (chrome/m156)
    pub fn emit_subset(&self, doc: &DocHandle) {
        match self.0.font_type {
            FontType::Type1Cid | FontType::TrueType => emit_subset_type0(self, doc),
            FontType::Type1 => emit_type1_font(self, doc),
            _ => emit_subset_type3(self, doc),
        }
    }
}

// Port of: src/pdf/SkPDFFont.cpp#L207-L213 (can_embed, chrome/m156)
fn can_embed(metrics: &AdvancedTypefaceMetrics) -> bool {
    !metrics.flags.contains(FontFlags::NOT_EMBEDDABLE)
}

// Port of: src/pdf/SkPDFFont.cpp#L215-L217 (can_subset, chrome/m156)
fn can_subset(metrics: &AdvancedTypefaceMetrics) -> bool {
    !metrics.flags.contains(FontFlags::NOT_SUBSETTABLE)
}

/// `SkPDFFont::GetMetrics`: the metrics of the typeface, cached in the document. `None` only when
/// the typeface is bad.
// Port of: src/pdf/SkPDFFont.cpp#L219-L276 (chrome/m156)
#[doc(alias = "GetMetrics")]
#[must_use]
pub fn get_metrics(typeface: &Typeface, doc: &DocHandle) -> Option<Rc<AdvancedTypefaceMetrics>> {
    let id: TypefaceId = typeface.unique_id();
    if let Some(cached) = doc.with(|d| d.typeface_metrics.get(&id).cloned()) {
        return cached; // canon retains ownership.
    }

    let count = typeface.count_glyphs();
    if count <= 0 || count > 1 + i32::from(u16::MAX) {
        // Cache None to skip this check.
        doc.with(|d| d.typeface_metrics.insert(id, None));
        return None;
    }

    let mut metrics = typeface.advanced_metrics().unwrap_or_default();
    if 0 == metrics.stem_v || 0 == metrics.cap_height {
        let mut font = Font::from_typeface(Some(typeface.clone()));
        font.set_hinting(FontHinting::None);
        font.set_size(1000.0); // glyph coordinate system
        if 0 == metrics.stem_v {
            // Figure out a good guess for StemV - Min width of i, I, !, 1.
            // This probably isn't very good with an italic font.
            let mut stem_v = i16::MAX;
            for c in ['i', 'I', '!', '1'] {
                let g = font.unichar_to_glyph(c as Unichar);
                let mut bounds = [Rect::default()];
                font.get_widths_bounds(&[g], &mut [], &mut bounds, None);
                let width = i16::try_from(scalar_round_to_int(bounds[0].width())).expect("SkToS16");
                stem_v = stem_v.min(width);
            }
            metrics.stem_v = stem_v;
        }
        if 0 == metrics.cap_height {
            // Figure out a good guess for CapHeight: average the height of M and X.
            let mut cap_height: scalar = 0.0;
            for c in ['M', 'X'] {
                let g = font.unichar_to_glyph(c as Unichar);
                let mut bounds = [Rect::default()];
                font.get_widths_bounds(&[g], &mut [], &mut bounds, None);
                cap_height += bounds[0].height();
            }
            metrics.cap_height =
                i16::try_from(scalar_round_to_int(cap_height / 2.0)).expect("SkToS16");
        }
    }
    // Fonts are always subset, so always prepend the subset tag.
    let tag = doc.with(|d| d.next_font_subset_tag());
    metrics.post_script_name.insert_str(0, &tag);
    let metrics = Rc::new(metrics);
    doc.with(|d| d.typeface_metrics.insert(id, Some(Rc::clone(&metrics))));
    Some(metrics)
}

/// `SkPDFFont::GetUnicodeMap`: the unichar of each glyph of the typeface, cached in the
/// document.
// Port of: src/pdf/SkPDFFont.cpp#L278-L290 (chrome/m156)
#[doc(alias = "GetUnicodeMap")]
#[must_use]
pub fn get_unicode_map(typeface: &Typeface, doc: &DocHandle) -> Rc<Vec<Unichar>> {
    let id = typeface.unique_id();
    if let Some(map) = doc.with(|d| d.to_unicode_map.get(&id).cloned()) {
        return map;
    }
    let count = usize::try_from(typeface.count_glyphs()).unwrap_or(0);
    let mut buffer: Vec<Unichar> = vec![0; count];
    typeface.glyph_to_unicode_map(&mut buffer);
    let map = Rc::new(buffer);
    doc.with(|d| d.to_unicode_map.insert(id, Rc::clone(&map)));
    map
}

/// `SkPDFFont::GetUnicodeMapEx`: the glyphs of the typeface that map to text the font's cmap
/// does not have, cached in the document. The draws add to it.
// Port of: src/pdf/SkPDFFont.cpp#L292-L301 (chrome/m156)
#[doc(alias = "GetUnicodeMapEx")]
#[must_use]
pub fn get_unicode_map_ex(typeface: &Typeface, doc: &DocHandle) -> Rc<RefCell<GlyphToUnicodeEx>> {
    let id = typeface.unique_id();
    doc.with(|d| {
        Rc::clone(
            d.to_unicode_map_ex
                .entry(id)
                .or_insert_with(|| Rc::new(RefCell::new(GlyphToUnicodeEx::new(glyph_id_hash)))),
        )
    })
}

/// The hash of a `SkGlyphID` key (`SkGoodHash`).
fn glyph_id_hash(key: &GlyphId) -> u32 {
    skia_rust_core::checksum::GoodHash::good_hash(key)
}

/// `SkPDFFont::GetType1GlyphNames`: the PostScript names of the glyphs of the typeface.
// Port of: src/pdf/SkPDFFont.cpp#L63-L65 (chrome/m156)
#[doc(alias = "GetType1GlyphNames")]
pub fn get_type1_glyph_names(typeface: &Typeface, dst: &mut [String]) {
    typeface.post_script_glyph_names(dst);
}

/// `SkPDFFont::PopulateCommonFontDescriptor`.
// Port of: src/pdf/SkPDFFont.cpp#L416-L442 (chrome/m156)
#[doc(alias = "PopulateCommonFontDescriptor")]
pub fn populate_common_font_descriptor(
    descriptor: &mut PdfDict,
    metrics: &AdvancedTypefaceMetrics,
    em_size: u16,
    default_width: i16,
) {
    descriptor.insert_name_escaped("FontName", &metrics.post_script_name);
    descriptor.insert_int(
        "Flags",
        i32::try_from(metrics.style.bits() | PDF_SYMBOLIC as u32).expect("fits"),
    );
    descriptor.insert_scalar("Ascent", scale_from_font_units(metrics.ascent, em_size));
    descriptor.insert_scalar("Descent", scale_from_font_units(metrics.descent, em_size));
    descriptor.insert_scalar("StemV", scale_from_font_units(metrics.stem_v, em_size));
    descriptor.insert_scalar(
        "CapHeight",
        scale_from_font_units(metrics.cap_height, em_size),
    );
    descriptor.insert_int("ItalicAngle", i32::from(metrics.italic_angle));
    let mut bbox = PdfArray::new();
    bbox.append_scalar(from_font_units(metrics.bbox.left() as scalar, em_size));
    bbox.append_scalar(from_font_units(metrics.bbox.bottom() as scalar, em_size));
    bbox.append_scalar(from_font_units(metrics.bbox.right() as scalar, em_size));
    bbox.append_scalar(from_font_units(metrics.bbox.top() as scalar, em_size));
    descriptor.insert_object("FontBBox", Box::new(bbox));
    if default_width > 0 {
        descriptor.insert_scalar(
            "MissingWidth",
            scale_from_font_units(default_width, em_size),
        );
    }
}

/// `SkPDFFont::CanEmbedTypeface`: false iff the typeface has its `NotEmbeddable` flag set.
// Port of: src/pdf/SkPDFFont.cpp#L913-L917 (chrome/m156)
#[doc(alias = "CanEmbedTypeface")]
#[must_use]
pub fn can_embed_typeface(typeface: &Typeface, doc: &DocHandle) -> bool {
    get_metrics(typeface, doc).is_some_and(|metrics| can_embed(&metrics))
}

/// The bytes of a stream.
fn read_stream(stream: &mut dyn StreamAsset) -> Vec<u8> {
    let mut data = vec![0u8; stream.get_length()];
    let n = stream.read(&mut data);
    data.truncate(n);
    data
}

////////////////////////////////////////////////////////////////////////////////
//  Type0Font
////////////////////////////////////////////////////////////////////////////////

// Port of: src/pdf/SkPDFFont.cpp#L444-L572 (emit_subset_type0, chrome/m156)
fn emit_subset_type0(font: &PdfFont, doc: &DocHandle) {
    let strike = font.strike();
    let typeface = strike.path().strike_spec.typeface().clone();
    let Some(metrics) = get_metrics(&typeface, doc) else {
        debug_assert!(false);
        return;
    };
    debug_assert!(can_embed(&metrics));
    let font_type = font.font_type();

    let mut descriptor = PdfDict::new(Some("FontDescriptor"));
    let em_size = u16::try_from(scalar_round_to_int(strike.path().units_per_em)).expect("SkToU16");
    populate_common_font_descriptor(&mut descriptor, &metrics, em_size, 0);

    let font_asset = typeface.open_stream();
    let font_data = font_asset
        .map(|(mut asset, _ttc_index)| read_stream(asset.as_mut()))
        .unwrap_or_default();
    if font_data.is_empty() {
        // C++: "Error: (SkTypeface)(%p)::openStream() returned empty stream (%p) when
        // identified as kType1CID_Font or kTrueType_Font."
    } else if font_type == FontType::TrueType {
        let mut subset_font_data = None;
        if can_subset(&metrics) {
            debug_assert_eq!(font.first_glyph_id(), 1);
            subset_font_data = pdf_subset_font(&typeface, &font.glyph_usage());
        }
        // If subsetting fails, fall back to original font data.
        let subset_font_asset = subset_font_data.unwrap_or(font_data);
        let mut stream_dict = PdfDict::new(None);
        stream_dict.insert_int_usize("Length1", subset_font_asset.len());
        let stream = doc.stream_out(Some(stream_dict), &subset_font_asset, true);
        descriptor.insert_ref("FontFile2", stream);
    } else if font_type == FontType::Type1Cid {
        let mut stream_dict = PdfDict::new(None);
        stream_dict.insert_name("Subtype", "CIDFontType0C");
        let stream = doc.stream_out(Some(stream_dict), &font_data, true);
        descriptor.insert_ref("FontFile3", stream);
    } else {
        debug_assert!(false);
    }

    let mut new_cid_font = PdfDict::new(Some("Font"));
    new_cid_font.insert_ref("FontDescriptor", doc.emit_new(&descriptor));
    new_cid_font.insert_name_escaped("BaseFont", &metrics.post_script_name);

    match font_type {
        FontType::Type1Cid => new_cid_font.insert_name("Subtype", "CIDFontType0"),
        FontType::TrueType => {
            new_cid_font.insert_name("Subtype", "CIDFontType2");
            new_cid_font.insert_name("CIDToGIDMap", "Identity");
        }
        _ => debug_assert!(false),
    }
    let mut sys_info = PdfDict::new(None);
    // These are actually ASCII strings.
    sys_info.insert_byte_string("Registry", "Adobe");
    sys_info.insert_byte_string("Ordering", "Identity");
    sys_info.insert_int("Supplement", 0);
    new_cid_font.insert_object("CIDSystemInfo", Box::new(sys_info));

    // Unfortunately, poppler enforces DW (default width) must be an integer.
    let mut default_width = 0i32;
    {
        let widths =
            make_cid_glyph_widths_array(strike.path(), &font.glyph_usage(), &mut default_width);
        if widths.size() > 0 {
            new_cid_font.insert_object("W", Box::new(widths));
        }
        new_cid_font.insert_int("DW", default_width);
    }

    ////////////////////////////////////////////////////////////////////////////

    let mut font_dict = PdfDict::new(Some("Font"));
    font_dict.insert_name("Subtype", "Type0");
    font_dict.insert_name_escaped("BaseFont", &metrics.post_script_name);
    font_dict.insert_name("Encoding", "Identity-H");
    let mut descendant_fonts = PdfArray::new();
    descendant_fonts.append_ref(doc.emit_new(&new_cid_font));
    font_dict.insert_object("DescendantFonts", Box::new(descendant_fonts));

    let glyph_to_unicode = get_unicode_map(&typeface, doc);
    debug_assert_eq!(
        usize::try_from(typeface.count_glyphs()).unwrap_or(0),
        glyph_to_unicode.len()
    );
    let unicode_map_ex = get_unicode_map_ex(&typeface, doc);
    let to_unicode = make_to_unicode_cmap(
        &glyph_to_unicode,
        &unicode_map_ex.borrow(),
        Some(&font.glyph_usage()),
        font.multi_byte_glyphs(),
        font.first_glyph_id(),
        font.last_glyph_id(),
    );
    font_dict.insert_ref("ToUnicode", doc.stream_out(None, &to_unicode, true));

    doc.emit(&font_dict, font.indirect_reference());
}

////////////////////////////////////////////////////////////////////////////////
// PDFType3Font
////////////////////////////////////////////////////////////////////////////////

/// `SingleByteGlyphIdIterator`: returns `[0, first, first+1, ... last-1, last]`.
// Port of: src/pdf/SkPDFFont.cpp#L574-L609 (chrome/m156)
fn single_byte_glyph_ids(first: GlyphId, last: GlyphId) -> impl Iterator<Item = GlyphId> {
    debug_assert!(first > 0);
    debug_assert!(last >= first);
    std::iter::once(0).chain(first..=last)
}

/// `ImageAndOffset`.
struct ImageAndOffset {
    image: Option<Image>,
    offset: (i32, i32),
}

// Port of: src/pdf/SkPDFFont.cpp#L611-L664 (to_image, chrome/m156)
fn to_image(gid: GlyphId, small_glyphs: &BulkGlyphMetricsAndImages) -> ImageAndOffset {
    let glyph = small_glyphs.glyph(PackedGlyphId::from_glyph_id(gid));
    let mask = glyph.mask();
    if mask.image.is_empty() {
        return ImageAndOffset {
            image: None,
            offset: (0, 0),
        };
    }
    let bounds = mask.bounds;
    match mask.format {
        MaskFormat::BW => {
            // Make a gray image, used to smask a rectangle.
            // TODO: emit as MaskImage?
            let size = bounds.size();
            let width = usize::try_from(size.width).unwrap_or(0);
            let height = usize::try_from(size.height).unwrap_or(0);
            let mut pixels = vec![0u8; width * height];
            for y in 0..height {
                let mut x8 = 0;
                while x8 < width {
                    let v = mask.get_addr1(
                        i32::try_from(x8).expect("fits") + bounds.left,
                        i32::try_from(y).expect("fits") + bounds.top,
                    )[0];
                    let e = (x8 + 8).min(width);
                    for x in x8..e {
                        pixels[y * width + x] = if (v >> (x & 0x7)) & 0x1 != 0 {
                            0xFF
                        } else {
                            0x00
                        };
                    }
                    x8 += 8;
                }
            }
            ImageAndOffset {
                image: skia_rust_core::images::raster_from_data(
                    &ImageInfo::new(size, ColorType::Gray8, AlphaType::Unknown, None),
                    Data::new_copy(&pixels),
                    width,
                ),
                offset: (bounds.left, bounds.top),
            }
        }
        MaskFormat::A8 | MaskFormat::ThreeD => {
            // just do the A8 part
            // Make a gray image, used to smask a rectangle.
            ImageAndOffset {
                image: skia_rust_core::images::raster_from_data(
                    &ImageInfo::new(bounds.size(), ColorType::Gray8, AlphaType::Unknown, None),
                    Data::new_copy(&mask.image[..mask.compute_image_size()]),
                    mask.row_bytes as usize,
                ),
                offset: (bounds.left, bounds.top),
            }
        }
        MaskFormat::Argb32 => {
            // These will be drawn as images directly.
            ImageAndOffset {
                image: skia_rust_core::images::raster_from_data(
                    &ImageInfo::new_n32_premul(bounds.size(), None),
                    Data::new_copy(&mask.image[..mask.compute_total_image_size()]),
                    mask.row_bytes as usize,
                ),
                offset: (bounds.left, bounds.top),
            }
        }
        _ => {
            // kLCD16_Format and the rest
            debug_assert!(false);
            ImageAndOffset {
                image: None,
                offset: (0, 0),
            }
        }
    }
}

// Port of: src/pdf/SkPDFFont.cpp#L666-L725 (type3_descriptor, chrome/m156)
fn type3_descriptor(
    doc: &DocHandle,
    typeface: &Typeface,
    x_height: scalar,
) -> PdfIndirectReference {
    if let Some(reference) =
        doc.with(|d| d.type3_font_descriptors.get(&typeface.unique_id()).copied())
    {
        return reference;
    }

    let mut descriptor = PdfDict::new(Some("FontDescriptor"));
    let mut font_descriptor_flags = PDF_SYMBOLIC;

    // PDF32000_2008: FontFamily should be used for Type3 fonts in Tagged PDF documents.
    let family_name = typeface.family_name();
    if !family_name.is_empty() {
        descriptor.insert_byte_string("FontFamily", &family_name);
    }

    // PDF32000_2008: FontStretch should be used for Type3 fonts in Tagged PDF documents.
    const STRETCH_NAMES: [&str; 9] = [
        "UltraCondensed",
        "ExtraCondensed",
        "Condensed",
        "SemiCondensed",
        "Normal",
        "SemiExpanded",
        "Expanded",
        "ExtraExpanded",
        "UltraExpanded",
    ];
    let stretch_name = STRETCH_NAMES
        [usize::try_from(*typeface.font_style().width() - 1).expect("a width of at least 1")];
    descriptor.insert_name("FontStretch", stretch_name);

    // PDF32000_2008: FontWeight should be used for Type3 fonts in Tagged PDF documents.
    let weight = (*typeface.font_style().weight() + 50) / 100;
    descriptor.insert_int("FontWeight", t_pin(weight, 1, 9) * 100);

    if let Some(metrics) = get_metrics(typeface, doc) {
        // Type3 FontDescriptor does not require all the same fields.
        descriptor.insert_name_escaped("FontName", &metrics.post_script_name);
        descriptor.insert_int("ItalicAngle", i32::from(metrics.italic_angle));
        font_descriptor_flags |= i32::try_from(metrics.style.bits()).expect("fits");
        // Adobe requests CapHeight, XHeight, and StemV be added
        // to "greatly help our workflow downstream".
        if metrics.cap_height != 0 {
            descriptor.insert_int("CapHeight", i32::from(metrics.cap_height));
        }
        if metrics.stem_v != 0 {
            descriptor.insert_int("StemV", i32::from(metrics.stem_v));
        }
        if x_height != 0.0 {
            descriptor.insert_scalar("XHeight", x_height);
        }
    }
    descriptor.insert_int("Flags", font_descriptor_flags);
    let reference = doc.emit_new(&descriptor);
    doc.with(|d| {
        d.type3_font_descriptors
            .insert(typeface.unique_id(), reference)
    });
    reference
}

// Port of: src/pdf/SkPDFFont.cpp#L727-L896 (emit_subset_type3, chrome/m156)
#[allow(clippy::too_many_lines)] // one function in the C++, kept as is
#[allow(clippy::cast_precision_loss)] // SkIntToScalar-style casts mirror the C++
fn emit_subset_type3(pdf_font: &PdfFont, doc: &DocHandle) {
    let pdf_strike = pdf_font.strike();
    let first_glyph_id = pdf_font.first_glyph_id();
    let mut last_glyph_id = pdf_font.last_glyph_id();
    let subset = pdf_font.glyph_usage();
    debug_assert!(last_glyph_id >= first_glyph_id);
    // Remove unused glyphs at the end of the range.
    // Keep the lastGlyphID >= firstGlyphID invariant true.
    while last_glyph_id > first_glyph_id && !subset.has(last_glyph_id) {
        last_glyph_id -= 1;
    }
    let em_size = pdf_strike.path().units_per_em;
    let strike = pdf_strike.path().strike_spec.find_or_create_strike();
    let x_height = strike.font_metrics().x_height;
    let metrics_and_paths = BulkGlyphMetricsAndPaths::from_strike(strike.clone());
    let metrics_and_drawables = BulkGlyphMetricsAndDrawables::from_strike(strike);

    let small_glyphs = BulkGlyphMetricsAndImages::new(&pdf_strike.image().strike_spec);
    let bitmap_scale = em_size / pdf_strike.image().units_per_em;

    let mut font = PdfDict::new(Some("Font"));
    font.insert_name("Subtype", "Type3");
    // Flip about the x-axis and scale by 1/emSize.
    let mut font_matrix = Matrix::new_identity();
    font_matrix.set_scale((1.0 / em_size, -(1.0 / em_size)), None);
    font.insert_object("FontMatrix", Box::new(matrix_to_array(&font_matrix)));

    let mut char_procs = PdfDict::new(None);
    let mut encoding = PdfDict::new(Some("Encoding"));

    let mut enc_diffs = PdfArray::new();
    // length(firstGlyphID .. lastGlyphID) ==  lastGlyphID - firstGlyphID + 1
    // plus 1 for glyph 0;
    debug_assert!(first_glyph_id > 0);
    debug_assert!(last_glyph_id >= first_glyph_id);
    let glyph_count = usize::from(last_glyph_id - first_glyph_id) + 2;
    // one other entry for the index of first glyph.
    enc_diffs.reserve(glyph_count + 1);
    enc_diffs.append_int(0); // index of first glyph

    let mut width_array = PdfArray::new();
    width_array.reserve(glyph_count);

    let mut bbox = IRect::new_empty();

    let mut xobjects = PdfDict::new(None);
    let mut graphic_states = PdfDict::new(None);
    for g_id in single_byte_glyph_ids(first_glyph_id, last_glyph_id) {
        let character_name;
        let advance: scalar;

        if g_id != 0 && !subset.has(g_id) {
            character_name = String::from("g0");
            advance = 0.0;
            enc_diffs.append_name_escaped(&character_name);
            width_array.append_scalar(advance);
            continue;
        }

        let path_glyph = metrics_and_paths.glyph(g_id);
        let drawable_glyph = metrics_and_drawables.glyph(g_id);

        character_name = format!("g{g_id:X}");
        advance = path_glyph.advance_x();
        enc_diffs.append_name_escaped(&character_name);
        width_array.append_scalar(advance);

        let glyph_bbox = path_glyph.i_rect();
        bbox = IRect::join(&bbox, &glyph_bbox);
        let path = path_glyph.path();
        let drawable = drawable_glyph.drawable();
        let mut content = DynamicMemoryWStream::new();
        if let Some(drawable) = drawable.filter(|d| !d.bounds().is_empty()) {
            let glyph_device = PdfDevice::new(glyph_bbox.size(), doc, &Matrix::new_identity());
            let glyph_content = glyph_device.content_handle();
            let canvas = Canvas::from_device(Box::new(glyph_device));
            canvas.translate((-(glyph_bbox.left as scalar), -(glyph_bbox.top as scalar)));
            drawable.draw(&canvas, None);
            let glyph_stream = glyph_content.borrow_mut().content();
            let resource_dict = glyph_content.borrow().make_resource_dict();
            let xobject = make_form_x_object(
                doc,
                &glyph_stream,
                PdfParentTreeKey::default(),
                make_int_array(&[0, 0, glyph_bbox.width(), glyph_bbox.height()]),
                resource_dict,
                &Matrix::translate((glyph_bbox.left as scalar, glyph_bbox.top as scalar)),
                None,
            );
            xobjects.insert_ref_escaped_key(format!("Xg{g_id:X}"), xobject);
            append_scalar(drawable_glyph.advance_x(), &mut content);
            content.write_text(" 0 d0\n1 0 0 1 0 0 cm\n/X");
            content.write(character_name.as_bytes());
            content.write_text(" Do\n");
        } else if let Some(path) = path.filter(|p| !p.is_empty() && !pdf_strike.has_mask_filter()) {
            set_glyph_width_and_bounding_box(path_glyph.advance_x(), &glyph_bbox, &mut content);
            let style = if path_glyph.path_is_hairline() {
                Style::Stroke
            } else {
                Style::Fill
            };

            let empty_area = if style == Style::Fill {
                EmptyArea::Discard
            } else {
                EmptyArea::Preserve
            };
            if emit_path(
                path,
                EmptyPath::Discard,
                EmptyVerb::Discard,
                empty_area,
                &mut content,
                0.25, // the default tolerance of SkPDFUtils::EmitPath
            ) {
                paint_path(style, path.fill_type(), &mut content);
            }
        } else if let ImageAndOffset {
            image: Some(image),
            offset,
        } = to_image(g_id, &small_glyphs)
        {
            let mut image = image;
            if image.color_type() != ColorType::Gray8 {
                append_scalar(path_glyph.advance_x(), &mut content);
                content.write_text(" 0 d0\n");
                append_scalar(image.width() as scalar * bitmap_scale, &mut content);
                content.write_text(" 0 0 ");
                append_scalar(-(image.height() as scalar) * bitmap_scale, &mut content);
                content.write_text(" ");
                append_scalar(offset.0 as scalar * bitmap_scale, &mut content);
                content.write_text(" ");
                append_scalar(
                    (image.height() as scalar + offset.1 as scalar) * bitmap_scale,
                    &mut content,
                );
                content.write_text(" cm\n");
                content.write_text("/X");
                content.write(character_name.as_bytes());
                content.write_text(" Do\n");
                let image_ref = serialize_image_xobject(&image, doc, 101);
                xobjects.insert_ref_escaped_key(format!("Xg{g_id:X}"), image_ref);
            } else {
                // TODO: For A1, put ImageMask on the PDF image and draw the image?
                // The A8 mask has been converted to a Gray image

                // This is a `d1` glyph (shaded with the current fill)
                let small_glyph = small_glyphs.glyph(PackedGlyphId::from_glyph_id(g_id));
                let small_bbox = small_glyph.rect();
                let small_ibox: IRect = Matrix::scale((bitmap_scale, bitmap_scale))
                    .map_rect(small_bbox)
                    .0
                    .round_out();
                bbox = IRect::join(&bbox, &small_ibox);
                set_glyph_width_and_bounding_box(path_glyph.advance_x(), &small_ibox, &mut content);

                append_scalar(bitmap_scale, &mut content);
                content.write_text(" 0 0 ");
                append_scalar(bitmap_scale, &mut content);
                content.write_text(" ");
                append_scalar(offset.0 as scalar * bitmap_scale, &mut content);
                content.write_text(" ");
                append_scalar(offset.1 as scalar * bitmap_scale, &mut content);
                content.write_text(" cm\n");

                // Convert Grey image to deferred jpeg image to emit as jpeg
                if pdf_strike.has_mask_filter() {
                    let metadata = doc.metadata();
                    if let (Some(encode_jpeg), Some(decode_jpeg)) =
                        (metadata.jpeg_encoder, metadata.jpeg_decoder)
                    {
                        let mut buffer = DynamicMemoryWStream::new();
                        let encoded = image
                            .peek_pixels()
                            .is_some_and(|pm| encode_jpeg(&mut buffer, &pm, PDF_MASK_QUALITY));
                        if encoded {
                            let codec = decode_jpeg(buffer.detach_as_data());
                            debug_assert!(codec.is_some());
                            let jpeg_image = skia_rust_codec::codecs::deferred_image(codec, None);
                            debug_assert!(jpeg_image.is_some());
                            if let Some(jpeg_image) = jpeg_image {
                                image = jpeg_image;
                            }
                        }
                    }
                }

                // Draw image into a Form XObject
                let image_size = image.dimensions();
                let glyph_device = PdfDevice::new(image_size, doc, &Matrix::new_identity());
                let glyph_content = glyph_device.content_handle();
                let canvas = Canvas::from_device(Box::new(glyph_device));
                canvas.draw_image(&image, (0.0, 0.0), None);
                let glyph_stream = glyph_content.borrow_mut().content();
                let resource_dict = glyph_content.borrow().make_resource_dict();
                let s_mask = make_form_x_object(
                    doc,
                    &glyph_stream,
                    PdfParentTreeKey::default(),
                    make_int_array(&[0, 0, image.width(), image.height()]),
                    resource_dict,
                    &Matrix::new_identity(),
                    Some("DeviceGray"),
                );

                // Use Form XObject as SMask (luminosity) on the graphics state
                let smask_graphic_state =
                    get_smask_graphic_state(s_mask, false, SMaskMode::Luminosity, doc);
                apply_graphic_state(smask_graphic_state.value, &mut content);

                // Draw a rectangle the size of the glyph (masked by SMask)
                append_rectangle(&Rect::from_irect(&image.bounds()), &mut content);
                paint_path(Style::Fill, PathFillType::Winding, &mut content);

                // Add glyph resources to font resource dict
                xobjects.insert_ref_escaped_key(format!("Xg{g_id:X}"), s_mask);
                // TODO: name must match ApplyGraphicState
                graphic_states.insert_ref_escaped_key(
                    format!("G{}", smask_graphic_state.value),
                    smask_graphic_state,
                );
            }
        } else {
            set_glyph_width_and_bounding_box(path_glyph.advance_x(), &glyph_bbox, &mut content);
        }
        let char_proc = doc.stream_out(None, &content.detach_as_vector(), true);
        char_procs.insert_ref_escaped_key(&character_name, char_proc);
    }

    if xobjects.size() > 0 || graphic_states.size() > 0 {
        let mut resources = PdfDict::new(None);
        if xobjects.size() > 0 {
            resources.insert_object("XObject", Box::new(xobjects));
        }
        if graphic_states.size() > 0 {
            resources.insert_object("ExtGState", Box::new(graphic_states));
        }
        font.insert_object("Resources", Box::new(resources));
    }

    encoding.insert_object("Differences", Box::new(enc_diffs));
    font.insert_int("FirstChar", 0);
    font.insert_int("LastChar", i32::from(last_glyph_id - first_glyph_id) + 1);
    // FontBBox: "A rectangle expressed in the glyph coordinate system, specifying the font
    // bounding box. This is the smallest rectangle enclosing the shape that would result if all
    // of the glyphs of the font were placed with their origins coincident and then filled."
    font.insert_object(
        "FontBBox",
        Box::new(make_int_array(&[
            bbox.left(),
            bbox.bottom(),
            bbox.right(),
            bbox.top(),
        ])),
    );

    font.insert_name("CIDToGIDMap", "Identity");

    let path_typeface = pdf_strike.path().strike_spec.typeface().clone();
    let glyph_to_unicode = get_unicode_map(&path_typeface, doc);
    debug_assert_eq!(
        glyph_to_unicode.len(),
        usize::try_from(path_typeface.count_glyphs()).unwrap_or(0)
    );
    let unicode_map_ex = get_unicode_map_ex(&path_typeface, doc);
    let to_unicode_cmap = make_to_unicode_cmap(
        &glyph_to_unicode,
        &unicode_map_ex.borrow(),
        Some(&subset),
        false,
        first_glyph_id,
        last_glyph_id,
    );
    font.insert_ref("ToUnicode", doc.stream_out(None, &to_unicode_cmap, true));
    font.insert_ref(
        "FontDescriptor",
        type3_descriptor(doc, &path_typeface, x_height),
    );
    font.insert_object("Widths", Box::new(width_array));
    font.insert_object("Encoding", Box::new(encoding));
    font.insert_object("CharProcs", Box::new(char_procs));

    doc.emit(&font, pdf_font.indirect_reference());
}
