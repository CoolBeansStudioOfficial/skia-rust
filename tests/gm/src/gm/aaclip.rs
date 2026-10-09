// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/aaclip.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: literals, short names, local constants, int/float
// conversions, index loops and long bodies are kept as they are there.
#![allow(
    clippy::approx_constant,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::needless_range_loop,
    clippy::trivially_copy_pass_by_ref,
    clippy::write_with_newline,
    clippy::excessive_precision,
    clippy::items_after_statements,
    clippy::many_single_char_names,
    clippy::mixed_case_hex_literals,
    clippy::similar_names,
    clippy::too_many_lines,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use crate::prelude::*;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;

// `ToolUtils::color_to_565`.
// Port of: tools/ToolUtils.cpp#L142-L151 (chrome/m156)
fn color_to_565(color: u32) -> Color {
    use skia_rust_core::color::pre_multiply_color;
    use skia_rust_core::color_data::{pixel16_to_color, pixel32_to_pixel16};
    let pm_color = pre_multiply_color(Color::new(color));
    let color16 = pixel32_to_pixel16(pm_color);
    pixel16_to_color(color16)
}

// Port of: gm/aaclip.cpp#L17-L44 (chrome/m156)
fn draw(canvas: &Canvas, target: &mut Rect, x: i32, y: i32) {
    let mut border_paint = Paint::default();
    border_paint.set_color(Color::from_rgb(0x0, 0xDD, 0x0));
    border_paint.set_anti_alias(true);
    let mut background_paint = Paint::default();
    background_paint.set_color(Color::from_rgb(0xDD, 0x0, 0x0));
    background_paint.set_anti_alias(true);
    let mut foreground_paint = Paint::default();
    foreground_paint.set_color(Color::from_rgb(0x0, 0x0, 0xDD));
    foreground_paint.set_anti_alias(true);
    canvas.save();
    canvas.translate((x as f32, y as f32));
    target.inset((-2.0, -2.0));
    canvas.draw_rect(*target, &border_paint);
    target.inset((2.0, 2.0));
    canvas.draw_rect(*target, &background_paint);
    canvas.clip_rect(*target, None, true);
    target.inset((-4.0, -4.0));
    canvas.draw_rect(*target, &foreground_paint);
    canvas.restore();
}

// Port of: gm/aaclip.cpp#L46-L50 (chrome/m156)
fn draw_square(canvas: &Canvas, x: i32, y: i32) {
    let mut target = Rect::from_wh(10.0, 10.0);
    draw(canvas, &mut target, x, y);
}

// Port of: gm/aaclip.cpp#L52-L56 (chrome/m156)
fn draw_column(canvas: &Canvas, x: i32, y: i32) {
    let mut target = Rect::from_wh(1.0, 10.0);
    draw(canvas, &mut target, x, y);
}

// Port of: gm/aaclip.cpp#L58-L62 (chrome/m156)
fn draw_bar(canvas: &Canvas, x: i32, y: i32) {
    let mut target = Rect::from_wh(10.0, 1.0);
    draw(canvas, &mut target, x, y);
}

// Port of: gm/aaclip.cpp#L64-L69 (chrome/m156)
fn draw_rect_tests(canvas: &Canvas) {
    draw_square(canvas, 10, 10);
    draw_column(canvas, 30, 10);
    draw_bar(canvas, 10, 30);
}

// Port of: gm/aaclip.cpp#L71-L92 (chrome/m156)
crate::def_simple_gm!(aaclip, canvas, 240, 120, {
    draw_rect_tests(canvas);
    canvas.translate((1.0_f32 / 5.0, 1.0_f32 / 5.0));
    canvas.translate((50.0, 0.0));
    draw_rect_tests(canvas);
    canvas.translate((1.0_f32 / 5.0, 1.0_f32 / 5.0));
    canvas.translate((50.0, 0.0));
    draw_rect_tests(canvas);
    canvas.translate((1.0_f32 / 5.0, 1.0_f32 / 5.0));
    canvas.translate((50.0, 0.0));
    draw_rect_tests(canvas);
    canvas.translate((1.0_f32 / 5.0, 1.0_f32 / 5.0));
    canvas.translate((50.0, 0.0));
    draw_rect_tests(canvas);
});

// Port of: gm/aaclip.cpp#L121-L143 (chrome/m156)
struct ClipCubicGm {
    w: f32,
    h: f32,
    v_path: Path,
    h_path: Path,
}

impl ClipCubicGm {
    // Port of: gm/aaclip.cpp#L124-L133 (chrome/m156)
    fn new() -> Self {
        let w: f32 = 100.0;
        let h: f32 = 240.0;
        let v_path = PathBuilder::new()
            .move_to((w, 0.0))
            .cubic_to((w, h - 10.0), (0.0, 10.0), (0.0, h))
            .detach();
        let mut pivot = Matrix::new_identity();
        pivot.set_rotate(90.0, Point::new(w / 2.0, h / 2.0));
        let h_path = v_path.make_transform(&pivot);
        Self {
            w,
            h,
            v_path,
            h_path,
        }
    }

    // Port of: gm/aaclip.cpp#L137-L145 (chrome/m156)
    fn do_draw(&self, canvas: &Canvas, path: &Path) {
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        paint.set_color(Color::new(0xFFCCCCCC));
        canvas.draw_path(path, &paint);
        paint.set_color(Color::RED);
        paint.set_style(Style::Stroke);
        canvas.draw_path(path, &paint);
    }

    // Port of: gm/aaclip.cpp#L146-L160 (chrome/m156)
    fn draw_and_clip(&self, canvas: &Canvas, path: &Path, dx: f32, dy: f32) {
        canvas.save();
        let r = Rect::from_xywh(0.0, self.h / 4.0, self.w, self.h / 2.0);
        let mut paint = Paint::default();
        paint.set_color(color_to_565(0xFF8888FF));
        canvas.draw_rect(r, &paint);
        self.do_draw(canvas, path);
        canvas.translate((dx, dy));
        canvas.draw_rect(r, &paint);
        canvas.clip_rect(r, None, None);
        self.do_draw(canvas, path);
        canvas.restore();
    }
}

impl GM for ClipCubicGm {
    fn name(&self) -> String {
        "clipcubic".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 410)
    }

    // Port of: gm/aaclip.cpp#L161-L170 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.translate((80.0, 10.0));
        self.draw_and_clip(canvas, &self.v_path, 200.0, 0.0);
        canvas.translate((0.0, 200.0));
        self.draw_and_clip(canvas, &self.h_path, 200.0, 0.0);
    }
}

crate::def_gm!(ClipCubicGM, ClipCubicGm::new());
