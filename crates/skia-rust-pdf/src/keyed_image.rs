// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkBitmapKey.h, src/pdf/SkKeyedImage.{h,cpp} (chrome/m156)

//! `SkKeyedImage`: an image together with the key that identifies its pixels, so that a subset of
//! the same pixels is written to the PDF once.

use std::hash::{Hash, Hasher};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::rect::IRect;

/// `SkBitmapKey`: the pixels (by generation ID) and the subset of them an image shows.
// Port of: src/pdf/SkBitmapKey.h#L15-L22 (chrome/m156)
#[doc(alias = "SkBitmapKey")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitmapKey {
    /// `fSubset`.
    pub subset: IRect,
    /// `fID`.
    pub id: u32,
}

impl Hash for BitmapKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.subset.left.hash(state);
        self.subset.top.hash(state);
        self.subset.right.hash(state);
        self.subset.bottom.hash(state);
        self.id.hash(state);
    }
}

impl BitmapKey {
    /// `{{0, 0, 0, 0}, 0}`: the key of no image.
    pub const EMPTY: BitmapKey = BitmapKey {
        subset: IRect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        id: 0,
    };
}

/// `SkBitmapKeyFromImage`.
// Port of: src/pdf/SkKeyedImage.cpp#L15-L25 (chrome/m156)
#[doc(alias = "SkBitmapKeyFromImage")]
#[must_use]
pub fn bitmap_key_from_image(image: Option<&Image>) -> BitmapKey {
    let Some(image) = image else {
        return BitmapKey::EMPTY;
    };
    if let Some(bm) = image.as_base().on_peek_bitmap() {
        let o = bm.pixel_ref_origin();
        return BitmapKey {
            subset: image.bounds().with_offset((o.x, o.y)),
            id: bm.generation_id(),
        };
    }
    BitmapKey {
        subset: image.bounds(),
        id: image.unique_id(),
    }
}

/// `SkKeyedImage`: this class has all the advantages of bitmaps and images. The image holds on to
/// encoded data. The `BitmapKey` properly de-dups subsets.
// Port of: src/pdf/SkKeyedImage.h#L20-L44 (chrome/m156)
#[doc(alias = "SkKeyedImage")]
#[derive(Debug, Clone)]
pub struct KeyedImage {
    image: Option<Image>,
    key: BitmapKey,
}

impl Default for KeyedImage {
    /// `SkKeyedImage()`: no image.
    fn default() -> Self {
        Self {
            image: None,
            key: BitmapKey::EMPTY,
        }
    }
}

impl KeyedImage {
    /// `SkKeyedImage(sk_sp<SkImage>)`.
    // Port of: src/pdf/SkKeyedImage.cpp#L27-L29 (chrome/m156)
    #[must_use]
    pub fn new(image: Option<Image>) -> Self {
        let key = bitmap_key_from_image(image.as_ref());
        Self { image, key }
    }

    /// `SkKeyedImage(const SkBitmap&)`.
    // Port of: src/pdf/SkKeyedImage.cpp#L31-L35 (chrome/m156)
    #[must_use]
    pub fn from_bitmap(bm: &Bitmap) -> Self {
        let image = bm.as_image();
        let key = if image.is_some() {
            BitmapKey {
                subset: bm.get_subset(),
                id: bm.generation_id(),
            }
        } else {
            BitmapKey::EMPTY
        };
        Self { image, key }
    }

    /// `explicit operator bool`.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.image.is_some()
    }

    /// `key`.
    #[must_use]
    pub fn key(&self) -> &BitmapKey {
        &self.key
    }

    /// `image`.
    #[must_use]
    pub fn image(&self) -> Option<&Image> {
        self.image.as_ref()
    }

    /// `release`.
    // Port of: src/pdf/SkKeyedImage.cpp#L47-L53 (chrome/m156)
    pub fn release(&mut self) -> Option<Image> {
        self.key = BitmapKey::EMPTY;
        self.image.take()
    }

    /// `subset`.
    // Port of: src/pdf/SkKeyedImage.cpp#L37-L45 (chrome/m156)
    #[must_use]
    pub fn subset(&self, subset: &IRect) -> KeyedImage {
        let mut img = KeyedImage::default();
        if let Some(image) = &self.image {
            if let Some(subset) = IRect::intersect(subset, &image.bounds()) {
                img.image = image.make_subset(subset, RequiredProperties::default());
                if img.image.is_some() {
                    img.key = BitmapKey {
                        subset: subset.with_offset(self.key.subset.top_left()),
                        id: self.key.id,
                    };
                }
            }
        }
        img
    }
}
