// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/savelayer.cpp (chrome/m156)
//
// Ported here: `savelayer_f16`, `savelayer_initfromprev` and `skbug_14554`. `save_behind` needs
// SkCanvasPriv::SaveBehind/DrawBehind, which is not on main yet.

// The int-to-scalar cast of the layer count mirrors the C++ arithmetic (exact in f32).
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{
    Canvas, ImageSetEntry, PointMode, QuadAAFlags, SaveLayerFlags, SaveLayerRec, SrcRectConstraint,
};
use skia_rust_core::color::{Color, colors};
use skia_rust_core::image::Image;
use skia_rust_core::paint::{Cap, Paint};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};

// Port of: gm/savelayer.cpp#L179-L199 (chrome/m156), DEF_SIMPLE_GM(savelayer_f16)
crate::def_simple_gm!(savelayer_f16, canvas, 900, 300, {
    let n = 15;
    let r = Rect::from_ltrb(0.0, 0.0, 300.0, 300.0);
    let mut paint = Paint::default();

    let grad_colors = [colors::RED, colors::GREEN, colors::BLUE, colors::RED];
    let grad = Gradient::new(
        Colors::new(&grad_colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    // SkShaders::SweepGradient(center, colors) sweeps the full circle.
    paint.set_shader(shaders::sweep_gradient(
        (r.center_x(), r.center_y()),
        (0.0, 360.0),
        &grad,
        None,
    ));

    canvas.draw_oval(r, &paint);

    paint.set_alpha_f(1.0 / n as f32);
    paint.set_blend_mode(BlendMode::Plus);

    for flags in [SaveLayerFlags::empty(), SaveLayerFlags::F16_COLOR_TYPE] {
        canvas.translate((r.width(), 0.0));

        canvas.save_layer(&SaveLayerRec::default().flags(flags));
        for _ in 0..n {
            canvas.draw_oval(r, &paint);
        }
        canvas.restore();
    }
});

// Port of: gm/savelayer.cpp#L44-L65 (chrome/m156), DEF_SIMPLE_GM(savelayer_initfromprev)
crate::def_simple_gm!(savelayer_initfromprev, canvas, 256, 256, {
    canvas.draw_image(
        crate::tool_utils::get_resource_as_image("images/mandrill_256.png")
            .expect("mandrill_256.png"),
        (0.0, 0.0),
        None,
    );

    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Plus);
    let rec = SaveLayerRec::default()
        .flags(SaveLayerFlags::INIT_WITH_PREVIOUS)
        .paint(&paint);
    canvas.save_layer(&rec);
    paint.set_blend_mode(BlendMode::Clear);
    canvas.draw_circle((128.0, 128.0), 96.0, &paint);
    canvas.restore();
});

// Port of: gm/savelayer.cpp#L157-L169 (chrome/m156), draw_atlas
fn draw_atlas(canvas: &Canvas, image: &Image) {
    let xforms = [
        RSXform::new(1.0, 0.0, (0.0, 0.0)),
        RSXform::new(1.0, 0.0, (50.0, 50.0)),
    ];
    let tex = [
        Rect::from_ltrb(0.0, 0.0, 100.0, 100.0),
        Rect::from_ltrb(0.0, 0.0, 100.0, 100.0),
    ];
    let colors = [Color::new(0xffff_ffff), Color::new(0xffff_ffff)];
    let paint = Paint::default();
    canvas.draw_atlas(
        image,
        &xforms,
        &tex,
        Some(&colors[..]),
        BlendMode::SrcIn,
        SamplingOptions::from(FilterMode::Nearest),
        None,
        &paint,
    );
}

// Port of: gm/savelayer.cpp#L171-L183 (chrome/m156), draw_vertices
fn draw_vertices(canvas: &Canvas, image: &Image) {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(0.0, 100.0),
        Point::new(100.0, 100.0),
        Point::new(100.0, 0.0),
        Point::new(100.0, 100.0),
        Point::new(0.0, 100.0),
    ];
    let verts =
        Vertices::new_copy(VertexMode::Triangles, &pts, None, None, None).expect("a triangle list");

    let mut paint = Paint::default();
    paint.set_shader(image.to_shader(None, SamplingOptions::from(FilterMode::Nearest), None));

    canvas.draw_vertices(&verts, BlendMode::Src, &paint);
}

// Port of: gm/savelayer.cpp#L185-L193 (chrome/m156), draw_points
fn draw_points(canvas: &Canvas, image: &Image) {
    let pts = [Point::new(50.0, 50.0), Point::new(75.0, 75.0)];
    let mut paint = Paint::default();
    paint.set_shader(image.to_shader(None, SamplingOptions::from(FilterMode::Nearest), None));
    paint.set_stroke_width(100.0);
    paint.set_stroke_cap(Cap::Square);

    canvas.draw_points(PointMode::Points, &pts, &paint);
}

// Port of: gm/savelayer.cpp#L195-L206 (chrome/m156), draw_image_set
fn draw_image_set(canvas: &Canvas, image: &Image) {
    let r = Rect::from_ltrb(0.0, 0.0, 100.0, 100.0);
    let entries = [
        ImageSetEntry::new(image.clone(), r, r, None, 1.0, QuadAAFlags::NONE, false),
        ImageSetEntry::new(
            image.clone(),
            r,
            r.with_offset((50.0, 50.0)),
            None,
            1.0,
            QuadAAFlags::NONE,
            false,
        ),
    ];

    let paint = Paint::default();
    canvas.experimental_draw_edge_aa_image_set(
        &entries,
        &[],
        &[],
        SamplingOptions::from(FilterMode::Nearest),
        Some(&paint),
        SrcRectConstraint::Strict,
    );
}

// Port of: gm/savelayer.cpp#L233-L260 (chrome/m156), DEF_SIMPLE_GM(skbug_14554)
crate::def_simple_gm!(
    #[ignore = "see notes/gm_savelayer_cpp_skbug_14554.md"]
    skbug_14554,
    canvas,
    310,
    630,
    {
        let image = crate::tool_utils::get_resource_as_image("images/mandrill_128.png")
            .expect("images/mandrill_128.png");
        let mut rec = PictureRecorder::new();

        let procs: [fn(&Canvas, &Image); 4] =
            [draw_atlas, draw_vertices, draw_points, draw_image_set];
        for proc_fn in procs {
            canvas.save();
            for inject_extra_op in [false, true] {
                let c = rec.begin_recording(Rect::from_ltrb(0.0, 0.0, 150.0, 150.0), false);
                c.save_layer_alpha_f(None, 0.6);
                proc_fn(c, &image);
                // For the second draw of each test-case, we inject an extra (useless) operation, which
                // inhibits the optimization and produces the correct result.
                if inject_extra_op {
                    c.translate((1.0, 0.0));
                }
                c.restore();

                let pic = rec.finish_recording_as_picture(None).expect("a picture");

                canvas.draw_picture(&pic, None, None);
                canvas.translate((160.0, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, 160.0));
        }
    }
);
