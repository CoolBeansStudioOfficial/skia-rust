// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/BitmapCopyTest.cpp (chrome/m156)
//
// Both tests convert pixels, so they run once per CPU tier (`def_tier_test!`).

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::color::{Color, PMColor};
use skia_rust_core::color_priv::{
    get_packed_a32, get_packed_b32, get_packed_g32, get_packed_r32, pack_argb32,
};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::size::ISize;

use crate::tools::tool_utils;
use crate::{def_tier_test, reporter_assert};

// Port of: tests/BitmapCopyTest.cpp#L27-L31 (chrome/m156)
fn init_src(bitmap: &mut Bitmap) {
    if bitmap.peek_pixels().is_some() {
        bitmap.erase_color(Color::WHITE);
    }
}

// Port of: tests/BitmapCopyTest.cpp#L33-L36 (chrome/m156)
struct Pair {
    color_type: ColorType,
    #[allow(dead_code)] // kept as in the C++ (the tests no longer read it)
    valid: &'static str,
}

// Port of: tests/BitmapCopyTest.cpp#L65-L72 (chrome/m156)
const G_PAIRS: [Pair; 6] = [
    Pair {
        color_type: ColorType::Unknown,
        valid: "0000000",
    },
    Pair {
        color_type: ColorType::Alpha8,
        valid: "0100000",
    },
    Pair {
        color_type: ColorType::RGB565,
        valid: "0101011",
    },
    Pair {
        color_type: ColorType::ARGB4444,
        valid: "0101111",
    },
    Pair {
        color_type: ColorType::N32,
        valid: "0101111",
    },
    Pair {
        color_type: ColorType::RGBAF16,
        valid: "0101011",
    },
];

// Port of: tests/BitmapCopyTest.cpp#L74-L87 (chrome/m156)
fn setup_src_bitmaps(src_opaque: &mut Bitmap, src_premul: &mut Bitmap, ct: ColorType) {
    const W: i32 = 20;
    const H: i32 = 33;
    let mut color_space = None;
    if ColorType::RGBAF16 == ct {
        color_space = Some(ColorSpace::new_srgb());
    }

    src_opaque.alloc_pixels_info(
        &ImageInfo::new((W, H), ct, AlphaType::Opaque, color_space.clone()),
        None,
    );
    src_premul.alloc_pixels_info(
        &ImageInfo::new((W, H), ct, AlphaType::Premul, color_space),
        None,
    );
    init_src(src_opaque);
    init_src(src_premul);
}

// Port of: tests/BitmapCopyTest.cpp#L89-L132 (chrome/m156)
def_tier_test!(BitmapCopy_extractSubset, |reporter| {
    const W: i32 = 20;
    for pair in &G_PAIRS {
        let mut src_opaque = Bitmap::new();
        let mut src_premul = Bitmap::new();
        setup_src_bitmaps(&mut src_opaque, &mut src_premul, pair.color_type);

        let mut bitmap = src_opaque.clone();
        let mut subset = Bitmap::new();
        // Extract a subset which has the same width as the original. This
        // catches a bug where we cloned the genID incorrectly.
        let r = IRect::from_ltrb(0, 1, W, 3);
        // Relies on old behavior of extractSubset failing if colortype is unknown
        if ColorType::Unknown != bitmap.color_type() && bitmap.extract_subset(&mut subset, r) {
            reporter_assert!(reporter, subset.width() == W);
            reporter_assert!(reporter, subset.height() == 2);
            reporter_assert!(reporter, subset.alpha_type() == bitmap.alpha_type());

            // Test copying an extracted subset.
            for pair_j in &G_PAIRS {
                let mut copy = Bitmap::new();
                let success = tool_utils::copy_to(&mut copy, pair_j.color_type, &subset);
                if !success {
                    // Skip checking that success matches fValid, which is redundant
                    // with the code below.
                    reporter_assert!(reporter, pair.color_type != pair_j.color_type);
                    continue;
                }

                // When performing a copy of an extracted subset, the gen id should
                // change.
                reporter_assert!(reporter, copy.generation_id() != subset.generation_id());

                reporter_assert!(reporter, copy.width() == W);
                reporter_assert!(reporter, copy.height() == 2);
            }
        }

        bitmap = src_premul.clone();
        if bitmap.extract_subset(&mut subset, r) {
            reporter_assert!(reporter, subset.alpha_type() == bitmap.alpha_type());
        }
    }
});

// Construct 4x4 pixels where we can look at a color and determine where it should be in the grid.
// alpha = 0xFF, blue = 0x80, red = x, green = y
// Port of: tests/BitmapCopyTest.cpp#L134-L144 (chrome/m156)
fn fill_4x4_pixels(colors: &mut [PMColor; 16]) {
    for y in 0..4u32 {
        for x in 0..4u32 {
            colors[(y * 4 + x) as usize] = pack_argb32(0xFF, x, y, 0x80);
        }
    }
}

// Port of: tests/BitmapCopyTest.cpp#L146-L152 (chrome/m156)
fn check_4x4_pixel(color: PMColor, x: u32, y: u32) -> bool {
    debug_assert!(x < 4 && y < 4);
    0xFF == get_packed_a32(color)
        && x == get_packed_r32(color)
        && y == get_packed_g32(color)
        && 0x80 == get_packed_b32(color)
}

// Fill with all zeros, which will never match any value from fill_4x4_pixels
// Port of: tests/BitmapCopyTest.cpp#L154-L159 (chrome/m156)
fn clear_4x4_pixels(colors: &mut [u8; 64]) {
    colors.fill(0); // SkOpts::memset32(colors, 0, 16)
}

fn to_bytes(colors: &[PMColor; 16]) -> Vec<u8> {
    colors.iter().flat_map(|c| c.to_ne_bytes()).collect()
}

fn pixel(bytes: &[u8; 64], i: usize) -> PMColor {
    u32::from_ne_bytes([
        bytes[4 * i],
        bytes[4 * i + 1],
        bytes[4 * i + 2],
        bytes[4 * i + 3],
    ])
}

// Much of readPixels is exercised by copyTo testing, since readPixels is the backend for that
// method. Here we explicitly test subset copies.
//
// Port of: tests/BitmapCopyTest.cpp#L161-L222 (chrome/m156)
def_tier_test!(BitmapReadPixels, |reporter| {
    const W: i32 = 4;
    const H: i32 = 4;
    let row_bytes = W as usize * size_of::<PMColor>();
    let src_info = ImageInfo::new_n32_premul((W, H), None);
    let mut src_pixels = [0 as PMColor; 16];
    fill_4x4_pixels(&mut src_pixels);
    let mut src_bm = Bitmap::new();
    let _ = src_bm.install_pixels(&src_info, to_bytes(&src_pixels), row_bytes);

    let mut dst_info = ImageInfo::new_n32_premul((W, H), None);
    let mut dst_pixels = [0u8; 64];

    #[allow(clippy::items_after_statements)] // the C++ declares `struct Rec` here
    struct Rec {
        expected_success: bool,
        requested_src_loc: IPoint,
        requested_dst_size: ISize,
        // If expected_success, check these, otherwise ignore
        expected_dst_loc: IPoint,
        expected_src_r: IRect,
    }

    let rec = |s: bool, src: (i32, i32), size: (i32, i32), dst: (i32, i32), r: [i32; 4]| Rec {
        expected_success: s,
        requested_src_loc: IPoint { x: src.0, y: src.1 },
        requested_dst_size: ISize::new(size.0, size.1),
        expected_dst_loc: IPoint { x: dst.0, y: dst.1 },
        expected_src_r: IRect::from_ltrb(r[0], r[1], r[2], r[3]),
    };
    let g_rec = [
        rec(true, (0, 0), (4, 4), (0, 0), [0, 0, 4, 4]),
        rec(true, (1, 1), (2, 2), (0, 0), [1, 1, 3, 3]),
        rec(true, (2, 2), (4, 4), (0, 0), [2, 2, 4, 4]),
        rec(true, (-1, -1), (2, 2), (1, 1), [0, 0, 1, 1]),
        rec(false, (-1, -1), (1, 1), (0, 0), [0, 0, 0, 0]),
    ];

    for g in &g_rec {
        clear_4x4_pixels(&mut dst_pixels);

        dst_info = dst_info.with_dimensions(g.requested_dst_size);
        let success = src_bm.read_pixels(
            &dst_info,
            &mut dst_pixels,
            row_bytes,
            g.requested_src_loc.x,
            g.requested_src_loc.y,
        );

        reporter_assert!(reporter, g.expected_success == success);
        if success {
            let src_r = g.expected_src_r;
            let dst_x = g.expected_dst_loc.x;
            let dst_y = g.expected_dst_loc.y;
            // Walk the dst pixels, and check if we got what we expected
            for y in 0..H {
                for x in 0..W {
                    #[allow(clippy::cast_sign_loss)] // x, y are in [0, 4)
                    let dst_c = pixel(&dst_pixels, (y * 4 + x) as usize);
                    // get into src coordinates
                    let sx = x - dst_x + src_r.x();
                    let sy = y - dst_y + src_r.y();
                    if src_r.contains(IPoint { x: sx, y: sy }) {
                        #[allow(clippy::cast_sign_loss)] // contained, so non-negative
                        let ok = check_4x4_pixel(dst_c, sx as u32, sy as u32);
                        reporter_assert!(reporter, ok);
                    } else {
                        reporter_assert!(reporter, 0 == dst_c);
                    }
                }
            }
        }
    }
});
