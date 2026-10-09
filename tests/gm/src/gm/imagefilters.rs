// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagefilters.cpp (chrome/m156)
//
// Not ported here: `imagefilters_effect_order` (it needs SkShaderMaskFilter, which is not ported)
// and `multiple_filters` (it needs SkCanvasPriv::ScaledBackdropLayer and SkCanvas::FilterSpan).

// GM ports mirror the C++ integer and scalar casts.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::too_many_lines
)]

use crate::prelude::*;
use crate::tool_utils::{get_resource_as_image, make_surface};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::{AutoCanvasRestore, SaveLayerRec};
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::{blur, color_filter, matrix_convolution, matrix_transform};

// Port of: gm/imagefilters.cpp#L13-L30 (chrome/m156), do_draw
fn do_draw(canvas: &Canvas, mode: BlendMode, imf: Option<ImageFilter>) {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    canvas.clip_rect(Rect::new(0.0, 0.0, 220.0, 220.0), None, None);
    // want to force a layer, so modes like DstIn can combine meaningfully, but the final
    // image can still be shown against our default (opaque) background. non-opaque GMs
    // are a lot more trouble to compare/triage.
    canvas.save_layer(&SaveLayerRec::default());
    canvas.draw_color(Color::GREEN, None);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let r0 = Rect::new(10.0, 60.0, 210.0, 160.0);
    let r1 = Rect::new(60.0, 10.0, 160.0, 210.0);
    paint.set_color(Color::RED);
    canvas.draw_oval(r0, &paint);
    paint.set_color(Color::from(0x6600_00FF));
    paint.set_image_filter(imf);
    paint.set_blend_mode(mode);
    canvas.draw_oval(r1, &paint);
}

// Port of: gm/imagefilters.cpp#L32-L47 (chrome/m156), imagefilters_xfermodes
crate::def_simple_gm!(imagefilters_xfermodes, canvas, 480, 480, {
    canvas.translate((10.0, 10.0));
    // just need an imagefilter to trigger the code-path (which creates a tmp layer)
    let imf = matrix_transform(&Matrix::new_identity(), SamplingOptions::default(), None);
    let modes = [BlendMode::SrcATop, BlendMode::DstIn];
    for mode in modes {
        canvas.save();
        do_draw(canvas, mode, None);
        canvas.translate((240.0, 0.0));
        do_draw(canvas, mode, imf.clone());
        canvas.restore();
        canvas.translate((0.0, 240.0));
    }
});

// Port of: gm/imagefilters.cpp#L49-L53 (chrome/m156), make_image
fn make_image(canvas: &Canvas) -> Image {
    // SkImageInfo::MakeS32 has no colour space.
    let info = ImageInfo::new((100, 100), ColorType::N32, AlphaType::Premul, None);
    let mut surface = make_surface(canvas, &info, None).expect("a surface");
    surface
        .canvas()
        .draw_rect(Rect::new(25.0, 25.0, 75.0, 75.0), &Paint::default());
    surface.image_snapshot().expect("a snapshot")
}

// Port of: gm/imagefilters.cpp#L55-L72 (chrome/m156), fast_slow_blurimagefilter
crate::def_simple_gm!(fast_slow_blurimagefilter, canvas, 620, 260, {
    let image = make_image(canvas);
    let r = Rect::from_iwh(image.width(), image.height());
    canvas.translate((10.0, 10.0));
    let mut sigma: f32 = 8.0;
    while sigma <= 128.0 {
        let mut paint = Paint::default();
        paint.set_image_filter(blur(sigma, sigma, TileMode::Decal, None, None));
        canvas.save();
        // we outset the clip by 1, to fall out of the fast-case in drawImage
        // i.e. the clip is larger than the image
        let mut outset: f32 = 0.0;
        while outset <= 1.0 {
            canvas.save();
            let mut clip = r;
            clip.outset((outset, outset));
            canvas.clip_rect(clip, None, None);
            canvas.draw_image_with_sampling_options(
                &image,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            canvas.restore();
            canvas.translate((0.0, r.height() + 20.0));
            outset += 1.0;
        }
        canvas.restore();
        canvas.translate((r.width() + 20.0, 0.0));
        sigma *= 2.0;
    }
});

// Port of: gm/imagefilters.cpp#L74-L89 (chrome/m156), draw_set
fn draw_set(canvas: &Canvas, filters: &[Option<ImageFilter>]) {
    let r = Rect::new(30.0, 30.0, 230.0, 230.0);
    let offset: f32 = 250.0;
    let mut dx: f32 = 0.0;
    let mut dy: f32 = 0.0;
    for filter in filters {
        canvas.save();
        let mut moved = r;
        moved.offset((dx, dy));
        let rr = RRect::new_rect_xy(moved, 20.0, 20.0);
        canvas.clip_rrect(rr, None, true);
        let bounds = rr.bounds();
        let mut rec = SaveLayerRec::default().bounds(bounds);
        if let Some(filter) = filter {
            rec = rec.backdrop(filter);
        }
        canvas.save_layer(&rec);
        canvas.draw_color(Color::from(0x40FF_FFFF), None);
        canvas.restore();
        canvas.restore();
        if dx == 0.0 {
            dx = offset;
        } else {
            dx = 0.0;
            dy = offset;
        }
    }
}

// Port of: gm/imagefilters.cpp#L91-L131 (chrome/m156), SaveLayerWithBackdropGM
struct SaveLayerWithBackdropGm;

impl GM for SaveLayerWithBackdropGm {
    fn name(&self) -> String {
        "savelayer_with_backdrop".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(830, 550)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut cm = ColorMatrix::default();
        cm.set_saturation(10.0);
        let cf = color_filters::matrix(&cm, Clamp::Yes);
        #[rustfmt::skip]
        let kernel = [
            4.0, 0.0, 4.0,
            0.0, -15.0, 0.0,
            4.0, 0.0, 4.0,
        ];
        let filters: [Option<ImageFilter>; 4] = [
            blur(10.0, 10.0, TileMode::Decal, None, None),
            skia_rust_effects::image_filters::dilate((8.0, 8.0), None, None),
            matrix_convolution(
                (3, 3),
                &kernel,
                1.0,
                0.0,
                (0, 0),
                TileMode::Decal,
                true,
                None,
                None,
            ),
            color_filter(cf, None, None),
        ];
        // (sx, sy, tx, ty)
        let xforms: [(f32, f32, f32, f32); 4] = [
            (1.0, 1.0, 0.0, 0.0),
            (0.5, 0.5, 530.0, 0.0),
            (0.25, 0.25, 530.0, 275.0),
            (0.125, 0.125, 530.0, 420.0),
        ];
        let sampling = SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear);
        let image = get_resource_as_image("images/mandrill_512.png");
        canvas.translate((20.0, 20.0));
        for (sx, sy, tx, ty) in xforms {
            canvas.save();
            canvas.translate((tx, ty));
            canvas.scale((sx, sy));
            if let Some(image) = &image {
                canvas.draw_image_with_sampling_options(image, (0.0, 0.0), sampling, None);
            }
            draw_set(canvas, &filters);
            canvas.restore();
        }
    }
}

// Port of: gm/imagefilters.cpp#L131 (chrome/m156), DEF_GM(return new SaveLayerWithBackdropGM();)
crate::def_gm!(
    SaveLayerWithBackdropGM_ = "SaveLayerWithBackdropGM()",
    SaveLayerWithBackdropGm
);
