// Copyright 2015 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SwizzlerTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::swizzle::swap_rb;
use skia_rust_simd::Tier;
use skia_rust_simd::selection;
use skia_rust_simd::swizzle::{
    pixel_round_as_rp, reciprocal_alpha, reciprocal_alpha_portable, reciprocal_alpha_times_255,
    reciprocal_alpha_times_255_portable, rgba_to_bgra, rgba_to_bgra_premul, rgba_to_rgba_premul,
    unpremul_simulating_rp,
};

use skia_rust_codec::codec::ZeroInitialized;
use skia_rust_codec::sampler;
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;

use crate::{Reporter, def_test, def_tier_test, reporter_assert};

/// Runs a one-pixel byte kernel (`SkOpts::...(&dst, &src, 1)`) and returns the output pixel.
fn one_pixel(kernel: fn(&mut [u8], &[u8], usize), src: u32) -> u32 {
    let mut dst = [0u8; 4];
    kernel(&mut dst, &src.to_ne_bytes(), 1);
    u32::from_ne_bytes(dst)
}

// Port of: tests/SwizzlerTest.cpp#L21-L70 (chrome/m156)
fn check_fill(
    reporter: &mut Reporter,
    image_info: &ImageInfo,
    start_row: u32,
    end_row: u32,
    row_bytes: usize,
    offset: usize,
) {
    // Calculate the total size of the image in bytes. Use the smallest possible size.
    // The offset value tells us to adjust the pointer from the memory we allocate in order
    // to test on different memory alignments. If offset is nonzero, we need to increase the
    // size of the memory we allocate in order to make sure that we have enough. We are
    // still allocating the smallest possible size.
    let total_bytes = image_info.compute_byte_size(row_bytes) + offset;

    // Create fake image data where every byte has a value of 0
    let mut storage = vec![0u8; total_bytes];

    // Adjust the pointer in order to test on different memory alignments
    let image_data = offset;
    let image_start = image_data + row_bytes * start_row as usize;
    let fill_info = image_info.with_wh(
        image_info.width(),
        i32::try_from(end_row - start_row + 1).expect("height fits in i32"),
    );
    sampler::fill(
        &fill_info,
        &mut storage[image_start..],
        row_bytes,
        ZeroInitialized::No,
    );

    // Ensure that the pixels are filled properly
    // The bots should catch any memory corruption
    let mut index_ptr = image_data + start_row as usize * row_bytes;
    for _y in start_row..=end_row {
        for x in 0..usize::try_from(image_info.width()).expect("non-negative width") {
            let pixel = match image_info.color_type() {
                ColorType::N32 => {
                    let p = index_ptr + 4 * x;
                    u32::from_ne_bytes(storage[p..p + 4].try_into().expect("4 bytes")) == 0
                }
                ColorType::Gray8 => storage[index_ptr + x] == 0,
                ColorType::RGB565 => {
                    let p = index_ptr + 2 * x;
                    u16::from_ne_bytes(storage[p..p + 2].try_into().expect("2 bytes")) == 0
                }
                _ => false,
            };
            reporter_assert!(reporter, pixel);
        }
        index_ptr += row_bytes;
    }
}

// Port of: tests/SwizzlerTest.cpp#L73-L120 (chrome/m156)
def_test!(SwizzlerFill, |r| {
    // Test on an invalid width and representative widths
    let widths: [u32; 3] = [0, 10, 50];
    // In order to call Fill(), there must be at least one row to fill
    // Test on the smallest possible height and representative heights
    let heights: [u32; 3] = [1, 5, 10];
    // Test on interesting possibilities for row padding
    let paddings: [usize; 2] = [0, 4];
    // Iterate over test dimensions
    for width in widths {
        for height in heights {
            // Create image info objects
            let color_info = ImageInfo::new_n32(
                (
                    i32::try_from(width).expect("width fits in i32"),
                    i32::try_from(height).expect("height fits in i32"),
                ),
                AlphaType::Unknown,
                None,
            );
            let gray_info = color_info.with_color_type(ColorType::Gray8);
            let color565_info = color_info.with_color_type(ColorType::RGB565);
            let width = width as usize;
            for &padding in &paddings {
                // Calculate row bytes
                let color_row_bytes = ColorType::N32.bytes_per_pixel() * width + padding;
                let index_row_bytes = width + padding;
                let gray_row_bytes = index_row_bytes;
                let color565_row_bytes = ColorType::RGB565.bytes_per_pixel() * width + padding;
                // If there is padding, we can invent an offset to change the memory alignment
                for offset in (0..=padding).step_by(4) {
                    // Test all possible start rows with all possible end rows
                    for start_row in 0..height {
                        for end_row in start_row..height {
                            // Test fill with each color type
                            check_fill(r, &color_info, start_row, end_row, color_row_bytes, offset);
                            check_fill(r, &gray_info, start_row, end_row, gray_row_bytes, offset);
                            check_fill(
                                r,
                                &color565_info,
                                start_row,
                                end_row,
                                color565_row_bytes,
                                offset,
                            );
                        }
                    }
                }
            }
        }
    }
});

// Port of: tests/SwizzlerTest.cpp#L122-L152 (chrome/m156)
def_test!(SwizzleOpts, |r| {
    // forall c, c*255 == c, c*0 == 0
    for c in 0..=255u32 {
        let src = (255 << 24) | c;
        reporter_assert!(r, one_pixel(rgba_to_rgba_premul, src) == src);
        reporter_assert!(
            r,
            one_pixel(rgba_to_bgra_premul, src) == (255 << 24) | (c << 16)
        );

        let src = c;
        reporter_assert!(r, one_pixel(rgba_to_rgba_premul, src) == 0);
        reporter_assert!(r, one_pixel(rgba_to_bgra_premul, src) == 0);
    }

    // check a totally arbitrary color
    let src = 0xFACE_B004;
    reporter_assert!(r, one_pixel(rgba_to_rgba_premul, src) == 0xFACA_AD04);

    // swap red and blue
    reporter_assert!(r, one_pixel(rgba_to_bgra, src) == 0xFA04_B0CE);

    // all together now
    reporter_assert!(r, one_pixel(rgba_to_bgra_premul, src) == 0xFA04_ADCA);
});

// Port of: tests/SwizzlerTest.cpp#L154-L161 (chrome/m156)
def_test!(PublicSwizzleOpts, |r| {
    // check a totally arbitrary color
    let src = 0xFACE_B004;
    let mut dst = [0u32; 1];
    swap_rb(&mut dst, &[src]);
    reporter_assert!(r, dst[0] == 0xFA04_B0CE);
});

// Port of: tests/SwizzlerTest.cpp#L163-L181 (chrome/m156)
#[allow(clippy::float_cmp)] // the test checks bit-exact results, as Skia's REPORTER_ASSERT does
fn test_reciprocal_alpha(r: &mut Reporter, test255: fn(f32) -> f32, test1: fn(f32) -> f32) {
    reporter_assert!(r, test255(0.0) == 0.0);
    for i in 1..=255u8 {
        let rv = test255(f32::from(i));
        let e = 255.0f32 / f32::from(i);
        reporter_assert!(r, rv == e);
    }

    reporter_assert!(r, test1(0.0) == 0.0);
    for i in 1..=255u8 {
        let normalized = f32::from(i) / 255.0f32;
        let rv = test1(normalized);
        let e = 1.0f32 / normalized;
        reporter_assert!(r, rv == e);
    }
}

// Port of: tests/SwizzlerTest.cpp#L187-L191 (chrome/m156)
// The SSE and NEON reciprocals are the same IEEE divisions as the portable ones, so this test
// exercises the same functions as `ReciprocalAlphaPortable`.
def_test!(ReciprocalAlphaOptimized, |r| {
    test_reciprocal_alpha(r, reciprocal_alpha_times_255, reciprocal_alpha);
});

// Port of: tests/SwizzlerTest.cpp#L193-L197 (chrome/m156)
def_test!(ReciprocalAlphaPortable, |r| {
    test_reciprocal_alpha(
        r,
        reciprocal_alpha_times_255_portable,
        reciprocal_alpha_portable,
    );
});

// Port of: tests/SwizzlerTest.cpp#L199-L232 (chrome/m156), `calcExpected`.
/// The unpremultiplied byte the raster pipeline stores for `comp` at alpha `alpha`, as the
/// test simulates it; rounds with the rounding of `tier`.
fn calc_expected(alpha: f32, comp: f32, tier: Tier) -> u32 {
    if alpha == 0.0 {
        return 0;
    }
    let normalized = comp * (1.0f32 / 255.0f32);
    let normalized_a = alpha * (1.0f32 / 255.0f32);
    let inverse_alpha = 1.0f32 / normalized_a;
    let unpremul = normalized * inverse_alpha;
    let scaled_and_pinned = 255.0f32.min(unpremul * 255.0f32);
    pixel_round_as_rp(scaled_and_pinned, tier)
}

// Port of: tests/SwizzlerTest.cpp#L234-L247 (chrome/m156)
// The rounding of `unpremul_simulating_RP` depends on the tier, so the test runs on each tier.
def_tier_test!(UnpremulSimulatingRP, |r| {
    let tier = selection().tier;
    for a in 0..=255u8 {
        for c in 0..=255u8 {
            let (a, c) = (f32::from(a), f32::from(c));
            let expected = calc_expected(a, c, tier);
            let normalized_a = a * (1.0f32 / 255.0f32);
            let inv_a = reciprocal_alpha(normalized_a);
            let actual = unpremul_simulating_rp(inv_a, c, tier);
            reporter_assert!(
                r,
                actual == expected,
                "a: {a} c: {c} expected: {expected} actual: {actual}"
            );
        }
    }
});
