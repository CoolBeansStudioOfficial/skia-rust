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
use skia_rust_core::color_filters;
use skia_rust_core::vertices::{VertexMode, Vertices};

use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;

use skia_rust_core::color::{Color, Color4f, colors};
use skia_rust_core::font::Font;
use skia_rust_core::font_priv::get_font_bounds;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::path::Path;
use skia_rust_core::path_measure::PathMeasure;
use skia_rust_core::path_types::PathDirection;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::scalar::{SCALAR_PI, degrees_to_radians, scalar_cos, scalar_sin};
use skia_rust_core::shader::Shader;
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
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

// Port of: gm/drawatlas.cpp#L134-L136 (chrome/m156), the `std::max` of two scalars
fn sk_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

// Port of: gm/drawatlas.cpp#L133-L183 (chrome/m156), draw_text_on_path
fn draw_text_on_path(
    canvas: &Canvas,
    text: &[u8],
    xy: &[Point],
    path: &Path,
    font: &Font,
    paint: &Paint,
    baseline_offset: f32,
) {
    let meas = PathMeasure::new(path, false, None);

    let count = font.count_text(text, TextEncoding::UTF8);
    let mut xform = vec![RSXform::default(); count];
    let mut widths = vec![0.0_f32; count];

    // Compute a conservative bounds so we can cull the draw
    let fontb = get_font_bounds(font);
    let max = sk_max(
        sk_max(fontb.left.abs(), fontb.right.abs()),
        sk_max(fontb.top.abs(), fontb.bottom.abs()),
    );
    let mut bounds = *path.bounds();
    bounds.outset((max, max));

    let mut glyphs = vec![GlyphId::default(); count];
    font.text_to_glyphs(text, TextEncoding::UTF8, &mut glyphs);
    font.get_widths(&glyphs, &mut widths);

    for i in 0..count {
        // we want to position each character on the center of its advance
        let offset = widths[i] / 2.0;
        let mut pos = Point::new(0.0, 0.0);
        let mut tan = Vector::new(0.0, 0.0);
        if !meas.get_pos_tan(xy[i].x + offset, Some(&mut pos), Some(&mut tan)) {
            pos = xy[i];
            tan.set(1.0, 0.0);
        }
        pos += Vector::new(-tan.y, tan.x) * baseline_offset;

        xform[i].scos = tan.x;
        xform[i].ssin = tan.y;
        xform[i].tx = pos.x - tan.y * xy[i].y - tan.x * offset;
        xform[i].ty = pos.y + tan.x * xy[i].y - tan.y * offset;
    }

    if let Some(blob) = TextBlob::from_rsxform_glyphs(&glyphs, &xform, font) {
        canvas.draw_text_blob(&blob, (0.0, 0.0), paint);
    }

    {
        let mut p = Paint::default();
        p.set_style(Style::Stroke);
        canvas.draw_rect(bounds, &p);
    }
}

// Port of: gm/drawatlas.cpp#L185-L189 (chrome/m156), make_shader
fn make_shader() -> Option<Shader> {
    let pts = [Point::new(0.0, 0.0), Point::new(220.0, 0.0)];
    let grad_colors = [colors::RED, colors::BLUE];
    gradient_shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&grad_colors, None, TileMode::Mirror, None),
            Interpolation::default(),
        ),
        None,
    )
}

// Port of: gm/drawatlas.cpp#L191-L225 (chrome/m156), drawTextPath
fn draw_text_path(canvas: &Canvas, do_stroke: bool) {
    let text0 = b"ABCDFGHJKLMNOPQRSTUVWXYZ";
    let n = text0.len();
    let mut pos = vec![Point::new(0.0, 0.0); n];

    let mut font = default_portable_font();
    font.set_size(100.0);

    let mut paint = Paint::default();
    paint.set_shader(make_shader());
    paint.set_anti_alias(true);
    if do_stroke {
        paint.set_style(Style::Stroke);
        paint.set_stroke_width(2.25);
        paint.set_stroke_join(skia_rust_core::paint::Join::Round);
    }

    let mut x: f32 = 0.0;
    for i in 0..n {
        pos[i].set(x, 0.0);
        x += font
            .measure_text(&text0[i..=i], TextEncoding::UTF8, Some(&paint))
            .0;
    }

    let mut path = Path::default();
    let baseline_offset: f32 = -5.0;

    let dirs = [PathDirection::CW, PathDirection::CCW];
    for d in dirs {
        path = Path::oval(Rect::from_xywh(160.0, 160.0, 540.0, 540.0), d);
        draw_text_on_path(canvas, text0, &pos, &path, &font, &paint, baseline_offset);
    }

    paint.reset();
    paint.set_style(Style::Stroke);
    canvas.draw_path(&path, &paint);
}

// Port of: gm/drawatlas.cpp#L227-L235 (chrome/m156), drawTextRSXform
crate::def_simple_gm!(drawTextRSXform, canvas, 430, 860, {
    canvas.scale((0.5, 0.5));
    let do_stroke = [false, true];
    for st in do_stroke {
        draw_text_path(canvas, st);
        canvas.translate((0.0, 860.0));
    }
});

// Port of: gm/drawatlas.cpp#L304-L310 (chrome/m156), make_vertices
fn make_vertices(image: &Image, r: Rect, color: Color) -> Vertices {
    let _ = image;
    let pos = r.to_quad(None);
    let colors = [color, color, color, color];
    Vertices::new_copy(
        VertexMode::TriangleFan,
        &pos,
        Some(&pos),
        Some(&colors),
        None,
    )
    .expect("a triangle fan")
}

// Port of: gm/drawatlas.cpp#L323-L359 (chrome/m156), DEF_SIMPLE_GM(compare_atlas_vertices)
crate::def_simple_gm!(compare_atlas_vertices, canvas, 560, 585, {
    let tex = Rect::from_wh(128.0, 128.0);
    let xform = RSXform::new(1.0, 0.0, (0.0, 0.0));
    let color = Color::new(0x8844_88CC);

    let image = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
        .expect("images/mandrill_128.png");
    let verts = make_vertices(&image, tex, color);
    let filters = [
        None,
        color_filters::blend(
            Color4f::from_color(Color::new(0xFF00_FF88)),
            None,
            BlendMode::Modulate,
        ),
    ];
    let modes = [BlendMode::SrcOver, BlendMode::Plus];

    canvas.translate((10.0, 10.0));
    let mut paint = Paint::default();
    for mode in modes {
        for alpha in [1.0_f32, 0.5] {
            paint.set_alpha_f(alpha);
            canvas.save();
            for cf in &filters {
                paint.set_color_filter(cf.clone());
                canvas.draw_atlas(
                    &image,
                    &[xform],
                    &[tex],
                    Some(&[color][..]),
                    mode,
                    SamplingOptions::default(),
                    Some(tex),
                    &paint,
                );
                canvas.translate((128.0, 0.0));
                paint.set_shader(image.to_shader(None, SamplingOptions::default(), None));
                canvas.draw_vertices(&verts, mode, &paint);
                paint.set_shader(None);
                canvas.translate((145.0, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, 145.0));
        }
    }
});
