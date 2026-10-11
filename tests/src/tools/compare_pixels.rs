// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ComparePixels.cpp (chrome/m156)

//! `ComparePixels`: compares two pixmaps in floating point, within a per-channel tolerance.

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::convert_pixels::convert_pixels;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;

/// Port of `compare_colors`: `false` (after reporting) if any channel differs by more than its
/// tolerance.
// Port of: tests/ComparePixels.cpp#L27-L44 (chrome/m156)
fn compare_colors(
    x: i32,
    y: i32,
    rgba_a: &[f32; 4],
    rgba_b: &[f32; 4],
    tol_rgba: &[f32; 4],
    error: &mut dyn FnMut(i32, i32, &[f32; 4]),
) -> bool {
    let mut diffs = [0.0f32; 4];
    let mut bad = false;
    for (((diff, a), b), tol) in diffs.iter_mut().zip(rgba_a).zip(rgba_b).zip(tol_rgba) {
        *diff = b - a;
        if diff.abs() > tol.abs() {
            bad = true;
        }
    }
    if bad {
        error(x, y, &diffs);
        return false;
    }
    true
}

/// Reads the `RGBA_F32` pixel at `(x, y)` of `buf`, whose rows are `row_bytes` apart.
fn f32_pixel(buf: &[u8], row_bytes: usize, x: usize, y: usize) -> [f32; 4] {
    let base = y * row_bytes + x * 4 * 4;
    std::array::from_fn(|i| {
        let at = base + i * 4;
        f32::from_ne_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
    })
}

/// Port of `ComparePixels`: converts both pixmaps to `RGBA_F32` and compares them channel by
/// channel within `tol_rgba`. A premul and an unpremul pixmap are compared in premul space.
// Port of: tests/ComparePixels.cpp#L46-L92 (chrome/m156)
pub fn compare_pixels(
    a: &Pixmap<'_>,
    b: &Pixmap<'_>,
    tol_rgba: &[f32; 4],
    error: &mut dyn FnMut(i32, i32, &[f32; 4]),
) -> bool {
    if a.dimensions() != b.dimensions() {
        error(-1, -1, &[0.0; 4]);
        return false;
    }

    let mut float_alpha_type = a.alpha_type();
    // If one is premul and the other is unpremul we do the comparison in premul space.
    if (a.alpha_type() == AlphaType::Premul || b.alpha_type() == AlphaType::Premul)
        && (a.alpha_type() == AlphaType::Unpremul || b.alpha_type() == AlphaType::Unpremul)
    {
        float_alpha_type = AlphaType::Premul;
    }

    let float_cs = if ColorSpace::equals(a.color_space().as_ref(), b.color_space().as_ref()) {
        a.color_space()
    } else {
        Some(ColorSpace::new_srgb_linear())
    };
    let float_info = ImageInfo::new(a.dimensions(), ColorType::RGBAF32, float_alpha_type, float_cs);
    let row_bytes = float_info.min_row_bytes();
    let size = float_info.compute_byte_size(row_bytes);
    let mut float_a = vec![0u8; size];
    let mut float_b = vec![0u8; size];
    let (Some(a_pixels), Some(b_pixels)) = (a.addr(), b.addr()) else {
        panic!("ComparePixels needs pixmaps with pixels");
    };
    assert!(
        convert_pixels(
            &float_info,
            &mut float_a,
            row_bytes,
            a.info(),
            a_pixels,
            a.row_bytes()
        ),
        "SkConvertPixels(floatA, a)"
    );
    assert!(
        convert_pixels(
            &float_info,
            &mut float_b,
            row_bytes,
            b.info(),
            b_pixels,
            b.row_bytes()
        ),
        "SkConvertPixels(floatB, b)"
    );

    for y in 0..float_info.height() {
        for x in 0..float_info.width() {
            let (xu, yu) = (x as usize, y as usize);
            let rgba_a = f32_pixel(&float_a, row_bytes, xu, yu);
            let rgba_b = f32_pixel(&float_b, row_bytes, xu, yu);
            if !compare_colors(x, y, &rgba_a, &rgba_b, tol_rgba, error) {
                return false;
            }
        }
    }
    true
}

/// Port of `CheckSolidPixels`: whether every pixel of `pixmap` is `col` (converted to the
/// pixmap's alpha type and colour space), within `tol_rgba`.
// Port of: tests/ComparePixels.cpp#L94-L134 (chrome/m156)
pub fn check_solid_pixels(
    col: &Color4f,
    pixmap: &Pixmap<'_>,
    tol_rgba: &[f32; 4],
    error: &mut dyn FnMut(i32, i32, &[f32; 4]),
) -> bool {
    let float_bpp = ColorType::RGBAF32.bytes_per_pixel();
    // First convert 'col' to be compatible with 'pixmap'
    let src_info = ImageInfo::new(
        (1, 1),
        ColorType::RGBAF32,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb_linear()),
    );
    let mut src_bytes = Vec::with_capacity(float_bpp);
    for channel in [col.r, col.g, col.b, col.a] {
        src_bytes.extend_from_slice(&channel.to_ne_bytes());
    }
    let dst_info = src_info
        .with_alpha_type(pixmap.alpha_type())
        .with_color_space(pixmap.color_space());
    let color_rb = dst_info.min_row_bytes();
    let mut color_pixels = vec![0u8; dst_info.compute_byte_size(color_rb)];
    assert!(
        convert_pixels(
            &dst_info,
            &mut color_pixels,
            color_rb,
            &src_info,
            &src_bytes,
            float_bpp
        ),
        "SkConvertPixels(colorPixmap, srcPixmap)"
    );

    // Then convert 'pixmap' to F32_RGBA
    let f32_info = pixmap.info().with_color_type(ColorType::RGBAF32);
    let f32_rb = f32_info.min_row_bytes();
    let mut f32_pixels = vec![0u8; f32_info.compute_byte_size(f32_rb)];
    let Some(pixels) = pixmap.addr() else {
        panic!("CheckSolidPixels needs a pixmap with pixels");
    };
    assert!(
        convert_pixels(
            &f32_info,
            &mut f32_pixels,
            f32_rb,
            pixmap.info(),
            pixels,
            pixmap.row_bytes()
        ),
        "SkConvertPixels(f32Pixmap, pixmap)"
    );

    let rgba_b = f32_pixel(&color_pixels, color_rb, 0, 0);
    for y in 0..f32_info.dimensions().height {
        for x in 0..f32_info.dimensions().width {
            let (xu, yu) = (x as usize, y as usize);
            let rgba_a = f32_pixel(&f32_pixels, f32_rb, xu, yu);
            if !compare_colors(x, y, &rgba_a, &rgba_b, tol_rgba, error) {
                return false;
            }
        }
    }
    true
}
