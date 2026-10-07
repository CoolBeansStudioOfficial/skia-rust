// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `model_sse41` with [`Estimates::AmdZen4`](crate::Estimates::AmdZen4).

/// The `Sse2` model with the same estimates, which this one extends.
use crate::rp::lanes::model_sse2::amd_zen4 as base;

// The same body as `host`, mounted again with another `base` (design §2.8).
#[allow(clippy::duplicate_mod)]
#[path = "imp.rs"]
mod imp;
pub use imp::*;
