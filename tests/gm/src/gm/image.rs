// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/image.cpp (chrome/m156)
//
// Only `new_texture_image` is ported here (the text-module entry of this file). The other GMs in
// gm/image.cpp have their own manifest entries.
//
// Skip-matching only (docs/design/text.md §1.2): this GM is GPU-only. `isGPU` is false on a raster
// sink, so it reports `kErrorMsg_DrawSkippedGpuOnly` and skips before it draws.

// Mirrors the C++ int/scalar casts and sizes of the GM: the values are small constants.
#![allow(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

use crate::prelude::*;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color_priv::pack_argb32;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::sampling_options::{CubicResampler, FilterMode, MipmapMode, SamplingOptions};
use skia_rust_raster::pixmap_draw::scale_pixels;

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
