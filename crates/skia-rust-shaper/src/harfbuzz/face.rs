// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia (chrome/m156): modules/skshaper/src/SkShaper_harfbuzz.cpp (the font functions,
// the face and font creation, `HBLockedFaceCache`).

//! Skia's HarfBuzz font: the `skhb_*` font functions over a [`Font`], the face of a typeface, and
//! the cache of faces.
//!
//! HarfBuzz gets the metrics of a glyph from Skia through `hb_font_funcs`; here they are
//! [`FontFuncs`] of HarfRust. A typeface whose font data is an sfnt in memory is shaped with that
//! data. Any other typeface (the portable `TestTypeface` has no tables) is shaped with the
//! synthesized sfnt that holds the tables the typeface reports (`hb_face_create_for_tables`) and a
//! `head` table with its units per em (docs/design/modules.md §2.1, risk R1).

use std::sync::{Arc, Mutex, OnceLock};

use harfrust::funcs::{AdvanceWidthBatch, BuiltinFontFuncs, FontFuncs};
use harfrust::{
    FontRef, GlyphExtents, GlyphId as HbGlyphId, ShaperData, ShaperInstance, Tag, Variation,
};
use read_fonts::TableProvider as _;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::lru_cache::LruCache;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect, RoundOut as _};
use skia_rust_core::scalar::{scalar, scalar_round_to_int};
use skia_rust_core::typeface::{Typeface, TypefaceId};

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L72-L76 (chrome/m156), `skhb_position`.
/// Skia's value as a HarfBuzz `hb_position_t`: 16.16 fixed point.
#[must_use]
pub fn skhb_position(value: scalar) -> i32 {
    // Treat HarfBuzz hb_position_t as 16.16 fixed-point.
    const HB_POSITION_1: i32 = 1 << 16;
    #[allow(clippy::cast_precision_loss)] // mirrors `value * kHbPosition1` (int to float)
    scalar_round_to_int(value * HB_POSITION_1 as f32)
}

/// The font functions Skia installs on the sub-font of every shape call
/// (`skhb_get_font_funcs`), over a copy of the [`Font`] (`new SkFont(font)`).
#[derive(Debug)]
pub struct SkiaFontFuncs {
    /// The font the functions read.
    pub font: Font,
}

impl SkiaFontFuncs {
    /// `skhb_nominal_glyph`: the glyph of a code point, `None` for glyph 0.
    #[must_use]
    pub fn nominal_glyph_of(&self, c: u32) -> Option<GlyphId> {
        #[allow(clippy::cast_possible_wrap)] // SkUnichar is int32_t
        let glyph = self.font.unichar_to_glyph(c as i32);
        (glyph != 0).then_some(glyph)
    }

    /// `skhb_glyph_h_advance`: the 16.16 advance of a glyph.
    #[must_use]
    pub fn advance_of_glyph(&self, glyph: GlyphId) -> i32 {
        let mut width = [0.0];
        self.font.get_widths(&[glyph], &mut width);
        self.advance_of(width[0])
    }

    /// `skhb_glyph_extents`: the 16.16 extents of a glyph (y up).
    #[must_use]
    pub fn extents_of_glyph(&self, glyph: GlyphId) -> GlyphExtents {
        let mut bounds = [Rect::default()];
        self.font
            .get_widths_bounds(&[glyph], &mut [], &mut bounds, None);
        let mut sk_bounds = bounds[0];
        if !self.font.is_subpixel() {
            // sk_bounds.set(sk_bounds.roundOut())
            let rounded: IRect = sk_bounds.round_out();
            #[allow(clippy::cast_precision_loss)] // mirrors `SkRect::set(const SkIRect&)`
            {
                sk_bounds = Rect::new(
                    rounded.left as f32,
                    rounded.top as f32,
                    rounded.right as f32,
                    rounded.bottom as f32,
                );
            }
        }

        // Skia is y-down but HarfBuzz is y-up.
        GlyphExtents {
            x_bearing: skhb_position(sk_bounds.left),
            y_bearing: skhb_position(-sk_bounds.top),
            width: skhb_position(sk_bounds.width()),
            height: skhb_position(-sk_bounds.height()),
        }
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L129-L142 (chrome/m156),
    // `skhb_glyph_h_advance` after the width is read.
    fn advance_of(&self, width: scalar) -> i32 {
        let mut advance = width;
        if !self.font.is_subpixel() {
            #[allow(clippy::cast_precision_loss)] // mirrors `SkScalarRoundToInt(advance)` to float
            {
                advance = scalar_round_to_int(advance) as f32;
            }
        }
        skhb_position(advance)
    }
}

impl FontFuncs for SkiaFontFuncs {
    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L89-L96 (chrome/m156),
    // `skhb_nominal_glyph`.
    fn nominal_glyph(&mut self, _: &BuiltinFontFuncs<'_>, c: u32) -> Option<HbGlyphId> {
        self.nominal_glyph_of(c)
            .map(|glyph| HbGlyphId::new(u32::from(glyph)))
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L119-L127 (chrome/m156),
    // `skhb_glyph_h_advance`.
    fn advance_width(&mut self, _: &BuiltinFontFuncs<'_>, glyph: HbGlyphId) -> i32 {
        self.advance_of_glyph(glyph_id_of(glyph))
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L144-L180 (chrome/m156),
    // `skhb_glyph_h_advances`: one `getWidths` call for the batch.
    fn populate_advance_widths(&mut self, _: &BuiltinFontFuncs<'_>, batch: AdvanceWidthBatch<'_>) {
        let mut entries: Vec<(HbGlyphId, &mut i32)> = batch.into_iter().collect();
        let glyphs: Vec<GlyphId> = entries.iter().map(|(g, _)| glyph_id_of(*g)).collect();
        let mut advances = vec![0.0; glyphs.len()];
        self.font.get_widths(&glyphs, &mut advances);
        for ((_, out), advance) in entries.iter_mut().zip(&advances) {
            **out = self.advance_of(*advance);
        }
    }

    // Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L182-L208 (chrome/m156),
    // `skhb_glyph_extents`.
    fn extents(&mut self, _: &BuiltinFontFuncs<'_>, glyph: HbGlyphId) -> Option<GlyphExtents> {
        Some(self.extents_of_glyph(glyph_id_of(glyph)))
    }
}

// `SkTo<SkGlyphID>(hbGlyph)`.
fn glyph_id_of(glyph: HbGlyphId) -> GlyphId {
    GlyphId::try_from(glyph.to_u32()).unwrap_or(0)
}

/// The bounds of glyphs as `SkFont::getBounds(glyphs, bounds, &paint)` with a default paint.
pub(crate) fn glyph_bounds(font: &Font, glyphs: &[GlyphId]) -> Vec<Rect> {
    let mut bounds = vec![Rect::default(); glyphs.len()];
    let paint = Paint::default();
    font.get_widths_bounds(glyphs, &mut [], &mut bounds, Some(&paint));
    bounds
}

/// An `HBFont` of the typeface cache: the face (`hb_face_t`) with the variations of the typeface
/// (`hb_font_set_variations`). The sub-font with Skia's functions and scale is made per call.
pub(crate) struct HbFont {
    data: Vec<u8>,
    index: u32,
    shaper_data: ShaperData,
    instance: ShaperInstance,
}

impl std::fmt::Debug for HbFont {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HbFont")
            .field("index", &self.index)
            .field("bytes", &self.data.len())
            .finish_non_exhaustive()
    }
}

impl HbFont {
    /// The font file and the face index to build a [`FontRef`] with.
    pub(crate) fn font_ref(&self) -> Option<FontRef<'_>> {
        FontRef::from_index(&self.data, self.index).ok()
    }

    pub(crate) fn shaper_data(&self) -> &ShaperData {
        &self.shaper_data
    }

    pub(crate) fn instance(&self) -> &ShaperInstance {
        &self.instance
    }
}

const HEAD_UPEM_OFFSET: usize = 18;

// The offset of the `head` table, if the font has one.
fn head_offset(data: &[u8], index: u32) -> Option<usize> {
    let font = FontRef::from_index(data, index).ok()?;
    font.table_directory
        .table_records()
        .iter()
        .find(|r| r.tag() == Tag::new(b"head"))
        .map(|r| r.offset() as usize)
}

// `hb_face_set_upem (face, typeface.getUnitsPerEm ())`: HarfRust reads the upem from `head`, so
// the value is written there when it differs.
fn set_upem(data: &mut [u8], index: u32, upem: i32) {
    let Ok(upem) = u16::try_from(upem) else {
        return;
    };
    let Some(head) = head_offset(data, index) else {
        return;
    };
    let at = head + HEAD_UPEM_OFFSET;
    if let Some(bytes) = data.get_mut(at..at + 2) {
        bytes.copy_from_slice(&upem.to_be_bytes());
    }
}

// The sfnt of the tables of a typeface, as `hb_face_create_for_tables` with `skhb_get_table`
// sees them: every table the typeface reports, and a `head` table that holds the units per em
// when the typeface has none.
fn sfnt_from_tables(typeface: &Typeface, upem: Option<i32>) -> Vec<u8> {
    let mut tags = vec![0; typeface.count_tables()];
    let count = typeface.read_table_tags(&mut tags);
    tags.truncate(count);
    let mut tables: Vec<(u32, Vec<u8>)> = tags
        .into_iter()
        .filter_map(|tag| {
            typeface
                .copy_table_data(tag)
                .map(|d| (tag, d.as_bytes().to_vec()))
        })
        .collect();
    let head_tag = u32::from_be_bytes(*b"head");
    if !tables.iter().any(|(tag, _)| *tag == head_tag) {
        // head: version 1.0, fontRevision, checkSumAdjustment, magic, flags, unitsPerEm, created,
        // modified, bbox, macStyle, lowestRecPPEM, fontDirectionHint = 2, indexToLocFormat,
        // glyphDataFormat.
        let upem = upem.and_then(|u| u16::try_from(u).ok()).unwrap_or(1000);
        let mut head = Vec::with_capacity(54);
        head.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        head.extend_from_slice(&[0; 8]);
        head.extend_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
        head.extend_from_slice(&0u16.to_be_bytes());
        head.extend_from_slice(&upem.to_be_bytes());
        head.extend_from_slice(&[0; 16]);
        head.extend_from_slice(&[0; 8]);
        head.extend_from_slice(&0u16.to_be_bytes());
        head.extend_from_slice(&0u16.to_be_bytes());
        head.extend_from_slice(&2i16.to_be_bytes());
        head.extend_from_slice(&0i16.to_be_bytes());
        head.extend_from_slice(&0i16.to_be_bytes());
        tables.push((head_tag, head));
    }
    tables.sort_by_key(|(tag, _)| *tag);

    let num_tables = u16::try_from(tables.len()).unwrap_or(u16::MAX);
    let mut entry_selector = 0u16;
    while (1u32 << (entry_selector + 1)) <= u32::from(num_tables) {
        entry_selector += 1;
    }
    let search_range = (1u16 << entry_selector) * 16;
    let range_shift = num_tables * 16 - search_range;

    let mut out = Vec::new();
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&num_tables.to_be_bytes());
    out.extend_from_slice(&search_range.to_be_bytes());
    out.extend_from_slice(&entry_selector.to_be_bytes());
    out.extend_from_slice(&range_shift.to_be_bytes());
    let mut offset = 12 + 16 * tables.len();
    let mut records = Vec::new();
    let mut bodies = Vec::new();
    for (tag, bytes) in &tables {
        records.extend_from_slice(&tag.to_be_bytes());
        records.extend_from_slice(&0u32.to_be_bytes());
        records.extend_from_slice(&u32::try_from(offset).unwrap_or(u32::MAX).to_be_bytes());
        records.extend_from_slice(&u32::try_from(bytes.len()).unwrap_or(u32::MAX).to_be_bytes());
        bodies.extend_from_slice(bytes);
        let padding = (4 - bytes.len() % 4) % 4;
        bodies.extend(std::iter::repeat_n(0, padding));
        offset += bytes.len() + padding;
    }
    out.extend_from_slice(&records);
    out.extend_from_slice(&bodies);
    out
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L274-L311 (chrome/m156), `create_hb_face`,
// and #L313-L337, `create_typeface_hb_font`.
fn create_typeface_hb_font(typeface: &Typeface) -> Option<HbFont> {
    let upem = typeface.units_per_em();

    let mut face: Option<(Vec<u8>, u32)> = None;
    if let Some((stream, ttc_index)) = typeface.open_existing_stream()
        && let Some(base) = stream.get_memory_base()
    {
        {
            // hb_face_create always succeeds. Check that the format is minimally recognized first.
            // hb_face_create_for_tables may still create a working hb_face.
            if let Some(font) = u32::try_from(ttc_index)
                .ok()
                .and_then(|i| FontRef::from_index(base, i).ok().map(|f| (i, f)))
            {
                let (index, font) = font;
                // Check the number of glyphs as a basic sanitization step.
                if font.maxp().is_ok_and(|m| m.num_glyphs() != 0) {
                    face = Some((base.to_vec(), index));
                }
            }
        }
    }
    let (mut data, index) = face.unwrap_or_else(|| (sfnt_from_tables(typeface, upem), 0));
    if let Some(upem) = upem {
        set_upem(&mut data, index, upem);
    }

    let font = FontRef::from_index(&data, index).ok()?;
    let shaper_data = ShaperData::new(&font);
    let mut instance = ShaperInstance::default();
    if let Some(position) = typeface.variation_design_position()
        && !position.is_empty()
    {
        instance = ShaperInstance::from_variations(
            &font,
            position.iter().map(|c| Variation {
                tag: Tag::from_u32(c.axis),
                value: c.value,
            }),
        );
    }
    Some(HbFont {
        data,
        index,
        shaper_data,
        instance,
    })
}

type FaceCache = LruCache<TypefaceId, Arc<HbFont>>;

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1196-L1233 (chrome/m156),
// `HBLockedFaceCache` and `get_hbFace_cache`: one process-wide cache of 100 faces under a mutex.
fn face_cache() -> &'static Mutex<FaceCache> {
    static CACHE: OnceLock<Mutex<FaceCache>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(LruCache::new(100)))
}

/// The cached [`HbFont`] of a typeface, made on first use.
// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1290-L1305 (chrome/m156), in `shape`.
pub(crate) fn hb_font_for(typeface: &Typeface) -> Option<Arc<HbFont>> {
    // TODO (as in Skia): better cache HBFace (data) / hbfont (typeface).
    // An HBFace is expensive (it sanitizes the bits).
    // An HBFont is fairly inexpensive.
    // An HBFace is actually tied to the data, not the typeface.
    // The size of 100 here is completely arbitrary and used to match libtxt.
    let mut cache = face_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let id = typeface.unique_id();
    if let Some(found) = cache.find(&id) {
        return Some(found.clone());
    }
    let font = Arc::new(create_typeface_hb_font(typeface)?);
    Some(cache.insert(id, font).clone())
}

// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L1664-L1667 (chrome/m156), `PurgeCaches`.
/// Empties the cache of faces.
pub fn purge_caches() {
    face_cache()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .reset();
}
