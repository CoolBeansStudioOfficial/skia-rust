// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureUtils.cpp (ComputeSize only)

//! Texture helpers (`TextureUtils.h`). Only `ComputeSize`, which the resource model needs, is
//! ported so far; the image/upload helpers come with images and surfaces.

use skia_rust_core::compressed_data_utils::compressed_format_data_size;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

use crate::gpu::gpu_types::Mipmapped;
use crate::graphite::texture_format::{
    texture_format_bytes_per_block, texture_format_compression_type,
};
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};

/// `ComputeSize`: the approximate GPU memory a texture of `dimensions` described by `info` uses.
// Port of: src/gpu/graphite/TextureUtils.cpp#L370-L393 (chrome/m156)
#[doc(alias = "ComputeSize")]
#[allow(clippy::cast_sign_loss)] // mirrors the (size_t) casts on non-negative dimensions
#[must_use]
pub fn compute_size(dimensions: ISize, info: &TextureInfo) -> usize {
    let format = texture_info_priv::view_format(info);
    let compression = texture_format_compression_type(format);

    let color_size = if compression == TextureCompressionType::None {
        // TODO(b/401016699): Add logic to handle multiplanar formats
        let bytes_per_pixel = texture_format_bytes_per_block(format);

        (dimensions.width as usize)
            .wrapping_mul(dimensions.height as usize)
            .wrapping_mul(bytes_per_pixel as usize)
    } else {
        compressed_format_data_size(compression, dimensions, info.mipmapped() == Mipmapped::Yes)
    };

    // size_t arithmetic wraps in C++.
    let mut final_size = color_size.wrapping_mul(info.sample_count() as usize);

    if info.mipmapped() == Mipmapped::Yes {
        final_size = final_size.wrapping_add(color_size / 3);
    }
    final_size
}
