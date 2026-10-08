// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/aarectmodes.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_core::shader::Shader;
use skia_rust_core::tile_mode::TileMode;

// Port of: gm/aarectmodes.cpp#L27-L66 (chrome/m156)
#[allow(dead_code)] // "avoid bit rot, suppress warning"
fn test4(canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    #[rustfmt::skip]
    let pts = [
        Point::new(10.0, 160.0), Point::new(610.0, 160.0),
        Point::new(610.0, 160.0), Point::new(10.0, 160.0),

        Point::new(610.0, 160.0), Point::new(610.0, 160.0),
        Point::new(610.0, 199.0), Point::new(610.0, 199.0),

        Point::new(10.0, 198.0), Point::new(610.0, 198.0),
        Point::new(610.0, 199.0), Point::new(10.0, 199.0),

        Point::new(10.0, 160.0), Point::new(10.0, 160.0),
        Point::new(10.0, 199.0), Point::new(10.0, 199.0),
    ];
    #[rustfmt::skip]
    let verbs: [u8; 20] = [
        0, 1, 1, 1, 4,
        0, 1, 1, 1, 4,
        0, 1, 1, 1, 4,
        0, 1, 1, 1, 4,
    ];
    let mut path = PathBuilder::new();
    let mut pt_ptr = 0;
    for verb in verbs {
        match verb {
            0 => {
                path.move_to(pts[pt_ptr]);
                pt_ptr += 1;
            }
            1 => {
                path.line_to(pts[pt_ptr]);
                pt_ptr += 1;
            }
            4 => {
                path.close();
            }
            _ => unreachable!(),
        }
    }
    let clip = Rect::new(0.0, 130.0, 772.0, 531.0);
    canvas.clip_rect(clip, None, None);
    canvas.draw_path(&path.detach(), &paint);
}

// Port of: gm/aarectmodes.cpp#L68-L81 (chrome/m156)
const G_MODES: [BlendMode; 12] = [
    BlendMode::Clear,
    BlendMode::Src,
    BlendMode::Dst,
    BlendMode::SrcOver,
    BlendMode::DstOver,
    BlendMode::SrcIn,
    BlendMode::DstIn,
    BlendMode::SrcOut,
    BlendMode::DstOut,
    BlendMode::SrcATop,
    BlendMode::DstATop,
    BlendMode::Xor,
];

// Port of: gm/aarectmodes.cpp#L83-L86 (chrome/m156)
const G_WIDTH: i32 = 64;
const G_HEIGHT: i32 = 64;
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const W: scalar = G_WIDTH as scalar;
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
const H: scalar = G_HEIGHT as scalar;

// Port of: gm/aarectmodes.cpp#L88-L111 (chrome/m156)
fn draw_cell(canvas: &Canvas, mode: BlendMode, a0: u8, a1: u8) -> scalar {
    let mut paint = Paint::default();
    paint.set_anti_alias(true);

    let mut r = Rect::from_wh(W, H);
    r.inset((W / 10.0, H / 10.0));

    paint.set_color(Color::BLUE);
    paint.set_alpha(a0);
    canvas.draw_oval(r, &paint);

    paint.set_color(Color::RED);
    paint.set_alpha(a1);
    paint.set_blend_mode(mode);

    let offset = 1.0 / 3.0;
    let rect = Rect::from_xywh(W / 4.0 + offset, H / 4.0 + offset, W / 2.0, H / 2.0);
    canvas.draw_rect(rect, &paint);

    H
}

// Port of: gm/aarectmodes.cpp#L113-L122 (chrome/m156)
fn make_bg_shader() -> Option<Shader> {
    let mut bm = Bitmap::new();
    bm.alloc_n32_pixels((2, 2), None);
    bm.set_addr32(0, 0, 0xFFFF_FFFF);
    bm.set_addr32(1, 1, 0xFFFF_FFFF);
    let v = pack_argb32(0xFF, 0xCE, 0xCF, 0xCE);
    bm.set_addr32(1, 0, v);
    bm.set_addr32(0, 1, v);

    bm.to_shader(
        (TileMode::Repeat, TileMode::Repeat),
        SamplingOptions::default(),
        &Matrix::scale((6.0, 6.0)),
    )
}

// Port of: gm/aarectmodes.cpp#L124-L164 (chrome/m156)
crate::def_simple_gm!(aarectmodes, canvas, 640, 480, {
    const G_ALPHA_VALUE: [u8; 3] = [0xFF, 0x88, 0x88];
    let mut bg_paint = Paint::default();
    bg_paint.set_shader(make_bg_shader());
    if false {
        // avoid bit rot, suppress warning
        test4(canvas);
    }
    let bounds = Rect::from_wh(W, H);

    canvas.translate((4.0, 4.0));

    for alpha in 0..4usize {
        canvas.save();
        canvas.save();
        for (i, mode) in G_MODES.iter().enumerate() {
            if 6 == i {
                canvas.restore();
                canvas.translate((W * 5.0, 0.0));
                canvas.save();
            }
            canvas.draw_rect(bounds, &bg_paint);
            canvas.save_layer(&SaveLayerRec::default().bounds(&bounds));
            let dy = draw_cell(
                canvas,
                *mode,
                G_ALPHA_VALUE[alpha & 1],
                G_ALPHA_VALUE[alpha & 2],
            );
            canvas.restore();

            canvas.translate((0.0, dy * 5.0 / 4.0));
        }
        canvas.restore();
        canvas.restore();
        canvas.translate((W * 5.0 / 4.0, 0.0));
    }
});
