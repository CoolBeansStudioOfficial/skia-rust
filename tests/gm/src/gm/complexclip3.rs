// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip3.cpp (chrome/m156)

// The GM mirrors C++ arithmetic: scalar and integer conversions of small loop counts, the C++
// variable names (doAAA, doAAB) and one C++ function per GM body, so these lints do not apply.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use crate::prelude::*;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::Font;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/complexclip3.cpp#L18 (chrome/m156), gPathColor
const PATH_COLOR: Color = Color::from_argb(0xFF, 0xFF, 0xFF, 0x00);

// Port of: gm/complexclip3.cpp#L20-L125 (chrome/m156), ComplexClip3GM
struct ComplexClip3Gm {
    do_simple_clip_first: bool,
}

impl GM for ComplexClip3Gm {
    // Port of: gm/complexclip3.cpp#L28-L33 (chrome/m156), getName
    fn name(&self) -> String {
        format!(
            "complexclip3_{}",
            if self.do_simple_clip_first {
                "simple"
            } else {
                "complex"
            }
        )
    }

    // Port of: gm/complexclip3.cpp#L35 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(400, 950)
    }

    // Port of: gm/complexclip3.cpp#L21-L24 (chrome/m156), the constructor's setBGColor
    fn bg_color(&self) -> Color {
        Color::from_argb(0xFF, 0xDD, 0xDD, 0xDD)
    }

    // Port of: gm/complexclip3.cpp#L37-L122 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let clip_simple = Path::circle((70.0, 50.0), 20.0, None);
        let r1 = Rect::from_ltrb(10.0, 20.0, 70.0, 80.0);
        let clip_complex = PathBuilder::new()
            .move_to((40.0, 50.0))
            .arc_to(r1, 30.0, 300.0, false)
            .close()
            .detach();
        let (mut first_clip, mut second_clip) = (clip_simple, clip_complex);
        if !self.do_simple_clip_first {
            std::mem::swap(&mut first_clip, &mut second_clip);
        }

        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let font = Font::from_size(default_portable_typeface(), 20.0);

        // (clip op, name)
        let ops = [(ClipOp::Intersect, "I"), (ClipOp::Difference, "D")];

        canvas.translate((20.0, 20.0));
        canvas.scale((0.75, 0.75));

        let mut path_paint = Paint::default();
        path_paint.set_anti_alias(true);
        path_paint.set_color(PATH_COLOR);

        for inv_a in 0..2 {
            for aa_bits in 0..4 {
                canvas.save();
                for &(op, op_name) in &ops {
                    for inv_b in 0..2 {
                        let do_aaa = aa_bits & 1 != 0;
                        let do_aab = aa_bits & 2 != 0;
                        let do_inv_a = inv_a != 0;
                        let do_inv_b = inv_b != 0;

                        canvas.save();
                        // set clip
                        let mut first = first_clip.clone();
                        let mut second = second_clip.clone();
                        first.set_fill_type(if do_inv_a {
                            PathFillType::InverseEvenOdd
                        } else {
                            PathFillType::EvenOdd
                        });
                        second.set_fill_type(if do_inv_b {
                            PathFillType::InverseEvenOdd
                        } else {
                            PathFillType::EvenOdd
                        });
                        canvas.clip_path(&first, None, do_aaa);
                        canvas.clip_path(&second, op, do_aab);

                        // draw rect clipped
                        let r = Rect::from_ltrb(0.0, 0.0, 100.0, 100.0);
                        canvas.draw_rect(r, &path_paint);
                        canvas.restore();

                        let txt_x = 10.0;
                        paint.set_color(Color::BLACK);
                        let text = format!(
                            "{}{} {} {}{}",
                            if do_aaa { "A" } else { "B" },
                            if do_inv_a { "I" } else { "N" },
                            op_name,
                            if do_aab { "A" } else { "B" },
                            if do_inv_b { "I" } else { "N" },
                        );
                        canvas.draw_str(&text, (txt_x, 130.0), &font, &paint);

                        if do_inv_b {
                            canvas.translate((150.0, 0.0));
                        } else {
                            canvas.translate((120.0, 0.0));
                        }
                    }
                }
                canvas.restore();
                canvas.translate((0.0, 150.0));
            }
        }
    }
}

// Simple clip first
// Port of: gm/complexclip3.cpp#L127-L128 (chrome/m156)
crate::def_gm!(
    ComplexClip3GM_true = "ComplexClip3GM(true)",
    ComplexClip3Gm {
        do_simple_clip_first: true
    }
);

// Complex clip first
// Port of: gm/complexclip3.cpp#L130-L131 (chrome/m156)
crate::def_gm!(
    ComplexClip3GM_false = "ComplexClip3GM(false)",
    ComplexClip3Gm {
        do_simple_clip_first: false
    }
);
