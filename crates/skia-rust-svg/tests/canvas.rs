// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Output of the SVG canvas for scenes that the ported Skia tests do not cover.

use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient_shader;
use skia_rust_raster::surfaces;
use skia_rust_svg::canvas::{Canvas, Flags};

fn svg(draw: impl FnOnce(&Canvas), flags: Option<Flags>) -> String {
    let canvas = Canvas::new(Rect::from_wh(100.0, 80.0), flags);
    draw(&canvas);
    let data = canvas.end();
    String::from_utf8(data.as_bytes().to_vec()).expect("utf-8")
}

#[test]
fn rect_with_transform_and_clip() {
    let out = svg(
        |canvas| {
            let mut paint = Paint::default();
            paint.set_color(Color::new(0x80_12_34_56));
            canvas.translate((10.5, 20.0));
            canvas.clip_rect(
                Rect::from_xywh(0.0, 0.0, 50.0, 40.0),
                ClipOp::Intersect,
                false,
            );
            canvas.clip_rrect(
                RRect::new_rect_xy(Rect::from_xywh(1.0, 2.0, 30.0, 30.0), 4.0, 5.0),
                ClipOp::Intersect,
                false,
            );
            canvas.rotate(30.0, None);
            canvas.draw_rect(Rect::from_xywh(0.0, 0.0, 10.0, 10.0), &paint);
            paint.set_style(Style::Stroke);
            paint.set_stroke_width(0.0);
            canvas.draw_oval(Rect::from_xywh(1.0, 1.0, 10.0, 6.0), &paint);
        },
        None,
    );
    println!("{out}");
    assert!(out.starts_with("<?xml version=\"1.0\" encoding=\"utf-8\" ?>\n<svg xmlns="));
    assert!(out.contains("<clipPath id=\"cl_"));
    assert!(out.contains("fill-opacity=\"0.50196081\""));
    assert!(out.contains("vector-effect=\"non-scaling-stroke\""));
    assert!(out.trim_end().ends_with("</svg>"));
}

#[test]
fn gradient_and_image() {
    let out = svg(
        |canvas| {
            let mut paint = Paint::default();
            let colors = [
                Color4f::new(1.0, 0.0, 0.0, 1.0),
                Color4f::new(0.0, 0.0, 1.0, 0.5),
            ];
            paint.set_shader(gradient_shader::linear(
                ((0.0, 0.0), (100.0, 0.0)),
                (&colors[..], None),
                None,
                TileMode::Clamp,
                None,
                None,
            ));
            canvas.draw_paint(&paint);

            let mut surface =
                surfaces::raster(&ImageInfo::new_n32_premul((2, 2), None), None, None).expect("s");
            surface.canvas().clear(Color::RED);
            let image = surface.image_snapshot().expect("image");
            canvas.draw_image_rect(
                &image,
                Some((
                    &Rect::from_xywh(0.0, 0.0, 1.0, 1.0),
                    SrcRectConstraint::Strict,
                )),
                Rect::from_xywh(10.0, 10.0, 20.0, 20.0),
                &Paint::default(),
            );
        },
        Some(Flags::NO_PRETTY_XML),
    );
    println!("{out}");
    assert!(out.contains("<linearGradient id=\"gradient_0\""));
    assert!(out.contains("data:image/png;base64,"));
}
