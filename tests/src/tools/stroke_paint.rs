// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: the stroke members of include/core/SkPaint.h and src/core/SkPathUtils.cpp (chrome/m156)

//! A stand-in for the stroke state of `SkPaint`, until `SkPaint` is ported, so that the ported
//! stroker tests keep their `SkPaint` + `skpathutils::FillPathWithPaint` shape.

use skia_rust_core::paint::{Cap, DEFAULT_MITER_LIMIT, Join, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_stroke_rec;
use skia_rust_core::scalar::scalar;
use skia_rust_core::stroke_rec::StrokeRec;

/// The stroke members of `SkPaint`: style, stroke width, miter limit, cap and join.
#[derive(Copy, Clone, Debug)]
pub struct Paint {
    width: scalar,
    miter_limit: scalar,
    cap: Cap,
    join: Join,
    style: Style,
}

impl Default for Paint {
    // Port of: src/core/SkPaint.cpp#L33-L48 (chrome/m156)
    fn default() -> Self {
        Self {
            width: 0.0,
            miter_limit: DEFAULT_MITER_LIMIT,
            cap: Cap::DEFAULT,
            join: Join::DEFAULT,
            style: Style::Fill,
        }
    }
}

impl Paint {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    // Port of: src/core/SkPaint.cpp#L159-L167 (chrome/m156)
    pub fn set_stroke_width(&mut self, width: scalar) {
        // a negative (or NaN) width is ignored, as in SkPaint::setStrokeWidth
        if width >= 0.0 {
            self.width = width;
        }
    }

    pub fn set_stroke_join(&mut self, join: Join) {
        self.join = join;
    }

    #[must_use]
    pub fn stroke_width(&self) -> scalar {
        self.width
    }
}

/// `skpathutils::FillPathWithPaint(src, paint, builder)` with no cull rect and an identity ctm.
pub fn fill_path_with_paint_builder(src: &Path, paint: &Paint, builder: &mut PathBuilder) -> bool {
    // resScale == ComputeResScaleForStroking(identity) == 1
    let rec = StrokeRec::from_paint_params(
        paint.style,
        paint.width,
        paint.miter_limit,
        paint.cap,
        paint.join,
        1.0,
    );
    fill_path_with_stroke_rec(src, &rec, builder)
}

/// `SkPath skpathutils::FillPathWithPaint(src, paint)`.
#[must_use]
pub fn fill_path_with_paint(src: &Path, paint: &Paint) -> Path {
    let mut builder = PathBuilder::new();
    let _ = fill_path_with_paint_builder(src, paint, &mut builder);
    builder.detach()
}
