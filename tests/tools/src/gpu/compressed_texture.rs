// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: tools/gpu/CompressedTexture.cpp

//! `sk_gpu_test::TwoColorBC1Compress`: builds BC1 blocks for the compressed-texture GMs.

use skia_rust_core::color::Color;
use skia_rust_core::math::mul_div_255_round;
use skia_rust_core::pixmap::Pixmap;

// The four helpers are copied from src/gpu/DataUtils.cpp in Skia (see the C++ comment).

// Port of: tools/gpu/CompressedTexture.cpp#L13-L14 (chrome/m156), `num_4x4_blocks`
fn num_4x4_blocks(size: i32) -> i32 {
    ((size + 3) & !3) >> 2
}

// Port of: tools/gpu/CompressedTexture.cpp#L16-L20 (chrome/m156), `BC1Block`
#[derive(Clone, Copy, Default)]
struct BC1Block {
    color0: u16,
    color1: u16,
    indices: u32,
}

impl BC1Block {
    // Writes the block in little-endian layout, the host layout Skia uses on every target we run.
    fn write(self, dst: &mut [u8]) {
        dst[0..2].copy_from_slice(&self.color0.to_le_bytes());
        dst[2..4].copy_from_slice(&self.color1.to_le_bytes());
        dst[4..8].copy_from_slice(&self.indices.to_le_bytes());
    }
}

// Port of: tools/gpu/CompressedTexture.cpp#L22-L28 (chrome/m156), `to565`
fn to_565(col: Color) -> u16 {
    let r5 = mul_div_255_round(31, u32::from(col.r()));
    let g6 = mul_div_255_round(63, u32::from(col.g()));
    let b5 = mul_div_255_round(31, u32::from(col.b()));

    // Each channel is at most 31 or 63, so the packed value fits in 16 bits.
    u16::try_from((r5 << 11) | (g6 << 5) | b5).expect("a 16-bit 565 value")
}

// Port of: tools/gpu/CompressedTexture.cpp#L30-L45 (chrome/m156), `create_BC1_block`
fn create_bc1_block(col0: Color, col1: Color, block: &mut BC1Block) {
    block.color0 = to_565(col0);
    block.color1 = to_565(col1);
    debug_assert!(block.color0 <= block.color1); // we always assume transparent blocks

    if col0 == Color::TRANSPARENT {
        // This sets all 16 pixels to just use color3 (under the assumption
        // that this is a kBC1_RGBA8_UNORM texture. Note that in this case
        // fColor0 will be opaque black.
        block.indices = 0xFFFF_FFFF;
    } else {
        // This sets all 16 pixels to just use 'fColor0'
        block.indices = 0;
    }
}

/// Fills `dst_pixels` with BC1 blocks derived from `pixmap` (an `RGBA_8888` pixmap): black maps to
/// color 0, transparent black to color 3 (for RGBA BC1), and every other color to `other_color`
/// (color 1).
// Port of: tools/gpu/CompressedTexture.cpp#L47-L87 (chrome/m156), `TwoColorBC1Compress`
#[allow(clippy::similar_names)] // the names are Skia's
pub fn two_color_bc1_compress(pixmap: &Pixmap<'_>, other_color: Color, dst_pixels: &mut [u8]) {
    debug_assert_eq!(
        pixmap.color_type(),
        skia_rust_core::color_type::ColorType::RGBA8888
    );

    let mut block = BC1Block::default();

    // black -> fColor0, otherColor -> fColor1
    create_bc1_block(Color::BLACK, other_color, &mut block);

    let num_x_blocks = num_4x4_blocks(pixmap.width());
    let num_y_blocks = num_4x4_blocks(pixmap.height());

    for y in 0..num_y_blocks {
        for x in 0..num_x_blocks {
            let offset_x = 4 * x;
            let offset_y = 4 * y;
            block.indices = 0; // init all the pixels to color0 (i.e., opaque black)
            for i in 0..4_i32 {
                for j in 0..4_i32 {
                    // Skia advances `shift` by 2 on every (i, j), including skipped ones.
                    let shift = 2 * (4 * i + j).cast_unsigned();
                    if offset_x + j >= pixmap.width() || offset_y + i >= pixmap.height() {
                        // This can happen for the topmost levels of a mipmap and for
                        // non-multiple of 4 textures
                        continue;
                    }

                    let tmp = pixmap.get_color((offset_x + j, offset_y + i));
                    if tmp == Color::TRANSPARENT {
                        // For RGBA BC1 images color3 is set to transparent black
                        block.indices |= 3 << shift;
                    } else if tmp != Color::BLACK {
                        block.indices |= 1 << shift; // color1
                    }
                }
            }

            let at = (y * num_x_blocks + x).cast_unsigned() as usize * 8;
            block.write(&mut dst_pixels[at..at + 8]);
        }
    }
}
