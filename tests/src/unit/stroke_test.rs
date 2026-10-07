// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/StrokeTest.cpp (chrome/m156)

#![cfg(test)]

use crate::tools::stroke_paint::{Paint, fill_path_with_paint};
use crate::{Reporter, def_test, reporter_assert};
use skia_rust_core::float_bits::bits_to_float;
use skia_rust_core::paint::{Cap, Join, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_priv;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::scalar::Scalar;
use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};

// Port of: tests/StrokeTest.cpp#L26-L31 (chrome/m156)
fn equal(a: &Rect, b: &Rect) -> bool {
    f32::nearly_equal(a.left(), b.left(), None)
        && f32::nearly_equal(a.top(), b.top(), None)
        && f32::nearly_equal(a.right(), b.right(), None)
        && f32::nearly_equal(a.bottom(), b.bottom(), None)
}

// Port of: tests/StrokeTest.cpp#L33-L66 (chrome/m156)
#[allow(clippy::excessive_precision)] // float literals are copied verbatim from the C++
fn test_strokecubic(_reporter: &mut Reporter) {
    let hex_cubic_vals: [u32; 8] = [
        0x424c_1086,
        0x44bc_f0cb, // fX=51.0161362 fY=1511.52478
        0x424c_107c,
        0x44bc_f0cb, // fX=51.0160980 fY=1511.52478
        0x424c_10c2,
        0x44bc_f0cb, // fX=51.0163651 fY=1511.52478
        0x424c_1119,
        0x44bc_f0ca, // fX=51.0166969 fY=1511.52466
    ];
    let cubic_vals = [
        Point::new(51.016_136_2, 1_511.524_78),
        Point::new(51.016_098_0, 1_511.524_78),
        Point::new(51.016_365_1, 1_511.524_78),
        Point::new(51.016_696_9, 1_511.524_66),
    ];
    let mut paint = Paint::new();

    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.394_537_568);

    let mut builder = PathBuilder::new();
    builder.move_to(cubic_vals[0]);
    builder.cubic_to(cubic_vals[1], cubic_vals[2], cubic_vals[3]);
    let _ = fill_path_with_paint(&builder.detach(), &paint);

    let b = bits_to_float;
    builder.move_to((b(hex_cubic_vals[0]), b(hex_cubic_vals[1])));
    builder.cubic_to(
        (b(hex_cubic_vals[2]), b(hex_cubic_vals[3])),
        (b(hex_cubic_vals[4]), b(hex_cubic_vals[5])),
        (b(hex_cubic_vals[6]), b(hex_cubic_vals[7])),
    );
    let _ = fill_path_with_paint(&builder.detach(), &paint);
}

// Port of: tests/StrokeTest.cpp#L68-L100 (chrome/m156)
fn test_strokerect(reporter: &mut Reporter) {
    let width: f32 = 10.0;
    let mut paint = Paint::new();

    paint.set_style(Style::Stroke);
    paint.set_stroke_width(width);

    let r = Rect::new(0.0, 0.0, 200.0, 100.0);

    let mut outer = r;
    outer.outset((width / 2.0, width / 2.0));

    let joins = [Join::Miter, Join::Round, Join::Bevel];

    for join in joins {
        paint.set_stroke_join(join);

        let path = Path::rect(r, None);
        let fill_path = fill_path_with_paint(&path, &paint);

        reporter_assert!(reporter, equal(&outer, fill_path.bounds()));

        let is_miter = Join::Miter == join;
        let nested = path_priv::is_nested_fill_rects_path(&fill_path);
        reporter_assert!(reporter, nested.is_some() == is_miter);
        if is_miter {
            let mut inner = r;
            inner.inset((width / 2.0, width / 2.0));
            let nested = nested.map(|(rects, _)| rects).unwrap_or_default();
            reporter_assert!(reporter, equal(&nested[0], &outer));
            reporter_assert!(reporter, equal(&nested[1], &inner));
        }
    }
}

// Port of: tests/StrokeTest.cpp#L102-L164 (chrome/m156)
fn test_strokerec_equality(reporter: &mut Reporter) {
    {
        let mut s1 = StrokeRec::new(InitStyle::Fill);
        let mut s2 = StrokeRec::new(InitStyle::Fill);
        reporter_assert!(reporter, s1.has_equal_effect(&s2));

        // Test that style mismatch is detected.
        s2.set_hairline_style();
        reporter_assert!(reporter, !s1.has_equal_effect(&s2));

        s1.set_hairline_style();
        reporter_assert!(reporter, s1.has_equal_effect(&s2));

        // ResScale is not part of equality.
        s1.set_res_scale(2.1);
        s2.set_res_scale(1.2);
        reporter_assert!(reporter, s1.has_equal_effect(&s2));
        s1.set_fill_style();
        s2.set_fill_style();
        reporter_assert!(reporter, s1.has_equal_effect(&s2));
        s1.set_stroke_style(1.0, false);
        s2.set_stroke_style(1.0, false);
        s1.set_stroke_params(Cap::Butt, Join::Round, 2.9);
        s2.set_stroke_params(Cap::Butt, Join::Round, 2.9);
        reporter_assert!(reporter, s1.has_equal_effect(&s2));
    }

    // Stroke parameters on fill or hairline style are not part of equality.
    {
        let mut s1 = StrokeRec::new(InitStyle::Fill);
        let mut s2 = StrokeRec::new(InitStyle::Fill);
        for _ in 0..2 {
            s1.set_stroke_params(Cap::Butt, Join::Round, 2.9);
            s2.set_stroke_params(Cap::Butt, Join::Round, 2.1);
            reporter_assert!(reporter, s1.has_equal_effect(&s2));
            s2.set_stroke_params(Cap::Butt, Join::Bevel, 2.9);
            reporter_assert!(reporter, s1.has_equal_effect(&s2));
            s2.set_stroke_params(Cap::Round, Join::Round, 2.9);
            reporter_assert!(reporter, s1.has_equal_effect(&s2));
            s1.set_hairline_style();
            s2.set_hairline_style();
        }
    }

    // Stroke parameters on stroke style are part of equality.
    {
        let mut s1 = StrokeRec::new(InitStyle::Fill);
        let mut s2 = StrokeRec::new(InitStyle::Fill);
        s1.set_stroke_params(Cap::Butt, Join::Round, 2.9);
        s2.set_stroke_params(Cap::Butt, Join::Round, 2.9);
        s1.set_stroke_style(1.0, false);

        s2.set_stroke_style(1.0, true);
        reporter_assert!(reporter, !s1.has_equal_effect(&s2));

        s2.set_stroke_style(2.1, false);
        reporter_assert!(reporter, !s1.has_equal_effect(&s2));

        s2.set_stroke_style(1.0, false);
        reporter_assert!(reporter, s1.has_equal_effect(&s2));

        s2.set_stroke_params(Cap::Butt, Join::Round, 2.1);
        reporter_assert!(reporter, s1.has_equal_effect(&s2)); // Miter limit not relevant to butt caps.
        s2.set_stroke_params(Cap::Butt, Join::Bevel, 2.9);
        reporter_assert!(reporter, !s1.has_equal_effect(&s2));
        s2.set_stroke_params(Cap::Round, Join::Round, 2.9);
        reporter_assert!(reporter, !s1.has_equal_effect(&s2));

        // Sets fill.
        s1.set_stroke_style(0.0, true);
        s2.set_stroke_style(0.0, true);
        reporter_assert!(reporter, s1.has_equal_effect(&s2));
    }
}

// From skbug.com/40037699. The large stroke width can cause numerical instabilities.
// Port of: tests/StrokeTest.cpp#L166-L183 (chrome/m156)
fn test_big_stroke(_reporter: &mut Reporter) {
    let mut paint = Paint::new();
    paint.set_style(Style::StrokeAndFill);
    paint.set_stroke_width(1.496_790_7e10);

    let b = bits_to_float;
    let mut builder = PathBuilder::new();
    builder.move_to((b(0x4638_0000), b(0xc638_0000))); // 11776, -11776
    builder.line_to((b(0x46a0_0000), b(0xc6a0_0000))); // 20480, -20480
    builder.line_to((b(0x468c_0000), b(0xc68c_0000))); // 17920, -17920
    builder.line_to((b(0x4610_0000), b(0xc610_0000))); // 9216, -9216
    builder.line_to((b(0x4638_0000), b(0xc638_0000))); // 11776, -11776
    builder.close();

    let _ = fill_path_with_paint(&builder.detach(), &paint);
}

// Port of: tests/StrokeTest.cpp#L185-L190 (chrome/m156)
def_test!(Stroke, |reporter| {
    test_strokecubic(reporter);
    test_strokerect(reporter);
    test_strokerec_equality(reporter);
    test_big_stroke(reporter);
});
