// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkPixmapUtils.cpp (chrome/m156)

//! `SkPixmapUtils`: orientation of decoded pixels by their [`EncodedOrigin`].

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_raster::surfaces::wrap_pixels;

/// `SkPixmapUtils::SwapWidthHeight`.
// Port of: src/codec/SkPixmapUtils.cpp#L69-L71 (chrome/m156)
pub fn swap_width_height(info: &ImageInfo) -> ImageInfo {
    info.with_wh(info.height(), info.width())
}

/// `SkPixmapUtils::Orient(dst, src, origin)`: draws `src`, which holds the image in its encoded
/// orientation, into `dst` upright. Returns false if the sizes or color types do not match.
// Port of: src/codec/SkPixmapUtils.cpp#L14-L58 (chrome/m156)
pub fn orient(dst: &mut Pixmap<'_>, src: &Pixmap<'_>, origin: EncodedOrigin) -> bool {
    if src.info().color_type() != dst.info().color_type() {
        return false;
    }
    // note: we just ignore alphaType and colorSpace for this transformation

    let mut w = src.info().width();
    let mut h = src.info().height();
    if origin.swaps_width_height() {
        std::mem::swap(&mut w, &mut h);
    }
    if dst.info().width() != w || dst.info().height() != h {
        return false;
    }
    if w == 0 || h == 0 {
        return true;
    }

    // The C++ checks for `src` aliasing `dst` here. Every caller in this crate decodes into a
    // separate allocation, so the aliasing case cannot arise.
    draw_orientation(dst, src, origin)
}

/// `draw_orientation` in `SkPixmapUtils.cpp`: draws `src` through the origin's matrix onto a
/// surface that wraps `dst`, with `kSrc` blending.
// Port of: src/codec/SkPixmapUtils.cpp#L20-L35 (chrome/m156)
fn draw_orientation(dst: &mut Pixmap<'_>, src: &Pixmap<'_>, origin: EncodedOrigin) -> bool {
    let dst_info = dst.info().clone();
    let dst_row_bytes = dst.row_bytes();
    let (dst_w, dst_h) = (dst_info.width(), dst_info.height());
    let Some(dst_pixels) = dst.writable_addr() else {
        return false;
    };
    let Some(mut surface) = wrap_pixels(&dst_info, dst_pixels, dst_row_bytes, None) else {
        return false;
    };

    // SkBitmap::installPixels(src): the bitmap borrows the pixels, which is a copy here.
    let mut bitmap = Bitmap::new();
    let Some(src_pixels) = src.addr() else {
        return false;
    };
    if !bitmap.install_pixels(src.info(), src_pixels.to_vec(), src.row_bytes()) {
        return false;
    }
    let Some(image) = images::raster_from_bitmap(&bitmap) else {
        return false;
    };

    let matrix = origin.to_matrix(dst_w, dst_h);
    let mut paint = Paint::default();
    paint.set_blend_mode(BlendMode::Src);
    let canvas = surface.canvas();
    canvas.concat(&matrix);
    canvas.draw_image_with_sampling_options(
        &image,
        (0, 0),
        SamplingOptions::default(),
        Some(&paint),
    );
    true
}

