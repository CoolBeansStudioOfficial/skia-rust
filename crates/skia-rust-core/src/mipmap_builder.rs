// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkMipmapBuilder.{h,cpp}

//! `SkMipmapBuilder`: builds the mip levels of an image by hand, to fill them in and attach them
//! to the image with [`MipmapBuilder::attach_to`] (`SkImage::withMipmaps`).

use std::sync::Arc;

use crate::image::Image;
use crate::image_info::ImageInfo;
use crate::mipmap::Mipmap;
use crate::pixmap::Pixmap;

/// Holds the mip levels of an image of the given info (`SkMipmapBuilder`). The levels start with
/// zeroed pixels.
// Port of: src/core/SkMipmapBuilder.h#L14-L43 (chrome/m156)
#[doc(alias = "SkMipmapBuilder")]
#[derive(Debug)]
pub struct MipmapBuilder {
    mm: Option<Arc<Mipmap>>,
}

impl MipmapBuilder {
    /// Makes the levels for an image of `info`. The levels are empty (`None`) when the info has no
    /// levels (`SkMipmap::Build` returns null for a 1x1 image).
    // Port of: src/core/SkMipmapBuilder.cpp#L11-L15 (chrome/m156)
    #[doc(alias = "SkMipmapBuilder")]
    #[must_use]
    pub fn new(info: &ImageInfo) -> Self {
        // `SkMipmap::Build({info, nullptr, 0}, nullptr, false)`: the source pixels are never read
        // without computing the contents, so zeroed storage stands in for the null address.
        let row_bytes = info.min_row_bytes();
        let mut pixels = vec![0_u8; info.compute_byte_size(row_bytes)];
        let mm =
            Pixmap::new(info, &mut pixels, row_bytes).and_then(|src| Mipmap::build(&src, false));
        Self {
            mm: mm.map(Arc::new),
        }
    }

    /// The number of levels (`countLevels`).
    // Port of: src/core/SkMipmapBuilder.cpp#L21-L23 (chrome/m156)
    #[doc(alias = "countLevels")]
    #[must_use]
    pub fn count_levels(&self) -> i32 {
        self.mm.as_ref().map_or(0, |mm| mm.count_levels())
    }

    /// The writable level `index`: its info, its row bytes and its pixel bytes (`level`). `None`
    /// if the index is out of range, or if the levels are already attached to an image.
    // Port of: src/core/SkMipmapBuilder.cpp#L25-L33 (chrome/m156)
    #[doc(alias = "level")]
    pub fn level_mut(&mut self, index: i32) -> Option<(ImageInfo, usize, &mut [u8])> {
        let mm = Arc::get_mut(self.mm.as_mut()?)?;
        mm.level_pixels_mut(usize::try_from(index).ok()?)
    }

    /// Returns an image that combines `src`'s base level with these levels as its mip levels
    /// (`SkImage::withMipmaps`).
    // Port of: src/core/SkMipmapBuilder.cpp#L35-L37 (chrome/m156)
    #[doc(alias = "attachTo")]
    #[must_use]
    pub fn attach_to(&self, src: &Image) -> Image {
        src.with_mipmaps(self.mm.clone())
    }
}
