// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkPathUtils.h, src/core/SkPathUtils.cpp

//! `skpathutils`: path helpers built on the stroker.

use crate::matrix::Matrix;
use crate::matrix_priv::compute_res_scale_for_stroking;
use crate::paint::Paint;
use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::rect::Rect;
use crate::stroke_rec::StrokeRec;

/// Returns the filled equivalent of the stroked path in `dst`: applies the paint's path effect
/// (if any) and its stroke to `src`. `cull_rect` (`None` for no culling) is a hint to the path
/// effect; `ctm` (the identity if `None`) sets the stroke's resolution scale and is passed to
/// the path effect.
///
/// Returns true if the result is meant to be filled, false if it is a hairline. If `src` is not
/// finite, `dst` is reset and false is returned.
// Port of: src/core/SkPathUtils.cpp#L21-L60 (chrome/m156)
#[doc(alias = "FillPathWithPaint")]
pub fn fill_path_with_paint<'a>(
    src: &Path,
    paint: &Paint,
    dst: &mut PathBuilder,
    cull_rect: impl Into<Option<&'a Rect>>,
    ctm: impl Into<Option<Matrix>>,
) -> bool {
    let orig_src = src;
    let builder = dst;
    let cull_rect = cull_rect.into();
    let ctm = ctm.into().unwrap_or_else(Matrix::new_identity);

    if !orig_src.is_finite() {
        builder.reset();
        return false;
    }

    let res_scale = compute_res_scale_for_stroking(&ctm);
    let mut rec = StrokeRec::from_paint(paint, None, res_scale);

    let mut path_storage = None;
    if let Some(pe) = paint.path_effect()
        && pe.filter_path_inplace_with_matrix(builder, orig_src, &mut rec, cull_rect, &ctm)
    {
        path_storage = Some(builder.detach());
    }
    let src_ptr = path_storage.as_ref().unwrap_or(orig_src);
    if !rec.apply_to_path(builder, src_ptr) {
        builder.assign_path(src_ptr);
    }

    if !builder.is_finite() {
        builder.reset();
    }
    !rec.is_hairline_style()
}

/// Like [`fill_path_with_paint`] with no cull rect and the identity matrix, returning the
/// resulting path and whether it is meant to be filled
/// (`SkPath FillPathWithPaint(const SkPath&, const SkPaint&, bool* isFill)`).
// Port of: src/core/SkPathUtils.cpp#L66-L73 (chrome/m156)
#[doc(alias = "FillPathWithPaint")]
#[must_use]
pub fn fill_path_with_paint_to_path(src: &Path, paint: &Paint) -> (Path, bool) {
    let mut builder = PathBuilder::new();
    let is_fill = fill_path_with_paint(src, paint, &mut builder, None, None);
    (builder.detach(), is_fill)
}
