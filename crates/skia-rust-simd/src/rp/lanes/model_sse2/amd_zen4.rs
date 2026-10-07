// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `model_sse2` with [`Estimates::AmdZen4`].

use crate::tier::Estimates;

/// The estimate source of this instantiation.
const EST: Estimates = Estimates::AmdZen4;

// The same body as `host`, mounted again with another `EST` (design §2.8).
#[allow(clippy::duplicate_mod)]
#[path = "imp.rs"]
mod imp;
pub use imp::*;
