// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/Swizzle.h, src/gpu/Swizzle.cpp

//! `skgpu::Swizzle`.
//!
//! The implementation lives in `skia-rust-core` (the raster pipeline's `apply` needs it below
//! this crate); it is re-exported here at Skia's path.

pub use skia_rust_core::swizzle::Swizzle;
