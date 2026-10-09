// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/complexclip.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::fn_params_excessive_bools,
    clippy::struct_excessive_bools,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::font::Font;
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::rect::Rect;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/complexclip.cpp#L14-L16 (chrome/m156)
const G_PATH_COLOR: Color = Color::BLACK;
// Port of: gm/complexclip.cpp#L14-L16 (chrome/m156)
const G_CLIP_A_COLOR: Color = Color::BLUE;
// Port of: gm/complexclip.cpp#L14-L16 (chrome/m156)
const G_CLIP_B_COLOR: Color = Color::RED;

// Port of: gm/complexclip.cpp#L18-L187 (chrome/m156)
struct ComplexClipGm {
    do_aa_clip: bool,
    do_save_layer: bool,
    invert_draw: bool,
}

impl ComplexClipGm {
    // Port of: gm/complexclip.cpp#L24-L30 (chrome/m156)
    fn new(aaclip: bool, save_layer: bool, invert_draw: bool) -> Self {
        Self {
            do_aa_clip: aaclip,
            do_save_layer: save_layer,
            invert_draw,
        }
    }

    // Port of: gm/complexclip.cpp#L168-L184 (chrome/m156)
    fn draw_hairlines(&self, canvas: &Canvas, path: &Path, clip_a: &Path, clip_b: &Path) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_style(Style::Stroke);
        let fade: u8 = 0x33;
        paint.set_color(G_PATH_COLOR);
        paint.set_alpha(fade);
        canvas.draw_path(path, &paint);
        paint.set_color(G_CLIP_A_COLOR);
        paint.set_alpha(fade);
        canvas.draw_path(clip_a, &paint);
        paint.set_color(G_CLIP_B_COLOR);
        paint.set_alpha(fade);
        canvas.draw_path(clip_b, &paint);
    }
}

impl GM for ComplexClipGm {
    fn name(&self) -> String {
        format!(
            "complexclip_{}{}{}",
            if self.do_aa_clip { "aa" } else { "bw" },
            if self.do_save_layer { "_layer" } else { "" },
            if self.invert_draw { "_invert" } else { "" },
        )
    }

    fn size(&mut self) -> ISize {
        ISize::new(388, 780)
    }

    fn bg_color(&self) -> Color {
        Color::new(0xFFDEDFDE)
    }

    // Port of: gm/complexclip.cpp#L36-L144 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut path = PathBuilder::new()
            .move_to((0.0, 50.0))
            .quad_to((0.0, 0.0), (50.0, 0.0))
            .line_to((175.0, 0.0))
            .quad_to((200.0, 0.0), (200.0, 25.0))
            .line_to((200.0, 150.0))
            .quad_to((200.0, 200.0), (150.0, 200.0))
            .line_to((0.0, 200.0))
            .close()
            .move_to((50.0, 50.0))
            .line_to((150.0, 50.0))
            .line_to((150.0, 125.0))
            .quad_to((150.0, 150.0), (125.0, 150.0))
            .line_to((50.0, 150.0))
            .close()
            .detach();
        if self.invert_draw {
            path.set_fill_type(PathFillType::InverseEvenOdd);
        } else {
            path.set_fill_type(PathFillType::EvenOdd);
        }

        let mut path_paint = Paint::default();
        path_paint.set_anti_alias(true);
        path_paint.set_color(G_PATH_COLOR);

        let mut clip_a = Path::polygon(
            &[
                (10.0, 20.0).into(),
                (165.0, 22.0).into(),
                (70.0, 105.0).into(),
                (165.0, 177.0).into(),
                (-5.0, 180.0).into(),
            ],
            true,
            None,
            None,
        );
        let mut clip_b = Path::polygon(
            &[
                (40.0, 10.0).into(),
                (190.0, 15.0).into(),
                (195.0, 190.0).into(),
                (40.0, 185.0).into(),
                (155.0, 100.0).into(),
            ],
            true,
            None,
            None,
        );

        let font = Font::from_size(default_portable_typeface(), 20.0);
        // extra spaces in names for measureText
        let ops: [(ClipOp, &str); 2] =
            [(ClipOp::Intersect, "Isect "), (ClipOp::Difference, "Diff ")];

        canvas.translate((20.0, 20.0));
        canvas.scale((3.0 / 4.0, 3.0 / 4.0));
        if self.do_save_layer {
            let mut bounds = Rect::from_ltrb(
                4.0_f32 / 3.0 * -20.0,
                4.0_f32 / 3.0 * -20.0,
                4.0_f32 / 3.0 * (388 - 20) as f32,
                4.0_f32 / 3.0 * (780 - 20) as f32,
            );
            bounds.inset((100.0, 100.0));
            let mut bound_paint = Paint::default();
            bound_paint.set_color(Color::RED);
            bound_paint.set_style(Style::Stroke);
            canvas.draw_rect(bounds, &bound_paint);
            canvas.clip_rect(bounds, None, None);
            canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
        }
        for inv_bits in 0..4 {
            canvas.save();
            for (op, op_name) in ops {
                self.draw_hairlines(canvas, &path, &clip_a, &clip_b);
                let do_inv_a = (inv_bits & 1) != 0;
                let do_inv_b = (inv_bits & 2) != 0;
                canvas.save();
                clip_a.set_fill_type(if do_inv_a {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                });
                clip_b.set_fill_type(if do_inv_b {
                    PathFillType::InverseEvenOdd
                } else {
                    PathFillType::EvenOdd
                });
                canvas.clip_path(&clip_a, None, self.do_aa_clip);
                canvas.clip_path(&clip_b, op, self.do_aa_clip);
                if self.invert_draw {
                    let mut rect_clip = *clip_a.bounds();
                    rect_clip.join(path.bounds());
                    rect_clip.join(path.bounds());
                    rect_clip.outset((5.0, 5.0));
                    canvas.clip_rect(rect_clip, None, None);
                }
                canvas.draw_path(&path, &path_paint);
                canvas.restore();

                let mut paint = Paint::default();
                let mut txt_x: f32 = 45.0;
                paint.set_color(G_CLIP_A_COLOR);
                let a_txt = if do_inv_a { "InvA " } else { "A " };
                canvas.draw_simple_text(a_txt, TextEncoding::UTF8, (txt_x, 220.0), &font, &paint);
                txt_x += font
                    .measure_text(a_txt.as_bytes(), TextEncoding::UTF8, None)
                    .0;
                paint.set_color(Color::BLACK);
                canvas.draw_simple_text(op_name, TextEncoding::UTF8, (txt_x, 220.0), &font, &paint);
                txt_x += font
                    .measure_text(op_name.as_bytes(), TextEncoding::UTF8, None)
                    .0;
                paint.set_color(G_CLIP_B_COLOR);
                let b_txt = if do_inv_b { "InvB " } else { "B " };
                canvas.draw_simple_text(b_txt, TextEncoding::UTF8, (txt_x, 220.0), &font, &paint);
                canvas.translate((250.0, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, 250.0));
        }
        if self.do_save_layer {
            canvas.restore();
        }
    }
}

// Port of: gm/complexclip.cpp#L207-L214 (chrome/m156)
crate::def_gm!(
    ComplexClipGM_false_false_false = "ComplexClipGM(false, false, false)",
    ComplexClipGm::new(false, false, false)
);
crate::def_gm!(
    ComplexClipGM_false_false_true = "ComplexClipGM(false, false, true)",
    ComplexClipGm::new(false, false, true)
);
crate::def_gm!(
    ComplexClipGM_false_true_false = "ComplexClipGM(false, true, false)",
    ComplexClipGm::new(false, true, false)
);
crate::def_gm!(
    ComplexClipGM_false_true_true = "ComplexClipGM(false, true, true)",
    ComplexClipGm::new(false, true, true)
);
crate::def_gm!(
    ComplexClipGM_true_false_false = "ComplexClipGM(true, false, false)",
    ComplexClipGm::new(true, false, false)
);
crate::def_gm!(
    ComplexClipGM_true_false_true = "ComplexClipGM(true, false, true)",
    ComplexClipGm::new(true, false, true)
);
crate::def_gm!(
    ComplexClipGM_true_true_false = "ComplexClipGM(true, true, false)",
    ComplexClipGm::new(true, true, false)
);
crate::def_gm!(
    ComplexClipGM_true_true_true = "ComplexClipGM(true, true, true)",
    ComplexClipGm::new(true, true, true)
);

// Port of: gm/complexclip.cpp#L254-L268 (chrome/m156)
crate::def_simple_gm!(clip_shader_layer, canvas, 430, 320, {
    let img = crate::tool_utils::get_resource_as_image("images/yellow_rose.png")
        .expect("images/yellow_rose.png (set SKIA_RESOURCES)");
    let sh = img
        .to_shader(
            None,
            skia_rust_core::sampling_options::SamplingOptions::default(),
            None,
        )
        .expect("shader");

    let r = Rect::from_wh(
        crate::tool_utils::int_to_scalar(img.width()),
        crate::tool_utils::int_to_scalar(img.height()),
    );

    canvas.translate((10.0, 10.0));
    // now add the cool clip
    canvas.clip_rect(r, None, None);
    canvas.clip_shader(sh, None);
    // now draw a layer with the same image, and watch it get restored w/ the clip
    canvas.save_layer(&SaveLayerRec::default().bounds(&r));
    canvas.draw_color(Color::new(0xFFFF_0000), None);
    canvas.restore();
});
