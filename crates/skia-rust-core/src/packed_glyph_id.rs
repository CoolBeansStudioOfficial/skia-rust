// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkGlyph.h (SkPackedGlyphID)

//! [`PackedGlyphId`]: a glyph index together with its sub-pixel position, packed into 20 bits
//! (`SkPackedGlyphID`).
//!
//! Layout, from the least significant bit: 2 bits of x sub-pixel position, 2 bits of y sub-pixel
//! position is at bit 18, the 16-bit glyph id at bit 2 (`kSubPixelX`, `kGlyphID`, `kSubPixelY`).

use crate::checksum::cheap_mix;
use crate::fixed::Fixed;
use crate::font_types::GlyphId;
use crate::point::{IPoint, Point};

/// `SkPackedGlyphID::kGlyphIDLen`.
const GLYPH_ID_LEN: u32 = 16;
/// `SkPackedGlyphID::kSubPixelPosLen`.
const SUB_PIXEL_POS_LEN: u32 = 2;
/// `SkPackedGlyphID::kSubPixelX`.
const SUB_PIXEL_X: u32 = 0;
/// `SkPackedGlyphID::kGlyphID`.
const GLYPH_ID: u32 = SUB_PIXEL_POS_LEN;
/// `SkPackedGlyphID::kSubPixelY`.
const SUB_PIXEL_Y: u32 = GLYPH_ID_LEN + SUB_PIXEL_POS_LEN;
/// `SkPackedGlyphID::kEndData`: the number of bits used.
const END_DATA: u32 = GLYPH_ID_LEN + 2 * SUB_PIXEL_POS_LEN;
/// `SkPackedGlyphID::kGlyphIDMask`.
const GLYPH_ID_MASK: u32 = (1 << GLYPH_ID_LEN) - 1;
/// `SkPackedGlyphID::kSubPixelPosMask`.
const SUB_PIXEL_POS_MASK: u32 = (1 << SUB_PIXEL_POS_LEN) - 1;
/// `SkPackedGlyphID::kMaskAll`.
const MASK_ALL: u32 = (1 << END_DATA) - 1;
/// `SkPackedGlyphID::kFixedPointBinaryPointPos`.
const FIXED_POINT_BINARY_POINT_POS: u32 = 16;
/// `SkPackedGlyphID::kFixedPointSubPixelPosBits`.
const FIXED_POINT_SUB_PIXEL_POS_BITS: u32 = FIXED_POINT_BINARY_POINT_POS - SUB_PIXEL_POS_LEN;

/// A glyph id and its sub-pixel x and y position, in one `u32` (`SkPackedGlyphID`).
///
/// The default value is `kImpossibleID`, which no glyph has.
// Port of: src/core/SkGlyph.h#L46-L213 (chrome/m156)
#[doc(alias = "SkPackedGlyphID")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct PackedGlyphId {
    id: u32,
}

impl Default for PackedGlyphId {
    /// `SkPackedGlyphID()`: `kImpossibleID`.
    // Port of: src/core/SkGlyph.h#L94 (chrome/m156)
    fn default() -> Self {
        Self { id: u32::MAX }
    }
}

impl PartialOrd for PackedGlyphId {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PackedGlyphId {
    /// `SkPackedGlyphID::operator<`: compares the packed values.
    // Port of: src/core/SkGlyph.h#L102-L104 (chrome/m156)
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

impl PackedGlyphId {
    /// `kImpossibleID`: the default value.
    // Port of: src/core/SkGlyph.h#L47 (chrome/m156)
    pub const IMPOSSIBLE_ID: u32 = u32::MAX;

    /// `kSubpixelRound`: half of one sub-pixel step, as a scalar.
    // Port of: src/core/SkGlyph.h#L69-L70 (chrome/m156)
    pub const SUBPIXEL_ROUND: f32 = 0.125;

    /// `kXYFieldMask`: the sub-pixel bits of x and y.
    // Port of: src/core/SkGlyph.h#L72-L73 (chrome/m156)
    pub const XY_FIELD_MASK: IPoint = IPoint::new(
        (SUB_PIXEL_POS_MASK << SUB_PIXEL_X).cast_signed(),
        (SUB_PIXEL_POS_MASK << SUB_PIXEL_Y).cast_signed(),
    );

    /// `SkPackedGlyphID(SkGlyphID glyphID)`: the glyph with no sub-pixel offset.
    // Port of: src/core/SkGlyph.h#L81-L82 (chrome/m156)
    #[must_use]
    pub const fn from_glyph_id(glyph_id: GlyphId) -> Self {
        Self {
            id: (glyph_id as u32) << GLYPH_ID,
        }
    }

    /// `SkPackedGlyphID(SkGlyphID glyphID, Fixed x, Fixed y)`: takes the sub-pixel bits from
    /// the top of the fractional part of the fixed-point positions.
    // Port of: src/core/SkGlyph.h#L84-L85 (chrome/m156)
    #[must_use]
    pub const fn from_fixed(glyph_id: GlyphId, x: Fixed, y: Fixed) -> Self {
        Self {
            id: Self::pack_sub_x_sub_y(glyph_id, Self::fixed_to_sub(x), Self::fixed_to_sub(y)),
        }
    }

    /// `SkPackedGlyphID(SkGlyphID glyphID, uint32_t x, uint32_t y)`: the sub-pixel positions are
    /// already in `0..4`.
    // Port of: src/core/SkGlyph.h#L87-L88 (chrome/m156)
    #[must_use]
    pub const fn from_sub_pixel(glyph_id: GlyphId, x: u32, y: u32) -> Self {
        Self {
            id: Self::pack_sub_x_sub_y(glyph_id, x, y),
        }
    }

    /// `SkPackedGlyphID(SkGlyphID glyphID, SkPoint pt, SkIPoint mask)`: the sub-pixel bits of a
    /// device point, masked by `mask`.
    // Port of: src/core/SkGlyph.h#L90-L91 (chrome/m156)
    #[must_use]
    pub fn from_point(glyph_id: GlyphId, pt: Point, mask: IPoint) -> Self {
        Self {
            id: pack_id_sk_point(glyph_id, pt, mask),
        }
    }

    /// `SkPackedGlyphID(uint32_t v)`: keeps the low 20 bits of `v`.
    // Port of: src/core/SkGlyph.h#L93 (chrome/m156)
    #[must_use]
    pub const fn from_raw(v: u32) -> Self {
        Self { id: v & MASK_ALL }
    }

    /// `SkPackedGlyphID::glyphID`.
    // Port of: src/core/SkGlyph.h#L106-L108 (chrome/m156)
    #[must_use]
    pub const fn glyph_id(self) -> GlyphId {
        ((self.id >> GLYPH_ID) & GLYPH_ID_MASK) as GlyphId
    }

    /// `SkPackedGlyphID::value`: the packed bits.
    // Port of: src/core/SkGlyph.h#L110-L112 (chrome/m156)
    #[must_use]
    pub const fn value(self) -> u32 {
        self.id
    }

    /// `SkPackedGlyphID::getSubXFixed`: the sub-pixel x position as a fixed-point fraction.
    // Port of: src/core/SkGlyph.h#L114-L116 (chrome/m156)
    #[must_use]
    pub const fn sub_x_fixed(self) -> Fixed {
        self.sub_to_fixed(SUB_PIXEL_X)
    }

    /// `SkPackedGlyphID::getSubYFixed`: the sub-pixel y position as a fixed-point fraction.
    // Port of: src/core/SkGlyph.h#L118-L120 (chrome/m156)
    #[must_use]
    pub const fn sub_y_fixed(self) -> Fixed {
        self.sub_to_fixed(SUB_PIXEL_Y)
    }

    /// `SkPackedGlyphID::hash`: a cheap mix of the packed value.
    // Port of: src/core/SkGlyph.h#L122-L124 (chrome/m156)
    #[must_use]
    pub fn hash(self) -> u32 {
        cheap_mix(self.id)
    }

    /// `SkPackedGlyphID::shortDump` as `"0x<glyph>|<x>|<y>"`.
    // Port of: src/core/SkGlyph.h#L132-L138 (chrome/m156)
    #[must_use]
    pub fn short_dump(self) -> String {
        format!(
            "0x{:x}|{}|{}",
            self.glyph_id(),
            self.sub_pixel_field(SUB_PIXEL_X),
            self.sub_pixel_field(SUB_PIXEL_Y)
        )
    }

    /// `PackIDSubXSubY`.
    // Port of: src/core/SkGlyph.h#L141-L146 (chrome/m156)
    const fn pack_sub_x_sub_y(glyph_id: GlyphId, x: u32, y: u32) -> u32 {
        debug_assert!(x < (1 << SUB_PIXEL_POS_LEN));
        debug_assert!(y < (1 << SUB_PIXEL_POS_LEN));
        (x << SUB_PIXEL_X) | (y << SUB_PIXEL_Y) | ((glyph_id as u32) << GLYPH_ID)
    }

    /// `FixedToSub`: the top two fractional bits of a fixed-point position.
    // Port of: src/core/SkGlyph.h#L199-L201 (chrome/m156)
    const fn fixed_to_sub(n: Fixed) -> u32 {
        (n.cast_unsigned() >> FIXED_POINT_SUB_PIXEL_POS_BITS) & SUB_PIXEL_POS_MASK
    }

    /// `subPixelField`.
    // Port of: src/core/SkGlyph.h#L203-L205 (chrome/m156)
    const fn sub_pixel_field(self, sub_pixel_pos_bit: u32) -> u32 {
        (self.id >> sub_pixel_pos_bit) & SUB_PIXEL_POS_MASK
    }

    /// `subToFixed`.
    // Port of: src/core/SkGlyph.h#L207-L210 (chrome/m156)
    const fn sub_to_fixed(self, sub_pixel_pos_bit: u32) -> Fixed {
        let sub_pixel_position = self.sub_pixel_field(sub_pixel_pos_bit);
        (sub_pixel_position << FIXED_POINT_SUB_PIXEL_POS_BITS).cast_signed()
    }
}

/// `SkPackedGlyphID::PackIDSkPoint`: the sub-pixel bits of a point, with the fractional part
/// biased into `[1, 2)` so the truncation picks the right quarter.
// Port of: src/core/SkGlyph.h#L164-L193 (chrome/m156)
#[allow(clippy::cast_possible_truncation)] // (int)(float) truncation, as in C++
fn pack_id_sk_point(glyph_id: GlyphId, pt: Point, mask: IPoint) -> u32 {
    // 1.f * (1u << (kSubPixelPosLen + kSubPixelX)) and the same for y: exact powers of two.
    let magic_x = 4.0_f32;
    let magic_y = 1_048_576.0_f32;
    let mut x = pt.x;
    let mut y = pt.y;
    x = (x - x.floor()) + 1.0;
    y = (y - y.floor()) + 1.0;
    let sub = [
        ((x * magic_x) as i32) & mask.x,
        ((y * magic_y) as i32) & mask.y,
    ];
    #[allow(clippy::cast_sign_loss)] // the masked values are non-negative
    {
        debug_assert!(((sub[0] as u32) >> SUB_PIXEL_X) < (1 << SUB_PIXEL_POS_LEN));
        debug_assert!(((sub[1] as u32) >> SUB_PIXEL_Y) < (1 << SUB_PIXEL_POS_LEN));
        ((u32::from(glyph_id)) << GLYPH_ID) | (sub[0] as u32) | (sub[1] as u32)
    }
}
