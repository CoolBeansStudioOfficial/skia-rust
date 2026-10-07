// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/nested.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;

const IMAGE_WIDTH: i32 = 269;
const IMAGE_HEIGHT: i32 = 134;

// Test out various combinations of nested rects, ovals and rrects.
// Port of: gm/nested.cpp#L25-L129 (chrome/m156)
struct NestedGM {
    do_aa: bool,
    flipped: bool,
}

#[derive(Clone, Copy)]
enum Shapes {
    Rect = 0,
    RRect,
    Oval,
}

const SHAPE_COUNT: i32 = 3;

impl Shapes {
    fn from_i32(i: i32) -> Shapes {
        match i {
            0 => Shapes::Rect,
            1 => Shapes::RRect,
            _ => Shapes::Oval,
        }
    }
}

impl NestedGM {
    fn new(do_aa: bool, flipped: bool) -> Self {
        Self { do_aa, flipped }
    }

    // Port of: gm/nested.cpp#L56-L74 (chrome/m156)
    fn add_shape(b: &mut PathBuilder, rect: &Rect, shape: Shapes, dir: PathDirection) {
        match shape {
            Shapes::Rect => {
                b.add_rect(rect, dir, None);
            }
            Shapes::RRect => {
                let mut rr = RRect::default();
                rr.set_rect_xy(rect, 5.0, 5.0);
                b.add_rrect(rr, dir, None);
            }
            Shapes::Oval => {
                b.add_oval(rect, dir, None);
            }
        }
    }
}

impl GM for NestedGM {
    fn name(&self) -> String {
        let mut name = String::from("nested");
        if self.flipped {
            name.push_str("_flipY");
        }
        if self.do_aa {
            name.push_str("_aa");
        } else {
            name.push_str("_bw");
        }
        name
    }

    fn size(&mut self) -> ISize {
        ISize::new(IMAGE_WIDTH, IMAGE_HEIGHT)
    }

    fn bg_color(&self) -> Color {
        Color::from(0xFFDD_DDDD)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut shape_paint = Paint::default();
        shape_paint.set_color(Color::BLACK);
        shape_paint.set_anti_alias(self.do_aa);

        let outer_rect = Rect::from_wh(40.0, 40.0);

        let inner_rects = [
            Rect::new(10.0, 10.0, 30.0, 30.0), // small
            Rect::new(0.5, 18.0, 4.5, 22.0),   // smaller and offset to left
        ];

        // draw a background pattern to make transparency errors more apparent
        let mut rand = Random::default();

        let mut y = 0;
        while y < IMAGE_HEIGHT {
            let mut x = 0;
            while x < IMAGE_WIDTH {
                #[allow(clippy::cast_precision_loss)] // SkIntToScalar
                let r = Rect::from_xywh(x as f32, y as f32, 10.0, 10.0);
                let mut p = Paint::default();
                p.set_color(Color::from(rand.next_u() | 0xFF00_0000));
                canvas.draw_rect(r, &p);
                x += 10;
            }
            y += 10;
        }

        let mut x_off = 2.0;
        let mut y_off = 2.0;
        for outer_shape in 0..SHAPE_COUNT {
            for inner_shape in 0..SHAPE_COUNT {
                for inner_rect in &inner_rects {
                    let mut builder = PathBuilder::new();

                    Self::add_shape(
                        &mut builder,
                        &outer_rect,
                        Shapes::from_i32(outer_shape),
                        PathDirection::CW,
                    );
                    Self::add_shape(
                        &mut builder,
                        inner_rect,
                        Shapes::from_i32(inner_shape),
                        PathDirection::CCW,
                    );

                    canvas.save();
                    if self.flipped {
                        canvas.scale((1.0, -1.0));
                        canvas.translate((x_off, -y_off - 40.0));
                    } else {
                        canvas.translate((x_off, y_off));
                    }

                    canvas.draw_path(&builder.detach(), &shape_paint);
                    canvas.restore();

                    x_off += 45.0;
                }
            }

            x_off = 2.0;
            y_off += 45.0;
        }
    }
}

// Port of: gm/nested.cpp#L135-L138 (chrome/m156)
crate::def_gm!(
    NestedGM_true_false = "NestedGM(/* doAA = */ true, /* flipped = */ false)",
    NestedGM::new(true, false)
);
crate::def_gm!(
    NestedGM_false_false = "NestedGM(/* doAA = */ false, /* flipped = */ false)",
    NestedGM::new(false, false)
);
crate::def_gm!(
    NestedGM_true_true = "NestedGM(/* doAA = */ true, /* flipped = */ true)",
    NestedGM::new(true, true)
);
crate::def_gm!(
    NestedGM_false_true = "NestedGM(/* doAA = */ false, /* flipped = */ true)",
    NestedGM::new(false, true)
);

// Port of: gm/nested.cpp#L140-L181 (chrome/m156)
crate::def_simple_gm!(nested_hairline_square, canvas, 64, 64, {
    // See crbug.com/1234194 - This should draw 1 row of 3 stroked squares, with a second 0.5px
    // shifted row of squares below it.
    let draw_ellipses = || {
        canvas.save();
        // Originally the SVG string "M5,14H0V9h5V14Z M1,13h3v-3H1V13Z" but that just specifies a
        // 5px wide square outside a 3px wide square.
        let square: Path = PathBuilder::new()
            .add_rect(Rect::from_ltrb(0.0, 9.0, 5.0, 14.0), None, None)
            .add_rect(
                Rect::from_ltrb(1.0, 10.0, 4.0, 13.0),
                PathDirection::CCW,
                None,
            )
            .detach();

        // From the bug, SVG viewbox was (0, 0, 24, 24), so the above coordinates are relative to
        // that, but the svg was then the child of a div that was 16x16, so it's scaled down. This
        // converts the 1px wide nested rects into subpixel nested rects.
        canvas.scale((16.0 / 24.0, 16.0 / 24.0));

        let mut paint = Paint::default();
        paint.set_color(Color::from_argb(255, 70, 70, 70));
        paint.set_anti_alias(true);

        // The original SVG drew 3 separate paths, but these were just translations of the original
        // path baked into a path string.
        canvas.draw_path(&square, &paint);
        canvas.translate((10.0, 0.0));
        canvas.draw_path(&square, &paint);
        canvas.translate((10.0, 0.0));
        canvas.draw_path(&square, &paint);

        canvas.restore();
    };

    draw_ellipses();
    canvas.translate((0.5, 16.0));
    draw_ellipses();
});
