// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CoreBlittersTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color::{Alpha, Color};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mask::{Mask, MaskFormat};
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::IRect;
use skia_rust_raster::blitter::Blitter;
use skia_rust_raster::core_blitters::{Argb32BlackBlitter, Argb32Blitter, Argb32OpaqueBlitter};

use crate::{Reporter, def_tier_test, reporter_assert};

// Port of: tests/CoreBlittersTest.cpp#L23-L30 (chrome/m156)
fn all_pixels_same_color(buffer: &[u8]) -> bool {
    let pixel = |i: usize| &buffer[i * 4..i * 4 + 4];
    (1..buffer.len() / 4).all(|i| pixel(0) == pixel(i))
}

fn pixel0(buffer: &[u8]) -> u32 {
    u32::from_ne_bytes([buffer[0], buffer[1], buffer[2], buffer[3]])
}

// Port of: tests/CoreBlittersTest.cpp#L32 (chrome/m156)
type BlitterFactory = for<'a> fn(Pixmap<'a>, &Paint) -> Box<dyn Blitter + 'a>;

// Port of: tests/CoreBlittersTest.cpp#L34-L120 (chrome/m156)
fn compare_mask_and_anti_h(
    reporter: &mut Reporter,
    background_color: Color,
    paint_color: Color,
    make_blitter: BlitterFactory,
) {
    // 19 is big enough to exercise any multi-lane code (e.g. SIMD/NEON)
    // and have some remainder to exercise single-pixel code.
    const PIXELS_TO_BLIT: usize = 19;
    const _: () = assert!(!PIXELS_TO_BLIT.is_multiple_of(4));
    const _: () = assert!(!PIXELS_TO_BLIT.is_multiple_of(8));
    const _: () = assert!(!PIXELS_TO_BLIT.is_multiple_of(16));

    // Space for a kPixelsToBlit by 1 pixel image of 8888 color
    let mut buffer1;
    let mut buffer2;
    let ii = ImageInfo::new(
        (i32::try_from(PIXELS_TO_BLIT).unwrap(), 1),
        ColorType::N32,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    );

    let mut paint = Paint::default();
    paint.set_color(paint_color);

    let mut anti_alias = [0 as Alpha; 1];
    // The blitAntiH needs a buffer where the first value is the number of pixels
    // to blit and then at least that many 0s so we don't read off the end of the
    // buffer to figure out we need to stop.
    let mut runs = [0i16; PIXELS_TO_BLIT + 1];
    runs[0] = i16::try_from(PIXELS_TO_BLIT).unwrap();

    let mut cleared = [0u8; PIXELS_TO_BLIT * 4];
    Pixmap::new(&ii, &mut cleared, ii.min_row_bytes())
        .unwrap()
        .erase(background_color, None);

    let mask_bounds = IRect::from_xywh(0, 0, i32::try_from(PIXELS_TO_BLIT).unwrap(), 1);
    for alpha in 0..=255u8 {
        anti_alias[0] = alpha;
        let mask_image = [alpha; PIXELS_TO_BLIT];

        let mask = Mask::new(
            &mask_image,
            mask_bounds,
            u32::try_from(PIXELS_TO_BLIT).unwrap(),
            MaskFormat::A8,
        );

        // skia-rust: the C++ clears through a Canvas (D6) on the surfaces that wrap the buffers
        // while the blitters, which alias the same pixels, are alive. A Rust blitter borrows its
        // pixels, so each iteration restores the cleared pixels (`cleared`, made with
        // `Pixmap::erase`: the same pixels as a clear of these opaque colors) and makes the
        // blitter after it; the blitters keep no other state.
        buffer1 = cleared;
        {
            let device1 = Pixmap::new(&ii, &mut buffer1, ii.min_row_bytes()).unwrap();
            let mut blitter1 = make_blitter(device1, &paint);
            blitter1.blit_anti_h(0, 0, &mut anti_alias, &mut runs);
        }
        buffer2 = cleared;
        {
            let device2 = Pixmap::new(&ii, &mut buffer2, ii.min_row_bytes()).unwrap();
            let mut blitter2 = make_blitter(device2, &paint);
            blitter2.blit_mask(&mask, &mask.bounds);
        }

        if !all_pixels_same_color(&buffer1) {
            let extra = format!(
                "background={:08x}, paint={:08x}, alpha={}",
                u32::from(background_color),
                u32::from(paint_color),
                alpha
            );
            // REPORT_FAILURE(reporter, "blitAntiH was not the same for all pixels", extra)
            crate::errorf!(
                reporter,
                "blitAntiH was not the same for all pixels: {extra}"
            );
            return;
        }
        if !all_pixels_same_color(&buffer2) {
            let extra = format!(
                "background={:08x}, paint={:08x}, alpha={}",
                u32::from(background_color),
                u32::from(paint_color),
                alpha
            );
            crate::errorf!(
                reporter,
                "blitMask was not the same for all pixels: {extra}"
            );
            return;
        }

        let anti_h_color = pixel0(&buffer1);
        let mask_color = pixel0(&buffer2);
        let (a, m) = (Color::new(anti_h_color), Color::new(mask_color));
        reporter_assert!(
            reporter,
            anti_h_color == mask_color,
            "background={:08x}, paint={:08x}, alpha={}, blitAntiH={:08x}, blitMask={:08x}, \
             diff={:02x} {:02x} {:02x} {:02x}",
            u32::from(background_color),
            u32::from(paint_color),
            alpha,
            anti_h_color,
            mask_color,
            a.a().abs_diff(m.a()),
            a.r().abs_diff(m.r()),
            a.g().abs_diff(m.g()),
            a.b().abs_diff(m.b())
        );
    }
}

fn background_colors() -> [Color; 10] {
    [
        Color::WHITE,
        Color::BLACK,
        Color::GRAY,
        Color::RED,
        Color::GREEN,
        Color::BLUE,
        Color::YELLOW,
        Color::CYAN,
        Color::MAGENTA,
        // arbitrary opaque color with uneven channels
        Color::from_argb(255, 255 / 4, 255 / 3, 255 / 2),
    ]
}

fn paint_colors() -> [Color; 10] {
    background_colors()
}

fn make_opaque<'a>(device: Pixmap<'a>, paint: &Paint) -> Box<dyn Blitter + 'a> {
    Box::new(Argb32OpaqueBlitter::new(device, paint))
}

fn make_black<'a>(device: Pixmap<'a>, paint: &Paint) -> Box<dyn Blitter + 'a> {
    Box::new(Argb32BlackBlitter::new(device, paint))
}

fn make_argb32<'a>(device: Pixmap<'a>, paint: &Paint) -> Box<dyn Blitter + 'a> {
    Box::new(Argb32Blitter::new(device, paint))
}

// Port of: tests/CoreBlittersTest.cpp#L122-L147 (chrome/m156)
def_tier_test!(SkARGB32OpaqueBlitter_MaskAndAntiHDrawTheSame, |reporter| {
    for background_color in background_colors() {
        for paint_color in paint_colors() {
            compare_mask_and_anti_h(reporter, background_color, paint_color, make_opaque);
        }
    }
});

// Port of: tests/CoreBlittersTest.cpp#L149-L166 (chrome/m156)
def_tier_test!(SkARGB32BlackBlitter_MaskAndAntiHDrawTheSame, |reporter| {
    for background_color in background_colors() {
        compare_mask_and_anti_h(reporter, background_color, Color::BLACK, make_black);
    }
});

// Port of: tests/CoreBlittersTest.cpp#L168-L199 (chrome/m156)
def_tier_test!(SkARGB32Blitter_MaskAndAntiHDrawTheSame, |reporter| {
    let alpha_values: [Alpha; 5] = [
        0, 10, 100, 200, 245, /* SkARGB32_Opaque_Blitter is used when alpha is 255 */
    ];

    for background_color in background_colors() {
        for paint_color in paint_colors() {
            for alpha in alpha_values {
                let new_color = paint_color.with_a(alpha);
                compare_mask_and_anti_h(reporter, background_color, new_color, make_argb32);
            }
        }
    }
});
