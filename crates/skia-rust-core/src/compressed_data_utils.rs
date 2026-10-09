// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkCompressedDataUtils.h, src/core/SkCompressedDataUtils.cpp (sizes)

//! Size computations for compressed texture data (`SkCompressedDataSize` and friends).
//!
//! Only the size helpers are ported so far; the ETC1/BC1 decoders come with the code that needs
//! them.

use crate::mipmap::Mipmap;
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
