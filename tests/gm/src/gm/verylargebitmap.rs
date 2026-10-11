// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/verylargebitmap.cpp (chrome/m156)

use crate::prelude::*;
use crate::tool_utils::int_to_scalar;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image::Image;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_core::tiled_image_utils;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders as gradient_shaders};
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture};
use skia_rust_raster::surfaces;

// Port of: gm/verylargebitmap.cpp#L32-L41 (chrome/m156), draw
fn draw(canvas: &Canvas, width: i32, height: i32, colors: [Color; 2]) {
    let center = Point::new(int_to_scalar(width) / 2.0, int_to_scalar(height) / 2.0);
    let radius: f32 = 40.0;
    let c4 = [Color4f::from_color(colors[0]), Color4f::from_color(colors[1])];
    let gradient = Gradient::new(
        Colors::new(&c4, None, TileMode::Mirror, None),
        Interpolation::default(),
    );
    let mut paint = Paint::default();
    paint.set_shader(gradient_shaders::radial_gradient(
        (center, radius),
        &gradient,
        None,
    ));
    paint.set_blend_mode(BlendMode::Src);
    canvas.draw_paint(&paint);
}

// Port of: gm/verylargebitmap.cpp#L43-L47 (chrome/m156), make_raster_image
fn make_raster_image(width: i32, height: i32, colors: [Color; 2]) -> Option<Image> {
    let mut surface = surfaces::raster_n32_premul((width, height))?;
    draw(&surface.shared_canvas(), width, height, colors);
    surface.image_snapshot()
}

// Port of: gm/verylargebitmap.cpp#L49-L58 (chrome/m156), make_picture_image
fn make_picture_image(width: i32, height: i32, colors: [Color; 2]) -> Option<Image> {
    let mut recorder = PictureRecorder::new();
    let bounds = Rect::from_iwh(width, height);
    draw(recorder.begin_recording(bounds, false), width, height, colors);
    let picture = recorder.finish_recording_as_picture(None)?;
    deferred_from_picture(
        picture,
        (width, height),
        None,
        None,
        BitDepth::U8,
        Some(ColorSpace::new_srgb()),
        SurfaceProps::default(),
    )
}

// Port of: gm/verylargebitmap.cpp#L60 (chrome/m156), ImageMakerProc
type ImageMakerProc = fn(i32, i32, [Color; 2]) -> Option<Image>;

// Port of: include/core/SkTiledImageUtils.h#L52-L66 (chrome/m156), the `DrawImageRect(canvas,
// image, dst)` overload: the whole image, drawn into `dst` with the default sampling and paint.
fn draw_image_rect_dst(canvas: &Canvas, image: &Image, dst: &Rect) {
    let src = Rect::from_iwh(image.width(), image.height());
    tiled_image_utils::draw_image_rect(
        canvas,
        image,
        &src,
        dst,
        &SamplingOptions::default(),
        None,
        SrcRectConstraint::Fast,
    );
}

// Port of: gm/verylargebitmap.cpp#L62-L105 (chrome/m156), show_image
fn show_image(
    canvas: &Canvas,
    width: i32,
    height: i32,
    colors: [Color; 2],
    proc_: ImageMakerProc,
    manually_tile: bool,
) {
    let Some(image) = proc_(width, height, colors) else {
        return;
    };

    let mut border_paint = Paint::default();
    border_paint.set_style(Style::Stroke);

    let mut dst_rect = Rect::from_wh(128.0, 128.0);
    canvas.save();
    canvas.clip_rect(dst_rect, ClipOp::Intersect, false);
    if manually_tile {
        tiled_image_utils::draw_image(
            canvas,
            &image,
            0.0,
            0.0,
            &SamplingOptions::default(),
            None,
            SrcRectConstraint::Fast,
        );
    } else {
        canvas.draw_image(&image, (0.0, 0.0), None);
    }
    canvas.restore();
    canvas.draw_rect(&dst_rect, &border_paint);

    dst_rect.offset((150.0, 0.0));
    let hw = width / 2;
    let hh = height / 2;
    let subset = Rect::from_ltrb(
        int_to_scalar(hw - 64),
        int_to_scalar(hh - 32),
        int_to_scalar(hw + 64),
        int_to_scalar(hh + 32),
    );
    if manually_tile {
        tiled_image_utils::draw_image_rect(
            canvas,
            &image,
            &subset,
            &dst_rect,
            &SamplingOptions::default(),
            None,
            SrcRectConstraint::Strict,
        );
    } else {
        canvas.draw_image_rect_with_sampling_options(
            &image,
            Some((&subset, SrcRectConstraint::Strict)),
            &dst_rect,
            SamplingOptions::default(),
            &Paint::default(),
        );
    }
    canvas.draw_rect(&dst_rect, &border_paint);

    dst_rect.offset((150.0, 0.0));
    if manually_tile {
        draw_image_rect_dst(canvas, &image, &dst_rect);
    } else {
        canvas.draw_image_rect_with_sampling_options(
            &image,
            None,
            &dst_rect,
            SamplingOptions::default(),
            &Paint::default(),
        );
    }
    canvas.draw_rect(&dst_rect, &border_paint);
}

// Port of: gm/verylargebitmap.cpp#L107-L161 (chrome/m156), VeryLargeBitmapGM
struct VeryLargeBitmapGm {
    proc_: ImageMakerProc,
    base_name: &'static str,
    manually_tile: bool,
}

impl VeryLargeBitmapGm {
    fn new(proc_: ImageMakerProc, base_name: &'static str, manually_tile: bool) -> Self {
        Self {
            proc_,
            base_name,
            manually_tile,
        }
    }
}

impl GM for VeryLargeBitmapGm {
    // Port of: gm/verylargebitmap.cpp#L119-L127 (chrome/m156), getName
    fn name(&self) -> String {
        let mut name = self.base_name.to_string();
        if self.manually_tile {
            name.push_str("_manual");
        }
        name
    }

    // Port of: gm/verylargebitmap.cpp#L129 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(500, 600)
    }

    // Port of: gm/verylargebitmap.cpp#L131-L160 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let very_big = 65 * 1024; // 64K < size
        let big = 33 * 1024; // 32K < size < 64K
        // smaller than many max texture sizes, but large enough to gpu-tile for memory reasons.
        let medium = 5 * 1024;
        let small = 150;

        canvas.translate((10.0, 10.0));
        show_image(
            canvas,
            small,
            small,
            [Color::RED, Color::GREEN],
            self.proc_,
            self.manually_tile,
        );
        canvas.translate((0.0, 150.0));
        show_image(
            canvas,
            big,
            small,
            [Color::BLUE, Color::MAGENTA],
            self.proc_,
            self.manually_tile,
        );
        canvas.translate((0.0, 150.0));
        show_image(
            canvas,
            medium,
            medium,
            [Color::MAGENTA, Color::YELLOW],
            self.proc_,
            self.manually_tile,
        );
        canvas.translate((0.0, 150.0));
        // This used to be big enough that we didn't draw on CPU, but now we do.
        show_image(
            canvas,
            very_big,
            small,
            [Color::GREEN, Color::YELLOW],
            self.proc_,
            self.manually_tile,
        );
    }
}

// Port of: gm/verylargebitmap.cpp#L163-L170 (chrome/m156), DEF_GM registrations
crate::def_gm!(
    VeryLargeBitmapGM_raster_false = "VeryLargeBitmapGM(make_raster_image, \"verylargebitmap\", /* manuallyTile= */ false)",
    VeryLargeBitmapGm::new(make_raster_image, "verylargebitmap", false)
);
crate::def_gm!(
    VeryLargeBitmapGM_raster_true = "VeryLargeBitmapGM(make_raster_image, \"verylargebitmap\", /* manuallyTile= */ true)",
    VeryLargeBitmapGm::new(make_raster_image, "verylargebitmap", true)
);
crate::def_gm!(
    VeryLargeBitmapGM_picture_false = "VeryLargeBitmapGM(make_picture_image, \"verylarge_picture_image\", /* manuallyTile= */ false)",
    VeryLargeBitmapGm::new(make_picture_image, "verylarge_picture_image", false)
);
crate::def_gm!(
    VeryLargeBitmapGM_picture_true = "VeryLargeBitmapGM(make_picture_image, \"verylarge_picture_image\", /* manuallyTile= */ true)",
    VeryLargeBitmapGm::new(make_picture_image, "verylarge_picture_image", true)
);
