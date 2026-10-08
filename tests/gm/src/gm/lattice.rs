// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/lattice.cpp (chrome/m156)

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::canvas::lattice::{Lattice, RectType};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::scalar::scalar;
use skia_rust_core::size::Size;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

/// `ToolUtils::makeSurface`: a surface like the canvas's, or a raster one.
// Port of: tools/ToolUtils.cpp#L504-L512 (chrome/m156)
fn make_surface_for(root: &Canvas, info: &ImageInfo) -> Surface<'static> {
    root.new_surface(info, None)
        .or_else(|| surfaces::raster(info, None, None))
        .expect("a surface")
}

// Port of: gm/lattice.cpp#L43-L48 (chrome/m156)
fn make_surface(
    root: &Canvas,
    n: i32,
    pad_left: i32,
    pad_top: i32,
    pad_right: i32,
    pad_bottom: i32,
) -> Surface<'static> {
    let info =
        ImageInfo::new_n32_premul((n + pad_left + pad_right, n + pad_top + pad_bottom), None);
    make_surface_for(root, &info)
}

// Port of: gm/lattice.cpp#L50-L98 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // SkIntToScalar
fn make_image(
    root: &Canvas,
    x_divs: &mut [i32],
    y_divs: &mut [i32],
    pad_left: i32,
    pad_top: i32,
    pad_right: i32,
    pad_bottom: i32,
) -> Option<Image> {
    const K_CAP: i32 = 28;
    const K_MID: i32 = 8;
    const K_SIZE: i32 = 2 * K_CAP + 3 * K_MID;

    let mut surface = make_surface(root, K_SIZE, pad_left, pad_top, pad_right, pad_bottom);
    {
        let canvas = surface.canvas();
        canvas.translate((pad_left as scalar, pad_top as scalar));

        let mut r = Rect::from_wh(K_SIZE as scalar, K_SIZE as scalar);
        let stroke_width: scalar = 6.0;
        let radius = K_CAP as scalar - stroke_width / 2.0;

        x_divs[0] = K_CAP + pad_left;
        y_divs[0] = K_CAP + pad_top;
        x_divs[1] = K_CAP + K_MID + pad_left;
        y_divs[1] = K_CAP + K_MID + pad_top;
        x_divs[2] = K_CAP + 2 * K_MID + pad_left;
        y_divs[2] = K_CAP + 2 * K_MID + pad_top;
        x_divs[3] = K_CAP + 3 * K_MID + pad_left;
        y_divs[3] = K_CAP + 3 * K_MID + pad_top;

        let mut paint = Paint::default();
        paint.set_anti_alias(true);

        paint.set_color(Color::new(0xFFFF_FF00));
        canvas.draw_round_rect(r, radius, radius, &paint);

        r = Rect::from_xywh(K_CAP as scalar, 0.0, K_MID as scalar, K_SIZE as scalar);
        paint.set_color(Color::new(0x8800_FF00));
        canvas.draw_rect(r, &paint);
        r = Rect::from_xywh(
            (K_CAP + K_MID) as scalar,
            0.0,
            K_MID as scalar,
            K_SIZE as scalar,
        );
        paint.set_color(Color::new(0x8800_00FF));
        canvas.draw_rect(r, &paint);
        r = Rect::from_xywh(
            (K_CAP + 2 * K_MID) as scalar,
            0.0,
            K_MID as scalar,
            K_SIZE as scalar,
        );
        paint.set_color(Color::new(0x88FF_00FF));
        canvas.draw_rect(r, &paint);

        r = Rect::from_xywh(0.0, K_CAP as scalar, K_SIZE as scalar, K_MID as scalar);
        paint.set_color(Color::new(0x8800_FF00));
        canvas.draw_rect(r, &paint);
        r = Rect::from_xywh(
            0.0,
            (K_CAP + K_MID) as scalar,
            K_SIZE as scalar,
            K_MID as scalar,
        );
        paint.set_color(Color::new(0x8800_00FF));
        canvas.draw_rect(r, &paint);
        r = Rect::from_xywh(
            0.0,
            (K_CAP + 2 * K_MID) as scalar,
            K_SIZE as scalar,
            K_MID as scalar,
        );
        paint.set_color(Color::new(0x88FF_00FF));
        canvas.draw_rect(r, &paint);
    }

    surface.image_snapshot()
}

// This is similar to NinePatchStretchGM, but it also tests "ninepatch" images with more
// than nine patches.
// Port of: gm/lattice.cpp#L100-L277 (chrome/m156)
struct LatticeGm;

impl LatticeGm {
    // Port of: gm/lattice.cpp#L110-L236 (chrome/m156)
    #[allow(clippy::too_many_lines)] // mirrors the C++ function
    fn on_draw_helper(
        canvas: &Canvas,
        pad_left: i32,
        pad_top: i32,
        pad_right: i32,
        pad_bottom: i32,
    ) {
        canvas.save();

        let mut x_divs = [0i32; 5];
        let mut y_divs = [0i32; 5];
        x_divs[0] = pad_left;
        y_divs[0] = pad_top;

        let image = make_image(
            canvas,
            &mut x_divs[1..],
            &mut y_divs[1..],
            pad_left,
            pad_top,
            pad_right,
            pad_bottom,
        )
        .expect("an image");

        let size = [
            Size::new(50.0, 50.0),  // shrink in both axes
            Size::new(50.0, 200.0), // shrink in X
            Size::new(200.0, 50.0), // shrink in Y
            Size::new(200.0, 200.0),
        ];

        canvas.draw_image(&image, (10.0, 10.0), None);

        let x: scalar = 100.0;
        let y: scalar = 100.0;

        let bounds = IRect::new(
            pad_left,
            pad_top,
            image.width() - pad_right,
            image.height() - pad_bottom,
        );
        let lattice_bounds = if bounds == IRect::from_wh(image.width(), image.height()) {
            None
        } else {
            Some(bounds)
        };
        let lattice = Lattice {
            x_divs: &x_divs[1..],
            y_divs: &y_divs[1..],
            rect_types: None,
            bounds: lattice_bounds,
            colors: None,
        };

        for iy in 0..2usize {
            for ix in 0..2usize {
                let i = ix * 2 + iy;
                #[allow(clippy::cast_precision_loss)] // int * SkScalar
                let r = Rect::from_xywh(
                    x + ix as scalar * 60.0,
                    y + iy as scalar * 60.0,
                    size[i].width,
                    size[i].height,
                );
                canvas.draw_image_lattice(&image, &lattice, r, FilterMode::Nearest, None);
            }
        }

        // Provide hints about 3 solid color rects. These colors match
        // what was already in the bitmap.
        let fixed_color_x: [usize; 3] = [2, 4, 1];
        let fixed_color_y: [usize; 3] = [1, 1, 2];
        let mut fixed_color = [Color::BLACK; 3];
        let info = ImageInfo::new((1, 1), ColorType::BGRA8888, AlphaType::Unpremul, None);
        for rect_num in 0..3 {
            let src_x = x_divs[fixed_color_x[rect_num] - 1];
            let src_y = y_divs[fixed_color_y[rect_num] - 1];
            let mut pixel = [0u8; 4];
            if image.read_pixels(&info, &mut pixel, 4, (src_x, src_y)) {
                fixed_color[rect_num] = Color::new(u32::from_ne_bytes(pixel));
            }
        }

        // Include the degenerate first div.  While normally the first patch is "scalable",
        // this will mean that the first non-degenerate patch is "fixed".

        // Let's skip a few rects.
        let mut flags = [RectType::Default; 36];
        flags[4] = RectType::Transparent;
        flags[9] = RectType::Transparent;
        flags[12] = RectType::Transparent;
        flags[19] = RectType::Transparent;
        for rect_num in 0..3 {
            flags[fixed_color_y[rect_num] * 6 + fixed_color_x[rect_num]] = RectType::FixedColor;
        }

        let mut colors = [Color::new(0); 36];
        for rect_num in 0..3 {
            colors[fixed_color_y[rect_num] * 6 + fixed_color_x[rect_num]] = fixed_color[rect_num];
        }

        let lattice = Lattice {
            x_divs: &x_divs,
            y_divs: &y_divs,
            rect_types: Some(&flags),
            bounds: lattice_bounds,
            colors: Some(&colors),
        };

        canvas.translate((400.0, 0.0));
        for iy in 0..2usize {
            for ix in 0..2usize {
                let i = ix * 2 + iy;
                #[allow(clippy::cast_precision_loss)] // int * SkScalar
                let r = Rect::from_xywh(
                    x + ix as scalar * 60.0,
                    y + iy as scalar * 60.0,
                    size[i].width,
                    size[i].height,
                );
                canvas.draw_image_lattice(&image, &lattice, r, FilterMode::Nearest, None);
            }
        }

        canvas.restore();
    }
}

impl GM for LatticeGm {
    fn name(&self) -> String {
        "lattice".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    // Port of: gm/lattice.cpp#L238-L261 (chrome/m156)
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        Self::on_draw_helper(canvas, 0, 0, 0, 0);
        canvas.translate((0.0, 400.0));
        Self::on_draw_helper(canvas, 3, 7, 4, 11);
        DrawResult::Ok
    }
}

// Port of: gm/lattice.cpp#L265 (chrome/m156)
crate::def_gm!(LatticeGM, LatticeGm);

// LatticeGM2 exercises code paths that draw fixed color and 1x1 rectangles.
// Port of: gm/lattice.cpp#L268-L384 (chrome/m156)
struct LatticeGm2;

impl LatticeGm2 {
    // Port of: gm/lattice.cpp#L275-L329 (chrome/m156)
    fn make_image(
        root: &Canvas,
        pad_left: i32,
        pad_top: i32,
        pad_right: i32,
        pad_bottom: i32,
    ) -> Option<Image> {
        const K_SIZE: i32 = 80;
        let mut surface = make_surface(root, K_SIZE, pad_left, pad_top, pad_right, pad_bottom);
        {
            let canvas = surface.canvas();
            let mut paint = Paint::default();
            paint.set_anti_alias(false);

            #[allow(clippy::cast_precision_loss)] // SkIntToScalar
            let size = K_SIZE as scalar;

            //first line
            let mut r = Rect::from_xywh(0.0, 0.0, 4.0, 1.0); //4x1 green rect
            paint.set_color(Color::new(0xFF00_FF00));
            canvas.draw_rect(r, &paint);

            r = Rect::from_xywh(4.0, 0.0, 1.0, 1.0); //1x1 blue pixel -> draws as rectangle
            paint.set_color(Color::new(0xFF00_00FF));
            canvas.draw_rect(r, &paint);

            r = Rect::from_xywh(5.0, 0.0, size - 5.0, 1.0); //the rest of the line is red
            paint.set_color(Color::new(0xFFFF_0000));
            canvas.draw_rect(r, &paint);

            //second line -> draws as fixed color rectangles
            r = Rect::from_xywh(0.0, 1.0, 4.0, 1.0); //4x1 red rect
            paint.set_color(Color::new(0xFFFF_0000));
            canvas.draw_rect(r, &paint);

            r = Rect::from_xywh(4.0, 1.0, 1.0, 1.0); //1x1 blue pixel with alpha
            paint.set_color(Color::new(0x8800_00FF));
            canvas.draw_rect(r, &paint);

            r = Rect::from_xywh(5.0, 1.0, size - 5.0, 1.0); //the rest of the line is green
            paint.set_color(Color::new(0xFF00_FF00));
            canvas.draw_rect(r, &paint);

            //third line - does not draw, because it is transparent
            r = Rect::from_xywh(0.0, 2.0, 4.0, size - 2.0); //4x78 green rect
            paint.set_color(Color::new(0xFF00_FF00));
            canvas.draw_rect(r, &paint);

            r = Rect::from_xywh(4.0, 2.0, 1.0, size - 2.0); //1x78 red pixel with alpha
            paint.set_color(Color::new(0x88FF_0000));
            canvas.draw_rect(r, &paint);

            r = Rect::from_xywh(5.0, 2.0, size - 5.0, size - 2.0); //the rest of the image is blue
            paint.set_color(Color::new(0xFF00_00FF));
            canvas.draw_rect(r, &paint);
        }

        surface.image_snapshot()
    }

    // Port of: gm/lattice.cpp#L331-L371 (chrome/m156)
    fn on_draw_helper(
        canvas: &Canvas,
        pad_left: i32,
        pad_top: i32,
        pad_right: i32,
        pad_bottom: i32,
        paint: &mut Paint,
    ) {
        let x_divs = [4, 5];
        let y_divs = [1, 2];

        canvas.save();

        let image =
            Self::make_image(canvas, pad_left, pad_top, pad_right, pad_bottom).expect("an image");

        canvas.draw_image(&image, (10.0, 10.0), None);

        let mut flags = [RectType::Default; 9];
        flags[3] = RectType::FixedColor;
        flags[4] = RectType::FixedColor;
        flags[5] = RectType::FixedColor;

        flags[6] = RectType::Transparent;
        flags[7] = RectType::Transparent;
        flags[8] = RectType::Transparent;

        let colors = [
            Color::BLACK,
            Color::BLACK,
            Color::BLACK,
            Color::new(0xFFFF_0000),
            Color::new(0x8800_00FF),
            Color::new(0xFF00_FF00),
            Color::BLACK,
            Color::BLACK,
            Color::BLACK,
        ];
        let lattice = Lattice {
            x_divs: &x_divs,
            y_divs: &y_divs,
            rect_types: Some(&flags),
            bounds: None,
            colors: Some(&colors),
        };
        paint.set_color(Color::new(0xFFFF_FFFF));
        canvas.draw_image_lattice(
            &image,
            &lattice,
            Rect::from_xywh(100.0, 100.0, 200.0, 200.0),
            FilterMode::Nearest,
            Some(paint),
        );

        //draw the same content with alpha
        canvas.translate((400.0, 0.0));
        paint.set_color(Color::new(0x8000_0FFF));
        canvas.draw_image_lattice(
            &image,
            &lattice,
            Rect::from_xywh(100.0, 100.0, 200.0, 200.0),
            FilterMode::Nearest,
            Some(paint),
        );

        canvas.restore();
    }
}

impl GM for LatticeGm2 {
    fn name(&self) -> String {
        "lattice2".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(800, 800)
    }

    // Port of: gm/lattice.cpp#L373-L384 (chrome/m156)
    fn on_draw(&mut self, canvas: &Canvas) {
        //draw a rectangle in the background with transparent pixels
        let mut paint = Paint::default();
        paint.set_color(Color::new(0x7F12_3456));
        paint.set_blend_mode(BlendMode::Src);
        canvas.draw_rect(Rect::from_xywh(300.0, 0.0, 300.0, 800.0), &paint);

        //draw image lattice with kSrcOver blending
        paint.set_blend_mode(BlendMode::SrcOver);
        Self::on_draw_helper(canvas, 0, 0, 0, 0, &mut paint);

        //draw image lattice with kSrcATop blending
        canvas.translate((0.0, 400.0));
        paint.set_blend_mode(BlendMode::SrcATop);
        Self::on_draw_helper(canvas, 0, 0, 0, 0, &mut paint);
    }
}

// Port of: gm/lattice.cpp#L388 (chrome/m156)
crate::def_gm!(LatticeGM2, LatticeGm2);

// Code paths that incorporate the paint color when drawing the lattice (using an alpha image)
// Port of: gm/lattice.cpp#L390-L414 (chrome/m156)
crate::def_simple_gm_bg!(lattice_alpha, canvas, 120, 120, Color::WHITE, {
    let mut surface = make_surface_for(canvas, &ImageInfo::new_a8((100, 100)));
    {
        let c = surface.canvas();
        c.clear(Color::new(0));
        c.draw_circle((50.0, 50.0), 50.0, &Paint::default());
    }
    let image = surface.image_snapshot().expect("a snapshot");

    let divs = [20, 40, 60, 80];

    let lattice = Lattice {
        x_divs: &divs,
        y_divs: &divs,
        rect_types: None,
        bounds: None,
        colors: None,
    };

    let mut paint = Paint::default();
    paint.set_color(Color::MAGENTA);
    canvas.draw_image_lattice(
        &image,
        &lattice,
        Rect::from_wh(120.0, 120.0),
        FilterMode::Nearest,
        Some(&paint),
    );
});

// Port of: gm/lattice.cpp#L416-L424 (chrome/m156)
fn make_symmetry_test_image() -> Option<Image> {
    let mut surface =
        surfaces::raster(&ImageInfo::new_n32_premul((8, 8), None), None, None).expect("a surface");
    {
        let canvas = surface.canvas();
        canvas.draw_color(Color::BLUE, None);
        let mut p = Paint::default();
        p.set_color(Color::GREEN);
        canvas.draw_rect(Rect::from_xywh(2.0, 2.0, 4.0, 4.0), &p);
    }
    surface.image_snapshot()
}

// b/349428795 : A nine-patch should be able to have zero-sized regions on either end. Before
// fixing the bug, it only worked on the left/top, leading to non-symmetric results in this GM.
// Correct rendering is for each row to be symmetric.
// Port of: gm/lattice.cpp#L426-L441 (chrome/m156)
crate::def_simple_gm!(ninepatch_edge_case_349428795, canvas, 500, 150, {
    let nine = make_symmetry_test_image().expect("an image");

    for i in -1..6 {
        #[allow(clippy::cast_precision_loss)] // int * SkScalar
        let off = (i * 70 + 80) as scalar;
        canvas.draw_image_nine(
            &nine,
            IRect::from_xywh(i, 2, 4, 4),
            Rect::from_xywh(off, 10.0, 64.0, 64.0),
            FilterMode::Linear,
            None,
        );
        canvas.draw_image_nine(
            &nine,
            IRect::from_xywh(2, i, 4, 4),
            Rect::from_xywh(off, 80.0, 64.0, 64.0),
            FilterMode::Linear,
            None,
        );
    }
});
