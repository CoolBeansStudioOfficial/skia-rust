// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/DataUtils.h, src/gpu/DataUtils.cpp (the compressed texture sizes)

//! Block size helpers for compressed texture data (`skgpu::CompressedRowBytes`,
//! `CompressedDimensions` and `CompressedDimensionsInBlocks`). The ETC1/BC1 decoders of
//! `DataUtils` are not ported yet.

use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

/// `sizeof(ETC1Block)` and `sizeof(BC1Block)`.
// Port of: src/core/SkCompressedDataUtils.cpp#L28-L31 (chrome/m156)
const BLOCK_SIZE: usize = 8;

// Port of: src/gpu/DataUtils.cpp#L68-L70 (chrome/m156)
fn num_4x4_blocks(size: i32) -> i32 {
    ((size + 3) & !3) >> 2
}

/// `CompressedRowBytes`: the bytes in a row of `width` pixels of compressed blocks.
// Port of: src/gpu/DataUtils.cpp#L181-L195 (chrome/m156)
#[doc(alias = "CompressedRowBytes")]
#[allow(clippy::cast_sign_loss)] // numBlocksWidth is non-negative for a valid width
#[must_use]
pub fn compressed_row_bytes(ty: TextureCompressionType, width: i32) -> usize {
    match ty {
        TextureCompressionType::None => 0,
        TextureCompressionType::ETC2_RGB8_UNORM
        | TextureCompressionType::BC1_RGB8_UNORM
        | TextureCompressionType::BC1_RGBA8_UNORM => {
            let num_blocks_width = num_4x4_blocks(width);
            num_blocks_width as usize * BLOCK_SIZE
        }
    }
}

/// `CompressedDimensions`: the dimensions rounded up to whole blocks, in pixels.
// Port of: src/gpu/DataUtils.cpp#L197-L210 (chrome/m156)
#[doc(alias = "CompressedDimensions")]
#[must_use]
pub fn compressed_dimensions(ty: TextureCompressionType, base_dimensions: ISize) -> ISize {
    match ty {
        TextureCompressionType::None => base_dimensions,
        TextureCompressionType::ETC2_RGB8_UNORM
        | TextureCompressionType::BC1_RGB8_UNORM
        | TextureCompressionType::BC1_RGBA8_UNORM => {
            let block_dims = compressed_dimensions_in_blocks(ty, base_dimensions);
            // Each BC1_RGB8_UNORM and ETC1 block has 16 pixels
            ISize::new(4 * block_dims.width, 4 * block_dims.height)
        }
    }
}

/// `CompressedDimensionsInBlocks`: the dimensions in blocks (the dimensions themselves if the
/// type is `None`).
// Port of: src/gpu/DataUtils.cpp#L212-L225 (chrome/m156)
#[doc(alias = "CompressedDimensionsInBlocks")]
#[must_use]
pub fn compressed_dimensions_in_blocks(
    ty: TextureCompressionType,
    base_dimensions: ISize,
) -> ISize {
    match ty {
        TextureCompressionType::None => base_dimensions,
        TextureCompressionType::ETC2_RGB8_UNORM
        | TextureCompressionType::BC1_RGB8_UNORM
        | TextureCompressionType::BC1_RGBA8_UNORM => {
            let num_blocks_width = num_4x4_blocks(base_dimensions.width);
            let num_blocks_height = num_4x4_blocks(base_dimensions.height);

            // Each BC1_RGB8_UNORM and ETC1 block has 16 pixels
            ISize::new(num_blocks_width, num_blocks_height)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_helpers() {
        let bc1 = TextureCompressionType::BC1_RGB8_UNORM;
        assert_eq!(compressed_row_bytes(bc1, 5), 16);
        assert_eq!(compressed_row_bytes(TextureCompressionType::None, 5), 0);
        assert_eq!(
            compressed_dimensions_in_blocks(bc1, ISize::new(5, 9)),
            ISize::new(2, 3)
        );
        assert_eq!(
            compressed_dimensions(bc1, ISize::new(5, 9)),
            ISize::new(8, 12)
        );
        assert_eq!(
            compressed_dimensions(TextureCompressionType::None, ISize::new(5, 9)),
            ISize::new(5, 9)
        );
    }
}
