// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/wacky_yuv_formats.cpp (chrome/m156), the YUVSplitterGM class
//
// The other GMs of this file (WackyYUVFormatsGM, YUVMakeColorSpaceGM) are not ported yet.

// GM ports mirror the C++ int-to-float conversions of image sizes.
#![allow(clippy::cast_precision_loss)]

use crate::prelude::*;
use crate::tool_utils::get_resource_as_image;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_filters::{self, Clamp};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::{ImageInfo, YUVColorSpace};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::{FilterMode, MipmapMode, SamplingOptions};
use skia_rust_core::yuv_math::color_matrix_rgb2yuv;
use skia_rust_core::yuva_info::{
    PlaneConfig, Siting, Subsampling, YUVAInfo, plane_dimensions_array,
};
use skia_rust_raster::surfaces;

// Port of: tools/gpu/YUVUtils.cpp#L156-L205 (chrome/m156), sk_gpu_test::MakeYUVAPlanesAsA8 with no
// recording context: each plane is an A8 image, whose alpha is the plane's row of the RGB to YUV
// matrix applied to the source.
fn make_yuva_planes_as_a8(
    src: &Image,
    cs: YUVColorSpace,
    ss: Subsampling,
) -> Option<(Vec<Image>, YUVAInfo)> {
    let rgb_to_yuv = color_matrix_rgb2yuv(cs);

    let config = if src.is_opaque() {
        PlaneConfig::Y_U_V
    } else {
        PlaneConfig::Y_U_V_A
    };
    let (n, dims) = plane_dimensions_array(src.dimensions(), config, ss, EncodedOrigin::TopLeft);
    let mut planes = Vec::with_capacity(n);
    for (i, dim) in dims.iter().enumerate().take(n) {
        let info = ImageInfo::new(*dim, ColorType::Alpha8, AlphaType::Premul, None);
        let mut surf = surfaces::raster(&info, None, None)?;

        let mut paint = Paint::default();
        paint.set_blend_mode(BlendMode::Src);
        // Make a matrix with the ith row of rgbToYUV copied to the A row since we're drawing to A8.
        let mut m = [0.0f32; 20];
        m[15..20].copy_from_slice(&rgb_to_yuv[5 * i..5 * i + 5]);
        paint.set_color_filter(color_filters::matrix_row_major(&m, Clamp::Yes));
        surf.canvas().draw_image_rect_with_sampling_options(
            src,
            None,
            Rect::from_wh(dim.width as f32, dim.height as f32),
            SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
            &paint,
        );
        planes.push(surf.image_snapshot()?);
    }
    let info = YUVAInfo::new(
        src.dimensions(),
        config,
        ss,
        cs,
        EncodedOrigin::TopLeft,
        (Siting::Centered, Siting::Centered),
    )?;
    Some((planes, info))
}

// Exercises SkColorMatrix_RGB2YUV for yuv colorspaces, showing the planes, and the
// resulting (recombined) images (gpu only for now).
//
// On raster the recombined images are null (`SkImages::TextureFromYUVAPixmaps` needs a GPU
// context), so only the planes of the last colour space are drawn.
// Port of: gm/wacky_yuv_formats.cpp#L1338-L1393 (chrome/m156), YUVSplitterGM
struct YuvSplitterGm {
    orig: Option<Image>,
}

impl GM for YuvSplitterGm {
    fn name(&self) -> String {
        "yuv_splitter".to_string()
    }

    fn size(&mut self) -> ISize {
        ISize::new(1280, 768)
    }

    // Port of: gm/wacky_yuv_formats.cpp#L1346-L1348 (chrome/m156), onOnceBeforeDraw
    fn on_once_before_draw(&mut self) {
        self.orig = get_resource_as_image("images/mandrill_256.png");
    }

    // Port of: gm/wacky_yuv_formats.cpp#L1349-L1378 (chrome/m156), onDraw
    fn on_draw(&mut self, canvas: &Canvas) {
        let Some(orig) = self.orig.as_ref() else {
            return;
        };
        let ow = orig.width() as f32;
        canvas.translate((ow, 0.0));
        canvas.save();
        let mut last: Option<(Vec<Image>, YUVAInfo)> = None;
        for cs in [
            YUVColorSpace::REC709,
            YUVColorSpace::REC601,
            YUVColorSpace::JPEG,
            YUVColorSpace::BT2020,
        ] {
            // The C++ draws `TextureFromYUVAPixmaps`, which is null without a GPU context, and
            // then draws its difference (`draw_diff`), which is only reached for an image.
            last = make_yuva_planes_as_a8(orig, cs, Subsampling::S444);
            canvas.translate((ow, 0.0));
        }
        canvas.restore();
        canvas.translate((-ow, 0.0));
        if let Some((planes, info)) = last {
            let mut y = 0.0f32;
            for plane in planes.iter().take(info.num_planes()) {
                canvas.draw_image(plane, (0.0, y), None);
                y += plane.height() as f32;
            }
        }
    }
}

// Port of: gm/wacky_yuv_formats.cpp#L1394 (chrome/m156), DEF_GM(return new YUVSplitterGM;)
crate::def_gm!(YUVSplitterGM, YuvSplitterGm { orig: None });
