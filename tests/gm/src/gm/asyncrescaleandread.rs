// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/asyncrescaleandread.cpp (chrome/m156)
//
// The CPU readback of asyncRescaleAndReadPixels: the raster sink has no GPU context or Recorder,
// so every read is synchronous and goes through `skia_rust_raster::rescale_and_read_pixels`. The
// YUV[A] readbacks have no CPU implementation in Skia (they call back with no result), so the GMs
// that use them are skipped here and have no entry in this file.

use crate::prelude::*;
use crate::tool_utils::{draw_checkerboard, get_resource_as_image, int_to_scalar};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::{Image, RescaleGamma, RescaleMode};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::rescale_and_read_pixels::{
    AsyncReadResult, ImageAsyncRescaleAndReadPixels, SurfaceAsyncRescaleAndReadPixels,
};
use skia_rust_raster::surface::Surface;
use skia_rust_raster::surfaces;

/// `ReadSource`: whether a grid reads an image directly or a surface holding a copy of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReadSource {
    /// `ReadSource::kSurface`.
    Surface,
    /// `ReadSource::kImage`.
    Image,
}

/// `Source<ReadSource>`: the image or the surface whose pixels are read.
enum Source {
    Image(Image),
    Surface(Surface<'static>),
}

impl Source {
    /// `asyncRescaleAndReadPixels` of the source; the callback runs before this returns.
    fn async_rescale_and_read(
        &mut self,
        info: &ImageInfo,
        src_rect: IRect,
        gamma: RescaleGamma,
        mode: RescaleMode,
        callback: impl FnOnce(Option<AsyncReadResult>),
    ) {
        match self {
            Source::Image(image) => {
                image.async_rescale_and_read_pixels(info, src_rect, gamma, mode, callback);
            }
            Source::Surface(surface) => {
                surface.async_rescale_and_read_pixels(info, src_rect, gamma, mode, callback);
            }
        }
    }
}

/// `convert_image_to_source<ReadSource>` on the CPU: an image is read as it is; a surface is made
/// from the image (the raster sink's `canvas->makeSurface` is a raster surface of the same info).
// Port of: gm/asyncrescaleandread.cpp#L58-L104 (chrome/m156), convert_image_to_source (CPU)
fn convert_image_to_source(
    image: Image,
    read_source: ReadSource,
    error_msg: &mut String,
) -> Result<Source, DrawResult> {
    match read_source {
        ReadSource::Image => Ok(Source::Image(image)),
        ReadSource::Surface => {
            // Turn the image into a surface in order to call the read and rescale API
            let surf_info = image.image_info().with_dimensions(image.dimensions());
            let Some(mut surface) = surfaces::raster(&surf_info, None, None) else {
                error_msg.clear();
                error_msg.push_str("Could not create surface for image.");
                return Err(DrawResult::Fail);
            };
            let mut paint = Paint::default();
            paint.set_blend_mode(BlendMode::Src);
            surface.canvas().draw_image_with_sampling_options(
                &image,
                (0.0, 0.0),
                SamplingOptions::default(),
                Some(&paint),
            );
            Ok(Source::Surface(surface))
        }
    }
}

/// `AsyncReadGMBase::readAndScaleRGBA` on the CPU: the rescaled pixels as an image, or `None`.
// Port of: gm/asyncrescaleandread.cpp#L131-L204 (chrome/m156), readAndScaleRGBA (CPU)
fn read_and_scale_rgba(
    src: &mut Source,
    src_rect: IRect,
    ii: &ImageInfo,
    gamma: RescaleGamma,
    mode: RescaleMode,
) -> Option<Image> {
    let mut result = None;
    src.async_rescale_and_read(ii, src_rect, gamma, mode, |r| result = r);
    let (mut data, row_bytes) = result?.into_parts();
    let pixmap = Pixmap::new(ii, &mut data, row_bytes)?;
    images::raster_from_pixmap_copy(&pixmap)
}

/// `AsyncReadGMBase::drawRescaleGrid` for the RGBA type: a grid of rescales, with the columns for
/// nearest, repeated-linear and repeated-cubic rescaling and the rows for source and linear gamma.
// Port of: gm/asyncrescaleandread.cpp#L326-L380 (chrome/m156), drawRescaleGrid (kRGBA)
fn draw_rescale_grid(
    canvas: &Canvas,
    src: &mut Source,
    src_rect: IRect,
    read_size: ISize,
    error_msg: &mut String,
    pad: i32,
) -> DrawResult {
    canvas.save();
    for gamma in [RescaleGamma::Src, RescaleGamma::Linear] {
        canvas.save();
        for mode in [
            RescaleMode::Nearest,
            RescaleMode::RepeatedLinear,
            RescaleMode::RepeatedCubic,
        ] {
            let ii = canvas.image_info().with_dimensions(read_size);
            let Some(result) = read_and_scale_rgba(src, src_rect, &ii, gamma, mode) else {
                error_msg.clear();
                error_msg.push_str("async read call failed.");
                return DrawResult::Fail;
            };
            canvas.draw_image(&result, (0.0, 0.0), None);
            canvas.translate((int_to_scalar(read_size.width + pad), 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, int_to_scalar(read_size.height + pad)));
    }
    canvas.restore();
    DrawResult::Ok
}

/// `AsyncRescaleAndReadGridGM<ReadSource, Type::kRGBA>`: draws the image with a checkerboard and
/// the rescale grid of `src_rect` at `read_size`.
// Port of: gm/asyncrescaleandread.cpp#L433-L470 (chrome/m156), AsyncRescaleAndReadGridGM (kRGBA)
struct AsyncRescaleAndReadGridGm {
    name: &'static str,
    image_file: &'static str,
    src_rect: IRect,
    read_size: ISize,
    read_source: ReadSource,
}

impl AsyncRescaleAndReadGridGm {
    // Port of: gm/asyncrescaleandread.cpp#L437-L444 (chrome/m156), the constructor
    fn new(
        name: &'static str,
        image_file: &'static str,
        src_rect: IRect,
        read_size: ISize,
        read_source: ReadSource,
    ) -> Self {
        Self {
            name,
            image_file,
            src_rect,
            read_size,
            read_source,
        }
    }
}

impl GM for AsyncRescaleAndReadGridGm {
    fn name(&self) -> String {
        self.name.to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(3 * self.read_size.width, 2 * self.read_size.height)
    }

    // Port of: gm/asyncrescaleandread.cpp#L446-L472 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        draw_checkerboard(canvas, Color::DARK_GRAY, Color::LIGHT_GRAY, 25);
        let Some(image) = get_resource_as_image(self.image_file) else {
            error_msg.clear();
            *error_msg = format!("Could not load image file {}.", self.image_file);
            return DrawResult::Fail;
        };
        if canvas.image_info().color_type() == ColorType::Unknown {
            error_msg.clear();
            error_msg.push_str("Not supported on recording/vector backends.");
            return DrawResult::Skip;
        }
        let mut src = match convert_image_to_source(image, self.read_source, error_msg) {
            Ok(src) => src,
            Err(result) => return result,
        };
        draw_rescale_grid(
            canvas,
            &mut src,
            self.src_rect,
            self.read_size,
            error_msg,
            0,
        )
    }
}

// Port of: gm/asyncrescaleandread.cpp#L483-L545 (chrome/m156), the RGBA DEF_RESCALE_AND_READ_GRID_GM
// registrations (the YUV[A] ones have no CPU readback, so they are not registered).
crate::def_gm!(
    AsyncReadRoseGm = "images/yellow_rose.webp#3",
    AsyncRescaleAndReadGridGm::new(
        "async_rescale_and_read_rose",
        "images/yellow_rose.webp",
        IRect::from_xywh(100, 20, 100, 100),
        ISize::new(410, 410),
        ReadSource::Surface,
    )
);
crate::def_gm!(
    AsyncReadDogDownGm = "images/dog.jpg",
    AsyncRescaleAndReadGridGm::new(
        "async_rescale_and_read_dog_down",
        "images/dog.jpg",
        IRect::from_xywh(0, 10, 180, 150),
        ISize::new(45, 45),
        ReadSource::Surface,
    )
);
crate::def_gm!(
    AsyncReadDogUpGm = "images/dog.jpg#2",
    AsyncRescaleAndReadGridGm::new(
        "async_rescale_and_read_dog_up",
        "images/dog.jpg",
        IRect::from_wh(180, 180),
        ISize::new(800, 400),
        ReadSource::Image,
    )
);
crate::def_gm!(
    AsyncReadTextDownGm = "images/text.png",
    AsyncRescaleAndReadGridGm::new(
        "async_rescale_and_read_text_down",
        "images/text.png",
        IRect::from_wh(637, 105),
        ISize::new(445, 73),
        ReadSource::Image,
    )
);
crate::def_gm!(
    AsyncReadTextUpGm = "images/text.png#2",
    AsyncRescaleAndReadGridGm::new(
        "async_rescale_and_read_text_up",
        "images/text.png",
        IRect::from_wh(637, 105),
        ISize::new(764, 126),
        ReadSource::Surface,
    )
);
crate::def_gm!(
    AsyncReadTextUpLargeGm = "images/text.png#3",
    AsyncRescaleAndReadGridGm::new(
        "async_rescale_and_read_text_up_large",
        "images/text.png",
        IRect::from_xywh(300, 0, 300, 105),
        ISize::new(720, 252),
        ReadSource::Image,
    )
);

/// `AsyncRescaleAndReadNoBleedGM`: a small surface with a blue inner square on red, rescaled down
/// and up, to check that the rescale does not bleed the border.
// Port of: gm/asyncrescaleandread.cpp#L601-L676 (chrome/m156), AsyncRescaleAndReadNoBleedGM
struct AsyncRescaleAndReadNoBleedGm;

impl GM for AsyncRescaleAndReadNoBleedGm {
    fn name(&self) -> String {
        "async_rescale_and_read_no_bleed".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(60, 60)
    }

    // Port of: gm/asyncrescaleandread.cpp#L606-L674 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        const BORDER: i32 = 5;
        const INNER: i32 = 5;
        const PAD: i32 = 2;
        let src_rect = IRect::from_xywh(BORDER, BORDER, INNER, INNER);
        let surface_ii = ImageInfo::new(
            ISize::new(INNER + 2 * BORDER, INNER + 2 * BORDER),
            ColorType::RGBA8888,
            AlphaType::Premul,
            Some(skia_rust_core::color_space::ColorSpace::new_srgb()),
        );
        // canvas->makeSurface(surfaceII): a raster surface on the raster sink.
        let Some(mut surface) = surfaces::raster(&surface_ii, None, None) else {
            error_msg.clear();
            error_msg.push_str("Could not create surface for image.");
            return DrawResult::Fail;
        };
        surface.canvas().clear(Color::RED);
        surface.canvas().save();
        surface
            .canvas()
            .clip_rect(Rect::from_irect(src_rect), ClipOp::Intersect, false);
        surface.canvas().clear(Color::BLUE);
        surface.canvas().restore();

        canvas.translate((int_to_scalar(PAD), int_to_scalar(PAD)));
        let mut src = Source::Surface(surface);
        let down_size = ISize::new(INNER / 2, INNER / 2);
        let result = draw_rescale_grid(canvas, &mut src, src_rect, down_size, error_msg, PAD);
        if result != DrawResult::Ok {
            return result;
        }
        canvas.translate((0.0, int_to_scalar(4 * down_size.height)));
        // SkISize upSize = {static_cast<int>(kInner * 3.5), static_cast<int>(kInner * 4.6)}
        // static_cast<int>(kInner * 3.5) and static_cast<int>(kInner * 4.6) truncate.
        #[allow(clippy::cast_possible_truncation)]
        let up_size = ISize::new(
            (f64::from(INNER) * 3.5) as i32,
            (f64::from(INNER) * 4.6) as i32,
        );
        let result = draw_rescale_grid(canvas, &mut src, src_rect, up_size, error_msg, PAD);
        if result != DrawResult::Ok {
            return result;
        }
        DrawResult::Ok
    }
}

crate::def_gm!(
    AsyncRescaleAndReadNoBleedGM_ = "AsyncRescaleAndReadNoBleedGM()",
    AsyncRescaleAndReadNoBleedGm
);

/// `AsyncRescaleAndReadAlphaTypeGM`: an unpremul and a premul radial gradient, each read back as
/// premul and as unpremul.
// Port of: gm/asyncrescaleandread.cpp#L679-L771 (chrome/m156), AsyncRescaleAndReadAlphaTypeGM
struct AsyncRescaleAndReadAlphaTypeGm;

impl GM for AsyncRescaleAndReadAlphaTypeGm {
    fn name(&self) -> String {
        "async_rescale_and_read_alpha_type".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(512, 512)
    }

    // Port of: gm/asyncrescaleandread.cpp#L682-L727 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, error_msg: &mut String) -> DrawResult {
        let upm_ii = ImageInfo::new(
            ISize::new(200, 200),
            ColorType::RGBA8888,
            AlphaType::Unpremul,
            None,
        );
        let pm_ii = upm_ii.with_alpha_type(AlphaType::Premul);
        let Some(mut upm_surf) = surfaces::raster(&upm_ii, None, None) else {
            return DrawResult::Fail;
        };
        let Some(mut pm_surf) = surfaces::raster(&pm_ii, None, None) else {
            return DrawResult::Fail;
        };
        let colors = [
            Color4f::new(0.3, 0.3, 0.3, 0.3),
            Color4f::new(1.0, 0.2, 0.6, 0.9),
            Color4f::new(0.0, 0.1, 1.0, 0.1),
            Color4f::new(0.7, 0.8, 0.2, 0.7),
        ];
        let gradient = Gradient::new(
            Colors::new(&colors, None, TileMode::Repeat, None),
            Interpolation::default(),
        );
        let shader = shaders::radial_gradient((Point::new(100.0, 100.0), 230.0), &gradient, None)
            .expect("radial gradient");
        let mut paint = Paint::default();
        paint.set_shader(shader);
        upm_surf.canvas().draw_paint(&paint);
        pm_surf.canvas().draw_paint(&paint);
        let pm_img = pm_surf.image_snapshot();
        let upm_img = upm_surf.image_snapshot();
        let (Some(pm_img), Some(upm_img)) = (pm_img, upm_img) else {
            return DrawResult::Fail;
        };

        let size = 256;
        draw_checkerboard(canvas, Color::WHITE, Color::BLACK, 32);
        for img in [pm_img, upm_img] {
            canvas.save();
            for read_at in [AlphaType::Premul, AlphaType::Unpremul] {
                let read_info = img
                    .image_info()
                    .with_alpha_type(read_at)
                    .with_wh(size, size);
                let mut src = Source::Image(img.clone());
                let src_rect = IRect::from_wh(img.width(), img.height());
                let Some(result) = read_and_scale_rgba(
                    &mut src,
                    src_rect,
                    &read_info,
                    RescaleGamma::Src,
                    RescaleMode::RepeatedCubic,
                ) else {
                    error_msg.clear();
                    error_msg.push_str("async readback failed");
                    return DrawResult::Fail;
                };
                canvas.draw_image(&result, (0.0, 0.0), None);
                canvas.translate((int_to_scalar(size), 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, int_to_scalar(size)));
        }
        DrawResult::Ok
    }
}

crate::def_gm!(
    AsyncRescaleAndReadAlphaTypeGM_ = "AsyncRescaleAndReadAlphaTypeGM()",
    AsyncRescaleAndReadAlphaTypeGm
);

/// `AyncYUVNoScaleGM`: draws the image; the YUV readback has no CPU implementation, so it draws
/// nothing on top.
struct AyncYuvNoScaleGm;

impl GM for AyncYuvNoScaleGm {
    fn name(&self) -> String {
        "async_yuv_no_scale".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(400, 300)
    }

    // Port of: gm/asyncrescaleandread.cpp#L601-L626 (chrome/m156), onDraw
    fn on_draw_with_error(&mut self, canvas: &Canvas, _error_msg: &mut String) -> DrawResult {
        let Some(image) = get_resource_as_image("images/yellow_rose.webp") else {
            return DrawResult::Fail;
        };
        canvas.draw_image(&image, (15.0, 12.0), None);
        let yuv_image = async_rescale_and_read_yuv420_raster();
        canvas.clear(Color::WHITE);
        if let Some(yuv_image) = yuv_image {
            canvas.draw_image(&yuv_image, (0.0, 0.0), None);
        }
        DrawResult::Ok
    }
}

// The YUV readback of a raster surface. The C++ `SkSurface_Base::onAsyncRescaleAndReadPixelsYUV420`
// has no raster implementation: it calls the client's callback with no result, and
// `readAndScaleYUVA` then returns null.
// Port of: src/image/SkSurface_Base.cpp#L89-L96 (chrome/m156), `onAsyncRescaleAndReadPixelsYUV420`
#[allow(clippy::unnecessary_wraps)] // the `None` is the callback's result, which is always absent
fn async_rescale_and_read_yuv420_raster() -> Option<Image> {
    None
}

// Port of: gm/asyncrescaleandread.cpp#L598 (chrome/m156), DEF_GM(return new AyncYUVNoScaleGM();)
crate::def_gm!(AyncYUVNoScaleGM_ = "AyncYUVNoScaleGM()", AyncYuvNoScaleGm);
