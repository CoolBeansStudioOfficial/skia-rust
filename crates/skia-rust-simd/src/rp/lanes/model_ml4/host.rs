// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `model_ml4` with [`Estimates::Host`] (`vrcp14ps`/`vrsqrt14ps` on this host: AVX-512 only).

use crate::tier::Estimates;

/// The estimate source of this instantiation.
const EST: Estimates = Estimates::Host;

#[path = "imp.rs"]
mod imp;
pub use imp::*;
