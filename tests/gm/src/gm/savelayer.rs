// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/savelayer.cpp (chrome/m156)
//
// Ported here: `savelayer_f16`, `savelayer_initfromprev`, `save_behind` and `skbug_14554`.

// The int-to-scalar cast of the layer count mirrors the C++ arithmetic (exact in f32).
#![allow(clippy::cast_precision_loss)]

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{
    AutoCanvasRestore, Canvas, ImageSetEntry, PointMode, QuadAAFlags, SaveLayerFlags, SaveLayerRec,
    SrcRectConstraint,
};
use skia_rust_core::color::{Color, Color4f, colors};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::paint::{Cap, Paint, Style};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::Rect;
use skia_rust_core::rsxform::RSXform;
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::text_blob::TextBlob;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::vertices::{VertexMode, Vertices};
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_tools::font_tool_utils::default_portable_font;

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

const SAVE_BEHIND_MARGIN: f32 = 80.0;

// Port of: gm/savelayer.cpp#L67-L112 (chrome/m156), draw_cell
// The single-letter names mirror the C++ (`r`, `p`, `c`, `w`, `h`).
#[allow(clippy::many_single_char_names)]
fn draw_cell(canvas: &Canvas, blob: &TextBlob, c: Color, w: f32, h: f32, use_draw_behind: bool) {
    let mut r = Rect::new(0.0, 0.0, w, h);
    let mut p = Paint::default();
    p.set_color(c);
    p.set_blend_mode(BlendMode::Src);
    canvas.draw_rect(r, &p);
    p.set_blend_mode(BlendMode::SrcOver);

    r = Rect::new(w - SAVE_BEHIND_MARGIN, r.top(), r.right(), r.bottom());

    // save the behind image
    let sc0 = canvas.save_count();
    let sc1 = canvas.save_behind(Some(&r));
    let sc2 = canvas.save_count();
    debug_assert_eq!(sc0, sc1);
    debug_assert_eq!(sc0 + 1, sc2);

    // draw the foreground (including over the 'behind' section)
    p.set_color(Color::BLACK);
    canvas.draw_text_blob(blob, (10.0, 30.0), &p);

    // draw the treatment
    let pts = [Point::new(r.left(), 0.0), Point::new(r.right(), 0.0)];
    let colors = [
        Color4f::new(0.0, 0.0, 0.0, f32::from(0x88_u8) / 255.0),
        Color4f::new(0.0, 0.0, 0.0, 0.0),
    ];
    p.set_shader(shaders::linear_gradient(
        (pts[0], pts[1]),
        &Gradient::new(
            Colors::new(&colors, None, TileMode::Clamp, None),
            Interpolation::default(),
        ),
        None,
    ));
    p.set_blend_mode(BlendMode::DstIn);

    if use_draw_behind {
        canvas.draw_clipped_to_save_behind(&p);
    } else {
        canvas.draw_rect(r, &p);
    }

    // this should restore the behind image
    canvas.restore();
    debug_assert_eq!(canvas.save_count(), sc0);

    // just outline where we expect the treatment to appear
    p.reset();
    p.set_style(Style::Stroke);
    p.set_alpha_f(0.25);
}

// Port of: gm/savelayer.cpp#L114-L126 (chrome/m156), draw_list
fn draw_list(canvas: &Canvas, blob: &TextBlob, use_draw_behind: bool) {
    let guard = AutoCanvasRestore::guard(canvas, true);
    let canvas: &Canvas = &guard;

    let mut rand = Random::default();
    let w: f32 = 400.0;
    let h: f32 = 40.0;
    for _ in 0..8 {
        // ensure we're opaque
        let c = (rand.next_u() & 0xFF_FF_FF) | 0x80_00_00_00;
        draw_cell(canvas, blob, Color::from(c), w, h, use_draw_behind);
        canvas.translate((0.0, h));
    }
}

// Port of: gm/savelayer.cpp#L128-L147 (chrome/m156), DEF_SIMPLE_GM(save_behind)
crate::def_simple_gm!(save_behind, canvas, 830, 670, {
    let mut font = default_portable_font();
    font.set_size(30.0);

    let text = b"This is a very long line of text";
    let blob = TextBlob::from_text(text, TextEncoding::UTF8, &font).expect("a text blob");

    for use_draw_behind in [false, true] {
        canvas.save();

        draw_list(canvas, &blob, use_draw_behind);
        canvas.translate((0.0, 350.0));
        let bounds = Rect::new(0.0, 0.0, 400.0, 320.0);
        let rec = SaveLayerRec::default().bounds(&bounds);
        canvas.save_layer(&rec);
        draw_list(canvas, &blob, use_draw_behind);
        canvas.restore();

        canvas.restore();
        canvas.translate((430.0, 0.0));
    }
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
