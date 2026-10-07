// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `model_sse41` with [`Estimates::Host`](crate::Estimates::Host).

/// The `Sse2` model with the same estimates, which this one extends.
use crate::rp::lanes::model_sse2::host as base;

#[path = "imp.rs"]
mod imp;
pub use imp::*;
