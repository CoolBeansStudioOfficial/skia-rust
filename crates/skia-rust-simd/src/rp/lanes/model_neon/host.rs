// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `model_neon` with [`Estimates::Host`]: `FRECPE`/`FRSQRTE` executed by this host (`aarch64`
//! only).

use crate::tier::Estimates;

/// The estimate source of this instantiation.
const EST: Estimates = Estimates::Host;

#[path = "imp.rs"]
mod imp;
pub use imp::*;
