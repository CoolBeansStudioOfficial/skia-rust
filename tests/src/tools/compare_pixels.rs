// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/ComparePixels.cpp (chrome/m156)

//! `ComparePixels`: compares two pixmaps in floating point, within a per-channel tolerance.

use skia_rust_core::alpha_type::AlphaType;
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
