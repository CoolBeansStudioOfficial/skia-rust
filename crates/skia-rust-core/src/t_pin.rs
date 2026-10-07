// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/private/SkTPin.h

//! `SkTPin`.

/// Returns `x` pinned (clamped) between `lo` and `hi`, inclusively.
///
/// Unlike `Ord::clamp`, this always returns a value between `lo` and `hi`.
/// If `x` is NaN, it returns `lo`.
// Port of: include/private/SkTPin.h#L18-L21 (chrome/m156)
#[doc(alias = "SkTPin")]
#[must_use]
pub fn t_pin<T: PartialOrd>(x: T, lo: T, hi: T) -> T {
    // std::min(a, b) is `(b < a) ? b : a`; std::max(a, b) is `(a < b) ? b : a`.
    let min = if hi < x { hi } else { x };
    if lo < min { min } else { lo }
}
