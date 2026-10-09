// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/imagemagnifier.cpp (chrome/m156)

// GM ports mirror the C++ source line by line: int/float conversions, local constants and long
// bodies are kept as they are there.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::items_after_statements,
    clippy::too_many_lines
)]

use crate::prelude::*;
use crate::tool_utils::color_to_565;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SaveLayerRec};
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::point::Point;
use skia_rust_core::random::Random;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_effects::image_filters::{image_sampled, magnifier};
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::default_portable_font;

const WIDTH: i32 = 500;
const HEIGHT: i32 = 500;

/// `draw_content(canvas, maxTextSize, count)`: `count` random strings of the default portable font.
// Port of: gm/imagemagnifier.cpp#L19-L32 (chrome/m156)
fn draw_content(canvas: &Canvas, max_text_size: f32, count: i32) {
    let text = "The quick brown fox jumped over the lazy dog.";
    let mut rand = Random::default();
    let mut font: Font = default_portable_font();
    for _ in 0..count {
        let x = rand.next_u_less_than(WIDTH as u32) as i32;
        let y = rand.next_u_less_than(HEIGHT as u32) as i32;
        let mut paint = Paint::default();
        paint.set_color(color_to_565(rand.next_bits(24) | 0xFF00_0000));
        font.set_size(rand.next_range_scalar(0.0, max_text_size));
        canvas.draw_str(text, (x as f32, y as f32), &font, &paint);
    }
}

/// `make_img()`: a 256 by 256 blue grid on a transparent background.
// Port of: gm/imagemagnifier.cpp#L60-L77 (chrome/m156)
fn make_img() -> Option<Image> {
    const WIDTH_HEIGHT: i32 = 256;
    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((WIDTH_HEIGHT, WIDTH_HEIGHT), false);
    {
        let canvas = CoreCanvas::from_bitmap(&mut bitmap, None).expect("a canvas for the bitmap");
        canvas.clear(Color::new(0x0));
        let mut paint = Paint::default();
        paint.set_color(Color::BLUE);
        let mut pos = 0.0_f32;
        while pos < WIDTH_HEIGHT as f32 {
            canvas.draw_line((0.0, pos), (WIDTH_HEIGHT as f32, pos), &paint);
            canvas.draw_line((pos, 0.0), (pos, WIDTH_HEIGHT as f32), &paint);
            pos += 16.0;
        }
    }
    bitmap.as_image()
}

/// `imagemagnifier`: random text under a magnifier with a 100 pixel inset.
// Port of: gm/imagemagnifier.cpp#L48-L56 (chrome/m156)
fn draw_imagemagnifier(canvas: &Canvas) {
    let filter = magnifier(
        &Rect::from_wh(WIDTH as f32, HEIGHT as f32),
        2.0,
        100.0,
        SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
        None,
        None,
    );
    let mut filter_paint = Paint::default();
    filter_paint.set_image_filter(filter);
    canvas.save_layer(&SaveLayerRec::default().paint(&filter_paint));
    draw_content(canvas, 300.0, 25);
    canvas.restore();
}

/// `imagemagnifier_cropped`: a 16 pixel ring of the input is cropped away before magnifying.
// Port of: gm/imagemagnifier.cpp#L83-L97 (chrome/m156)
fn draw_imagemagnifier_cropped(canvas: &Canvas) {
    const WIDTH_HEIGHT: i32 = 256;
    let image_source: Option<ImageFilter> = image_sampled(
        make_img(),
        SamplingOptions::new(FilterMode::Nearest, MipmapMode::None),
    );
    // Crop out a 16 pixel ring around the result
    let crop_rect = IRect::from_xywh(16, 16, WIDTH_HEIGHT - 32, WIDTH_HEIGHT - 32);
    let filter = magnifier(
        &Rect::from_wh(WIDTH_HEIGHT as f32, WIDTH_HEIGHT as f32),
        WIDTH_HEIGHT as f32 / (WIDTH_HEIGHT as f32 - 96.0),
        64.0,
        SamplingOptions::default(),
        image_source,
        Some(Rect::from_irect(crop_rect)),
    );
    let mut filter_paint = Paint::default();
    filter_paint.set_image_filter(filter);
    canvas.save_layer(&SaveLayerRec::default().paint(&filter_paint));
    canvas.restore();
}

/// `ImageMagnifierBounds`: the magnifier as a backdrop filter, as a regular filter and unfiltered,
/// with the bounds of the lens drawn over each.
// Port of: gm/imagemagnifier.cpp#L99-L219 (chrome/m156)
struct ImageMagnifierBoundsGm {
    x: f32,
    y: f32,
}

/// `drawBorder`: a stroked rounded rectangle, inset by `border_inset`.
// Port of: gm/imagemagnifier.cpp#L130-L140 (chrome/m156)
fn draw_border(canvas: &Canvas, mut rect: Rect, color: Color, width: f32, border_inset: f32) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(width);
    paint.set_color(color);
    paint.set_anti_alias(true);
    // This draws the original rect (unrounded) when borderInset = 0
    rect.inset((border_inset, border_inset));
    canvas.draw_rrect(RRect::new_rect_xy(rect, border_inset, border_inset), &paint);
}

impl ImageMagnifierBoundsGm {
    /// `drawRow(canvas, inset)`.
    // Port of: gm/imagemagnifier.cpp#L118-L212 (chrome/m156)
    fn draw_row(&self, canvas: &Canvas, inset: f32) {
        // Logically there is a 'widgetBounds' that is the region of pixels to be filled with
        // magnified content. Pixels inside widgetBounds are scaled up by a factor of 'zoomAmount',
        // with a non linear distortion applied to pixels up to 'inset' inside 'widgetBounds'.
        let mut widget_bounds = Rect::new(16.0, 24.0, 220.0, 248.0);
        widget_bounds.offset((self.x, self.y)); // animating helps highlight magnifier behavior
        const ZOOM_AMOUNT: f32 = 2.5;
        // The available content for backdrops, which clips the widgetBounds as it animates.
        let out_bounds = Rect::new(0.0, 0.0, 256.0, 256.0);

        // The filter responds to any crop (explicit or from missing backdrop content). Compute
        // the corresponding clipped bounds and source bounds for visualization purposes.
        let widget_center = widget_bounds.center();
        let mut clipped_widget = widget_bounds;
        assert!(clipped_widget.intersect(out_bounds));
        let zoom_center_x = t_pin(
            widget_center.x,
            clipped_widget.left(),
            clipped_widget.right(),
        );
        let zoom_center_y = t_pin(
            widget_center.y,
            clipped_widget.top(),
            clipped_widget.bottom(),
        );
        let scale = 1.0 - 1.0 / ZOOM_AMOUNT;
        let zoom_center = Point::new(zoom_center_x * scale, zoom_center_y * scale);
        let src_rect = Rect::new(
            clipped_widget.left() / ZOOM_AMOUNT + zoom_center.x,
            clipped_widget.top() / ZOOM_AMOUNT + zoom_center.y,
            clipped_widget.right() / ZOOM_AMOUNT + zoom_center.x,
            clipped_widget.bottom() / ZOOM_AMOUNT + zoom_center.y,
        );

        // Internally, the magnifier filter performs equivalent calculations but responds to the
        // canvas matrix and available input automatically.
        let magnifier_filter = magnifier(
            &widget_bounds,
            ZOOM_AMOUNT,
            inset,
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            None,
            Some(out_bounds),
        )
        .expect("a magnifier filter");

        // Draw once as a backdrop filter
        canvas.save();
        canvas.clip_rect(out_bounds, None, None);
        draw_content(canvas, 32.0, 350);
        canvas.save_layer(&SaveLayerRec::default().backdrop(&magnifier_filter));
        canvas.restore();
        draw_border(canvas, widget_bounds, Color::BLACK, 2.0, 0.0);
        if inset > 0.0 {
            draw_border(canvas, clipped_widget, Color::RED, 2.0, inset);
        }
        canvas.restore();

        // Draw once as a regular filter
        canvas.save();
        canvas.translate((256.0, 0.0));
        canvas.clip_rect(out_bounds, None, None);
        let mut paint = Paint::default();
        paint.set_image_filter(Some(magnifier_filter.clone()));
        canvas.save_layer(&SaveLayerRec::default().paint(&paint));
        draw_content(canvas, 32.0, 350);
        canvas.restore();
        draw_border(canvas, widget_bounds, Color::BLACK, 2.0, 0.0);
        if inset > 0.0 {
            draw_border(canvas, clipped_widget, Color::RED, 2.0, inset);
        }
        canvas.restore();

        // Draw once unfiltered
        canvas.save();
        canvas.translate((512.0, 0.0));
        canvas.clip_rect(out_bounds, None, None);
        draw_content(canvas, 32.0, 350);
        draw_border(canvas, widget_bounds, Color::BLACK, 2.0, 0.0);
        draw_border(canvas, src_rect, Color::BLUE, 2.0, inset / ZOOM_AMOUNT);
        canvas.restore();
    }
}

/// `SkTPin(x, lo, hi)`: `std::max(lo, std::min(x, hi))`.
// Port of: include/private/SkTPin.h#L19-L21 (chrome/m156)
fn t_pin(x: f32, lo: f32, hi: f32) -> f32 {
    let m = if hi < x { hi } else { x };
    if lo < m { m } else { lo }
}

impl GM for ImageMagnifierBoundsGm {
    fn name(&self) -> String {
        "imagemagnifier_bounds".to_owned()
    }

    fn size(&mut self) -> ISize {
        ISize::new(768, 512)
    }

    // Port of: gm/imagemagnifier.cpp#L201-L209 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        self.draw_row(canvas, 16.0); // fish eye distortion
        canvas.translate((0.0, 256.0));
        self.draw_row(canvas, 0.0); // no distortion, just zoom
    }
}

crate::def_simple_gm_bg!(imagemagnifier, canvas, WIDTH, HEIGHT, Color::BLACK, {
    draw_imagemagnifier(canvas);
});

crate::def_simple_gm_bg!(imagemagnifier_cropped, canvas, 256, 256, Color::BLACK, {
    draw_imagemagnifier_cropped(canvas);
});

crate::def_gm!(
    ImageMagnifierBounds_GM = "ImageMagnifierBounds()",
    ImageMagnifierBoundsGm { x: 0.0, y: 0.0 }
);
