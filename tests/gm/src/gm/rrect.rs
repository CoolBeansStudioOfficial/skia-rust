// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/rrect.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Vector;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::{Corner, RRect};

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
