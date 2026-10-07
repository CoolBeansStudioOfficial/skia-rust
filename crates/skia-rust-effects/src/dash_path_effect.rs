// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/effects/SkDashPathEffect.h, src/effects/SkDashPathEffect.cpp

//! `SkDashPathEffect`: a path effect that dashes a path by copying it with the specified
//! intervals. Only affects stroked paths.
//!
//! skia-rust: flattening (`flatten`, `CreateProc`) is not ported.

use skia_rust_core::path_effect::PathEffect;
use skia_rust_core::scalar::scalar;

use crate::dash_impl::DashImpl;
use crate::dash_path::valid_dash_path;

/// Dashes the path by copying it with the specified intervals (`SkDashPathEffect::Make`).
///
/// - `intervals`: an even number of entries (>= 2), with the even indices specifying the length
///   of "on" intervals, and the odd indices specifying the length of "off" intervals. The slice
///   is copied.
/// - `phase`: offset into the intervals array (mod the sum of all of the intervals).
///
/// For example: if `intervals` is `[10, 20]` and `phase` is 25, this sets up a dashed path like
/// so: 5 pixels off, 10 pixels on, 20 pixels off, 10 pixels on, 20 pixels off, ... A phase of
/// -5, 25, 55, 85, etc. would all result in the same path, because the sum of all the intervals
/// is 30.
///
/// Returns `None` if the intervals or the phase are invalid. Only affects stroked paths.
// Port of: src/effects/SkDashPathEffect.cpp#L397-L402 (chrome/m156)
#[doc(alias = "SkDashPathEffect::Make")]
#[must_use]
pub fn new(intervals: &[scalar], phase: scalar) -> Option<PathEffect> {
    if !valid_dash_path(phase, intervals) {
        return None;
    }
    Some(PathEffect::from_base(DashImpl::new(intervals, phase)))
}

/// Provides `PathEffect::dash`, as `skia-safe` has it as an inherent method (Rust does not allow
/// inherent impls outside the defining crate).
pub trait DashPathEffectExt {
    /// Dashes the path by copying it with the specified intervals; see [`new`].
    #[doc(alias = "SkDashPathEffect::Make")]
    fn dash(intervals: &[scalar], phase: scalar) -> Option<PathEffect>;
}

impl DashPathEffectExt for PathEffect {
    fn dash(intervals: &[scalar], phase: scalar) -> Option<PathEffect> {
        new(intervals, phase)
    }
}
