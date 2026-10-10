// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/text/TextAtlasManager.h, src/gpu/graphite/text/TextAtlasManager.cpp
// (chrome/m156). Only `TextAtlasManager::AtlasConfig` is ported here; the manager itself is G12b.

//! `TextAtlasManager::AtlasConfig`: the dimensions of the glyph atlases, chosen from the caps'
//! maximum texture size and the client's byte budget for one atlas texture.

use crate::gpu::mask_format::MaskFormat;
use skia_rust_core::size::ISize;

/// `TextAtlasManager::AtlasConfig`.
#[doc(alias = "TextAtlasManager::AtlasConfig")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AtlasConfig {
    argb_dimensions: ISize,
    max_texture_size: i32,
}

impl AtlasConfig {
    /// `kMaxAtlasDim`: texture coordinates are half-precision on some systems, which limits atlas
    /// dimensions to 2048x2048.
    // Port of: src/gpu/graphite/text/TextAtlasManager.h#L54 (chrome/m156)
    const K_MAX_ATLAS_DIM: i32 = 2048;

    /// `AtlasConfig(maxTextureSize, maxBytes)`: `max_bytes` is the largest a single atlas texture
    /// should be; multitexturing may use more space temporarily.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L47-L69 (chrome/m156)
    #[must_use]
    pub fn new(max_texture_size: i32, max_bytes: usize) -> Self {
        // {width, height} for maxBytes in [2^18 << i, 2^18 << (i + 1)).
        const K_ARGB_DIMENSIONS: [(i32, i32); 6] = [
            (256, 256),
            (512, 256),
            (512, 512),
            (1024, 512),
            (1024, 1024),
            (2048, 1024),
        ];

        // Index 0 corresponds to maxBytes of 2^18, so start by dividing it by that.
        let max_bytes = max_bytes >> 18;
        // Take the floor of the log to get the index.
        let index = if max_bytes > 0 {
            let prev_log2 = (usize::BITS - 1 - max_bytes.leading_zeros()) as usize;
            prev_log2.clamp(0, K_ARGB_DIMENSIONS.len() - 1)
        } else {
            0
        };

        let (w, h) = K_ARGB_DIMENSIONS[index];
        debug_assert!(w <= Self::K_MAX_ATLAS_DIM);
        debug_assert!(h <= Self::K_MAX_ATLAS_DIM);
        AtlasConfig {
            argb_dimensions: ISize::new(w.min(max_texture_size), h.min(max_texture_size)),
            max_texture_size: max_texture_size.min(Self::K_MAX_ATLAS_DIM),
        }
    }

    /// `atlasDimensions(type)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L71-L79 (chrome/m156)
    #[must_use]
    pub fn atlas_dimensions(&self, mask_format: MaskFormat) -> ISize {
        if mask_format == MaskFormat::A8 {
            // A8 is always 2x the ARGB dimensions, clamped to the max allowed texture size.
            ISize::new(
                (2 * self.argb_dimensions.width).min(self.max_texture_size),
                (2 * self.argb_dimensions.height).min(self.max_texture_size),
            )
        } else {
            self.argb_dimensions
        }
    }

    /// `plotDimensions(type)`.
    // Port of: src/gpu/graphite/text/TextAtlasManager.cpp#L81-L98 (chrome/m156)
    #[must_use]
    pub fn plot_dimensions(&self, mask_format: MaskFormat) -> ISize {
        if mask_format == MaskFormat::A8 {
            // For A8 the plots grow at larger texture sizes, to accept the larger SDF glyphs.
            let atlas = self.atlas_dimensions(mask_format);
            // 512x256 plots for 2048x1024, 512x512 plots for 2048x2048, 256x256 otherwise.
            let plot_width = if atlas.width >= 2048 { 512 } else { 256 };
            let plot_height = if atlas.height >= 2048 { 512 } else { 256 };
            ISize::new(plot_width, plot_height)
        } else {
            // ARGB and LCD always use 256x256 plots.
            ISize::new(256, 256)
        }
    }
}
