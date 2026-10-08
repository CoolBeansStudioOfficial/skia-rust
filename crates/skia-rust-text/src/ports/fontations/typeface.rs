// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file.
// Port of: src/ports/SkTypeface_fontations.cpp#L1-L357 and #L987-L1175 (chrome/m156), and the
// `SkTypeface_Fontations` declaration in src/ports/SkTypeface_fontations_priv.h, the typeface half
// (T19a, docs/design/text.md §9). The outline and bitmap halves (the scaler context, the metrics
// and the COLR and bitmap glyphs) follow with T19b, T21 and T22.

//! `SkTypeface_Fontations`: a typeface read with skrifa, from font data and a collection index.

use std::sync::{Arc, OnceLock};

use skia_rust_core::color::Color;
use skia_rust_core::data::Data;
use skia_rust_core::descriptor::Descriptor;
use skia_rust_core::font_arguments::palette::Override;
use skia_rust_core::font_arguments::variation_position::Coordinate;
use skia_rust_core::font_arguments::{FontArguments, VariationPosition};
use skia_rust_core::font_descriptor::{FactoryId, FontDescriptor};
use skia_rust_core::font_parameters::variation::Axis;
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::{FontHinting, FourByteTag, GlyphId, set_four_byte_tag};
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::scaler_context::{
    ScalerContext, ScalerContextEffects, ScalerContextFlags, ScalerContextRec,
};
use skia_rust_core::stream::{MemoryStream, StreamAsset};
use skia_rust_core::typeface::{
    LocalizedStrings, Typeface, TypefaceBase, TypefaceCore, VecLocalizedStrings,
};
use skia_rust_core::utf::Unichar;

use super::base::{
    BridgeFontRef, BridgeFontStyle, BridgeMappingIndex, BridgeNormalizedCoords,
    coordinates_for_shifted_named_instance_index, fill_glyph_to_unicode_map, font_ref_is_valid,
    get_font_style, lookup_glyph_or_zero, make_font_ref, make_mapping_index,
    normalized_coords_equal, num_glyphs, populate_axes, resolve_into_normalized_coords, table_data,
    table_tags, units_per_em_or_zero, variation_position,
};
use super::colr::resolve_palette;
use super::names;

/// `SkTypefaces::Fontations::FactoryId` (`'fnta'`): the descriptor tag of these typefaces.
// Port of: src/ports/SkTypeface_fontations_factory.h#L14 (chrome/m156)
pub const FACTORY_ID: FactoryId = set_four_byte_tag(b'f', b'n', b't', b'a');

/// `SkSetFourByteTag('C', 'O', 'L', 'R')`: the table whose presence makes a glyph mask need the
/// current colour.
// Port of: src/ports/SkTypeface_fontations.cpp#L283-L288 (chrome/m156)
const COLR_TAG: FourByteTag = set_four_byte_tag(b'C', b'O', b'L', b'R');

/// A typeface read with Fontations (`SkTypeface_Fontations`).
///
/// It owns the font data and the state read from it at creation. The font itself is parsed on
/// demand from the data (`BridgeFontRef` borrows the bytes, so it cannot be stored beside them).
// Port of: src/ports/SkTypeface_fontations_priv.h#L186-L255 (chrome/m156)
#[derive(Debug)]
pub struct TypefaceFontations {
    core: TypefaceCore,
    /// `SkTypeface::getFontStyle` of the core, kept for the descriptor.
    style: FontStyle,
    /// `fFontData`: the bytes of the font.
    font_data: Data,
    /// `fTtcIndex`: the font index within the data (the low 16 bits of the collection index).
    ttc_index: u32,
    /// `fMappingIndex`: the character map.
    mapping_index: BridgeMappingIndex,
    /// `fBridgeNormalizedCoords`: the normalized and user variation coordinates.
    normalized_coords: BridgeNormalizedCoords,
    /// `fPalette`: the resolved palette, `0xAARRGGBB` per entry.
    palette: Vec<u32>,
    /// `fGlyphMasksMayNeedCurrentColor`, computed once (`SkOnce`).
    glyph_masks_may_need_current_color: OnceLock<bool>,
}

impl TypefaceFontations {
    /// The parsed font, over this typeface's data.
    fn font_ref(&self) -> BridgeFontRef<'_> {
        make_font_ref(self.font_data.as_bytes(), self.ttc_index)
    }

    /// `getTableSize`: the size of a table, 0 if it is missing.
    // Port of: src/core/SkTypeface.cpp#L310-L312 (chrome/m156)
    fn table_size(&self, tag: FourByteTag) -> usize {
        table_data(&self.font_ref(), tag, 0, &mut [])
    }
}

/// `SkTypeface_Fontations::MakeFromData`: the typeface of the font at the collection index of
/// `args`, with its variation position and palette. `None` if the data is not a font, or the
/// index names no font.
// Port of: src/ports/SkTypeface_fontations.cpp#L162-L242 (chrome/m156)
#[must_use]
pub fn make_from_data(data: Data, args: &FontArguments<'_, '_>) -> Option<Typeface> {
    // C++ collection indices are 32-bit ints. The low 16 bits select the font in the data, and
    // the high 16 bits may name a named instance.
    #[allow(clippy::cast_possible_truncation)] // the index is an `int` in C++
    let collection_index = args.collection_index() as u32;
    let ttc_index = collection_index & 0xFFFF;
    let font_ref = make_font_ref(data.as_bytes(), ttc_index);
    if !font_ref_is_valid(&font_ref) {
        return None;
    }
    let mapping_index = make_mapping_index(&font_ref)?;

    let mut variation_position = args.variation_design_position().coordinates.to_vec();
    // Handle FreeType behaviour of upper 15 bits of collection index representing a named
    // instance choice. If so, prepopulate the variation coordinates with the values from the
    // named instance and append the user coordinates after that so they can override the named
    // instance's coordinates.
    if (collection_index & 0xFFFF_0000) != 0 {
        let mut concatenated = named_instance_coordinates(&font_ref, collection_index)?;
        concatenated.extend_from_slice(&variation_position);
        variation_position = concatenated;
    }

    let normalized_coords = resolve_into_normalized_coords(&font_ref, &variation_position);
    let style = get_font_style(&font_ref, &normalized_coords)
        .map_or_else(FontStyle::default, font_style_from_bridge);

    let palette = resolve_palette(
        &font_ref,
        palette_base_index(args.palette().index),
        args.palette().overrides,
    );

    Some(Typeface::new(Arc::new(TypefaceFontations {
        core: TypefaceCore::new(style, true),
        style,
        font_data: data,
        ttc_index,
        mapping_index,
        normalized_coords,
        palette,
        glyph_masks_may_need_current_color: OnceLock::new(),
    })))
}

/// `SkTypeface_Make_Fontations(stream, args)` and `SkTypeface_Fontations::MakeFromStream`: reads
/// the whole stream, then makes the typeface as [`make_from_data`] does.
// Port of: src/ports/SkTypeface_fontations.cpp#L157-L160 and #L27-L43 (chrome/m156)
#[must_use]
pub fn make_from_stream(
    mut stream: Box<dyn StreamAsset>,
    args: &FontArguments<'_, '_>,
) -> Option<Typeface> {
    make_from_data(stream_to_data(stream.as_mut()), args)
}

/// Port of `streamToData`: the bytes of a stream, from its start. An unreadable stream gives
/// empty data, which does not make a typeface.
// Port of: src/ports/SkTypeface_fontations.cpp#L37-L50 (chrome/m156)
fn stream_to_data(stream: &mut dyn StreamAsset) -> Data {
    stream.rewind();
    let length = stream.get_length();
    Data::from_stream(stream, length).unwrap_or_default()
}

/// Port of the `GetNamedInstance` part of `MakeFromData`: the axis values of the named instance
/// that `shifted_index` selects, or `None` when the instance cannot be read.
// Port of: src/ports/SkTypeface_fontations.cpp#L184-L201 (chrome/m156)
fn named_instance_coordinates(
    font_ref: &BridgeFontRef<'_>,
    shifted_index: u32,
) -> Option<Vec<Coordinate>> {
    let count = usize::try_from(coordinates_for_shifted_named_instance_index(
        font_ref,
        shifted_index,
        &mut [],
    ))
    .unwrap_or(0);
    let mut coordinates = vec![Coordinate::default(); count];
    let retrieved =
        coordinates_for_shifted_named_instance_index(font_ref, shifted_index, &mut coordinates);
    (usize::try_from(retrieved).ok() == Some(count)).then_some(coordinates)
}

/// The `SkFontStyle` of the bridge's style values (`SkFontStyle(weight, width, slant)`).
// Port of: src/ports/SkTypeface_fontations.cpp#L224-L228 (chrome/m156)
pub(crate) fn font_style_from_bridge(style: BridgeFontStyle) -> FontStyle {
    let slant = match style.slant {
        1 => Slant::Italic,
        2 => Slant::Oblique,
        _ => Slant::Upright,
    };
    FontStyle::new(Weight::from(style.weight), Width::from(style.width), slant)
}

/// The palette index as the bridge takes it: C++ narrows the `int` index to `uint16_t`, which
/// keeps the low 16 bits.
// Port of: src/ports/SkTypeface_fontations.cpp#L226 and #L1033 (chrome/m156), the `uint16_t` argument
fn palette_base_index(index: i32) -> u16 {
    u16::try_from(index & 0xFFFF).unwrap_or(0)
}

/// `isLCD`: the glyph masks are LCD.
// Port of: src/ports/SkTypeface_fontations.cpp#L300 (chrome/m156)
fn is_lcd(rec: &ScalerContextRec) -> bool {
    rec.mask_format == MaskFormat::Lcd16
}

/// `bothZero`.
// Port of: src/ports/SkTypeface_fontations.cpp#L302 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in C++
fn both_zero(a: f32, b: f32) -> bool {
    a == 0.0 && b == 0.0
}

/// `isAxisAligned`.
// Port of: src/ports/SkTypeface_fontations.cpp#L304-L306 (chrome/m156)
#[allow(clippy::float_cmp)] // exact comparison, as in C++
fn is_axis_aligned(rec: &ScalerContextRec) -> bool {
    rec.pre_skew_x == 0.0
        && (both_zero(rec.post_2x2[0][1], rec.post_2x2[1][0])
            || both_zero(rec.post_2x2[0][0], rec.post_2x2[1][1]))
}

impl TypefaceBase for TypefaceFontations {
    fn core(&self) -> &TypefaceCore {
        &self.core
    }

    /// `SkTypeface_Fontations::onGetFontDescriptor`: the family, style, factory id, and the
    /// palette as overrides. Always serialized as data.
    // Port of: src/ports/SkTypeface_fontations.cpp#L1122-L1140 (chrome/m156)
    fn on_get_font_descriptor(&self) -> (FontDescriptor, bool) {
        let mut desc = FontDescriptor::new();
        desc.set_family_name(&self.on_get_family_name());
        desc.set_style(self.style);
        desc.set_factory_id(FACTORY_ID);

        // TODO: keep the index to emit here
        desc.set_palette_index(0);
        // TODO: omit override when palette[n] == CPAL[paletteIndex][n]
        let overrides = desc.set_palette_entry_overrides(self.palette.len());
        for (i, (out, color)) in overrides.iter_mut().zip(&self.palette).enumerate() {
            *out = Override {
                index: u16::try_from(i).unwrap_or(u16::MAX),
                color: Color::from(*color),
            };
        }
        (desc, true)
    }

    /// `SkTypeface_Fontations::onOpenStream`: the whole data, with the font index.
    // Port of: src/ports/SkTypeface_fontations.cpp#L987-L990 (chrome/m156)
    fn on_open_stream(&self) -> Option<(Box<dyn StreamAsset>, i32)> {
        #[allow(clippy::cast_possible_wrap)] // the index is an `int` in C++
        let index = self.ttc_index as i32;
        Some((
            Box::new(MemoryStream::from_data(Some(self.font_data.clone()))),
            index,
        ))
    }

    /// `SkTypeface_Fontations::onGetFamilyName`.
    // Port of: src/ports/SkTypeface_fontations.cpp#L266-L269 (chrome/m156)
    fn on_get_family_name(&self) -> String {
        names::family_name(&self.font_ref())
    }

    /// `SkTypeface_Fontations::onGetVariationDesignPosition`: the axes of the normalized
    /// coordinates, read in two steps as the C++ does (a count, then the values).
    // Port of: src/ports/SkTypeface_fontations.cpp#L1164-L1173 (chrome/m156)
    fn on_get_variation_design_position(&self) -> Option<Vec<Coordinate>> {
        let count = usize::try_from(variation_position(&self.normalized_coords, &mut [])).ok()?;
        let mut coordinates = vec![Coordinate::default(); count];
        let retrieved = variation_position(&self.normalized_coords, &mut coordinates);
        (usize::try_from(retrieved).ok() == Some(count)).then_some(coordinates)
    }

    /// `SkTypeface_Fontations::onGetVariationDesignParameters`: every axis of the font.
    // Port of: src/ports/SkTypeface_fontations.cpp#L1175-L1178 (chrome/m156)
    fn on_get_variation_design_parameters(&self) -> Option<Vec<Axis>> {
        let font_ref = self.font_ref();
        let count = usize::try_from(populate_axes(&font_ref, &mut [])).ok()?;
        let mut axes = vec![Axis::default(); count];
        let populated = populate_axes(&font_ref, &mut axes);
        (usize::try_from(populated).ok() == Some(count)).then_some(axes)
    }

    /// `SkTypeface_Fontations::onGetUPEM`.
    // Port of: src/ports/SkTypeface_fontations.cpp#L262-L264 (chrome/m156)
    fn on_get_upem(&self) -> i32 {
        i32::from(units_per_em_or_zero(&self.font_ref()))
    }

    /// `SkTypeface_Fontations::onGetPostScriptName`.
    // Port of: src/ports/SkTypeface_fontations.cpp#L271-L281 (chrome/m156)
    fn on_get_postscript_name(&self) -> Option<String> {
        names::postscript_name(&self.font_ref())
    }

    /// `SkTypeface_Fontations::onCreateFamilyNameIterator`: the family names, read up front.
    // Port of: src/ports/SkTypeface_fontations.cpp#L354-L356 (chrome/m156)
    fn on_create_family_name_iterator(&self) -> Box<dyn LocalizedStrings> {
        Box::new(VecLocalizedStrings::new(names::get_localized_strings(
            &self.font_ref(),
        )))
    }

    /// `SkTypeface_Fontations::onGetTableTags`.
    // Port of: src/ports/SkTypeface_fontations.cpp#L1155-L1162 (chrome/m156)
    fn on_get_table_tags(&self) -> Vec<FourByteTag> {
        let font_ref = self.font_ref();
        let mut tags = vec![0; usize::from(table_tags(&font_ref, &mut []))];
        let _count = table_tags(&font_ref, &mut tags);
        tags
    }

    /// `SkTypeface_Fontations::onGetTableData`.
    // Port of: src/ports/SkTypeface_fontations.cpp#L1142-L1153 (chrome/m156)
    fn on_get_table_data(
        &self,
        tag: FourByteTag,
        offset: usize,
        length: usize,
        data: &mut [u8],
    ) -> usize {
        table_data(&self.font_ref(), tag, offset, data).min(length)
    }

    /// `SkTypeface_Fontations::onGlyphMaskNeedsCurrentColor`: true if the font has a COLR table.
    // Port of: src/ports/SkTypeface_fontations.cpp#L283-L289 (chrome/m156)
    fn on_glyph_mask_needs_current_color(&self) -> bool {
        *self
            .glyph_masks_may_need_current_color
            .get_or_init(|| self.table_size(COLR_TAG) > 0)
    }

    /// `SkTypeface_Fontations::onMakeClone`: the same typeface when the arguments do not change
    /// it. Otherwise a new typeface, with the arguments' axes fused into the current ones.
    // Port of: src/ports/SkTypeface_fontations.cpp#L992-L1041 (chrome/m156)
    fn on_make_clone(&self, this: Typeface, args: &FontArguments<'_, '_>) -> Typeface {
        // Matching DWrite implementation, return self if ttc index mismatches.
        #[allow(clippy::cast_possible_truncation)] // the index is an `int` in C++
        if self.ttc_index != args.collection_index() as u32 {
            return this;
        }

        // The axis count cannot change between the two reads, so C++'s null return is
        // unreachable here, and the typeface itself stands in for it.
        let Some(mut fused_design_position) = self.on_get_variation_design_position() else {
            return this;
        };
        // We know the internally retrieved axes are normalized, contain a value for every
        // possible axis, other axes do not exist, so we only need to override any of those.
        let arg_position = args.variation_design_position().coordinates;
        for coordinate in &mut fused_design_position {
            for arg in arg_position {
                if coordinate.axis == arg.axis {
                    coordinate.value = arg.value;
                }
            }
        }

        // C++ builds `fusedArgs` without a collection index, so the rebuilt typeface is made from
        // index 0 of the data. This is mirrored as it is.
        let mut fused_args = FontArguments::new();
        fused_args
            .set_variation_design_position(VariationPosition {
                coordinates: &fused_design_position,
            })
            .set_palette(args.palette());

        let font_ref = self.font_ref();
        let normalized_args = resolve_into_normalized_coords(&font_ref, &fused_design_position);
        if !normalized_coords_equal(&normalized_args, &self.normalized_coords) {
            return make_from_data(self.font_data.clone(), &fused_args).unwrap_or(this);
        }

        // TODO(skbug.com/330149870): Palette differences are not fused, see DWrite backend impl.
        let new_palette = resolve_palette(
            &font_ref,
            palette_base_index(args.palette().index),
            args.palette().overrides,
        );
        if self.palette != new_palette {
            return make_from_data(self.font_data.clone(), &fused_args).unwrap_or(this);
        }

        this
    }

    /// `SkTypeface_Fontations::onCreateScalerContext`: the outline scaler context. It is the
    /// outline half of the typeface (T19b, docs/design/text.md §9), which is not ported yet.
    ///
    /// # Panics
    ///
    /// Always, until T19b: a glyph cannot be drawn from a Fontations typeface before then.
    // Port of: src/ports/SkTypeface_fontations.cpp#L1043-L1046 (chrome/m156), T19b
    fn on_create_scaler_context(
        &self,
        _this: Typeface,
        _effects: &ScalerContextEffects,
        _desc: &Descriptor,
    ) -> ScalerContext {
        unimplemented!(
            "the Fontations scaler context is T19b (docs/design/text.md §9); \
             a Fontations typeface cannot draw glyphs yet"
        )
    }

    /// `SkTypeface_Fontations::onFilterRec`: fake bold becomes a stroke, LCD-specific full
    /// hinting becomes normal hinting off LCD, and rotated text is not hinted.
    // Port of: src/ports/SkTypeface_fontations.cpp#L310-L331 (chrome/m156)
    fn on_filter_rec(&self, rec: &mut ScalerContextRec) {
        rec.use_stroke_for_fake_bold();

        // See https://issues.skia.org/issues/396360753
        // We would like Fontations anti-aliasing on a surface with unknown pixel geometry to
        // look like the FreeType backend in order to avoid perceived regressions
        // in sharpness, so we ignore SkScalerContext::kGenA8FromLCD_Flag in fRec.fFlags.
        rec.flags.remove(ScalerContextFlags::GEN_A8_FROM_LCD);

        // Opportunistic hinting downgrades copied from SkFontHost_FreeType.cpp
        let mut hinting = rec.hinting();
        if FontHinting::Full == hinting && !is_lcd(rec) {
            // Collapse full->normal hinting if we're not doing LCD.
            hinting = FontHinting::Normal;
        }

        // Rotated text looks bad with hinting, so we disable it as needed.
        if !is_axis_aligned(rec) {
            hinting = FontHinting::None;
        }
        rec.set_hinting(hinting);
    }

    /// `SkTypeface_Fontations::onCharsToGlyphs`: the glyph of each character, 0 for none.
    // Port of: src/ports/SkTypeface_fontations.cpp#L291-L297 (chrome/m156)
    fn on_chars_to_glyphs(&self, unichars: &[Unichar], glyphs: &mut [GlyphId]) {
        // C++ reinterprets the `SkUnichar`s as `uint32_t`, which is the same bit pattern.
        #[allow(clippy::cast_sign_loss)] // same bits as the C++ reinterpretation
        let codepoints: Vec<u32> = unichars.iter().map(|&c| c as u32).collect();
        lookup_glyph_or_zero(&self.font_ref(), &self.mapping_index, &codepoints, glyphs);
    }

    /// `SkTypeface_Fontations::onCountGlyphs`.
    // Port of: src/ports/SkTypeface_fontations.cpp#L299-L301 (chrome/m156)
    fn on_count_glyphs(&self) -> i32 {
        i32::from(num_glyphs(&self.font_ref()))
    }

    /// `SkTypeface_Fontations::getGlyphToUnicodeMap`: the first code point of each glyph, for as
    /// many glyphs as `dst` holds.
    // Port of: src/ports/SkTypeface_fontations.cpp#L303-L308 (chrome/m156)
    // C++ writes the `uint32_t` code points into the `SkUnichar` array: same bits, as `as` keeps.
    #[allow(clippy::cast_possible_wrap)]
    fn on_get_glyph_to_unicode_map(&self, dst: &mut [Unichar]) {
        let glyph_count = usize::try_from(self.on_count_glyphs()).unwrap_or(0);
        let count = glyph_count.min(dst.len());
        let mut codepoints = vec![0_u32; count];
        fill_glyph_to_unicode_map(&self.font_ref(), &mut codepoints);
        for (out, codepoint) in dst.iter_mut().zip(codepoints) {
            *out = codepoint as Unichar;
        }
    }
}
