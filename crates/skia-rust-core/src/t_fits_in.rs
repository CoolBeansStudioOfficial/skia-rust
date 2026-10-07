// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkTFitsIn.h

//! `SkTFitsIn`.

/// Returns true if `src` can be represented in `D` without changing its value.
///
/// Rust's `TryFrom` for integers has exactly the semantics `SkTFitsIn` spells out by hand.
// Port of: include/private/SkTFitsIn.h#L68-L87 (chrome/m156)
#[doc(alias = "SkTFitsIn")]
#[must_use]
pub fn t_fits_in<D: TryFrom<S>, S>(src: S) -> bool {
    D::try_from(src).is_ok()
}
