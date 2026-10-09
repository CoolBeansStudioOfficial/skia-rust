// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/bitmaprect.cpp (chrome/m156)
//
// `DrawBitmapRect2` (`bitmaprect_i`, `bitmaprect_s`) draws an image with a linear gradient
// (`SkShaders::LinearGradient`); it is ported with the gradients.

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{Canvas as CoreCanvas, SrcRectConstraint};
use skia_rust_core::color::pre_multiply_color;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect, RoundOut};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar;
use skia_rust_raster::raster_canvas::RasterCanvas;

// Port of: gm/bitmaprect.cpp#L98-L117 (chrome/m156)
fn make_3x3_bitmap(bitmap: &mut Bitmap) {
    const X_SIZE: i32 = 3;
    const Y_SIZE: i32 = 3;

    let texture_data: [[Color; Y_SIZE as usize]; X_SIZE as usize] = [
        [Color::RED, Color::WHITE, Color::BLUE],
        [Color::GREEN, Color::BLACK, Color::CYAN],
        [Color::YELLOW, Color::GRAY, Color::MAGENTA],
    ];

    bitmap.alloc_pixels_info(
        &ImageInfo::new_n32((X_SIZE, Y_SIZE), AlphaType::Opaque, None),
        None,
    );
    let canvas = CoreCanvas::from_bitmap(bitmap, None).expect("a canvas on the bitmap");
    let mut paint = Paint::default();

    for y in 0..Y_SIZE {
        for x in 0..X_SIZE {
            paint.set_color(
                texture_data[usize::try_from(x).expect("x")][usize::try_from(y).expect("y")],
            );
            canvas.draw_irect(IRect::from_xywh(x, y, 1, 1), &paint);
        }
    }
}

// This GM attempts to make visible any issues drawBitmapRect may have
// with partial source rects. In this case the eight pixels on the border
// should be half the width/height of the central pixel, i.e.:
//                         __|____|__
//                           |    |
//                         __|____|__
//                           |    |
// Port of: gm/bitmaprect.cpp#L119-L152 (chrome/m156)
struct DrawBitmapRect3Gm;

impl GM for DrawBitmapRect3Gm {
    fn name(&self) -> String {
        "3x3bitmaprect".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn bg_color(&self) -> Color {
        Color::BLACK
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        let mut bitmap = Bitmap::new();
        make_3x3_bitmap(&mut bitmap);

        let src_r = Rect::new(0.5, 0.5, 2.5, 2.5);
        let dst_r = Rect::new(100.0, 100.0, 300.0, 200.0);

        canvas.draw_image_rect_with_sampling_options(
            bitmap.as_image().expect("an image"),
            Some((&src_r, SrcRectConstraint::Strict)),
            dst_r,
            SamplingOptions::default(),
            &Paint::default(),
        );
    }
}

// Port of: gm/bitmaprect.cpp#L154-L172 (chrome/m156)
fn make_big_bitmap() -> Option<Image> {
    const G_X_SIZE: i32 = 4096;
    const G_Y_SIZE: i32 = 4096;
    const G_BORDER_WIDTH: i32 = 10;

    let mut bitmap = Bitmap::new();
    bitmap.alloc_n32_pixels((G_X_SIZE, G_Y_SIZE), None);
    let border = pre_multiply_color(Color::new(0x88FF_FFFF));
    let inside = pre_multiply_color(Color::new(0x88FF_0000));
    {
        let mut pixmap = bitmap.peek_pixels_mut().expect("pixels");
        for y in 0..G_Y_SIZE {
            for x in 0..G_X_SIZE {
                if x <= G_BORDER_WIDTH
                    || x >= G_X_SIZE - G_BORDER_WIDTH
                    || y <= G_BORDER_WIDTH
                    || y >= G_Y_SIZE - G_BORDER_WIDTH
                {
                    pixmap.set_addr32(x, y, border);
                } else {
                    pixmap.set_addr32(x, y, inside);
                }
            }
        }
    }
    bitmap.set_immutable();
    bitmap.as_image()
}

// This GM attempts to reveal any issues we may have when the GPU has to
// break up a large texture in order to draw it. The XOR transfer mode will
// create stripes in the image if there is imprecision in the destination
// tile placement.
// Port of: gm/bitmaprect.cpp#L174-L226 (chrome/m156)
struct DrawBitmapRect4Gm {
    use_irect: bool,
    big_image: Option<Image>,
}

impl DrawBitmapRect4Gm {
    fn new(use_irect: bool) -> DrawBitmapRect4Gm {
        DrawBitmapRect4Gm {
            use_irect,
            big_image: None,
        }
    }
}

impl GM for DrawBitmapRect4Gm {
    fn name(&self) -> String {
        format!("bigbitmaprect_{}", if self.use_irect { "i" } else { "s" })
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn bg_color(&self) -> Color {
        Color::new(0x8844_4444)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        if self.big_image.is_none() {
            self.big_image = make_big_bitmap();
        }
        let big_image = self.big_image.as_ref().expect("image");

        let mut paint = Paint::default();
        paint.set_alpha(128);
        paint.set_blend_mode(BlendMode::Xor);
        let sampling = SamplingOptions::default();

        let src_r1 = Rect::new(0.0, 0.0, 4096.0, 2040.0);
        let dst_r1 = Rect::new(10.1, 10.1, 629.9, 400.9);

        let src_r2 = Rect::new(4085.0, 10.0, 4087.0, 12.0);
        let dst_r2 = Rect::new(10.0, 410.0, 30.0, 430.0);

        if self.use_irect {
            canvas.draw_image_rect_with_sampling_options(
                big_image,
                Some((
                    &Rect::from_irect(RoundOut::<IRect>::round_out(&src_r1)),
                    SrcRectConstraint::Strict,
                )),
                dst_r1,
                sampling,
                &paint,
            );
            canvas.draw_image_rect_with_sampling_options(
                big_image,
                Some((
                    &Rect::from_irect(RoundOut::<IRect>::round_out(&src_r2)),
                    SrcRectConstraint::Strict,
                )),
                dst_r2,
                sampling,
                &paint,
            );
        } else {
            canvas.draw_image_rect_with_sampling_options(
                big_image,
                Some((&src_r1, SrcRectConstraint::Strict)),
                dst_r1,
                sampling,
                &paint,
            );
            canvas.draw_image_rect_with_sampling_options(
                big_image,
                Some((&src_r2, SrcRectConstraint::Strict)),
                dst_r2,
                sampling,
                &paint,
            );
        }
    }
}

// Port of: gm/bitmaprect.cpp#L228-L260 (chrome/m156)
struct BitmapRectRoundingGm {
    bm: Bitmap,
}

impl GM for BitmapRectRoundingGm {
    fn name(&self) -> String {
        "bitmaprect_rounding".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn on_once_before_draw(&mut self) {
        self.bm.alloc_n32_pixels((10, 10), None);
        self.bm.erase_color(Color::BLUE);
    }

    // This choice of coordinates and matrix land the bottom edge of the clip (and bitmap dst)
    // at exactly 1/2 pixel boundary. However, drawBitmapRect may lose precision along the way.
    // If it does, we may see a red-line at the bottom, instead of the bitmap exactly matching
    // the clip (in which case we should see all blue).
    // The correct image should be all blue.
    fn on_draw(&mut self, canvas: &Canvas) {
        let mut paint = Paint::default();
        paint.set_color(Color::RED);

        let r = Rect::from_xywh(1.0, 1.0, 110.0, 114.0);
        let s: scalar = 0.9;
        canvas.scale((s, s));

        // the drawRect shows the same problem as clipRect(r) followed by drawcolor(red)
        canvas.draw_rect(r, &paint);
        canvas.draw_image_rect_with_sampling_options(
            self.bm.as_image().expect("an image"),
            None,
            r,
            SamplingOptions::default(),
            &Paint::default(),
        );
    }
}

// Port of: gm/bitmaprect.cpp#L262 (chrome/m156)
crate::def_gm!(
    BitmapRectRounding,
    BitmapRectRoundingGm { bm: Bitmap::new() }
);

// Port of: gm/bitmaprect.cpp#L35-L46 (chrome/m156)
fn make_image_2() -> Option<Image> {
    let mut surf =
        skia_rust_raster::surfaces::raster(&ImageInfo::new_n32_premul((64, 64), None), None, None)?;
    let tmp_canvas = surf.canvas();
    tmp_canvas.draw_color(Color::RED, None);
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    let pts = [Point::new(0.0, 0.0), Point::new(64.0, 64.0)];
    let colors = [
        skia_rust_core::color::Color4f::new(1.0, 1.0, 1.0, 1.0),
        skia_rust_core::color::Color4f::new(0.0, 0.0, 1.0, 1.0),
    ];
    paint.set_shader(skia_rust_effects::gradient::shaders::linear_gradient(
        (pts[0], pts[1]),
        &skia_rust_effects::gradient::Gradient::new(
            skia_rust_effects::gradient::Colors::new(
                &colors,
                None,
                skia_rust_core::tile_mode::TileMode::Clamp,
                None,
            ),
            skia_rust_effects::gradient::Interpolation::default(),
        ),
        None,
    ));
    tmp_canvas.draw_circle((32.0, 32.0), 32.0, &paint);
    // `ToolUtils::MakeTextureImage` is the identity on a raster canvas.
    surf.image_snapshot()
}

// Port of: gm/bitmaprect.cpp#L47-L106 (chrome/m156)
struct DrawBitmapRect2Gm {
    use_irect: bool,
}

impl GM for DrawBitmapRect2Gm {
    fn name(&self) -> String {
        format!("bitmaprect_{}", if self.use_irect { "i" } else { "s" })
    }

    fn size(&mut self) -> ISize {
        ISize::new(640, 480)
    }

    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.draw_color(Color::new(0xFFCC_CCCC), None);

        let src = [
            IRect::from_ltrb(0, 0, 32, 32),
            IRect::from_ltrb(0, 0, 80, 80),
            IRect::from_ltrb(32, 32, 96, 96),
            IRect::from_ltrb(-32, -32, 32, 32),
        ];

        let mut paint = Paint::default();
        paint.set_style(skia_rust_core::paint::Style::Stroke);
        let sampling = SamplingOptions::default();

        let image = make_image_2().expect("an image");
        let dst_r = Rect::from_ltrb(0.0, 200.0, 128.0, 380.0);
        canvas.translate((16.0, 40.0));
        for s in src {
            // `srcR.set(src[i])` (use_irect false) and `SkRect::Make(src[i])` (use_irect true)
            // are the same rect, so `use_irect` only changes the GM's name.
            let src_r = Rect::from_irect(s);
            canvas.draw_image_with_sampling_options(&image, (0.0, 0.0), sampling, Some(&paint));
            canvas.draw_image_rect_with_sampling_options(
                &image,
                Some((&src_r, SrcRectConstraint::Strict)),
                dst_r,
                sampling,
                &paint,
            );
            canvas.draw_rect(dst_r, &paint);
            canvas.draw_rect(src_r, &paint);
            canvas.translate((160.0, 0.0));
        }
    }
}

// Port of: gm/bitmaprect.cpp#L266 (chrome/m156)
crate::def_gm!(DrawBitmapRect3_ = "DrawBitmapRect3()", DrawBitmapRect3Gm);

// Port of: gm/bitmaprect.cpp#L290-L291 (chrome/m156)
crate::def_gm!(
    DrawBitmapRect2_false = "DrawBitmapRect2(false)",
    DrawBitmapRect2Gm { use_irect: false }
);
crate::def_gm!(
    DrawBitmapRect2_true = "DrawBitmapRect2(true)",
    DrawBitmapRect2Gm { use_irect: true }
);

// Port of: gm/bitmaprect.cpp#L267-L270 (chrome/m156)
crate::def_gm!(
    DrawBitmapRect4_false = "DrawBitmapRect4(false)",
    DrawBitmapRect4Gm::new(false)
);
crate::def_gm!(
    DrawBitmapRect4_true = "DrawBitmapRect4(true)",
    DrawBitmapRect4Gm::new(true)
);
