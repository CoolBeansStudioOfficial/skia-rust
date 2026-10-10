// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/PackedGPUGlyphID.h

//! [`PackedGpuGlyphId`]: a packed glyph id and the assumptions around how the glyph mask will be
//! used on the GPU.

use skia_rust_core::checksum::cheap_mix;
use skia_rust_core::packed_glyph_id::PackedGlyphId;

use crate::gpu::mask_format::{MASK_FORMAT_COUNT, MaskFormat};

/// `SkPackedGlyphID::kEndData`: the number of bits a packed glyph id uses.
const END_DATA: u32 = 20;

// Bit counts.
const MASK_FORMAT_BITS: u32 = 2;
const PADDING_BITS: u32 = 2; // only need 0, 1, or 2 pixels of padding currently
const SDF_BITS: u32 = 1;

// Bit offsets.
const MASK_FORMAT_OFFSET: u32 = END_DATA;
const PADDING_OFFSET: u32 = MASK_FORMAT_OFFSET + MASK_FORMAT_BITS;
const SDF_OFFSET: u32 = PADDING_OFFSET + PADDING_BITS;
const END_GPU_DATA: u32 = SDF_OFFSET + SDF_BITS;

// Masks.
const MASK_FORMAT_MASK: u32 = (1 << MASK_FORMAT_BITS) - 1;
const PADDING_MASK: u32 = (1 << PADDING_BITS) - 1;
const SDF_MASK: u32 = (1 << SDF_BITS) - 1;

const _: () = assert!(MASK_FORMAT_COUNT <= (1 << MASK_FORMAT_BITS));
const _: () = assert!(END_GPU_DATA <= 32); // Must still fit within a u32

/// A `PackedGlyphId` and the assumptions around how the glyph mask will be used on the GPU:
///  1. The mask format
///  2. What the mask data stores, e.g. coverage or distances
///  3. The amount of padding around the glyph that can be safely sampled
///
/// A `PackedGlyphId` is 20 bits, so five of the spare bits store the GPU metadata that determines
/// the full contents of a glyph entry in the atlas.
// Port of: src/text/gpu/PackedGPUGlyphID.h#L21-L87 (chrome/m156)
#[doc(alias = "PackedGPUGlyphID")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PackedGpuGlyphId {
    id: u32,
}

impl PackedGpuGlyphId {
    /// `PackedGPUGlyphID(id, format, padding, isSDF)`. `padding` must be 0, 1, or 2 and should be
    /// determined by the SubRun choice, it should never be a parameter controlled by SubRun data.
    // Port of: src/text/gpu/PackedGPUGlyphID.h#L23-L29 (chrome/m156)
    #[must_use]
    pub fn new(id: PackedGlyphId, format: MaskFormat, padding: i32, is_sdf: bool) -> Self {
        Self {
            id: id.value() | Self::pack_gpu_data(format, padding, is_sdf),
        }
    }

    /// `packedGlyphID()`. NOTE: `SkPackedGlyphID`'s raw `uint32_t` constructor masks off bits it
    /// doesn't know about.
    // Port of: src/text/gpu/PackedGPUGlyphID.h#L37-L38 (chrome/m156)
    #[must_use]
    pub fn packed_glyph_id(self) -> PackedGlyphId {
        PackedGlyphId::from_raw(self.id)
    }

    /// `maskFormat()`.
    // Port of: src/text/gpu/PackedGPUGlyphID.h#L40-L42 (chrome/m156)
    #[must_use]
    pub fn mask_format(self) -> MaskFormat {
        match (self.id >> MASK_FORMAT_OFFSET) & MASK_FORMAT_MASK {
            0 => MaskFormat::A8,
            1 => MaskFormat::A565,
            _ => MaskFormat::Argb,
        }
    }

    /// `padding()`.
    // Port of: src/text/gpu/PackedGPUGlyphID.h#L44-L46 (chrome/m156)
    #[must_use]
    pub fn padding(self) -> i32 {
        ((self.id >> PADDING_OFFSET) & PADDING_MASK).cast_signed()
    }

    /// `isSDF()`.
    // Port of: src/text/gpu/PackedGPUGlyphID.h#L48-L50 (chrome/m156)
    #[must_use]
    pub fn is_sdf(self) -> bool {
        (self.id >> SDF_OFFSET) & SDF_MASK != 0
    }

    /// This hash incorporates the GPU metadata so it will not necessarily hash to the same value
    /// as what `PackedGlyphId` hashes to by itself.
    // Port of: src/text/gpu/PackedGPUGlyphID.h#L52-L54 (chrome/m156)
    #[must_use]
    pub fn hash(self) -> u32 {
        cheap_mix(self.id)
    }

    // Port of: src/text/gpu/PackedGPUGlyphID.h#L78-L83 (chrome/m156)
    fn pack_gpu_data(format: MaskFormat, padding: i32, is_sdf: bool) -> u32 {
        debug_assert!((format as u32) < (1 << MASK_FORMAT_BITS));
        debug_assert!((0..(1 << PADDING_BITS)).contains(&padding));
        ((format as u32) << MASK_FORMAT_OFFSET)
            | (padding.cast_unsigned() << PADDING_OFFSET)
            | (u32::from(is_sdf) << SDF_OFFSET)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gpu_data_round_trips() {
        let id = PackedGlyphId::from_glyph_id(0x1234);
        for (format, padding, is_sdf) in [
            (MaskFormat::A8, 0, false),
            (MaskFormat::A565, 1, false),
            (MaskFormat::Argb, 2, false),
            (MaskFormat::A8, 2, true),
        ] {
            let packed = PackedGpuGlyphId::new(id, format, padding, is_sdf);
            assert_eq!(packed.packed_glyph_id(), id);
            assert_eq!(packed.mask_format(), format);
            assert_eq!(packed.padding(), padding);
            assert_eq!(packed.is_sdf(), is_sdf);
        }
    }

    #[test]
    fn differing_gpu_data_makes_differing_keys() {
        let id = PackedGlyphId::from_glyph_id(7);
        let a = PackedGpuGlyphId::new(id, MaskFormat::A8, 0, false);
        assert_ne!(a, PackedGpuGlyphId::new(id, MaskFormat::A8, 1, false));
        assert_ne!(a, PackedGpuGlyphId::new(id, MaskFormat::A8, 0, true));
        assert_ne!(a, PackedGpuGlyphId::new(id, MaskFormat::Argb, 0, false));
    }
}
