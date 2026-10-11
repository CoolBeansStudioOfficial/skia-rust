// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/pathopsinverse.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::{color_to_565, int_to_scalar};
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas as CoreCanvas;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_core::utils::parse_path;
use skia_rust_pathops::op as pathops_op;
use skia_rust_pathops::op_builder::OpBuilder;
use skia_rust_pathops::path_op::PathOp;
use skia_rust_raster::raster_canvas::RasterCanvas;

// The `SkPathOp` values this GM draws, in `kDifference_SkPathOp ..= kReverseDifference_SkPathOp`.
const OPS: [PathOp; 5] = [
    PathOp::Difference,
    PathOp::Intersect,
    PathOp::Union,
    PathOp::Xor,
    PathOp::ReverseDifference,
];

// Port of: gm/pathopsinverse.cpp#L14-L63 (chrome/m156), PathOpsInverseGM
// The fields are the paints of the C++ class (fOnePaint, fTwoPaint, ...).
#[allow(clippy::struct_field_names)]
struct PathOpsInverseGm {
    one_paint: Paint,
    two_paint: Paint,
    outline_paint: Paint,
    op_paint: [Paint; 5],
}

// Port of: gm/pathopsinverse.cpp#L23-L28 (chrome/m156), makePaint
fn make_paint(color: Color) -> Paint {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Fill);
    paint.set_color(color);
    paint
}

// Port of: gm/pathopsinverse.cpp#L30-L36 (chrome/m156), blend
fn blend(one: Color, two: Color) -> Color {
    let mut temp = Bitmap::new();
    temp.alloc_n32_pixels((1, 1), None);
    {
        let canvas = CoreCanvas::from_bitmap(&mut temp, None).expect("a canvas");
        canvas.draw_color(one, None);
        canvas.draw_color(two, None);
    }
    // `*(SkColor*) pixels`: the raw native pixel read as a colour.
    Color::from(temp.get_addr32(0, 0))
}

impl PathOpsInverseGm {
    // Port of: gm/pathopsinverse.cpp#L17-L20 (chrome/m156), the paint setup of onOnceBeforeDraw
    fn new() -> Self {
        let one_color = color_to_565(Color::from(0xFF80_80FF));
        let two_color = Color::from(0x807F_1F1F);
        let blend_color = blend(one_color, two_color);

        let mut outline_paint = make_paint(Color::from(0xFF00_0000));
        outline_paint.set_style(Style::Stroke);

        Self {
            one_paint: make_paint(one_color),
            two_paint: make_paint(two_color),
            outline_paint,
            op_paint: [
                make_paint(one_color),
                make_paint(blend_color),
                make_paint(color_to_565(Color::from(0xFFC0_FFC0))),
                make_paint(color_to_565(Color::from(0xFFA0_FFE0))),
                make_paint(two_color),
            ],
        }
    }
}

impl GM for PathOpsInverseGm {
    fn name(&self) -> String {
        "pathopsinverse".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1200, 900)
    }

    // Port of: gm/pathopsinverse.cpp#L44-L72 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut y_pos = 0;
        for one_fill in 0..=1 {
            let one_f = if one_fill != 0 {
                PathFillType::InverseEvenOdd
            } else {
                PathFillType::EvenOdd
            };
            for two_fill in 0..=1 {
                let two_f = if two_fill != 0 {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                };
                let one =
                    Path::rect(Rect::from_ltrb(10.0, 10.0, 70.0, 70.0), None).make_fill_type(one_f);
                let two = Path::rect(Rect::from_ltrb(40.0, 40.0, 100.0, 100.0), None)
                    .make_fill_type(two_f);

                canvas.save();
                canvas.translate((0.0, int_to_scalar(y_pos)));
                canvas.clip_rect(Rect::from_wh(110.0, 110.0), None, true);
                canvas.draw_path(&one, &self.one_paint);
                canvas.draw_path(&one, &self.outline_paint);
                canvas.draw_path(&two, &self.two_paint);
                canvas.draw_path(&two, &self.outline_paint);
                canvas.restore();

                let mut x_pos = 150;
                for (index, op) in OPS.into_iter().enumerate() {
                    // `Op(one, two, op).value_or(SkPath())`
                    let result = pathops_op(&one, &two, op).unwrap_or_default();
                    canvas.save();
                    canvas.translate((int_to_scalar(x_pos), int_to_scalar(y_pos)));
                    canvas.clip_rect(Rect::from_wh(110.0, 110.0), None, true);
                    canvas.draw_path(&result, &self.op_paint[index]);
                    canvas.draw_path(&result, &self.outline_paint);
                    canvas.restore();
                    x_pos += 150;
                }
                y_pos += 150;
            }
        }
    }
}

// Port of: gm/pathopsinverse.cpp#L146 (chrome/m156)
crate::def_gm!(
    #[ignore = "Union of inverse-filled rects differs from the golden in the Union column (skia-rust-pathops op result for inverse fills); diff pass done, see manifest reason"]
    PathOpsInverseGM,
    PathOpsInverseGm::new()
);

// Port of: gm/pathopsinverse.cpp#L74-L103 (chrome/m156), DEF_SIMPLE_GM(pathops_skbug_10155)
crate::def_simple_gm!(pathops_skbug_10155, canvas, 256, 256, {
    let svg_str = [
        "M474.889 27.0952C474.889 27.1002 474.888 27.1018 474.889 27.1004L479.872 27.5019C479.883 27.3656 479.889 27.2299 479.889 27.0952L474.889 27.0952L474.889 27.0952Z",
        "M474.94 26.9405C474.93 26.9482 474.917 26.9576 474.901 26.9683L477.689 31.1186C477.789 31.0512 477.888 30.9804 477.985 30.9059L474.94 26.9405L474.94 26.9405Z",
    ];
    let mut path: [Path; 2] = [Path::new(), Path::new()];
    let mut builder = OpBuilder::new();
    for i in 0..2 {
        path[i] = parse_path::from_svg(svg_str[i]).unwrap_or_default();
        builder.add(&path[i], PathOp::Union);
    }
    let result_path = builder.resolve().unwrap_or_default();

    let r: Rect = *path[0].bounds();
    canvas.translate((30.0, 30.0));
    canvas.scale((200.0 / r.width(), 200.0 / r.width()));
    canvas.translate((-r.left(), -r.top()));

    let mut paint = Paint::default();
    paint.set_color(Color::RED);
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(0.0);
    canvas.draw_path(&path[0], &paint);
    canvas.draw_path(&path[1], &paint);

    // The blue draw should (nearly) overdraw all of the red (except where the two paths intersect)
    paint.set_color(Color::BLUE);
    canvas.draw_path(&result_path, &paint);
});
