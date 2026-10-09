// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/matrixconvolution.cpp (chrome/m156)

// Coordinates, kernel offsets and sizes here are small integers: their f32 conversions are exact.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use skia_rust_core::font::Font;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::scalar::scalar_round_to_int;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_effects::image_filters;
use skia_rust_raster::surfaces;
use skia_rust_tools::font_tool_utils::default_portable_typeface;

// Port of: gm/matrixconvolution.cpp#L17-L22 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KernelFixture {
    Basic,
    Large,
    Larger,
    Largest,
}

// Port of: gm/matrixconvolution.cpp#L24-L166 (chrome/m156), MatrixConvolutionGM
struct MatrixConvolutionGm {
    name_suffix: &'static str,
    colors: [Color4f; 2],
    kernel_fixture: KernelFixture,
    image: Option<Image>,
}

impl MatrixConvolutionGm {
    // Port of: gm/matrixconvolution.cpp#L27-L35 (chrome/m156), the constructor
    fn new(
        color_one: Color,
        color_two: Color,
        kernel_fixture: KernelFixture,
        name_suffix: &'static str,
    ) -> Self {
        Self {
            name_suffix,
            colors: [Color4f::from(color_one), Color4f::from(color_two)],
            kernel_fixture,
            image: None,
        }
    }

    // Port of: gm/matrixconvolution.cpp#L37-L56 (chrome/m156), makeBitmap
    fn make_bitmap(&mut self) {
        // Draw our bitmap in N32, so legacy devices get "premul" values they understand
        let mut surf = surfaces::raster(&ImageInfo::new_n32_premul((80, 80), None), None, None)
            .expect("a raster surface");
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE);
        let pts = [Point::new(0.0, 0.0), Point::new(0.0, 80.0)];
        let pos = [0.0_f32, 80.0];
        let gradient = Gradient::new(
            Colors::new(&self.colors, Some(&pos), TileMode::Clamp, None),
            Interpolation::default(),
        );
        paint.set_shader(shaders::linear_gradient((pts[0], pts[1]), &gradient, None));
        let font = Font::from_size(default_portable_typeface(), 180.0);
        surf.canvas().draw_str("e", (-10.0, 80.0), &font, &paint);
        self.image = surf.image_snapshot();
    }

    // Port of: gm/matrixconvolution.cpp#L58-L110 (chrome/m156), makeFilter
    fn make_filter(
        &self,
        kernel_offset_in: (i32, i32),
        tile_mode: TileMode,
        convolve_alpha: bool,
    ) -> Option<skia_rust_core::image_filter::ImageFilter> {
        // The kernelOffset is specified in a 0..2 coordinate space.
        let normalized_x_offset = kernel_offset_in.0 as f32 / 2.0;
        let normalized_y_offset = kernel_offset_in.1 as f32 / 2.0;

        // Must provide a cropping geometry in order for 'tileMode' to be well defined.
        let image = self
            .image
            .as_ref()
            .expect("onOnceBeforeDraw made the image");
        let tile_boundary = Rect::from_irect(image.bounds());

        match self.kernel_fixture {
            KernelFixture::Basic => {
                let kernel_offset = (
                    scalar_round_to_int(2.0 * normalized_x_offset),
                    scalar_round_to_int(2.0 * normalized_y_offset),
                );
                // All 1s except center value, which is -7 (sum of 1).
                let mut kernel = vec![1.0_f32; 9];
                kernel[4] = -7.0;
                image_filters::matrix_convolution(
                    (3, 3),
                    &kernel,
                    0.3,
                    100.0,
                    kernel_offset,
                    tile_mode,
                    convolve_alpha,
                    None,
                    Some(tile_boundary),
                )
            }
            KernelFixture::Large => {
                let kernel_offset = (
                    scalar_round_to_int(6.0 * normalized_x_offset),
                    scalar_round_to_int(6.0 * normalized_y_offset),
                );
                // All 1s except center value, which is -47 (sum of 1).
                let mut kernel = vec![1.0_f32; 49];
                kernel[24] = -47.0;
                image_filters::matrix_convolution(
                    (7, 7),
                    &kernel,
                    0.3,
                    100.0,
                    kernel_offset,
                    tile_mode,
                    convolve_alpha,
                    None,
                    Some(tile_boundary),
                )
            }
            KernelFixture::Larger => {
                let kernel_offset = (scalar_round_to_int(127.0 * normalized_x_offset), 0);
                let mut kernel = vec![0.0_f32; 128];
                kernel[64] = 0.5;
                kernel[65] = -0.5;
                image_filters::matrix_convolution(
                    (128, 1),
                    &kernel,
                    0.3,
                    100.0,
                    kernel_offset,
                    tile_mode,
                    convolve_alpha,
                    None,
                    Some(tile_boundary),
                )
            }
            KernelFixture::Largest => {
                let kernel_offset = (0, scalar_round_to_int(254.0 * normalized_y_offset));
                let mut kernel = vec![0.0_f32; 255];
                kernel[126] = 0.5;
                kernel[128] = -0.5;
                image_filters::matrix_convolution(
                    (1, 255),
                    &kernel,
                    0.3,
                    100.0,
                    kernel_offset,
                    tile_mode,
                    convolve_alpha,
                    None,
                    Some(tile_boundary),
                )
            }
        }
    }

    // Port of: gm/matrixconvolution.cpp#L112-L126 (chrome/m156), draw
    #[allow(clippy::too_many_arguments)] // mirrors the C++ draw() parameter list
    fn draw(
        &self,
        canvas: &Canvas,
        x: i32,
        y: i32,
        kernel_offset: (i32, i32),
        tile_mode: TileMode,
        convolve_alpha: bool,
        crop_rect: Option<IRect>,
    ) {
        let mut paint = Paint::default();
        let mut filter = self.make_filter(kernel_offset, tile_mode, convolve_alpha);
        if let Some(crop_rect) = crop_rect {
            filter = image_filters::crop(&Rect::from_irect(crop_rect), TileMode::Decal, filter);
        }
        paint.set_image_filter(filter);
        let image = self
            .image
            .as_ref()
            .expect("onOnceBeforeDraw made the image");
        canvas.save();
        canvas.translate((x as f32, y as f32));
        canvas.draw_image_with_sampling_options(
            image,
            (0.0, 0.0),
            SamplingOptions::default(),
            Some(&paint),
        );
        canvas.restore();
    }
}

impl GM for MatrixConvolutionGm {
    // Port of: gm/matrixconvolution.cpp#L30-L31 (chrome/m156), setBGColor(0x00000000)
    fn bg_color(&self) -> Color {
        Color::TRANSPARENT
    }

    // Port of: gm/matrixconvolution.cpp#L43 (chrome/m156), getName
    fn name(&self) -> String {
        format!("matrixconvolution{}", self.name_suffix)
    }

    // Port of: gm/matrixconvolution.cpp#L65-L67 (chrome/m156), getISize
    fn size(&mut self) -> ISize {
        ISize::new(500, 300)
    }

    // Port of: gm/matrixconvolution.cpp#L166-L168 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.make_bitmap();
    }

    // Port of: gm/matrixconvolution.cpp#L168-L186 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        canvas.clear(Color::BLACK);
        let mut kernel_offset = (1, 0);
        for x in (10..310).step_by(100) {
            self.draw(canvas, x, 10, kernel_offset, TileMode::Clamp, true, None);
            self.draw(canvas, x, 110, kernel_offset, TileMode::Decal, true, None);
            self.draw(canvas, x, 210, kernel_offset, TileMode::Repeat, true, None);
            kernel_offset.1 += 1;
        }
        kernel_offset.1 = 1;
        let small_rect = IRect::from_xywh(10, 5, 60, 60);
        self.draw(
            canvas,
            310,
            10,
            kernel_offset,
            TileMode::Clamp,
            true,
            Some(small_rect),
        );
        self.draw(
            canvas,
            310,
            110,
            kernel_offset,
            TileMode::Decal,
            true,
            Some(small_rect),
        );
        self.draw(
            canvas,
            310,
            210,
            kernel_offset,
            TileMode::Repeat,
            true,
            Some(small_rect),
        );
        self.draw(canvas, 410, 10, kernel_offset, TileMode::Clamp, false, None);
        self.draw(
            canvas,
            410,
            110,
            kernel_offset,
            TileMode::Decal,
            false,
            None,
        );
        self.draw(
            canvas,
            410,
            210,
            kernel_offset,
            TileMode::Repeat,
            false,
            None,
        );
    }
}

// Port of: gm/matrixconvolution.cpp#L172-L177 (chrome/m156), DEF_GM registrations
crate::def_gm!(
    MatrixConvolutionGM_ =
        "MatrixConvolutionGM(0xFFFFFFFF, 0x40404040, KernelFixture::kBasic_KernelFixture, \"\")",
    MatrixConvolutionGm::new(
        Color::new(0xFFFF_FFFF),
        Color::new(0x4040_4040),
        KernelFixture::Basic,
        ""
    )
);
crate::def_gm!(
    MatrixConvolutionGM_color = "MatrixConvolutionGM(0xFFFF0000, 0xFF00FF00, KernelFixture::kBasic_KernelFixture, \"_color\")",
    MatrixConvolutionGm::new(
        Color::new(0xFFFF_0000),
        Color::new(0xFF00_FF00),
        KernelFixture::Basic,
        "_color"
    )
);
crate::def_gm!(
    MatrixConvolutionGM_big = "MatrixConvolutionGM(0xFFFFFFFF, 0x40404040, KernelFixture::kLarge_KernelFixture, \"_big\")",
    MatrixConvolutionGm::new(
        Color::new(0xFFFF_FFFF),
        Color::new(0x4040_4040),
        KernelFixture::Large,
        "_big"
    )
);
crate::def_gm!(
    MatrixConvolutionGM_big_color = "MatrixConvolutionGM(0xFFFF0000, 0xFF00FF00, KernelFixture::kLarge_KernelFixture, \"_big_color\")",
    MatrixConvolutionGm::new(
        Color::new(0xFFFF_0000),
        Color::new(0xFF00_FF00),
        KernelFixture::Large,
        "_big_color"
    )
);
crate::def_gm!(
    MatrixConvolutionGM_bigger = "MatrixConvolutionGM(0xFFFFFFFF, 0x40404040, KernelFixture::kLarger_KernelFixture, \"_bigger\")",
    MatrixConvolutionGm::new(
        Color::new(0xFFFF_FFFF),
        Color::new(0x4040_4040),
        KernelFixture::Larger,
        "_bigger"
    )
);
crate::def_gm!(
    MatrixConvolutionGM_biggest = "MatrixConvolutionGM(0xFFFFFFFF, 0x40404040, KernelFixture::kLargest_KernelFixture, \"_biggest\")",
    MatrixConvolutionGm::new(
        Color::new(0xFFFF_FFFF),
        Color::new(0x4040_4040),
        KernelFixture::Largest,
        "_biggest"
    )
);
