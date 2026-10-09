// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/text/gpu/Slug.h (chrome/m156), as a placeholder.

//! `sktext::gpu::Slug`: text cached for the GPU.
//!
//! Only GPU devices make slugs (`SkDevice::convertGlyphRunListToSlug` returns null on every other
//! device, `SkDevice.cpp#L481-L488`), so the CPU raster path never has one. [`Slug`] is the type
//! the canvas API takes; it has no values until a GPU phase fills it in.

/// A slug of text, from `SkCanvas::convertBlobToSlug`. Uninhabited: no CPU device makes one.
// Port of: src/text/gpu/TextBlob.h#L37 (chrome/m156), the forward declaration, as a placeholder
#[derive(Debug)]
pub enum Slug {}
