// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image.cpp (chrome/m156)
//
// Only the GMs with manifest entries of their own are ported here: `new_texture_image`,
// `scalepixels_unpremul` and `ScalePixelsGM`. The other GMs in gm/image.cpp are separate entries.
//
// Skip-matching only (docs/design/text.md §1.2): `new_texture_image` is GPU-only. `isGPU` is false
// on a raster sink, so it reports `kErrorMsg_DrawSkippedGpuOnly` and skips before it draws.

// Mirrors the C++ int/scalar casts and sizes of the GM: the values are small constants.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_codec::codecs::deferred_image;
use skia_rust_codec::encode::png_encoder::{Options, encode_image};
use skia_rust_codec::png_codec;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::{Paint, Style};
use skia_rust_core::picture_recorder::PictureRecorder;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::point::Point;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::stream::MemoryStream;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;
use skia_rust_effects::gradient::{Colors, Gradient, Interpolation, shaders};
use skia_rust_raster::image_picture::{BitDepth, deferred_from_picture};
use skia_rust_raster::pixmap_draw::{ImageMakeScaled, ImageScalePixels, scale_pixels};
use skia_rust_raster::surfaces;

// Port of: gm/image.cpp#L374-L385 (chrome/m156), new_texture_image (the GPU-only skip)
crate::def_simple_gm_can_fail!(new_texture_image, canvas, error_msg, 280, 115, {
    // No GPU context or Graphite recorder on a raster sink, so `isGPU` is false.
    error_msg.clear();
    error_msg.push_str(crate::ERROR_MSG_DRAW_SKIPPED_GPU_ONLY);
    DrawResult::Skip
});

// Port of: gm/image.cpp#L71-L76 (chrome/m156), gSamplings
fn g_samplings() -> [SamplingOptions; 4] {
    [
        SamplingOptions::from(FilterMode::Nearest),
        SamplingOptions::from(FilterMode::Linear),
        SamplingOptions::new(FilterMode::Linear, MipmapMode::Linear),
        SamplingOptions::from(CubicResampler::mitchell()),
    ]
}

// Port of: gm/image.cpp#L506-L508 (chrome/m156), draw_pixmap
fn draw_pixmap(canvas: &Canvas, pm: &Pixmap<'_>, x: f32, y: f32) {
    if let Some(image) = images::raster_from_pixmap_copy(pm) {
        canvas.draw_image(&image, (x, y), None);
    }
}

// Port of: gm/image.cpp#L510-L516 (chrome/m156), slam_ff
fn slam_ff(bm: &mut Bitmap) {
    for y in 0..bm.height() {
        for x in 0..bm.width() {
            let value = bm.get_addr32(x, y) | pack_argb32(0xFF, 0, 0, 0);
            bm.set_addr32(x, y, value);
        }
    }
}

// Port of: gm/image.cpp#L518-L536 (chrome/m156), scalepixels_unpremul
crate::def_simple_gm!(scalepixels_unpremul, canvas, 1080, 280, {
    let info = ImageInfo::new_n32((16, 16), AlphaType::Unpremul, None);
    let mut pm = Bitmap::new();
    pm.alloc_pixels_flags(&info);
    for y in 0..16 {
        for x in 0..16 {
            // SkPackARGB32(0, (y << 4) | y, (x << 4) | x, 0xFF)
            pm.set_addr32(
                x,
                y,
                pack_argb32(0, ((y << 4) | y) as u32, ((x << 4) | x) as u32, 0xFF),
            );
        }
    }
    let mut pm2 = Bitmap::new();
    pm2.alloc_pixels_flags(&ImageInfo::new_n32((256, 256), AlphaType::Unpremul, None));
    for s in g_samplings() {
        let src = pm.peek_pixels().expect("pm has pixels");
        let mut dst = pm2.peek_pixels_mut().expect("pm2 has pixels");
        scale_pixels(&src, &mut dst, &s);
        drop(dst);
        slam_ff(&mut pm2);
        draw_pixmap(
            canvas,
            &pm2.peek_pixels().expect("pm2 has pixels"),
            10.0,
            10.0,
        );
        canvas.translate((pm2.width() as f32 + 10.0, 0.0));
    }
});

// Port of: gm/image.cpp#L255-L260 (chrome/m156), draw_contents
fn draw_contents(canvas: &Canvas) {
    let mut paint = Paint::default();
    paint.set_style(Style::Stroke);
    paint.set_stroke_width(20.0);
    canvas.draw_circle((50.0, 50.0), 35.0, &paint);
}

// Port of: gm/image.cpp#L329-L332 (chrome/m156), ImageMakerProc (without the GPU contexts, which
// the raster sink never has)
type ImageMakerProc = fn(&ImageInfo, fn(&Canvas)) -> Option<Image>;

// Port of: gm/image.cpp#L262-L269 (chrome/m156), make_raster
fn make_raster(info: &ImageInfo, draw: fn(&Canvas)) -> Option<Image> {
    let mut surface = surfaces::raster(info, None, None)?;
    draw(surface.canvas());
    surface.image_snapshot()
}

// Port of: gm/image.cpp#L271-L283 (chrome/m156), make_picture
fn make_picture(info: &ImageInfo, draw: fn(&Canvas)) -> Option<Image> {
    let mut recorder = PictureRecorder::new();
    draw(recorder.begin_recording(Rect::from_iwh(info.width(), info.height()), false));
    let picture = recorder.finish_recording_as_picture(None)?;
    deferred_from_picture(
        picture,
        info.dimensions(),
        None,
        None,
        BitDepth::U8,
        Some(ColorSpace::new_srgb()),
        SurfaceProps::default(),
    )
}

// Port of: gm/image.cpp#L285-L304 (chrome/m156), make_codec
fn make_codec(info: &ImageInfo, draw: fn(&Canvas)) -> Option<Image> {
    let image = make_raster(info, draw)?;
    // SkPngEncoder::Encode(nullptr, image, {}), then SkPngDecoder::Decode of the same bytes.
    let data = encode_image(&image, &Options::default()).expect("PNG encode");
    let Ok(codec) = png_codec::make_from_stream(MemoryStream::make(Some(data))) else {
        panic!("PNG decode");
    };
    deferred_image(Some(codec), None)
}

// Port of: gm/image.cpp#L306-L327 (chrome/m156), make_gpu
// Without a GrRecordingContext or a Graphite Recorder (the raster sink), Skia's make_gpu returns
// nullptr, so there is no image.
fn make_gpu(_info: &ImageInfo, _draw: fn(&Canvas)) -> Option<Image> {
    None
}

// Port of: gm/image.cpp#L222-L253 (chrome/m156), show_scaled_pixels
fn show_scaled_pixels(canvas: &Canvas, image: &Image, use_image_scaling: bool) {
    canvas.save();

    canvas.draw_image(image, (0.0, 0.0), None);
    canvas.translate((110.0, 10.0));

    let info = ImageInfo::new_n32((40, 40), AlphaType::Premul, None);
    let mut storage = Bitmap::new();
    storage.alloc_pixels_flags(&info);

    // The caching hint does not change the pixels `scalePixels` produces (see
    // `skia_rust_raster::pixmap_draw`), so both passes of the C++ loop draw the same.
    for _ch in 0..2 {
        canvas.save();
        for s in g_samplings() {
            if use_image_scaling {
                if let Some(scaled) = image.make_scaled(&info, &s) {
                    canvas.draw_image(&scaled, (0.0, 0.0), None);
                }
            } else {
                let scaled = {
                    let mut dst = storage.peek_pixels_mut().expect("storage has pixels");
                    image.scale_pixels(&mut dst, &s)
                };
                if scaled {
                    draw_pixmap(
                        canvas,
                        &storage.peek_pixels().expect("storage has pixels"),
                        0.0,
                        0.0,
                    );
                }
            }
            canvas.translate((70.0, 0.0));
        }
        canvas.restore();
        canvas.translate((0.0, 45.0));
    }

    canvas.restore();
}

// Port of: gm/image.cpp#L334-L370 (chrome/m156), ScalePixelsGM
struct ScalePixelsGm {
    use_image_scaling: bool,
}

impl ScalePixelsGm {
    // Port of: gm/image.cpp#L336 (chrome/m156), ScalePixelsGM(bool useImageScaling)
    fn new(use_image_scaling: bool) -> Self {
        Self { use_image_scaling }
    }
}

impl GM for ScalePixelsGm {
    fn name(&self) -> String {
        if self.use_image_scaling {
            "scale-pixels-via-image".to_string()
        } else {
            "scale-pixels".to_string()
        }
    }

    fn size(&mut self) -> ISize {
        ISize::new(960, 1200)
    }

    // Port of: gm/image.cpp#L349-L363 (chrome/m156), ScalePixelsGM::onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let info = ImageInfo::new_n32((100, 100), AlphaType::Premul, None);

        let procs: [ImageMakerProc; 5] =
            [make_codec, make_raster, make_picture, make_codec, make_gpu];
        for proc in procs {
            if let Some(image) = proc(&info, draw_contents) {
                show_scaled_pixels(canvas, &image, self.use_image_scaling);
            }
            canvas.translate((0.0, 120.0));
        }
    }
}

// Port of: gm/image.cpp#L369 (chrome/m156), DEF_GM( return new ScalePixelsGM(false); )
crate::def_gm!(
    ScalePixelsGM_false = "ScalePixelsGM(false)",
    ScalePixelsGm::new(false)
);
// Port of: gm/image.cpp#L370 (chrome/m156), DEF_GM( return new ScalePixelsGM(true); )
crate::def_gm!(
    ScalePixelsGM_true = "ScalePixelsGM(true)",
    ScalePixelsGm::new(true)
);

// Port of: gm/image.cpp#L615-L644 (chrome/m156), crbug_404394639
crate::def_simple_gm!(crbug_404394639, canvas, 500, 500, {
    // Define SkImage with height > 32768.
    const SOURCE_WIDTH: i32 = 500;
    const SOURCE_HEIGHT: i32 = 40000;
    let source_info = ImageInfo::new_n32((SOURCE_WIDTH, SOURCE_HEIGHT), AlphaType::Premul, None);

    let mut surf = surfaces::raster(&source_info, None, None).expect("raster surface");

    let pts = [Point::new(0.0, 0.0), Point::new(0.0, 40000.0)];
    let colors = [
        Color4f::from_color(Color::CYAN),
        Color4f::from_color(Color::MAGENTA),
    ];
    let gradient = Gradient::new(
        Colors::new(&colors, None, TileMode::Clamp, None),
        Interpolation::default(),
    );
    let gradient_shader =
        shaders::linear_gradient((pts[0], pts[1]), &gradient, None).expect("gradient shader");

    let mut paint = Paint::default();
    paint.set_shader(gradient_shader);

    surf.canvas()
        .draw_rect(Rect::from_xywh(0.0, 0.0, 500.0, 40000.0), &paint);

    // Create an immutable source image.
    let large_source_image = surf.image_snapshot().expect("snapshot");

    let sampling = SamplingOptions::from(FilterMode::Linear);
    let scaled_image = large_source_image
        .make_scaled(
            &large_source_image.image_info().with_wh(500, 500),
            &sampling,
        )
        .expect("scaled image");
    // Image shouldn't be different based on compiler.
    canvas.draw_image(&scaled_image, (0.0, 0.0), None);
});
