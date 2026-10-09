// Copyright 2014 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkImageGenerator.h, src/core/SkImageGenerator.cpp

//! `SkImageGenerator`: the source of pixels for a lazy image ([`ImageGenerator`]).
//!
//! A generator decodes or draws the pixels of an image on demand, and is wrapped in a lazy image
//! ([`crate::image_lazy`]) by [`crate::images::deferred_from_generator`]. The codec generators
//! (`SkCodecImageGenerator`) live in `skia-rust-codec`, so this crate never depends on it.
//!
//! skia-rust: the YUVA planes API (`queryYUVAInfo`, `getYUVAPlanes`, `SkYUVAPixmaps`) and the
//! recorder argument of `isValid` are not ported; the picture generator
//! (`SkImageGenerators::MakeFromPicture`) is not ported.

use crate::color_type::ColorType;
use crate::data::Data;
use crate::image_base::NEED_NEW_IMAGE_UNIQUE_ID;
use crate::image_info::ImageInfo;
use crate::pixel_ref::next_image_id;
use crate::pixmap::Pixmap;

/// The unique ID a generator gets: `uniqueID` if it is not `kNeedNewImageUniqueID`, else a new
/// one (the id selection of the `SkImageGenerator` constructor).
// Port of: src/core/SkImageGenerator.cpp#L14-L17 (chrome/m156)
#[must_use]
pub fn generator_unique_id(unique_id: u32) -> u32 {
    if NEED_NEW_IMAGE_UNIQUE_ID == unique_id {
        next_image_id()
    } else {
        unique_id
    }
}

/// The virtual interface of an image generator (`SkImageGenerator`).
///
/// Implementors store the info and unique ID they were built with (see
/// [`generator_unique_id`]) and return them from [`info`](Self::info) and
/// [`unique_id`](Self::unique_id). `Send` is required so a generator can back a lazy image
/// shared between threads.
// Port of: include/core/SkImageGenerator.h#L23-L127 (chrome/m156)
#[doc(alias = "SkImageGenerator")]
pub trait ImageGenerator: Send {
    /// The info of the pixels the generator makes (`getInfo`).
    #[doc(alias = "getInfo")]
    fn info(&self) -> &ImageInfo;

    /// The unique ID of the pixels the generator makes (`uniqueID`).
    #[doc(alias = "uniqueID")]
    fn unique_id(&self) -> u32;

    /// The encoded data the pixels were made from, if any (`refEncodedData`,
    /// `onRefEncodedData`).
    // Port of: include/core/SkImageGenerator.h#L40 and #L119 (chrome/m156)
    #[doc(alias = "refEncodedData")]
    #[doc(alias = "onRefEncodedData")]
    fn ref_encoded_data(&mut self) -> Option<Data> {
        None
    }

    /// Whether the generator can make its pixels (`onIsValid`; the recorder is not ported).
    // Port of: include/core/SkImageGenerator.h#L123 (chrome/m156)
    #[doc(alias = "onIsValid")]
    fn is_valid(&self) -> bool {
        true
    }

    /// Whether the pixels are protected content (`onIsProtected`).
    // Port of: include/core/SkImageGenerator.h#L124 (chrome/m156)
    #[doc(alias = "onIsProtected")]
    fn is_protected(&self) -> bool {
        false
    }

    /// Whether the generator produces textures (`isTextureGenerator`).
    // Port of: include/core/SkImageGenerator.h#L112 (chrome/m156)
    #[doc(alias = "isTextureGenerator")]
    fn is_texture_generator(&self) -> bool {
        false
    }

    /// Makes the pixels described by `info` into `pixels` (`onGetPixels`). The caller has
    /// checked the arguments.
    // Port of: include/core/SkImageGenerator.h#L122 (chrome/m156)
    #[doc(alias = "onGetPixels")]
    fn on_get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        let _ = (info, pixels, row_bytes);
        false
    }

    /// Makes the pixels described by `info` into `pixels`, `row_bytes` apart (`getPixels`).
    ///
    /// Returns false if the color type is unknown, the buffer is empty (Skia's null pointer), or
    /// `row_bytes` is too small for a row; or if the generator fails. skia-rust: a buffer too
    /// small for the image also returns false, where Skia would write past its end.
    // Port of: src/core/SkImageGenerator.cpp#L19-L35 (chrome/m156)
    #[doc(alias = "getPixels")]
    fn get_pixels(&mut self, info: &ImageInfo, pixels: &mut [u8], row_bytes: usize) -> bool {
        if ColorType::Unknown == info.color_type() {
            return false;
        }
        if pixels.is_empty() {
            return false;
        }
        if row_bytes < info.min_row_bytes() {
            return false;
        }
        if pixels.len() < info.compute_byte_size(row_bytes) {
            return false;
        }

        self.on_get_pixels(info, pixels, row_bytes)
    }

    /// Makes the pixels of `pm` (`getPixels(const SkPixmap&)`): the info, the address and the
    /// row bytes of the pixmap.
    // Port of: include/core/SkImageGenerator.h#L84-L86 (chrome/m156)
    #[doc(alias = "getPixels")]
    fn get_pixels_into(&mut self, pm: &mut Pixmap<'_>) -> bool {
        let info = pm.info().clone();
        let row_bytes = pm.row_bytes();
        match pm.writable_addr() {
            Some(pixels) => self.get_pixels(&info, pixels, row_bytes),
            None => false,
        }
    }
}
