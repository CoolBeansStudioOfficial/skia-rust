// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rrect.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_types::BlurStyle;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask_filter::MaskFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::{Corner, RRect};
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/rrect.cpp#L40 (chrome/m156)
type InsetProc = fn(&RRect, f32, f32, &mut RRect);

const CORNERS: [Corner; 4] = [
    Corner::UpperLeft,
    Corner::UpperRight,
    Corner::LowerRight,
    Corner::LowerLeft,
];

// Port of: gm/rrect.cpp#L42-L60 (chrome/m156)
fn inset0(src: &RRect, dx: f32, dy: f32, dst: &mut RRect) {
    let mut r = *src.rect();

    r.inset((dx, dy));
    if r.is_empty() {
        dst.set_empty();
        return;
    }

    let mut radii = [Vector::default(); 4];
    for i in 0..4 {
        radii[i] = src.radii(CORNERS[i]);
    }
    for radius in &mut radii {
        radius.x -= dx;
        radius.y -= dy;
    }
    dst.set_rect_radii(r, &radii);
}

// Port of: gm/rrect.cpp#L62-L76 (chrome/m156)
fn inset1(src: &RRect, dx: f32, dy: f32, dst: &mut RRect) {
    let mut r = *src.rect();

    r.inset((dx, dy));
    if r.is_empty() {
        dst.set_empty();
        return;
    }

    let mut radii = [Vector::default(); 4];
    for i in 0..4 {
        radii[i] = src.radii(CORNERS[i]);
    }
    dst.set_rect_radii(r, &radii);
}

// Port of: gm/rrect.cpp#L78-L99 (chrome/m156)
#[allow(clippy::float_cmp)] // mirrors the C++ truthiness test of a float
fn inset2(src: &RRect, dx: f32, dy: f32, dst: &mut RRect) {
    let mut r = *src.rect();

    r.inset((dx, dy));
    if r.is_empty() {
        dst.set_empty();
        return;
    }

    let mut radii = [Vector::default(); 4];
    for i in 0..4 {
        radii[i] = src.radii(CORNERS[i]);
    }
    for radius in &mut radii {
        if radius.x != 0.0 {
            radius.x -= dx;
        }
        if radius.y != 0.0 {
            radius.y -= dy;
        }
    }
    dst.set_rect_radii(r, &radii);
}

// Port of: gm/rrect.cpp#L101-L103 (chrome/m156)
fn prop(radius: f32, new_size: f32, old_size: f32) -> f32 {
    new_size * radius / old_size
}

// Port of: gm/rrect.cpp#L105-L123 (chrome/m156)
fn inset3(src: &RRect, dx: f32, dy: f32, dst: &mut RRect) {
    let mut r = *src.rect();

    r.inset((dx, dy));
    if r.is_empty() {
        dst.set_empty();
        return;
    }

    let mut radii = [Vector::default(); 4];
    for i in 0..4 {
        radii[i] = src.radii(CORNERS[i]);
    }
    for radius in &mut radii {
        radius.x = prop(radius.x, r.width(), src.rect().width());
        radius.y = prop(radius.y, r.height(), src.rect().height());
    }
    dst.set_rect_radii(r, &radii);
}

// Port of: gm/rrect.cpp#L125-L142 (chrome/m156)
fn draw_rrect_color(canvas: &Canvas, rrect: &RRect) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_style(Style::Stroke);

    if rrect.is_rect() {
        paint.set_color(Color::RED);
    } else if rrect.is_oval() {
        paint.set_color(crate::tool_utils::color_to_565(0xFF00_8800));
    } else if rrect.is_simple() {
        paint.set_color(Color::BLUE);
    } else {
        paint.set_color(Color::BLACK);
    }
    canvas.draw_rrect(rrect, &paint);
}

// Port of: gm/rrect.cpp#L144-L150 (chrome/m156)
fn drawrr(canvas: &Canvas, rrect: &RRect, proc: InsetProc) {
    let mut rr = RRect::default();
    let mut d: f32 = -30.0;
    while d <= 30.0 {
        proc(rrect, d, d, &mut rr);
        draw_rrect_color(canvas, &rr);
        d += 5.0;
    }
}

// Port of: gm/rrect.cpp#L152-L197 (chrome/m156)
struct RRectGM;

impl GM for RRectGM {
    fn name(&self) -> String {
        "rrect".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(820, 710)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let inset_procs: [InsetProc; 4] = [inset0, inset1, inset2, inset3];

        let mut rrect = [RRect::default(); 4];
        let r = Rect::new(0.0, 0.0, 120.0, 100.0);
        let radii: [Vector; 4] = [
            Vector::new(0.0, 0.0),
            Vector::new(30.0, 1.0),
            Vector::new(10.0, 40.0),
            Vector::new(40.0, 40.0),
        ];

        rrect[0].set_rect(r);
        rrect[1].set_oval(r);
        rrect[2].set_rect_xy(r, 20.0, 20.0);
        rrect[3].set_rect_radii(r, &radii);

        canvas.translate((50.5, 50.5));
        for proc in inset_procs {
            canvas.save();
            for rr in &rrect {
                drawrr(canvas, rr, proc);
                canvas.translate((200.0, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, 170.0));
        }
    }
}

// Port of: gm/rrect.cpp#L199 (chrome/m156)
crate::def_gm!(RRectGM_ = "RRectGM", RRectGM);

const RRECT_BLUR_WIDTH: i32 = 300;
const RRECT_BLUR_HEIGHT: i32 = 400;
const RRECT_BLUR_CELL_SIZE: i32 = 100;
// how much to exagerate the diffs
const RRECT_BLUR_DIFF_MAGINIFICATION: u32 = 16;

// Port of: gm/rrect.cpp#L198-L255 (chrome/m156), draw_blurry_rrect
fn draw_blurry_rrect(
    canvas: &Canvas,
    cell_y: i32,
    mf: Option<MaskFilter>,
    color: Color,
    rr: &RRect,
) {
    let cell = RRECT_BLUR_CELL_SIZE;
    let mut rrect_paint = Paint::default();
    rrect_paint.set_color(color);
    rrect_paint.set_mask_filter(mf);

    // `const int paddingX = (kCellSize - rr.width()) / 2;` converts the float to an int, which
    // truncates towards zero, as `as i32` does.
    #[allow(clippy::cast_possible_truncation)]
    let padding_x = ((int_to_scalar(cell) - rr.width()) / 2.0) as i32;
    #[allow(clippy::cast_possible_truncation)]
    let padding_y = ((int_to_scalar(cell) - rr.height()) / 2.0) as i32;
    let left = rr.with_offset((int_to_scalar(padding_x), int_to_scalar(padding_y + cell_y)));
    canvas.draw_rrect(left, &rrect_paint);

    let right = rr.with_offset((
        int_to_scalar(2 * cell + padding_x),
        int_to_scalar(padding_y + cell_y),
    ));
    canvas.draw_path(&Path::rrect(right, None), &rrect_paint);

    // In an ideal world, there would be no diffs at all between the two drawing
    // methods. The point of this gm is to show those differences and allow us to
    // measure the differences.
    let info = ImageInfo::new_n32_premul((cell, cell), None);
    let mut left_bitmap = Bitmap::new();
    left_bitmap.alloc_pixels_flags(&info);
    if !canvas.read_pixels_to_bitmap(&mut left_bitmap, (0, cell_y)) {
        return;
    }

    let mut right_bitmap = Bitmap::new();
    right_bitmap.alloc_pixels_flags(&info);
    if !canvas.read_pixels_to_bitmap(&mut right_bitmap, (2 * cell, cell_y)) {
        return;
    }

    let mut diff_bitmap = Bitmap::new();
    diff_bitmap.alloc_pixels_flags(&info);
    for y in 0..cell {
        for x in 0..cell {
            let left_color = left_bitmap.get_color((x, y));
            let right_color = right_bitmap.get_color((x, y));
            // Add up the diffs in the 4 channels, then treat that as how bright
            // to draw the diff
            let diff = u32::from(left_color.a().abs_diff(right_color.a()))
                + u32::from(left_color.r().abs_diff(right_color.r()))
                + u32::from(left_color.g().abs_diff(right_color.g()))
                + u32::from(left_color.b().abs_diff(right_color.b()));
            let grey = (diff * RRECT_BLUR_DIFF_MAGINIFICATION).min(255) as u8;
            diff_bitmap.set_addr32(x, y, Color::from_argb(0xFF, grey, grey, grey).into());
        }
    }
    canvas.write_pixels_from_bitmap(&diff_bitmap, (cell, cell_y));
}

// Port of: gm/rrect.cpp#L183-L267 (chrome/m156), RRectBlurGM
struct RRectBlurGm;

impl GM for RRectBlurGm {
    fn name(&self) -> String {
        "rrect_blurs".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(RRECT_BLUR_WIDTH, RRECT_BLUR_HEIGHT)
    }

    // Port of: gm/rrect.cpp#L232-L267 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        // Because of the read/write pixels, this doesn't draw right if viewer zooms in.
        canvas.reset_matrix();
        canvas.clear(Color::DARK_GRAY);
        draw_blurry_rrect(
            canvas,
            0,
            MaskFilter::blur(BlurStyle::Normal, 1.0, false),
            Color::WHITE,
            &RRect::new_rect_xy(Rect::from_wh(50.0, 50.0), 10.0, 15.0),
        );
        draw_blurry_rrect(
            canvas,
            100,
            MaskFilter::blur(BlurStyle::Normal, 0.5, false),
            Color::YELLOW,
            &RRect::new_rect_xy(Rect::from_wh(60.0, 80.0), 3.1, 1.5),
        );
        let mut rr = RRect::default();
        rr.set_nine_patch(Rect::from_wh(70.0, 80.0), 5.0, 10.0, 13.0, 7.0);
        draw_blurry_rrect(
            canvas,
            200,
            MaskFilter::blur(BlurStyle::Normal, 2.5, false),
            Color::from_argb(255, 200, 100, 30),
            &rr,
        );
        let radii = [
            Vector::new(0.0, 0.0),
            Vector::new(20.0, 1.0),
            Vector::new(10.0, 30.0),
            Vector::new(30.0, 30.0),
        ];
        rr.set_rect_radii(Rect::from_wh(90.0, 90.0), &radii);
        draw_blurry_rrect(
            canvas,
            300,
            MaskFilter::blur(BlurStyle::Normal, 1.1, false),
            Color::from_argb(255, 35, 120, 220),
            &rr,
        );
        // labels after to avoid contaminating the diffs
        let mut label_paint = Paint::default();
        label_paint.set_color(Color::WHITE);
        label_paint.set_anti_alias(true);
        let font = default_portable_font();
        canvas.draw_str("drawRRect", (15.0, 15.0), &font, &label_paint);
        canvas.draw_str("diff", (140.0, 15.0), &font, &label_paint);
        canvas.draw_str("drawPath", (220.0, 15.0), &font, &label_paint);
        let h = int_to_scalar(RRECT_BLUR_HEIGHT);
        let w = int_to_scalar(RRECT_BLUR_WIDTH);
        canvas.draw_line((100.0, 0.0), (100.0, h), &label_paint);
        canvas.draw_line((200.0, 0.0), (200.0, h), &label_paint);
        canvas.draw_line((0.0, 100.0), (w, 100.0), &label_paint);
        canvas.draw_line((0.0, 200.0), (w, 200.0), &label_paint);
        canvas.draw_line((0.0, 300.0), (w, 300.0), &label_paint);
    }
}

// Port of: gm/rrect.cpp#L332 (chrome/m156), DEF_GM(return new RRectBlurGM;)
crate::def_gm!(RRectBlurGM_ = "RRectBlurGM", RRectBlurGm);
