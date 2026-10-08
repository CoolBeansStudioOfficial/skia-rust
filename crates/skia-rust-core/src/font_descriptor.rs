// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkFontDescriptor.h, src/core/SkFontDescriptor.cpp

//! [`FontDescriptor`]: the serializable description of a typeface (`SkFontDescriptor`). A
//! descriptor holds the names, style, variation and palette settings and, optionally, the font
//! data itself. [`FontDescriptor::serialize`] and [`FontDescriptor::deserialize`] write and read
//! the packed, tagged format of `SkFontDescriptor.cpp`.

use std::fmt;

use crate::data::Data;
use crate::font_arguments::palette::Override;
use crate::font_arguments::variation_position::Coordinate;
use crate::font_style::{FontStyle, Slant, Weight, Width};
use crate::scalar::{float_interp_func, scalar, scalar_round_to_int};
use crate::stream::{MemoryStream, Stream, StreamAsset, WStream};
use crate::stream_priv::remaining_length_is_below;

/// Tags of the descriptor's fields (`SkFontDescriptor.cpp`'s anonymous enum). Each one is
/// written as a packed id, followed by its data as documented on the constant.
mod tag {
    pub const INVALID: usize = 0x00;
    /// `int length, data[length]`.
    pub const FONT_FAMILY_NAME: usize = 0x01;
    /// `int length, data[length]`.
    pub const FULL_NAME: usize = 0x04;
    /// `int length, data[length]`.
    pub const POSTSCRIPT_NAME: usize = 0x06;
    /// `scalar` (1 - 1000).
    pub const WEIGHT: usize = 0x10;
    /// `scalar` (percentage, 100 is 'normal').
    pub const WIDTH: usize = 0x11;
    /// `scalar` (ccw angle, -10 is a normal right leaning oblique).
    pub const SLANT: usize = 0x12;
    /// `scalar` (0 is Roman, 1 is fully Italic).
    pub const ITALIC: usize = 0x13;
    /// No data.
    pub const SYNTHETIC_BOLD: usize = 0xF6;
    /// No data.
    pub const SYNTHETIC_OBLIQUE: usize = 0xF7;
    /// `int`.
    pub const PALETTE_INDEX: usize = 0xF8;
    /// `int count, (int, u32)[count]`.
    pub const PALETTE_ENTRY_OVERRIDES: usize = 0xF9;
    /// `int count, (u32, scalar)[count]`.
    pub const FONT_VARIATION: usize = 0xFA;
    /// `int`.
    pub const FACTORY_ID: usize = 0xFC;
    /// `int`.
    pub const FONT_INDEX: usize = 0xFD;
    /// No data.
    pub const SENTINEL: usize = 0xFF;
}

/// `usWidths`: the OS/2 width classes, 1 to 9.
// Port of: src/core/SkFontDescriptor.cpp#L87-L89 (chrome/m156)
const US_WIDTHS: [scalar; 9] = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0];

/// `width_for_usWidth`: the width axis value of each OS/2 width class.
// Port of: src/core/SkFontDescriptor.cpp#L90-L94 (chrome/m156)
const WIDTH_FOR_US_WIDTH: [scalar; 16] = [
    50.0, 50.0, 62.5, 75.0, 87.5, 100.0, 112.5, 125.0, 150.0, 200.0, 200.0, 200.0, 200.0, 200.0,
    200.0, 200.0,
];

/// The factory id of a typeface, `SkTypeface::FactoryId` (a four character tag).
// Port of: include/core/SkTypeface.h (FactoryId is uint32_t, chrome/m156)
#[doc(alias = "SkTypeface::FactoryId")]
pub type FactoryId = u32;

/// Describes a typeface so it can be serialized and recreated (`SkFontDescriptor`).
// Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
#[doc(alias = "SkFontDescriptor")]
pub struct FontDescriptor {
    family_name: String,
    full_name: String,
    postscript_name: String,
    style: FontStyle,
    stream: Option<Box<dyn StreamAsset>>,
    collection_index: i32,
    palette_index: i32,
    variation: Vec<Coordinate>,
    palette_entry_overrides: Vec<Override>,
    synthetic_bold: bool,
    synthetic_oblique: bool,
    factory_id: FactoryId,
}

impl fmt::Debug for FontDescriptor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FontDescriptor")
            .field("family_name", &self.family_name)
            .field("full_name", &self.full_name)
            .field("postscript_name", &self.postscript_name)
            .field("style", &self.style)
            .field("has_stream", &self.stream.is_some())
            .field("collection_index", &self.collection_index)
            .field("palette_index", &self.palette_index)
            .field("variation", &self.variation)
            .field("palette_entry_overrides", &self.palette_entry_overrides)
            .field("synthetic_bold", &self.synthetic_bold)
            .field("synthetic_oblique", &self.synthetic_oblique)
            .field("factory_id", &self.factory_id)
            .finish()
    }
}

impl Default for FontDescriptor {
    fn default() -> Self {
        Self::new()
    }
}

impl FontDescriptor {
    /// `SkFontDescriptor::SkFontDescriptor()`: no names, normal style, no data.
    // Port of: src/core/SkFontDescriptor.cpp#L46 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            family_name: String::new(),
            full_name: String::new(),
            postscript_name: String::new(),
            style: FontStyle::default(),
            stream: None,
            collection_index: 0,
            palette_index: 0,
            variation: Vec::new(),
            palette_entry_overrides: Vec::new(),
            synthetic_bold: false,
            synthetic_oblique: false,
            factory_id: 0,
        }
    }

    /// `SkFontDescriptor::getStyle`.
    // Port of: src/core/SkFontDescriptor.h#L95 (chrome/m156)
    #[must_use]
    pub fn style(&self) -> FontStyle {
        self.style
    }

    /// `SkFontDescriptor::setStyle`.
    // Port of: src/core/SkFontDescriptor.h#L96 (chrome/m156)
    pub fn set_style(&mut self, style: FontStyle) {
        self.style = style;
    }

    /// `SkFontDescriptor::getFamilyName`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn family_name(&self) -> &str {
        &self.family_name
    }

    /// `SkFontDescriptor::getFullName`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn full_name(&self) -> &str {
        &self.full_name
    }

    /// `SkFontDescriptor::getPostscriptName`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn postscript_name(&self) -> &str {
        &self.postscript_name
    }

    /// `SkFontDescriptor::setFamilyName`.
    // Port of: src/core/SkFontDescriptor.h#L102 (chrome/m156)
    pub fn set_family_name(&mut self, name: &str) {
        name.clone_into(&mut self.family_name);
    }

    /// `SkFontDescriptor::setFullName`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    pub fn set_full_name(&mut self, name: &str) {
        name.clone_into(&mut self.full_name);
    }

    /// `SkFontDescriptor::setPostscriptName`.
    // Port of: src/core/SkFontDescriptor.h#L104 (chrome/m156)
    pub fn set_postscript_name(&mut self, name: &str) {
        name.clone_into(&mut self.postscript_name);
    }

    /// `SkFontDescriptor::hasStream`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn has_stream(&self) -> bool {
        self.stream.is_some()
    }

    /// `SkFontDescriptor::detachStream`: takes the font data out of the descriptor.
    // Port of: src/core/SkFontDescriptor.h#L124 (chrome/m156)
    pub fn detach_stream(&mut self) -> Option<Box<dyn StreamAsset>> {
        self.stream.take()
    }

    /// `SkFontDescriptor::setStream`.
    // Port of: src/core/SkFontDescriptor.h#L125 (chrome/m156)
    pub fn set_stream(&mut self, stream: Option<Box<dyn StreamAsset>>) {
        self.stream = stream;
    }

    /// `SkFontDescriptor::getCollectionIndex`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn collection_index(&self) -> i32 {
        self.collection_index
    }

    /// `SkFontDescriptor::setCollectionIndex`.
    // Port of: src/core/SkFontDescriptor.h#L126 (chrome/m156)
    pub fn set_collection_index(&mut self, collection_index: i32) {
        self.collection_index = collection_index;
    }

    /// `SkFontDescriptor::getPaletteIndex`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn palette_index(&self) -> i32 {
        self.palette_index
    }

    /// `SkFontDescriptor::setPaletteIndex`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    pub fn set_palette_index(&mut self, palette_index: i32) {
        self.palette_index = palette_index;
    }

    /// `SkFontDescriptor::getVariation`: the variation coordinates, with the count given by
    /// `getVariationCoordinateCount`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn variation(&self) -> &[Coordinate] {
        &self.variation
    }

    /// `SkFontDescriptor::setVariationCoordinates(count)`: replaces the coordinates with `count`
    /// zeroed ones and returns them for filling in.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    pub fn set_variation_coordinates(&mut self, count: usize) -> &mut [Coordinate] {
        self.variation = vec![Coordinate::default(); count];
        &mut self.variation
    }

    /// `SkFontDescriptor::getPaletteEntryOverrides`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn palette_entry_overrides(&self) -> &[Override] {
        &self.palette_entry_overrides
    }

    /// `SkFontDescriptor::setPaletteEntryOverrides(count)`: replaces the overrides with `count`
    /// zeroed ones and returns them for filling in.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    pub fn set_palette_entry_overrides(&mut self, count: usize) -> &mut [Override] {
        self.palette_entry_overrides = vec![Override::default(); count];
        &mut self.palette_entry_overrides
    }

    /// `SkFontDescriptor::getSyntheticBold`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    #[must_use]
    pub fn synthetic_bold(&self) -> bool {
        self.synthetic_bold
    }

    /// `SkFontDescriptor::setSyntheticBold`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    pub fn set_synthetic_bold(&mut self, bold: bool) {
        self.synthetic_bold = bold;
    }

    /// `SkFontDescriptor::getSyntheticOblique`.
    // Port of: src/core/SkFontDescriptor.h#L119 (chrome/m156)
    #[must_use]
    pub fn synthetic_oblique(&self) -> bool {
        self.synthetic_oblique
    }

    /// `SkFontDescriptor::setSyntheticOblique`.
    // Port of: src/core/SkFontDescriptor.h#L85-L152 (chrome/m156)
    pub fn set_synthetic_oblique(&mut self, oblique: bool) {
        self.synthetic_oblique = oblique;
    }

    /// `SkFontDescriptor::getFactoryId`.
    // Port of: src/core/SkFontDescriptor.h#L120 (chrome/m156)
    #[must_use]
    pub fn factory_id(&self) -> FactoryId {
        self.factory_id
    }

    /// `SkFontDescriptor::setFactoryId`.
    // Port of: src/core/SkFontDescriptor.h#L138 (chrome/m156)
    pub fn set_factory_id(&mut self, factory_id: FactoryId) {
        self.factory_id = factory_id;
    }

    /// `SkFontDescriptor::SkFontStyleWidthForWidthAxisValue`.
    // Port of: src/core/SkFontDescriptor.cpp#L303-L306 (chrome/m156)
    #[must_use]
    pub fn style_width_for_width_axis_value(width: scalar) -> Width {
        let us_width = scalar_round_to_int(float_interp_func(
            width,
            // `&width_for_usWidth[1]` with length 9 in C++.
            &WIDTH_FOR_US_WIDTH[1..10],
            &US_WIDTHS,
        ));
        Width::from(us_width)
    }

    /// `SkFontDescriptor::SkFontWidthAxisValueForStyleWidth`.
    // Port of: src/core/SkFontDescriptor.cpp#L308-L310 (chrome/m156)
    #[must_use]
    pub fn font_width_axis_value_for_style_width(width: i32) -> scalar {
        // `width & 0xF` is in 0..16, so the index is always inside the table.
        WIDTH_FOR_US_WIDTH[usize::try_from(width & 0xF).unwrap_or(0)]
    }

    /// `SkFontDescriptor::serialize`: writes the descriptor, then its font data if it has any.
    /// Returns false if any write fails.
    // Port of: src/core/SkFontDescriptor.cpp#L245-L301 (chrome/m156)
    #[allow(clippy::too_many_lines)] // one write per field, in the order of SkFontDescriptor::serialize
    pub fn serialize(&self, stream: &mut dyn WStream) -> bool {
        let style = self.style;
        let style_bits = ((*style.weight()).cast_unsigned() << 16)
            | ((*style.width()).cast_unsigned() << 8)
            | (style.slant() as u32);
        if !stream.write_packed_uint(style_bits as usize) {
            return false;
        }
        if !write_string(stream, &self.family_name, tag::FONT_FAMILY_NAME) {
            return false;
        }
        if !write_string(stream, &self.full_name, tag::FULL_NAME) {
            return false;
        }
        if !write_string(stream, &self.postscript_name, tag::POSTSCRIPT_NAME) {
            return false;
        }
        if !write_scalar(stream, scalar_from_i32(*style.weight()), tag::WEIGHT) {
            return false;
        }
        let width_index = usize::try_from(*style.width()).unwrap_or(0);
        if !write_scalar(stream, WIDTH_FOR_US_WIDTH[width_index], tag::WIDTH) {
            return false;
        }
        let slant = if style.slant() == Slant::Upright {
            0.0
        } else {
            -20.0
        };
        if !write_scalar(stream, slant, tag::SLANT) {
            return false;
        }
        let italic = if style.slant() == Slant::Italic {
            1.0
        } else {
            0.0
        };
        if !write_scalar(stream, italic, tag::ITALIC) {
            return false;
        }
        if self.collection_index > 0
            && !write_uint(
                stream,
                usize::try_from(self.collection_index).unwrap_or(0),
                tag::FONT_INDEX,
            )
        {
            return false;
        }
        if self.palette_index > 0
            && !write_uint(
                stream,
                usize::try_from(self.palette_index).unwrap_or(0),
                tag::PALETTE_INDEX,
            )
        {
            return false;
        }
        if !self.variation.is_empty() {
            if !write_uint(stream, self.variation.len(), tag::FONT_VARIATION) {
                return false;
            }
            for coordinate in &self.variation {
                if !stream.write32(coordinate.axis) || !stream.write_scalar(coordinate.value) {
                    return false;
                }
            }
        }
        if !self.palette_entry_overrides.is_empty() {
            if !write_uint(
                stream,
                self.palette_entry_overrides.len(),
                tag::PALETTE_ENTRY_OVERRIDES,
            ) {
                return false;
            }
            for entry in &self.palette_entry_overrides {
                if !stream.write_packed_uint(usize::from(entry.index))
                    || !stream.write32(u32::from(entry.color))
                {
                    return false;
                }
            }
        }
        if self.synthetic_bold && !write_id(stream, tag::SYNTHETIC_BOLD) {
            return false;
        }
        if self.synthetic_oblique && !write_id(stream, tag::SYNTHETIC_OBLIQUE) {
            return false;
        }
        if !write_uint(stream, self.factory_id as usize, tag::FACTORY_ID) {
            return false;
        }
        if !stream.write_packed_uint(tag::SENTINEL) {
            return false;
        }
        match self.stream.as_ref() {
            Some(font_stream) => {
                // `fStream->duplicate()` so the serialized position does not move the original.
                let mut duplicate = font_stream.duplicate_asset();
                let length = duplicate.get_length();
                stream.write_packed_uint(length) && stream.write_stream(duplicate.as_mut(), length)
            }
            None => stream.write_packed_uint(0),
        }
    }

    /// `SkFontDescriptor::Deserialize`. Reads a descriptor written by [`FontDescriptor::serialize`].
    /// `sanitizer` may rewrite the font data before it is kept (`SkTypefaceStreamSanitizerProc`).
    /// Returns `None` if the stream is malformed.
    // Port of: src/core/SkFontDescriptor.cpp#L96-L243 (chrome/m156)
    #[must_use]
    pub fn deserialize(
        stream: &mut dyn Stream,
        sanitizer: Option<&dyn Fn(Data) -> Option<Data>>,
    ) -> Option<Self> {
        let mut result = Self::new();
        let style_bits = stream.read_packed_uint()?;
        // The masked value is at most 0xFFFF, which is exact in f32.
        let mut weight = f32::from(u16::try_from((style_bits >> 16) & 0xFFFF).unwrap_or(0));
        let mut width = WIDTH_FOR_US_WIDTH[(style_bits >> 8) & 0x000F];
        // kUpright_Slant is 0.
        let mut slant = if (style_bits & 0x000F) != 0 {
            -20.0
        } else {
            0.0
        };
        let mut italic = if (style_bits & 0x000F) == Slant::Italic as usize {
            1.0
        } else {
            0.0
        };

        loop {
            let id = read_id(stream);
            if id == tag::SENTINEL {
                break;
            }
            match id {
                tag::FONT_FAMILY_NAME => result.family_name = read_string(stream)?,
                tag::FULL_NAME => result.full_name = read_string(stream)?,
                tag::POSTSCRIPT_NAME => result.postscript_name = read_string(stream)?,
                tag::WEIGHT => weight = stream.read_scalar()?,
                tag::WIDTH => width = stream.read_scalar()?,
                tag::SLANT => slant = stream.read_scalar()?,
                tag::ITALIC => italic = stream.read_scalar()?,
                tag::FONT_VARIATION => {
                    let count = i32::try_from(stream.read_packed_uint()?).ok()?;
                    let count = usize::try_from(count).ok()?;
                    if remaining_length_is_below(stream, count) {
                        return None;
                    }
                    result.variation = vec![Coordinate::default(); count];
                    for coordinate in &mut result.variation {
                        coordinate.axis = stream.read_u32()?;
                        coordinate.value = stream.read_scalar()?;
                    }
                }
                tag::FONT_INDEX => {
                    result.collection_index = i32::try_from(stream.read_packed_uint()?).ok()?;
                }
                tag::PALETTE_INDEX => {
                    result.palette_index = i32::try_from(stream.read_packed_uint()?).ok()?;
                }
                tag::PALETTE_ENTRY_OVERRIDES => {
                    let count = i32::try_from(stream.read_packed_uint()?).ok()?;
                    let count = usize::try_from(count).ok()?;
                    if remaining_length_is_below(stream, count) {
                        return None;
                    }
                    result.palette_entry_overrides = vec![Override::default(); count];
                    for entry in &mut result.palette_entry_overrides {
                        entry.index = u16::try_from(stream.read_packed_uint()?).ok()?;
                        entry.color = crate::color::Color::new(stream.read_u32()?);
                    }
                }
                tag::SYNTHETIC_BOLD => result.synthetic_bold = true,
                tag::SYNTHETIC_OBLIQUE => result.synthetic_oblique = true,
                tag::FACTORY_ID => {
                    result.factory_id = u32::try_from(stream.read_packed_uint()?).ok()?;
                }
                // Unknown id (including INVALID): the C++ debug build fails here, and it returns
                // false in every build.
                _ => return None,
            }
        }

        let mut slant_enum = Slant::Upright;
        if slant != 0.0 {
            slant_enum = Slant::Oblique;
        }
        if 0.0 < italic {
            slant_enum = Slant::Italic;
        }
        let width_enum = Self::style_width_for_width_axis_value(width);
        result.style = FontStyle::new(
            Weight::from(scalar_round_to_int(weight)),
            width_enum,
            slant_enum,
        );

        let length = stream.read_packed_uint()?;
        if length > 0 {
            if remaining_length_is_below(stream, length) {
                return None;
            }
            let mut buffer = vec![0u8; length];
            if stream.read(&mut buffer) != length {
                return None;
            }
            let mut data = Data::new_copy(&buffer);
            if let Some(sanitize) = sanitizer {
                data = sanitize(data)?;
            }
            result.stream = Some(MemoryStream::make(Some(data)));
        }
        Some(result)
    }
}

/// `read_id`: the next packed id, or `INVALID` if it cannot be read.
// Port of: src/core/SkFontDescriptor.cpp#L68-L72 (chrome/m156)
fn read_id(stream: &mut dyn Stream) -> usize {
    stream.read_packed_uint().unwrap_or(tag::INVALID)
}

/// `read_string`: a packed length, then that many bytes. Non-UTF-8 bytes are replaced.
// Port of: src/core/SkFontDescriptor.cpp#L74-L85 (chrome/m156)
fn read_string(stream: &mut dyn Stream) -> Option<String> {
    let length = stream.read_packed_uint()?;
    let mut string = String::new();
    if length > 0 {
        if remaining_length_is_below(stream, length) {
            return None;
        }
        let mut buffer = vec![0u8; length];
        if stream.read(&mut buffer) != length {
            return None;
        }
        string = String::from_utf8_lossy(&buffer).into_owned();
    }
    Some(string)
}

/// `write_id`: the packed id.
// Port of: src/core/SkFontDescriptor.cpp#L48-L50 (chrome/m156)
fn write_id(stream: &mut dyn WStream, id: usize) -> bool {
    stream.write_packed_uint(id)
}

/// `write_string`: nothing for an empty string, otherwise the id, a packed length and the bytes.
// Port of: src/core/SkFontDescriptor.cpp#L51-L57 (chrome/m156)
fn write_string(stream: &mut dyn WStream, string: &str, id: usize) -> bool {
    if string.is_empty() {
        return true;
    }
    write_id(stream, id)
        && stream.write_packed_uint(string.len())
        && stream.write(string.as_bytes())
}

/// `write_uint`: the id, then the packed value.
// Port of: src/core/SkFontDescriptor.cpp#L58-L62 (chrome/m156)
fn write_uint(stream: &mut dyn WStream, n: usize, id: usize) -> bool {
    write_id(stream, id) && stream.write_packed_uint(n)
}

/// `write_scalar`: the id, then the scalar.
// Port of: src/core/SkFontDescriptor.cpp#L63-L66 (chrome/m156)
fn write_scalar(stream: &mut dyn WStream, n: scalar, id: usize) -> bool {
    write_id(stream, id) && stream.write_scalar(n)
}

/// Exact for the weights and widths written here (both well inside `f32`'s integer range).
#[allow(clippy::cast_precision_loss)] // weights are at most 1000, exact in f32
fn scalar_from_i32(value: i32) -> scalar {
    value as scalar
}
