// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/hdr_pip_blur.cpp (chrome/m156)

use skia_rust_core::canvas::SrcRectConstraint;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RequiredProperties};
use skia_rust_core::image_filter::ImageFilter;
use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::rrect::RRect;
use skia_rust_core::sampling_options::FilterMode;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::image_filters::blur;
use skia_rust_raster::images;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;

/// `kFullSize`: the size the GM is designed at.
const FULL_SIZE: ISize = ISize::new(2560, 1440);
/// `kNonBenchSize`: the size DM draws the GM at.
const NON_BENCH_SIZE: ISize = ISize::new(640, 360);
/// The sizes above as the `float`s the blur scale is computed in.
const FULL_W_F: f32 = 2560.0;
const FULL_H_F: f32 = 1440.0;
const NON_BENCH_W_F: f32 = 640.0;
const NON_BENCH_H_F: f32 = 360.0;

/// Port of `class HDRPiPBlurGM` (hdr_pip_blur.cpp#L24-L198). DM runs it in `kGM_Mode`, so the GM
/// is always `NON_BENCH_SIZE` and its blur is scaled to that size (the `kSample_Mode` view
/// transform and the bench size are not part of the raster golden).
#[derive(Default)]
struct HdrPipBlurGm {
    background_image: Option<Image>,
    pip_image: Option<Image>,
    shade_blur: Option<ImageFilter>,
    paint: Option<Paint>,
}

impl GM for HdrPipBlurGm {
    // Port of: HDRPiPBlurGM::getName (hdr_pip_blur.cpp#L36)
    fn name(&self) -> String {
        "hdr-pip-blur".to_owned()
    }

    // Port of: HDRPiPBlurGM::getISize (hdr_pip_blur.cpp#L38-L44) in kGM_Mode
    fn size(&mut self) -> ISize {
        NON_BENCH_SIZE
    }

    // Port of: HDRPiPBlurGM::onDraw (hdr_pip_blur.cpp#L49-L144)
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let (Some(background), Some(pip)) = (&self.background_image, &self.pip_image) else {
            error_msg.clear();
            error_msg.push_str("Failed to load images. Did you forget to set the resourcePath?");
            return DrawResult::Fail;
        };
        let (Some(shade_blur), Some(paint)) = (&self.shade_blur, &self.paint) else {
            return DrawResult::Fail;
        };

        // When running in DM, all offscreen passes and the final rendering will be small.
        let screen_bounds = IRect::from_wh(NON_BENCH_SIZE.width, NON_BENCH_SIZE.height);

        let input = {
            // The main surface is RGBA8 but with a wider gamut sRGB colorspace.
            let Some(wide_gamut) =
                ColorSpace::new_rgb(&named_transfer_fn::REC2020, &named_gamut::DISPLAY_P3)
            else {
                return DrawResult::Fail;
            };
            let info = canvas
                .image_info()
                .with_dimensions((screen_bounds.width(), screen_bounds.height()))
                .with_color_type(ColorType::RGBA8888)
                .with_color_space(Some(wide_gamut));
            let Some(mut content) = canvas.new_surface(&info, None) else {
                // This occurs in DDL dm configs
                error_msg.clear();
                error_msg.push_str("Could not create offscreen surface");
                return DrawResult::Skip;
            };

            let c = content.canvas();
            c.clear(Color4f::from_color(Color::DARK_GRAY));

            let to_screen_bounds = Matrix::rect_to_rect_or_identity(
                Rect::from(IRect::from_wh(FULL_SIZE.width, FULL_SIZE.height)),
                Rect::from(screen_bounds),
                None,
            );
            c.concat(&to_screen_bounds);

            // Now render everything to `c` as if it were a kFullSize image.
            c.draw_image_rect_with_sampling_options(
                background,
                Some((&Rect::from(background.bounds()), SrcRectConstraint::Fast)),
                Rect::from(IRect::from_wh(FULL_SIZE.width, FULL_SIZE.height)),
                FilterMode::Linear,
                paint,
            );

            let pip_rrect =
                RRect::new_rect_xy(Rect::from_xywh(1500.0, 700.0, 800.0, 600.0), 20.0, 20.0);
            c.draw_rrect(pip_rrect, paint);
            c.save();
            c.clip_rrect(pip_rrect, None, true);
            c.draw_image_rect_with_sampling_options(
                pip,
                Some((
                    &Rect::from(IRect::from_wh(pip.width(), pip.height())),
                    SrcRectConstraint::Fast,
                )),
                pip_rrect.rect(),
                FilterMode::Linear,
                paint,
            );
            c.restore();
            content.make_temporary_image()
        };
        let Some(input) = input else {
            return DrawResult::Fail;
        };

        canvas.save();

        canvas.clip_irect(screen_bounds, None);
        canvas.draw_image_with_sampling_options(
            &input,
            (0.0, 0.0),
            FilterMode::Linear,
            Some(paint),
        );

        // Explicitly blur the input image (the CPU branch: DM's raster sink has no recorder or
        // recording context).
        let Some((blur_image, out_subset, out_offset)) =
            images::make_with_filter(&input, shade_blur, &screen_bounds, &screen_bounds)
        else {
            return DrawResult::Fail;
        };

        let mut faded_blur = Paint::default();
        faded_blur.set_alpha_f(0.9);
        canvas.draw_image_rect_with_sampling_options(
            &blur_image,
            Some((&Rect::from(out_subset), SrcRectConstraint::Fast)),
            Rect::from(IRect::from_xywh(
                out_offset.x,
                out_offset.y,
                out_subset.width(),
                out_subset.height(),
            )),
            FilterMode::Linear,
            &faded_blur,
        );

        canvas.restore();
        DrawResult::Ok
    }

    // Port of: HDRPiPBlurGM::onOnceBeforeDraw (hdr_pip_blur.cpp#L146-L177)
    fn on_once_before_draw(&mut self) {
        let mut paint = Paint::new(Color4f::new(0.0, 0.0, 0.0, 1.0), None);

        let mut cm = ColorMatrix::default();
        cm.set_saturation(1.5);
        paint.set_color_filter(color_filters::matrix(&cm, Clamp::Yes));
        self.paint = Some(paint);

        // The background image is standard sRGB. `makeRasterImage(nullptr)` is the image itself
        // for a decoded resource (it has pixels).
        self.background_image = get_resource_as_image("images/yellow_rose.png")
            .and_then(|image| image.to_raster_image())
            .and_then(|image| {
                image.make_color_space(ColorSpace::new_srgb(), RequiredProperties::default())
            });
        // The pip will be PQ-ish HDR.
        self.pip_image = get_resource_as_image("images/mandrill_512.png")
            .and_then(|image| image.to_raster_image())
            .and_then(|image| {
                image.make_color_space(
                    ColorSpace::new_rgb(&named_transfer_fn::PQ, &named_gamut::REC2020),
                    RequiredProperties::default(),
                )
            });

        let sigma = 32.0_f32;
        // `scaleSigma` is `kGM_Mode`.
        let sigma_x = sigma * NON_BENCH_W_F / FULL_W_F;
        let sigma_y = sigma * NON_BENCH_H_F / FULL_H_F;
        self.shade_blur = blur(sigma_x, sigma_y, TileMode::Clamp, None, None);
    }
}

// Port of: gm/hdr_pip_blur.cpp#L199 (DEF_GM, chrome/m156)
#[ignore = "see notes/gm_hdr_pip_blur_cpp_HDRPiPBlurGM.md"]
crate::def_gm!(HDRPiPBlurGM, HdrPipBlurGm::default());
