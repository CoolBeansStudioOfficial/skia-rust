// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkDrawTypes.h

//! `SkDrawTypes.h`: small types shared by the draw and blitter code.
//!
//! skia-rust: `kSkBlitterContextSize` (the size of the stack arena for shader contexts) is not
//! ported; blitters own their contexts.

/// Whether a draw produces coverage (an A8 mask of what the geometry covers) instead of color
/// (`SkDrawCoverage`). Passed to `SkBlitter::Choose`.
// Port of: src/core/SkDrawTypes.h#L14-L17 (chrome/m156)
#[doc(alias = "SkDrawCoverage")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum DrawCoverage {
    /// `kNo`: draw colors.
    #[default]
    No,
    /// `kYes`: draw coverage.
    Yes,
}
