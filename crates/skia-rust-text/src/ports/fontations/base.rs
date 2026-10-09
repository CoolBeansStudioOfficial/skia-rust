// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/fontations/src/base.rs (chrome/m156), with the `cxx` FFI replaced by a plain
// Rust API: `Box<T>` handles become values, `rust::Slice` becomes `&[T]`, `bool` plus out-params
// become `Option`, and `Pin<&mut AxisWrapper>` becomes `&mut [Axis]`.

//! The font-reading half of the Fontations bridge: parsing a font with read-fonts, and the
//! queries Skia makes of it (glyph mapping, tables, variation axes, style, metrics).

use font_types::GlyphId;
use read_fonts::{FileRef, FontRef, ReadError, TableProvider};
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_parameters::variation::Axis;
use skrifa::{
    MetadataProvider, OutlineGlyphCollection, Tag,
    attribute::Style,
    charmap::MappingIndex,
    instance::{Location, Size},
    metrics::{GlyphMetrics, Metrics as SkrifaMetrics},
    outline::OutlineGlyphFormat,
    setting::VariationSetting,
};

/// The font-reading state of one font, `BridgeFontRef`.
///
/// Unlike the C++ bridge, this does not own the font bytes: read-fonts borrows them, and the
/// typeface that owns the data builds a `BridgeFontRef` over them on demand. Parsing a table
/// directory is cheap, so the typeface does not keep one around.
// Port of: src/ports/fontations/src/base.rs#L20-L29 (chrome/m156)
pub struct BridgeFontRef<'a> {
    font: Option<FontRef<'a>>,
    has_any_color: bool,
}

// read-fonts' `FontRef` is not `Debug`, so the debug output shows only the bridge's flags.
impl std::fmt::Debug for BridgeFontRef<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BridgeFontRef")
            .field("valid", &self.font.is_some())
            .field("has_any_color", &self.has_any_color)
            .finish()
    }
}

impl<'a> BridgeFontRef<'a> {
    /// Runs `f` on the parsed font, or returns `None` when the data is not a valid font.
    // Port of: src/ports/fontations/src/base.rs#L26-L28 (chrome/m156)
    pub fn with_font<T>(&self, f: impl FnOnce(&FontRef<'a>) -> Option<T>) -> Option<T> {
        f(self.font.as_ref()?)
    }
}

/// The outlines of a font (`BridgeOutlineCollection`). `None` when the font failed to parse.
// Port of: src/ports/fontations/src/base.rs#L31-L32 (chrome/m156)
#[derive(Default, Debug)]
pub struct BridgeOutlineCollection<'a>(pub Option<OutlineGlyphCollection<'a>>);

/// The variation coordinates of an instance, normalized and in design space (`BridgeNormalizedCoords`).
// Port of: src/ports/fontations/src/base.rs#L34-L38 (chrome/m156)
#[derive(Default, Debug)]
pub struct BridgeNormalizedCoords {
    /// The normalized coordinates, used for outlines and metrics.
    pub normalized_coords: Location,
    /// The user coordinates, merged with the axis defaults and filtered to the font's axes.
    pub filtered_user_coords: Vec<VariationSetting>,
}

/// The character-to-glyph index of a font (`BridgeMappingIndex`).
// Port of: src/ports/fontations/src/base.rs#L40 (chrome/m156)
#[derive(Debug)]
pub struct BridgeMappingIndex(MappingIndex);

/// The style values Skia reads from a font, as `SkFontStyle` integers: weight, width (1-9) and
/// slant (`SkFontStyle::Slant` value). Mirrors `BridgeFontStyle` in `ffi.rs`.
// Port of: src/ports/fontations/src/ffi.rs (BridgeFontStyle, chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BridgeFontStyle {
    /// `SkFontStyle::Weight` (1 to 1000).
    pub weight: i32,
    /// `SkFontStyle::Slant` value: 0 upright, 1 italic, 2 oblique.
    pub slant: i32,
    /// `SkFontStyle::Width` (1 to 9).
    pub width: i32,
}

/// The outline format of a font (`OutlineFormat` in `ffi.rs`).
// Port of: src/ports/fontations/src/ffi.rs (OutlineFormat, chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutlineFormat {
    /// No outlines.
    NoOutlines,
    /// TrueType outlines.
    Glyf,
    /// CFF outlines.
    Cff,
    /// CFF2 outlines.
    Cff2,
}

/// The font metrics Skia reads from skrifa (`Metrics` in `ffi.rs`).
// Port of: src/ports/fontations/src/ffi.rs (Metrics, chrome/m156)
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Metrics {
    pub top: f32,
    pub ascent: f32,
    pub descent: f32,
    pub bottom: f32,
    pub leading: f32,
    pub avg_char_width: f32,
    pub max_char_width: f32,
    pub x_min: f32,
    pub x_max: f32,
    pub x_height: f32,
    pub cap_height: f32,
    pub underline_position: f32,
    pub underline_thickness: f32,
    pub strikeout_position: f32,
    pub strikeout_thickness: f32,
}

/// Port of `make_mapping_index`. The caller has checked that the font is valid (C++ unwrapped
/// here); `None` if it is not.
// Port of: src/ports/fontations/src/base.rs#L42-L46 (chrome/m156)
#[must_use]
pub fn make_mapping_index(font_ref: &BridgeFontRef<'_>) -> Option<BridgeMappingIndex> {
    font_ref.with_font(|f| Some(BridgeMappingIndex(MappingIndex::new(f))))
}

// Port of: src/ports/fontations/src/base.rs#L48-L65 (chrome/m156)
pub fn lookup_glyph_or_zero(
    font_ref: &BridgeFontRef<'_>,
    map: &BridgeMappingIndex,
    codepoints: &[u32],
    glyphs: &mut [u16],
) {
    glyphs.fill(0);
    font_ref.with_font(|f| {
        let mappings = map.0.charmap(f);
        for (codepoint, glyph) in codepoints.iter().zip(glyphs.iter_mut()) {
            // Remove u16 conversion when implementing large glyph id support in Skia.
            *glyph = u16::try_from(mappings.map(*codepoint).unwrap_or_default().to_u32())
                .unwrap_or_default();
        }
        Some(())
    });
}

// Port of: src/ports/fontations/src/base.rs#L67-L71 (chrome/m156)
#[must_use]
pub fn num_glyphs(font_ref: &BridgeFontRef<'_>) -> u16 {
    font_ref
        .with_font(|f| Some(f.maxp().ok()?.num_glyphs()))
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L73-L84 (chrome/m156)
pub fn fill_glyph_to_unicode_map(font_ref: &BridgeFontRef<'_>, map: &mut [u32]) {
    map.fill(0);
    font_ref.with_font(|f| {
        let mappings = f.charmap().mappings();
        for (codepoint, glyphid) in mappings {
            if let Some(c) = map.get_mut(glyphid.to_u32() as usize).filter(|c| **c == 0) {
                *c = codepoint;
            }
        }
        Some(())
    });
}

// Port of: src/ports/fontations/src/base.rs#L86-L98 (chrome/m156)
#[must_use]
pub fn unhinted_advance_width_or_zero(
    font_ref: &BridgeFontRef<'_>,
    size: f32,
    coords: &BridgeNormalizedCoords,
    glyph_id: u16,
) -> f32 {
    font_ref
        .with_font(|f| {
            GlyphMetrics::new(f, Size::new(size), coords.normalized_coords.coords())
                .advance_width(GlyphId::from(glyph_id))
        })
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L100-L108 (chrome/m156)
#[must_use]
pub fn outline_format(outlines: &BridgeOutlineCollection<'_>) -> OutlineFormat {
    let outlines = outlines.0.as_ref();
    match outlines.and_then(OutlineGlyphCollection::format) {
        Some(OutlineGlyphFormat::Glyf) => OutlineFormat::Glyf,
        Some(OutlineGlyphFormat::Cff) => OutlineFormat::Cff,
        Some(OutlineGlyphFormat::Cff2) => OutlineFormat::Cff2,
        _ => OutlineFormat::NoOutlines,
    }
}

// Port of: src/ports/fontations/src/base.rs#L110-L114 (chrome/m156)
#[must_use]
pub fn units_per_em_or_zero(font_ref: &BridgeFontRef<'_>) -> u16 {
    font_ref
        .with_font(|f| Some(f.head().ok()?.units_per_em()))
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L116-L134 (chrome/m156)
#[must_use]
pub fn convert_metrics(skrifa_metrics: &SkrifaMetrics) -> Metrics {
    Metrics {
        top: skrifa_metrics.bounds.map_or(0.0, |b| b.y_max),
        bottom: skrifa_metrics.bounds.map_or(0.0, |b| b.y_min),
        x_min: skrifa_metrics.bounds.map_or(0.0, |b| b.x_min),
        x_max: skrifa_metrics.bounds.map_or(0.0, |b| b.x_max),
        ascent: skrifa_metrics.ascent,
        descent: skrifa_metrics.descent,
        leading: skrifa_metrics.leading,
        avg_char_width: skrifa_metrics.average_width.unwrap_or(0.0),
        max_char_width: skrifa_metrics.max_width.unwrap_or(0.0),
        x_height: -skrifa_metrics.x_height.unwrap_or(0.0),
        cap_height: -skrifa_metrics.cap_height.unwrap_or(0.0),
        underline_position: skrifa_metrics.underline.map_or(f32::NAN, |u| u.offset),
        underline_thickness: skrifa_metrics.underline.map_or(f32::NAN, |u| u.thickness),
        strikeout_position: skrifa_metrics.strikeout.map_or(f32::NAN, |s| s.offset),
        strikeout_thickness: skrifa_metrics.strikeout.map_or(f32::NAN, |s| s.thickness),
    }
}

// Port of: src/ports/fontations/src/base.rs#L136-L148 (chrome/m156)
#[must_use]
pub fn get_skia_metrics(
    font_ref: &BridgeFontRef<'_>,
    size: f32,
    coords: &BridgeNormalizedCoords,
) -> Metrics {
    font_ref
        .with_font(|f| {
            let fontations_metrics =
                SkrifaMetrics::new(f, Size::new(size), coords.normalized_coords.coords());
            Some(convert_metrics(&fontations_metrics))
        })
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L150-L158 (chrome/m156)
#[must_use]
pub fn get_unscaled_metrics(
    font_ref: &BridgeFontRef<'_>,
    coords: &BridgeNormalizedCoords,
) -> Metrics {
    font_ref
        .with_font(|f| {
            let fontations_metrics =
                SkrifaMetrics::new(f, Size::unscaled(), coords.normalized_coords.coords());
            Some(convert_metrics(&fontations_metrics))
        })
        .unwrap_or_default()
}

/// Implements the behavior expected for `SkTypeface::getTableData`, compare
/// documentation for this method and the `FreeType` implementation in Skia.
/// * If the target data array is empty, do not copy any data into it, but
///   return the size of the table.
/// * If the target data buffer is shorted than from offset to the end of the
///   table, truncate the data.
/// * If offset is longer than the table's length, return 0.
// Port of: src/ports/fontations/src/base.rs#L160-L189 (chrome/m156)
#[must_use]
pub fn table_data(font_ref: &BridgeFontRef<'_>, tag: u32, offset: usize, data: &mut [u8]) -> usize {
    let table_data = font_ref
        .with_font(|f| f.table_data(Tag::from_be_bytes(tag.to_be_bytes())))
        .unwrap_or_default();
    let table_data = table_data.as_ref();
    // Remaining table data size measured from offset to end, or 0 if offset is
    // too large.
    let mut to_copy_length = table_data.len().saturating_sub(offset);
    if data.is_empty() {
        to_copy_length
    } else {
        to_copy_length = to_copy_length.min(data.len());
        let table_offset_data = table_data
            .get(offset..offset + to_copy_length)
            .unwrap_or_default();
        data.get_mut(..table_offset_data.len())
            .map_or(0, |data_slice| {
                data_slice.copy_from_slice(table_offset_data);
                data_slice.len()
            })
    }
}

// Port of: src/ports/fontations/src/base.rs#L191-L205 (chrome/m156)
#[must_use]
pub fn table_tags(font_ref: &BridgeFontRef<'_>, tags: &mut [u32]) -> u16 {
    font_ref
        .with_font(|f| {
            let table_directory = &f.table_directory;
            let table_tags_iter = table_directory
                .table_records()
                .iter()
                .map(|table| u32::from_be_bytes(table.tag.get().into_bytes()));
            tags.iter_mut()
                .zip(table_tags_iter)
                .for_each(|(out_tag, table_tag)| *out_tag = table_tag);
            Some(table_directory.num_tables())
        })
        .unwrap_or_default()
}

/// Port of `variation_position`: the user coordinates, or their count when `coordinates` is
/// empty. Returns -1 when the buffer is too small.
// Port of: src/ports/fontations/src/base.rs#L207-L228 (chrome/m156)
#[must_use]
pub fn variation_position(
    coords: &BridgeNormalizedCoords,
    coordinates: &mut [Coordinate],
) -> isize {
    if !coordinates.is_empty() {
        if coords.filtered_user_coords.len() > coordinates.len() {
            return -1;
        }
        let skia_design_coordinates =
            coords
                .filtered_user_coords
                .iter()
                .map(|setting| Coordinate {
                    axis: u32::from_be_bytes(setting.selector.into_bytes()),
                    value: setting.value,
                });
        for (i, coord) in skia_design_coordinates.enumerate() {
            coordinates[i] = coord;
        }
    }
    // A length always fits in an isize, so the -1 fallback is not reached.
    isize::try_from(coords.filtered_user_coords.len()).unwrap_or(-1)
}

/// Fills `coords` with the axis coordinates of a shifted named instance index (see
/// `ffi.rs`). Returns the number of coordinates, or 0 when the index names no instance or the
/// buffer is too small. An empty buffer is a count query.
// Port of: src/ports/fontations/src/base.rs#L230-L258 (chrome/m156)
#[must_use]
pub fn coordinates_for_shifted_named_instance_index(
    font_ref: &BridgeFontRef<'_>,
    shifted_index: u32,
    coords: &mut [Coordinate],
) -> isize {
    font_ref
        .with_font(|f| {
            let fvar = f.fvar().ok()?;
            let instances = fvar.instances().ok()?;
            // C++ wraps around here for an index without a named instance (`0 - 1`), and the
            // lookup then fails; `wrapping_sub` keeps that behavior without a debug panic.
            let index = usize::try_from((shifted_index >> 16).wrapping_sub(1)).ok()?;
            let instance_coords = instances.get(index).ok()?.coordinates;

            if !coords.is_empty() {
                if coords.len() < instance_coords.len() {
                    return None;
                }
                let axis_coords = f.axes().iter().zip(instance_coords.iter()).enumerate();
                for (i, axis_coord) in axis_coords {
                    coords[i] = Coordinate {
                        axis: u32::from_be_bytes(axis_coord.0.tag().to_be_bytes()),
                        value: axis_coord.1.get().to_f32(),
                    };
                }
            }

            isize::try_from(instance_coords.len()).ok()
        })
        .unwrap_or(0)
}

// Port of: src/ports/fontations/src/base.rs#L260-L264 (chrome/m156)
#[must_use]
pub fn num_axes(font_ref: &BridgeFontRef<'_>) -> usize {
    font_ref
        .with_font(|f| Some(f.axes().len()))
        .unwrap_or_default()
}

/// Port of `populate_axes`: writes the axes into `axes` when it is not empty (C++ fills an
/// `AxisWrapper` only when its size is positive). Returns the number of axes, or -1 when the
/// buffer holds fewer axes than the font has.
// Port of: src/ports/fontations/src/base.rs#L266-L289 (chrome/m156)
#[must_use]
pub fn populate_axes(font_ref: &BridgeFontRef<'_>, axes_out: &mut [Axis]) -> isize {
    font_ref
        .with_font(|f| {
            let axes = f.axes();
            // Populate incoming allocated SkFontParameters::Variation::Axis[] only when a
            // buffer is passed.
            if !axes_out.is_empty() {
                for (i, axis) in axes.iter().enumerate() {
                    // `AxisWrapper::populate_axis` fails past the end of the buffer.
                    let out = axes_out.get_mut(i)?;
                    *out = Axis::new(
                        u32::from_be_bytes(axis.tag().into_bytes()),
                        axis.min_value(),
                        axis.default_value(),
                        axis.max_value(),
                        axis.is_hidden(),
                    );
                }
            }
            isize::try_from(axes.len()).ok()
        })
        .unwrap_or(-1)
}

// Port of: src/ports/fontations/src/base.rs#L291-L309 (chrome/m156)
fn make_font_ref_internal(font_data: &[u8], index: u32) -> Result<FontRef<'_>, ReadError> {
    match FileRef::new(font_data) {
        Ok(file_ref) => match file_ref {
            FileRef::Font(font_ref) => {
                // Indices with the higher bits set are meaningful here and do not result in an
                // error, as they may refer to a named instance and are taken into account by the
                // Fontations typeface implementation,
                // compare `coordinates_for_shifted_named_instance_index()`.
                if (index & 0xFFFF) != 0 {
                    Err(ReadError::InvalidCollectionIndex(index))
                } else {
                    Ok(font_ref)
                }
            }
            FileRef::Collection(collection) => collection.get(index),
        },
        Err(e) => Err(e),
    }
}

// Port of: src/ports/fontations/src/base.rs#L311-L329 (chrome/m156)
#[must_use]
pub fn make_font_ref(font_data: &[u8], index: u32) -> BridgeFontRef<'_> {
    let font = make_font_ref_internal(font_data, index).ok();
    let has_any_color = font.as_ref().is_some_and(|f| {
        f.cbdt().is_ok() ||
            f.sbix().is_ok() ||
            // ColorGlyphCollection::get_with_format() first thing checks for presence of colr(),
            // so we do the same:
            f.colr().is_ok() ||
            f.ebdt().is_ok()
    });

    BridgeFontRef {
        font,
        has_any_color,
    }
}

// Port of: src/ports/fontations/src/base.rs#L331-L333 (chrome/m156)
#[must_use]
pub fn font_ref_is_valid(bridge_font_ref: &BridgeFontRef<'_>) -> bool {
    bridge_font_ref.font.is_some()
}

// Port of: src/ports/fontations/src/base.rs#L335-L337 (chrome/m156)
#[must_use]
pub fn has_any_color_table(bridge_font_ref: &BridgeFontRef<'_>) -> bool {
    bridge_font_ref.has_any_color
}

// Port of: src/ports/fontations/src/base.rs#L339-L347 (chrome/m156)
#[must_use]
pub fn get_outline_collection<'a>(font_ref: &BridgeFontRef<'a>) -> BridgeOutlineCollection<'a> {
    font_ref
        .with_font(|f| Some(BridgeOutlineCollection(Some(f.outline_glyphs()))))
        .unwrap_or_default()
}

/// Port of `font_or_collection`: `Some(0)` for a single font, `Some(n)` for a collection of `n`
/// fonts, and `None` when the data is neither.
// Port of: src/ports/fontations/src/base.rs#L349-L361 (chrome/m156)
#[must_use]
pub fn font_or_collection(font_data: &[u8]) -> Option<u32> {
    match FileRef::new(font_data) {
        Ok(FileRef::Collection(collection)) => Some(collection.len()),
        Ok(FileRef::Font(_)) => Some(0),
        _ => None,
    }
}

// Port of: src/ports/fontations/src/base.rs#L363-L367 (chrome/m156)
#[must_use]
pub fn num_named_instances(font_ref: &BridgeFontRef<'_>) -> usize {
    font_ref
        .with_font(|f| Some(f.named_instances().len()))
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L369-L395 (chrome/m156)
#[must_use]
pub fn resolve_into_normalized_coords(
    font_ref: &BridgeFontRef<'_>,
    design_coords: &[Coordinate],
) -> BridgeNormalizedCoords {
    let variation_tuples = design_coords
        .iter()
        .map(|coord| (Tag::from_be_bytes(coord.axis.to_be_bytes()), coord.value));
    font_ref
        .with_font(|f| {
            let merged_defaults_with_user = f
                .axes()
                .iter()
                .map(|axis| (axis.tag(), axis.default_value()))
                .chain(design_coords.iter().map(|user_coord| {
                    (
                        Tag::from_be_bytes(user_coord.axis.to_be_bytes()),
                        user_coord.value,
                    )
                }));
            Some(BridgeNormalizedCoords {
                filtered_user_coords: f.axes().filter(merged_defaults_with_user).collect(),
                normalized_coords: f.axes().location(variation_tuples),
            })
        })
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L397-L399 (chrome/m156)
#[must_use]
pub fn normalized_coords_equal(a: &BridgeNormalizedCoords, b: &BridgeNormalizedCoords) -> bool {
    a.normalized_coords.coords() == b.normalized_coords.coords()
}

/// Port of `get_font_style`: the style of the font at `coords`, or `None` when the font is not
/// valid.
// Port of: src/ports/fontations/src/base.rs#L400-L491 (chrome/m156)
#[must_use]
// The float-to-integer casts mirror the C++ conversions of the weight and the user coordinates.
#[allow(clippy::cast_possible_truncation)]
pub fn get_font_style(
    font_ref: &BridgeFontRef<'_>,
    coords: &BridgeNormalizedCoords,
) -> Option<BridgeFontStyle> {
    const SKIA_SLANT_UPRIGHT: i32 = 0; /* kUpright_Slant */
    const SKIA_SLANT_ITALIC: i32 = 1; /* kItalic_Slant */
    const SKIA_SLANT_OBLIQUE: i32 = 2; /* kOblique_Slant */
    const WGHT: Tag = Tag::new(b"wght");
    const WDTH: Tag = Tag::new(b"wdth");
    const SLNT: Tag = Tag::new(b"slnt");
    const ITAL: Tag = Tag::new(b"ital");

    font_ref.with_font(|f| {
        let attrs = f.attributes();
        let mut skia_weight = attrs.weight.value().round() as i32;
        let mut skia_slant = match attrs.style {
            Style::Normal => SKIA_SLANT_UPRIGHT,
            Style::Italic => SKIA_SLANT_ITALIC,
            Style::Oblique(_) => SKIA_SLANT_OBLIQUE,
        };
        //0.5, 0.625, 0.75, 0.875, 1.0, 1.125, 1.25, 1.5, 2.0 map to 1-9
        let mut skia_width = match attrs.stretch.ratio() {
            x if x <= 0.5625 => 1,
            x if x <= 0.6875 => 2,
            x if x <= 0.8125 => 3,
            x if x <= 0.9375 => 4,
            x if x <= 1.0625 => 5,
            x if x <= 1.1875 => 6,
            x if x <= 1.3750 => 7,
            x if x <= 1.7500 => 8,
            _ => 9,
        };

        let mut slnt_value = None;
        let mut ital_value = None;
        for user_coord in &coords.filtered_user_coords {
            match user_coord.selector {
                WGHT => skia_weight = user_coord.value.round() as i32,
                // 50, 62.5, 75, 87.5, 100, 112.5, 125, 150, 200 map to 1-9
                WDTH => {
                    skia_width = match user_coord.value {
                        x if x <= 56.25 => 1,
                        x if x <= 68.75 => 2,
                        x if x <= 81.25 => 3,
                        x if x <= 93.75 => 4,
                        x if x <= 106.25 => 5,
                        x if x <= 118.75 => 6,
                        x if x <= 137.50 => 7,
                        x if x <= 175.00 => 8,
                        _ => 9,
                    }
                }
                SLNT => slnt_value = Some(user_coord.value),
                ITAL => ital_value = Some(user_coord.value),
                _ => (),
            }
        }
        // Value > 0 => +, value == 0 => 0, no value => _
        // slnt\ital  _      0      +
        //       _   init  !ital   ital
        //       0  !oblq   uprt   ital
        //       +   oblq   oblq   ital
        if ital_value.is_some_and(|x| x != 0.0) {
            skia_slant = SKIA_SLANT_ITALIC;
        } else if slnt_value.is_some_and(|x| x != 0.0) {
            skia_slant = SKIA_SLANT_OBLIQUE;
        } else if (ital_value.is_some_and(|x| x == 0.0) && slnt_value.is_some_and(|x| x == 0.0))
            || (ital_value.is_some_and(|x| x == 0.0)
                && slnt_value.is_none()
                && skia_slant == SKIA_SLANT_ITALIC)
            || (ital_value.is_none()
                && slnt_value.is_some_and(|x| x == 0.0)
                && skia_slant == SKIA_SLANT_OBLIQUE)
        {
            // The three upright cases of the table above share one result.
            skia_slant = SKIA_SLANT_UPRIGHT;
        }

        Some(BridgeFontStyle {
            weight: skia_weight,
            slant: skia_slant,
            width: skia_width,
        })
    })
}

// Port of: src/ports/fontations/src/base.rs#L493-L504 (chrome/m156)
#[must_use]
pub fn is_embeddable(font_ref: &BridgeFontRef<'_>) -> bool {
    font_ref
        .with_font(|f| {
            let fs_type = f.os2().ok()?.fs_type();
            // https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fstype
            // Bit 2 and bit 9 must be cleared, "Restricted License embedding" and
            // "Bitmap embedding only" must both be unset.
            // Implemented to match SkTypeface_FreeType::onGetAdvancedMetrics.
            Some(fs_type & 0x202 == 0)
        })
        .unwrap_or(true)
}

// Port of: src/ports/fontations/src/base.rs#L506-L514 (chrome/m156)
#[must_use]
pub fn is_subsettable(font_ref: &BridgeFontRef<'_>) -> bool {
    font_ref
        .with_font(|f| {
            let fs_type = f.os2().ok()?.fs_type();
            // https://learn.microsoft.com/en-us/typography/opentype/spec/os2#fstype
            Some((fs_type & 0x100) == 0)
        })
        .unwrap_or(true)
}

// Port of: src/ports/fontations/src/base.rs#L516-L523 (chrome/m156)
#[must_use]
pub fn is_fixed_pitch(font_ref: &BridgeFontRef<'_>) -> bool {
    font_ref
        .with_font(|f| {
            // Compare DWriteFontTypeface::onGetAdvancedMetrics().
            Some(f.post().ok()?.is_fixed_pitch() != 0 || f.hhea().ok()?.number_of_h_metrics() == 1)
        })
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L525-L544 (chrome/m156)
#[must_use]
pub fn is_serif_style(font_ref: &BridgeFontRef<'_>) -> bool {
    const FAMILY_TYPE_TEXT_AND_DISPLAY: u8 = 2;
    const SERIF_STYLE_COVE: u8 = 2;
    const SERIF_STYLE_TRIANGLE: u8 = 10;
    font_ref
        .with_font(|f| {
            // Compare DWriteFontTypeface::onGetAdvancedMetrics().
            let panose = f.os2().ok()?.panose_10();
            let family_type = panose[0];

            match family_type {
                FAMILY_TYPE_TEXT_AND_DISPLAY => {
                    let serif_style = panose[1];
                    Some((SERIF_STYLE_COVE..=SERIF_STYLE_TRIANGLE).contains(&serif_style))
                }
                _ => None,
            }
        })
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L546-L555 (chrome/m156)
#[must_use]
pub fn is_script_style(font_ref: &BridgeFontRef<'_>) -> bool {
    const FAMILY_TYPE_SCRIPT: u8 = 3;
    font_ref
        .with_font(|f| {
            // Compare DWriteFontTypeface::onGetAdvancedMetrics().
            let family_type = f.os2().ok()?.panose_10()[0];
            Some(family_type == FAMILY_TYPE_SCRIPT)
        })
        .unwrap_or_default()
}

// Port of: src/ports/fontations/src/base.rs#L557-L561 (chrome/m156)
#[must_use]
pub fn italic_angle(font_ref: &BridgeFontRef<'_>) -> i32 {
    font_ref
        .with_font(|f| Some(f.post().ok()?.italic_angle().to_i32()))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    //! Tests of the bridge functions, as the Rust tests of `base.rs` in Skia. The fonts are
    //! read from Skia's `resources/fonts`; the tests skip when that directory is missing.

    use super::*;
    use std::path::PathBuf;

    /// The bytes of `fonts/<name>` in Skia's resources, or `None` (and a note) if absent.
    fn font_bytes(name: &str) -> Option<Vec<u8>> {
        let mut path = match std::env::var_os("SKIA_RESOURCES") {
            Some(dir) => PathBuf::from(dir),
            None => PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("..")
                .join("..")
                .join("third_party")
                .join("skia")
                .join("resources"),
        };
        path.push("fonts");
        path.push(name);
        let data = std::fs::read(&path);
        if data.is_err() {
            eprintln!("todo: skipping, missing Skia resource {}", path.display());
        }
        data.ok()
    }

    /// Runs `body` on the font bytes, or does nothing when the resource is missing.
    fn with_font_bytes(name: &str, body: impl FnOnce(&[u8])) {
        if let Some(bytes) = font_bytes(name) {
            body(&bytes);
        }
    }

    #[test]
    fn test_num_fonts_in_collection() {
        with_font_bytes("test.ttc", |collection_buffer| {
            with_font_bytes("test_glyphs-glyf_colr_1_variable.ttf", |font_buffer| {
                let garbage: [u8; 12] = *b"0ab0ab0ab0ab";
                assert_eq!(font_or_collection(collection_buffer), Some(2));
                assert_eq!(font_or_collection(font_buffer), Some(0));
                assert_eq!(font_or_collection(&garbage), None);
            });
        });
    }

    #[test]
    fn test_font_attributes() {
        with_font_bytes("cond-bold-italic.ttf", |file_buffer| {
            let font_ref = make_font_ref(file_buffer, 0);
            let coords = resolve_into_normalized_coords(&font_ref, &[]);
            assert!(font_ref_is_valid(&font_ref));
            let style = get_font_style(&font_ref, &coords).unwrap_or_default();
            // The font should have condensed width attribute but it's condensed itself so we
            // have the normal width.
            assert_eq!(style.width, 5);
            assert_eq!(style.slant, 1); // Skia italic
            assert_eq!(style.weight, 700); // Skia bold
        });
    }

    #[test]
    fn test_variable_font_attributes() {
        with_font_bytes("Variable.ttf", |file_buffer| {
            let font_ref = make_font_ref(file_buffer, 0);
            let coords = resolve_into_normalized_coords(&font_ref, &[]);
            assert!(font_ref_is_valid(&font_ref));
            let style = get_font_style(&font_ref, &coords);
            assert_eq!(
                style,
                Some(BridgeFontStyle {
                    weight: 400, // Skia normal
                    slant: 0,    // Skia upright
                    width: 5,    // Skia normal
                })
            );
        });
    }

    #[test]
    fn test_no_instances() {
        with_font_bytes("cond-bold-italic.ttf", |font_buffer| {
            let font_ref = make_font_ref(font_buffer, 0);
            assert_eq!(num_named_instances(&font_ref), 0);
        });
    }

    #[test]
    fn test_no_axes() {
        with_font_bytes("cond-bold-italic.ttf", |font_buffer| {
            let font_ref = make_font_ref(font_buffer, 0);
            assert_eq!(num_axes(&font_ref), 0);
        });
    }

    #[test]
    fn test_named_instances() {
        with_font_bytes("Variable.ttf", |font_buffer| {
            let font_ref = make_font_ref(font_buffer, 0);
            let num_instances = num_named_instances(&font_ref);
            assert_eq!(num_instances, 5);

            for index in 0..num_instances {
                let named_instance_index =
                    u32::try_from((index + 1) << 16).expect("named instance index fits in u32");
                let num_coords = coordinates_for_shifted_named_instance_index(
                    &font_ref,
                    named_instance_index,
                    &mut [],
                );
                assert_eq!(num_coords, 2);

                let mut received_coords = [Coordinate::default(); 2];
                let num_coords = coordinates_for_shifted_named_instance_index(
                    &font_ref,
                    named_instance_index,
                    &mut received_coords,
                );
                let size = isize::try_from(num_axes(&font_ref)).unwrap_or(-1);
                assert_eq!(num_coords, size);
                if index + 1 == 5 {
                    assert_eq!(num_coords, 2);
                    assert_eq!(
                        received_coords[0],
                        Coordinate {
                            axis: u32::from_be_bytes(*b"wght"),
                            value: 400.0
                        }
                    );
                    assert_eq!(
                        received_coords[1],
                        Coordinate {
                            axis: u32::from_be_bytes(*b"wdth"),
                            value: 200.0
                        }
                    );
                }
            }
        });
    }

    #[test]
    fn test_shifted_named_instance_index() {
        // Named instances are 1-indexed.
        const SHIFTED_NAMED_INSTANCE_INDEX: u32 = 5 << 16;
        const OUT_OF_BOUNDS_NAMED_INSTANCE_INDEX: u32 = 6 << 16;
        with_font_bytes("Variable.ttf", |file_buffer| {
            let font_ref = make_font_ref(file_buffer, 0);
            assert!(font_ref_is_valid(&font_ref));

            let num_coords = coordinates_for_shifted_named_instance_index(
                &font_ref,
                SHIFTED_NAMED_INSTANCE_INDEX,
                &mut [],
            );
            assert_eq!(num_coords, 2);

            let mut too_small = [Coordinate::default(); 1];
            let num_coords = coordinates_for_shifted_named_instance_index(
                &font_ref,
                SHIFTED_NAMED_INSTANCE_INDEX,
                &mut too_small,
            );
            assert_eq!(num_coords, 0);

            let mut received_coords = [Coordinate::default(); 2];
            let num_coords = coordinates_for_shifted_named_instance_index(
                &font_ref,
                SHIFTED_NAMED_INSTANCE_INDEX,
                &mut received_coords,
            );
            assert_eq!(num_coords, 2);
            assert_eq!(
                received_coords[0],
                Coordinate {
                    axis: u32::from_be_bytes(*b"wght"),
                    value: 400.0
                }
            );
            assert_eq!(
                received_coords[1],
                Coordinate {
                    axis: u32::from_be_bytes(*b"wdth"),
                    value: 200.0
                }
            );

            let mut too_large = [Coordinate::default(); 5];
            let num_coords = coordinates_for_shifted_named_instance_index(
                &font_ref,
                SHIFTED_NAMED_INSTANCE_INDEX,
                &mut too_large,
            );
            assert_eq!(num_coords, 2);

            // Index out of bounds:
            let num_coords = coordinates_for_shifted_named_instance_index(
                &font_ref,
                OUT_OF_BOUNDS_NAMED_INSTANCE_INDEX,
                &mut [],
            );
            assert_eq!(num_coords, 0);
        });
    }
}
