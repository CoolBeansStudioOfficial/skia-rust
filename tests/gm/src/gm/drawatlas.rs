// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/drawatlas.cpp (chrome/m156)

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::items_after_statements,
    clippy::needless_range_loop,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_core::blend_mode::BlendMode;

use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{SCALAR_PI, degrees_to_radians, scalar_cos, scalar_sin};
use skia_rust_core::text_blob::TextBlob;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_font;

// Port of: gm/drawatlas.cpp#L20-L38 (chrome/m156)
fn make_atlas(caller: &Canvas, target: Rect) -> Option<Image> {
    let info = ImageInfo::new_n32_premul((100, 100), None);
    let mut surface = caller
        .new_surface(&info, None)
        .or_else(|| surfaces::raster(&info, None, None))?;
    let canvas = surface.canvas();
    canvas.clear(Color::RED);
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Clear);
    let mut r = target;
    r.inset((-1.0, -1.0));
    canvas.draw_rect(r, &paint);
    paint.set_blend_mode(BlendMode::SrcOver);
    paint.set_color(Color::BLUE);
    paint.set_anti_alias(true);
    canvas.draw_oval(target, &paint);
    surface.image_snapshot()
}

// Port of: gm/drawatlas.cpp#L44-L55 (chrome/m156)
#[derive(Clone, Copy)]
struct Rec {
    scale: f32,
    degrees: f32,
    tx: f32,
    ty: f32,
}

impl Rec {
    // Port of: gm/drawatlas.cpp#L46-L54 (chrome/m156)
    fn apply(&self, xform: &mut RSXform) {
        let rad = degrees_to_radians(self.degrees);
        xform.scos = self.scale * scalar_cos(rad);
        xform.ssin = self.scale * scalar_sin(rad);
        xform.tx = self.tx;
        xform.ty = self.ty;
    }
}

// Port of: gm/drawatlas.cpp#L18-L77 (chrome/m156)
struct DrawAtlasGm;

impl GM for DrawAtlasGm {
    fn name(&self) -> String {
        "draw-atlas".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    // Port of: gm/drawatlas.cpp#L58-L84 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        let target = Rect::from_ltrb(50.0, 50.0, 80.0, 90.0);
        let atlas = make_atlas(canvas, target).expect("an atlas");
        let rec: [Rec; 4] = [
            // just translate
            Rec {
                scale: 1.0,
                degrees: 0.0,
                tx: 10.0,
                ty: 10.0,
            },
            // scale + translate
            Rec {
                scale: 2.0,
                degrees: 0.0,
                tx: 110.0,
                ty: 10.0,
            },
            // rotate + translate
            Rec {
                scale: 1.0,
                degrees: 30.0,
                tx: 210.0,
                ty: 10.0,
            },
            // scale + rotate + translate
            Rec {
                scale: 2.0,
                degrees: -30.0,
                tx: 310.0,
                ty: 30.0,
            },
        ];
        const N: usize = 4;
        let mut xform = [RSXform::default(); N];
        let mut tex = [target; N];
        let mut colors = [Color::BLACK; N];
        for i in 0..N {
            rec[i].apply(&mut xform[i]);
            tex[i] = target;
            colors[i] = Color::new(0x80FF0000 + (i as u32 * 40 * 256));
        }
        let mut paint = Paint::default();
        paint.set_anti_alias(true);
        let sampling = SamplingOptions::from(FilterMode::Linear);
        canvas.draw_atlas(
            &atlas,
            &xform,
            &tex,
            None,
            BlendMode::Dst,
            sampling,
            None,
            Some(&paint),
        );
        canvas.translate((0.0, 100.0));
        canvas.draw_atlas(
            &atlas,
            &xform,
            &tex,
            Some(&colors[..]),
            BlendMode::SrcIn,
            sampling,
            None,
            Some(&paint),
        );
    }
}

// Port of: gm/drawatlas.cpp#L79 (chrome/m156)
crate::def_gm!(DrawAtlasGM, DrawAtlasGm);

// Port of: gm/drawatlas.cpp#L237-L262 (chrome/m156)
crate::def_simple_gm!(blob_rsxform, canvas, 500, 100, {
    let mut font = default_portable_font();
    font.set_size(50.0);
    let text = b"CrazyXform";
    let len = text.len();
    let mut xforms = [RSXform::default(); 10];
    let mut x: f32 = 0.0;
    let y: f32 = 0.0;
    for i in 0..len {
        let scale = scalar_sin(i as f32 * SCALAR_PI / (len - 1) as f32) * 0.75 + 0.5;
        xforms[i] = RSXform::new(scale, 0.0, (x, y));
        x += 50.0 * scale;
    }
    let blob = TextBlob::from_rsxform(text, TextEncoding::UTF8, &xforms[..len], &font)
        .expect("a text blob");
    let offset = Point::new(20.0, 70.0);
    let mut paint = Paint::default();
    paint.set_color(Color::new(0xFFCCCCCC));
    let mut bounds = *blob.bounds();
    bounds.offset((offset.x, offset.y));
    canvas.draw_rect(bounds, &paint);
    paint.set_color(Color::BLACK);
    canvas.draw_text_blob(&blob, offset, &paint);
});
