// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCompressedDataUtils.h, src/core/SkCompressedDataUtils.cpp

//! Size computations for compressed texture data (`SkCompressedDataSize` and friends) and the CPU
//! decoder for ETC2/ETC1 RGB8 and BC1 RGB8/RGBA8 (`SkDecompress`).

use crate::bitmap::Bitmap;
use crate::color::PMColor;
use crate::color_data::{b16_to_b32, g16_to_g32, r16_to_r32};
use crate::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
};
use crate::mipmap::Mipmap;
use crate::pixmap::Pixmap;
use crate::scalar::scalar_round_to_int;
use crate::size::ISize;
use crate::texture_compression_type::TextureCompressionType;

/// `sizeof(ETC1Block)` (two `uint32_t`s).
// Port of: src/core/SkCompressedDataUtils.cpp#L28-L31 (chrome/m156)
const ETC1_BLOCK_SIZE: usize = 8;
/// `sizeof(BC1Block)` (two `uint16_t`s and a `uint32_t`).
// Port of: src/core/SkCompressedDataUtils.cpp#L165-L169 (chrome/m156)
const BC1_BLOCK_SIZE: usize = 8;

// static_assert(sizeof(ETC1Block) == sizeof(BC1Block));
const _: () = assert!(ETC1_BLOCK_SIZE == BC1_BLOCK_SIZE);

// Port of: src/core/SkCompressedDataUtils.cpp#L68-L70 (chrome/m156)
fn num_4x4_blocks(size: i32) -> i32 {
    ((size + 3) & !3) >> 2
}

/// `SkCompressedDataSize`: the total size of the compressed data, optionally with every mip level.
///
/// When `individual_mip_offsets` is given it receives the offset of each level.
// Port of: src/core/SkCompressedDataUtils.cpp#L255-L289 (chrome/m156)
#[doc(alias = "SkCompressedDataSize")]
#[allow(clippy::cast_sign_loss)] // numBlocks is non-negative for valid dimensions, as in Skia
#[must_use]
pub fn compressed_data_size(
    ty: TextureCompressionType,
    mut dimensions: ISize,
    mut individual_mip_offsets: Option<&mut Vec<usize>>,
    mipmapped: bool,
) -> usize {
    debug_assert!(individual_mip_offsets.as_ref().is_none_or(|o| o.is_empty()));

    let mut num_mip_levels = 1;
    if mipmapped {
        num_mip_levels = Mipmap::compute_level_count_size(dimensions) + 1;
    }

    let mut total_size = 0usize;
    match ty {
        TextureCompressionType::None => {}
        TextureCompressionType::ETC2_RGB8_UNORM
        | TextureCompressionType::BC1_RGB8_UNORM
        | TextureCompressionType::BC1_RGBA8_UNORM => {
            for _ in 0..num_mip_levels {
                let num_blocks =
                    num_4x4_blocks(dimensions.width) * num_4x4_blocks(dimensions.height);

                if let Some(offsets) = individual_mip_offsets.as_deref_mut() {
                    offsets.push(total_size);
                }

                total_size += num_blocks as usize * ETC1_BLOCK_SIZE;

                dimensions = ISize::new(1.max(dimensions.width / 2), 1.max(dimensions.height / 2));
            }
        }
    }

    total_size
}

/// `SkCompressedBlockSize`: the size of one 4x4 block.
// Port of: src/core/SkCompressedDataUtils.cpp#L291-L302 (chrome/m156)
#[doc(alias = "SkCompressedBlockSize")]
#[must_use]
pub fn compressed_block_size(ty: TextureCompressionType) -> usize {
    match ty {
        TextureCompressionType::None => 0,
        TextureCompressionType::ETC2_RGB8_UNORM => ETC1_BLOCK_SIZE,
        TextureCompressionType::BC1_RGB8_UNORM | TextureCompressionType::BC1_RGBA8_UNORM => {
            BC1_BLOCK_SIZE
        }
    }
}

/// `SkCompressedFormatDataSize`: [`compressed_data_size`] without the mip offsets.
// Port of: src/core/SkCompressedDataUtils.cpp#L304-L307 (chrome/m156)
#[doc(alias = "SkCompressedFormatDataSize")]
#[must_use]
pub fn compressed_format_data_size(
    compression_type: TextureCompressionType,
    dimensions: ISize,
    mipmapped: bool,
) -> usize {
    compressed_data_size(compression_type, dimensions, None, mipmapped)
}

/// `SkTextureCompressionTypeIsOpaque`: whether the format has no alpha channel.
// Port of: src/core/SkCompressedDataUtils.h#L22-L31 (chrome/m156)
#[doc(alias = "SkTextureCompressionTypeIsOpaque")]
#[must_use]
pub(crate) fn texture_compression_type_is_opaque(ty: TextureCompressionType) -> bool {
    match ty {
        TextureCompressionType::None
        | TextureCompressionType::ETC2_RGB8_UNORM
        | TextureCompressionType::BC1_RGB8_UNORM => true,
        TextureCompressionType::BC1_RGBA8_UNORM => false,
    }
}

// Port of: src/core/SkCompressedDataUtils.cpp#L22-L26 (chrome/m156)
const FLIP_BIT: u32 = 0x1; // set -> T/B sub-blocks; not-set -> L/R sub-blocks
const DIFF_BIT: u32 = 0x2; // set -> differential; not-set -> individual

// Port of: src/core/SkCompressedDataUtils.cpp#L36-L39 (chrome/m156)
fn extend_4_to_8_bits(b: i32) -> i32 {
    let c = b & 0xf;
    (c << 4) | c
}

// Port of: src/core/SkCompressedDataUtils.cpp#L41-L44 (chrome/m156)
fn extend_5_to_8_bits(b: i32) -> i32 {
    let c = b & 0x1f;
    (c << 3) | (c >> 2)
}

// Port of: src/core/SkCompressedDataUtils.cpp#L46-L50 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // `0x7 & diff` is in 0..=7
fn extend_5plus3_to_8_bits(base: i32, diff: i32) -> i32 {
    const LOOKUP: [i32; 8] = [0, 1, 2, 3, -4, -3, -2, -1];

    extend_5_to_8_bits((0x1f & base) + LOOKUP[(0x7 & diff) as usize])
}

// Port of: src/core/SkCompressedDataUtils.cpp#L57-L66 (chrome/m156)
const ETC1_MODIFIER_TABLES: [[i32; 4]; 8] = [
    [2, 8, -2, -8],
    [5, 17, -5, -17],
    [9, 29, -9, -29],
    [13, 42, -13, -42],
    [18, 60, -18, -60],
    [24, 80, -24, -80],
    [33, 106, -33, -106],
    [47, 183, -47, -183],
];

// Port of: src/core/SkCompressedDataUtils.cpp#L73-L82 (chrome/m156)
fn xy_to_subblock_index(x: i32, y: i32, flip: bool) -> usize {
    debug_assert!((0..4).contains(&x) && (0..4).contains(&y));

    if flip {
        // sub-block 1 is on top of sub-block 2
        usize::from(y >= 2)
    } else {
        // sub-block 1 is to the left of sub-block 2
        usize::from(x >= 2)
    }
}

// Port of: src/core/SkCompressedDataUtils.cpp#L84-L94 (chrome/m156), `add_delta_and_clamp`
#[allow(clippy::cast_sign_loss)] // each channel is pinned to 0..=255 first
fn add_delta_and_clamp(col: [i32; 3], delta: i32) -> PMColor {
    let r8 = (col[0] + delta).clamp(0, 255) as u32;
    let g8 = (col[1] + delta).clamp(0, 255) as u32;
    let b8 = (col[2] + delta).clamp(0, 255) as u32;

    pack_argb32(0xFF, r8, g8, b8)
}

// Port of: src/core/SkCompressedDataUtils.cpp#L96-L162 (chrome/m156), `decompress_etc1`
#[allow(
    clippy::cast_possible_wrap, // the shifted operands are at most 31, so they fit in i32
    clippy::cast_sign_loss, // block and sub-block indices are non-negative
    clippy::similar_names, // the names are SkDecompress's
)]
fn decompress_etc1(dimensions: ISize, src: &[u8], dst: &mut Pixmap<'_>) -> bool {
    let num_x_blocks = num_4x4_blocks(dimensions.width);
    let num_y_blocks = num_4x4_blocks(dimensions.height);

    for y in 0..num_y_blocks {
        for x in 0..num_x_blocks {
            // Skia reads each `uint32_t` in host order and byte-swaps it, which is the
            // big-endian reading of the block's bytes.
            let at = (y * num_x_blocks + x) as usize * 8;
            let high = u32::from_be_bytes(src[at..at + 4].try_into().expect("4 bytes"));
            let low = u32::from_be_bytes(src[at + 4..at + 8].try_into().expect("4 bytes"));

            let flipped = high & FLIP_BIT != 0;
            let differential = high & DIFF_BIT != 0;

            // IColor { fR, fG, fB }
            let mut colors = [[0i32; 3]; 2];

            if differential {
                colors[0][0] = extend_5_to_8_bits((high >> 27) as i32);
                colors[1][0] = extend_5plus3_to_8_bits((high >> 27) as i32, (high >> 24) as i32);
                colors[0][1] = extend_5_to_8_bits((high >> 19) as i32);
                colors[1][1] = extend_5plus3_to_8_bits((high >> 19) as i32, (high >> 16) as i32);
                colors[0][2] = extend_5_to_8_bits((high >> 11) as i32);
                colors[1][2] = extend_5plus3_to_8_bits((high >> 11) as i32, (high >> 8) as i32);
            } else {
                colors[0][0] = extend_4_to_8_bits((high >> 28) as i32);
                colors[1][0] = extend_4_to_8_bits((high >> 24) as i32);
                colors[0][1] = extend_4_to_8_bits((high >> 20) as i32);
                colors[1][1] = extend_4_to_8_bits((high >> 16) as i32);
                colors[0][2] = extend_4_to_8_bits((high >> 12) as i32);
                colors[1][2] = extend_4_to_8_bits((high >> 8) as i32);
            }

            let table_index0 = ((high >> 5) & 0x7) as usize;
            let table_index1 = ((high >> 2) & 0x7) as usize;
            let tables = [
                ETC1_MODIFIER_TABLES[table_index0],
                ETC1_MODIFIER_TABLES[table_index1],
            ];

            let offset_x = 4 * x;
            let offset_y = 4 * y;
            for i in 0..4 {
                // `baseShift` in Skia advances with `i`, so it always equals `i` here.
                let base_shift = i as u32;
                for j in 0..4 {
                    if offset_x + j >= dst.width() || offset_y + i >= dst.height() {
                        // This can happen for the topmost levels of a mipmap and for
                        // non-multiple of 4 textures
                        continue;
                    }

                    let sub_block_index = xy_to_subblock_index(j, i, flipped);
                    let k = base_shift + (j as u32) * 4;
                    let pixel_index = ((low >> k) & 0x1) | ((low >> (k + 15)) & 0x2);

                    let delta = tables[sub_block_index][pixel_index as usize];
                    dst.set_addr32(
                        offset_x + j,
                        offset_y + i,
                        add_delta_and_clamp(colors[sub_block_index], delta),
                    );
                }
            }
        }
    }

    true
}

// Port of: src/core/SkCompressedDataUtils.cpp#L168-L177 (chrome/m156), `from565`
fn from_565(rgb565: u16) -> PMColor {
    let r8 = r16_to_r32(u32::from((rgb565 >> 11) & 0x1F));
    let g8 = g16_to_g32(u32::from((rgb565 >> 5) & 0x3F));
    let b8 = b16_to_b32(u32::from(rgb565 & 0x1F));

    pack_argb32(0xFF, r8, g8, b8)
}

// Port of: src/core/SkCompressedDataUtils.cpp#L179-L188 (chrome/m156), `lerp`:
// return t*col0 + (1-t)*col1
#[allow(
    clippy::cast_precision_loss, // channels are 8-bit, exact in f32
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss, // rounded channels are in 0..=255
)]
fn lerp(t: f32, col0: PMColor, col1: PMColor) -> PMColor {
    debug_assert_eq!(get_packed_a32(col0), 0xFF);
    debug_assert_eq!(get_packed_a32(col1), 0xFF);

    // `SkScalarRoundToInt` of a convex combination of two 8-bit channels is in 0..=255.
    let r8 = scalar_round_to_int(
        t * get_packed_r32(col0) as f32 + (1.0 - t) * get_packed_r32(col1) as f32,
    ) as u32;
    let g8 = scalar_round_to_int(
        t * get_packed_g32(col0) as f32 + (1.0 - t) * get_packed_g32(col1) as f32,
    ) as u32;
    let b8 = scalar_round_to_int(
        t * get_packed_b32(col0) as f32 + (1.0 - t) * get_packed_b32(col1) as f32,
    ) as u32;
    pack_argb32(0xFF, r8, g8, b8)
}

// Port of: src/core/SkCompressedDataUtils.cpp#L190-L236 (chrome/m156), `decompress_bc1`
#[allow(clippy::cast_sign_loss, clippy::similar_names)] // block indices are non-negative; SkDecompress's names
fn decompress_bc1(dimensions: ISize, src: &[u8], is_opaque: bool, dst: &mut Pixmap<'_>) -> bool {
    let num_x_blocks = num_4x4_blocks(dimensions.width);
    let num_y_blocks = num_4x4_blocks(dimensions.height);

    for y in 0..num_y_blocks {
        for x in 0..num_x_blocks {
            // `BC1Block` is read in host order; every supported target is little-endian and the
            // encoder writes the same layout.
            let at = (y * num_x_blocks + x) as usize * 8;
            let color0 = u16::from_le_bytes([src[at], src[at + 1]]);
            let color1 = u16::from_le_bytes([src[at + 2], src[at + 3]]);
            let indices = u32::from_le_bytes(src[at + 4..at + 8].try_into().expect("4 bytes"));

            let mut colors = [0u32; 4];
            colors[0] = from_565(color0);
            colors[1] = from_565(color1);
            if color0 <= color1 {
                // signal for a transparent block
                // `(a + b) >> 1` of two 8-bit channels: `u32::midpoint` rounds down the same way.
                let r = u32::midpoint(get_packed_r32(colors[0]), get_packed_r32(colors[1]));
                let g = u32::midpoint(get_packed_g32(colors[0]), get_packed_g32(colors[1]));
                let b = u32::midpoint(get_packed_b32(colors[0]), get_packed_b32(colors[1]));
                colors[2] = pack_argb32(0xFF, r, g, b);
                // The opacity of the overall texture trumps the per-block transparency
                colors[3] = pack_argb32(if is_opaque { 0xFF } else { 0 }, 0, 0, 0);
            } else {
                colors[2] = lerp(2.0 / 3.0, colors[0], colors[1]);
                colors[3] = lerp(1.0 / 3.0, colors[0], colors[1]);
            }

            let offset_x = 4 * x;
            let offset_y = 4 * y;
            for i in 0..4 {
                for j in 0..4 {
                    // Skia advances `shift` by 2 on every (i, j), including skipped ones.
                    let shift = 2 * (4 * i + j) as u32;
                    if offset_x + j >= dst.width() || offset_y + i >= dst.height() {
                        // This can happen for the topmost levels of a mipmap and for
                        // non-multiple of 4 textures
                        continue;
                    }

                    let index = (indices >> shift) & 0x3;
                    dst.set_addr32(offset_x + j, offset_y + i, colors[index as usize]);
                }
            }
        }
    }

    true
}

/// `SkDecompress`: decodes `data` (in `compression_type`) into `dst`, an N32 bitmap with
/// `dimensions` and writable pixels. Returns false for [`TextureCompressionType::None`].
// Port of: src/core/SkCompressedDataUtils.cpp#L238-L253 (chrome/m156)
#[doc(alias = "SkDecompress")]
#[must_use]
pub fn decompress(
    data: &[u8],
    dimensions: ISize,
    compression_type: TextureCompressionType,
    dst: &mut Bitmap,
) -> bool {
    let Some(mut pixmap) = dst.peek_pixels_mut() else {
        return false;
    };
    match compression_type {
        TextureCompressionType::None => false,
        TextureCompressionType::ETC2_RGB8_UNORM => decompress_etc1(dimensions, data, &mut pixmap),
        TextureCompressionType::BC1_RGB8_UNORM => {
            decompress_bc1(dimensions, data, true, &mut pixmap)
        }
        TextureCompressionType::BC1_RGBA8_UNORM => {
            decompress_bc1(dimensions, data, false, &mut pixmap)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_counts_round_up() {
        let t = TextureCompressionType::BC1_RGBA8_UNORM;
        assert_eq!(compressed_format_data_size(t, ISize::new(5, 4), false), 16);
        let mut offsets = Vec::new();
        // 8x8 -> 4x4 -> 2x2 -> 1x1: 4 + 1 + 1 + 1 blocks.
        let size = compressed_data_size(t, ISize::new(8, 8), Some(&mut offsets), true);
        assert_eq!(size, 7 * 8);
        assert_eq!(offsets, [0, 32, 40, 48]);
    }
}
