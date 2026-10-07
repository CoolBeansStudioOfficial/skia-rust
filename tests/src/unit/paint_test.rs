// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PaintTest.cpp (chrome/m156)
//
// Not ported yet (each needs a type that is not ported):
// - `Paint_copy`: `SkMaskFilter::MakeBlur` (mask filters, Phase 3).
// - `Paint_flattening`, `Paint_MoreFlattening`: `SkBinaryWriteBuffer` / `SkReadBuffer`
//   (`SkPaintPriv::Flatten`, `SkReadBuffer::readPaint`).
// - `Paint_nothingToDraw`: `SkColorMatrix` and `SkColorFilters::Matrix` (color filters,
//   Phase 3).
// - `Paint_regression_measureText`, `Font_getpos`: `SkFont` (text, Phase 5).

#![cfg(test)]

use crate::{def_test, reporter_assert};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::paint::{Join, Paint, Style};
use skia_rust_core::paint_priv;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_utils::fill_path_with_paint;
use skia_rust_core::rect::Contains;
use skia_rust_core::scalar::{SCALAR_1, int_to_scalar};

// found and fixed for webkit: mishandling when we hit recursion limit on
// mostly degenerate cubic flatness test
// Port of: tests/PaintTest.cpp#L69-L101 (chrome/m156)
def_test!(
    #[allow(clippy::excessive_precision, clippy::unreadable_literal)] // the C++ literals, verbatim
    Paint_regression_cubic,
    |reporter| {
        let mut builder = PathBuilder::new();
        let mut paint = Paint::default();

        builder.move_to((460.2881309415525, 303.250847066498));
        builder.cubic_to(
            (463.36378422175284, 302.1169735073363),
            (456.32239330810046, 304.720354932878),
            (453.15255460013304, 305.788586869862),
        );
        let path = builder.detach();

        let fill_r = *path.bounds();

        paint.set_style(Style::Stroke);
        paint.set_stroke_width(int_to_scalar(2));
        let _ = fill_path_with_paint(&path, &paint, &mut builder, None, None);
        let stroke_r = builder.compute_bounds();

        let mut max_r = fill_r;
        // std::max(SK_Scalar1, paint.getStrokeMiter())
        let miter = if SCALAR_1 < paint.stroke_miter() {
            paint.stroke_miter()
        } else {
            SCALAR_1
        };
        let inset = if paint.stroke_join() == Join::Miter {
            paint.stroke_width() * miter
        } else {
            paint.stroke_width()
        };
        max_r.inset((-inset, -inset));

        // test that our stroke didn't explode
        reporter_assert!(reporter, max_r.contains(&stroke_r));
    }
);

// Port of: tests/PaintTest.cpp#L246-L253 (chrome/m156)
def_test!(Paint_dither, |reporter| {
    let mut p = Paint::default();
    p.set_dither(true);

    let should_dither = paint_priv::should_dither(&p, ColorType::BGRA8888);

    reporter_assert!(reporter, !should_dither);
});
