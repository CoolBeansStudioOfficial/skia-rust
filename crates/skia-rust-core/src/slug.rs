// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/chromium/Slug.h, src/text/gpu/Slug.cpp (chrome/m156)

//! `sktext::gpu::Slug`: text cached for the GPU.
//!
//! A slug encapsulates a text blob at a specific origin, using a specific paint. It can be
//! manipulated using matrix and clip changes to the canvas. If the canvas is transformed, then the
//! slug will also transform with smaller glyphs using bi-linear interpolation to render. You can
//! think of a slug as making a rubber stamp out of a text blob.
//!
//! Only GPU devices make slugs (`SkDevice::convertGlyphRunListToSlug` returns null on every other
//! device, `SkDevice.cpp#L481-L488`), so the CPU raster path never has one. The slug is an
//! opaque handle here; the GPU phase (`skia_rust_gpu::text_gpu::slug_impl`) implements
//! [`SlugBase`].
//!
//! Not ported: `serialize`, `Deserialize`, `MakeFromBuffer` and `AddDeserialProcs` (they need the
//! remote glyph cache, T23).

use core::any::Any;
use core::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::rect::Rect;

/// What a GPU back end provides for a [`Slug`] (the virtual functions of `sktext::gpu::Slug`).
// Port of: include/private/chromium/Slug.h#L28-L66 (chrome/m156)
pub trait SlugBase: Any + fmt::Debug + Send + Sync {
    /// `sourceBounds()`: the bounds of the glyphs, relative to the origin.
    fn source_bounds(&self) -> Rect;

    /// `sourceBoundsWithOrigin()`: the bounds of the glyphs in the canvas' coordinates.
    fn source_bounds_with_origin(&self) -> Rect;

    /// The slug as `Any`, so a device can recover its own type.
    fn as_any(&self) -> &dyn Any;
}

/// A slug of text, from [`Canvas::convert_blob_to_slug`](crate::canvas::Canvas::convert_blob_to_slug)
/// (`sktext::gpu::Slug`). Cloning shares the slug (it is an `sk_sp` in C++).
// Port of: include/private/chromium/Slug.h#L28-L66 (chrome/m156)
#[doc(alias = "sktext::gpu::Slug")]
#[derive(Clone, Debug)]
pub struct Slug {
    base: Arc<dyn SlugBase>,
    unique_id: u32,
}

impl Slug {
    /// Wraps a back end's slug.
    #[must_use]
    pub fn from_base(base: impl SlugBase) -> Self {
        static NEXT_ID: AtomicU32 = AtomicU32::new(1);
        Self {
            base: Arc::new(base),
            unique_id: NEXT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// The back end's slug.
    #[must_use]
    pub fn as_base(&self) -> &dyn SlugBase {
        &*self.base
    }

    /// `sourceBounds()`.
    #[must_use]
    pub fn source_bounds(&self) -> Rect {
        self.base.source_bounds()
    }

    /// `sourceBoundsWithOrigin()`.
    #[must_use]
    pub fn source_bounds_with_origin(&self) -> Rect {
        self.base.source_bounds_with_origin()
    }

    /// `uniqueID()`.
    #[must_use]
    pub fn unique_id(&self) -> u32 {
        self.unique_id
    }
}
