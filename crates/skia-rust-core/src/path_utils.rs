// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathUtils.h, src/core/SkPathUtils.cpp

//! `skpathutils`: path helpers built on the stroker.
//!
//! skia-rust: `SkPaint` and `SkPathEffect` are not ported yet, so `FillPathWithPaint` is
//! available as [`fill_path_with_stroke_rec`], which takes the already-built [`StrokeRec`]
//! (`SkStrokeRec(paint, resScale)`) and applies no path effect.

use crate::matrix::Matrix;
use crate::matrix_priv::compute_res_scale_for_stroking;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::scalar::scalar;
use crate::stroke_rec::StrokeRec;

/// Returns the resolution scale `FillPathWithPaint` uses for `ctm`
/// (`SkMatrixPriv::ComputeResScaleForStroking`), to be set on the [`StrokeRec`] with
/// [`StrokeRec::set_res_scale`].
// Port of: src/core/SkPathUtils.cpp#L26 (chrome/m156)
#[must_use]
pub fn res_scale_for_ctm(ctm: &Matrix) -> scalar {
    compute_res_scale_for_stroking(ctm)
}

/// Applies the stroke `rec` to `orig_src`, putting the result in `builder`: the stroked outline
/// for a stroke or stroke-and-fill rec, a copy of `orig_src` for a fill or hairline rec. Returns
/// true if the result is meant to be filled (that is, the rec is not a hairline).
///
/// If `orig_src` is not finite, `builder` is reset and false is returned.
// Port of: src/core/SkPathUtils.cpp#L21-L60 (chrome/m156)
#[doc(alias = "FillPathWithPaint")]
pub fn fill_path_with_stroke_rec(
    orig_src: &Path,
    rec: &StrokeRec,
    builder: &mut PathBuilder,
) -> bool {
    if !orig_src.is_finite() {
        builder.reset();
        return false;
    }

    if !rec.apply_to_path(builder, orig_src) {
        builder.assign_path(orig_src);
    }

    if !builder.is_finite() {
        builder.reset();
    }
    !rec.is_hairline_style()
}

/// Like [`fill_path_with_stroke_rec`], returning the resulting path and whether it is meant to
/// be filled.
// Port of: src/core/SkPathUtils.cpp#L66-L73 (chrome/m156)
#[doc(alias = "FillPathWithPaint")]
#[must_use]
pub fn fill_path_with_stroke_rec_to_path(orig_src: &Path, rec: &StrokeRec) -> (Path, bool) {
    let mut builder = PathBuilder::new();
    let is_fill = fill_path_with_stroke_rec(orig_src, rec, &mut builder);
    (builder.detach(), is_fill)
}
