// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: gm/all_bitmap_configs.cpp (chrome/m156)
#![allow(clippy::cast_precision_loss)] // mirrors the C++ int-to-scalar conversions of small sizes (exact in f32)
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::similar_names
)] // mirrors the C++ byte, int and float conversions of the GM

use crate::tool_utils::{copy_to, draw_checkerboard, get_resource_as_bitmap};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::{AutoCanvasRestore, Canvas as CoreCanvas};
use skia_rust_core::color::Color;
use skia_rust_core::color::Color4f;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::{FontStyle, Slant, Weight, Width};
use skia_rust_core::font_types::TextEncoding;
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_raster::raster_canvas::RasterCanvas;
use skia_rust_tools::font_tool_utils::{create_portable_typeface, default_portable_font};

const SCALE: i32 = 128;

// Port of: gm/all_bitmap_configs.cpp#L14-L25 (copy_bitmap)
fn copy_bitmap(src: &Bitmap, color_type: ColorType) -> Bitmap {
    let mut tmp = src.clone();
    let mut src_ptr = src;
    if color_type == ColorType::RGB565 {
        tmp.set_alpha_type(AlphaType::Opaque);
        src_ptr = &tmp;
    }

    let mut copy = Bitmap::new();
    // ToolUtils::copy_to's result is not checked by the C++ either.
    let _ = copy_to(&mut copy, color_type, src_ptr);
    copy.set_immutable();
    copy
}

// Make either A8 or gray8 bitmap.
// Port of: gm/all_bitmap_configs.cpp#L30-L52 (make_bitmap)
fn make_bitmap(ct: ColorType) -> Bitmap {
    let mut bm = Bitmap::new();
    match ct {
        ColorType::Alpha8 => {
            bm.alloc_pixels_info(
                &ImageInfo::new(
                    (SCALE, SCALE),
                    ColorType::Alpha8,
                    AlphaType::Premul,
                    None::<ColorSpace>,
                ),
                None,
            );
        }
        ColorType::Gray8 => {
            bm.alloc_pixels_info(
                &ImageInfo::new((SCALE, SCALE), ct, AlphaType::Opaque, None::<ColorSpace>),
                None,
            );
        }
        _ => unreachable!("make_bitmap is only called with A8 or Gray8"),
    }
    let mut spectrum = [0u8; 256];
    for (y, v) in spectrum.iter_mut().enumerate() {
        *v = y as u8;
    }
    for y in 0..128 {
        // Shift over one byte each scanline: memcpy(getAddr8(0, y), &spectrum[y], 128).
        for x in 0..128 {
            bm.set_addr8(x, y, spectrum[y as usize + x as usize]);
        }
    }
    bm.set_immutable();
    bm
}

// Port of: gm/all_bitmap_configs.cpp#L54-L62 (draw_center_letter)
fn draw_center_letter(c: char, font: &Font, color: Color, x: f32, y: f32, canvas: &CoreCanvas) {
    let text = c.to_string();
    let (_, bounds) = font.measure_text(text.as_bytes(), TextEncoding::UTF8, None);
    let paint = Paint::new(Color4f::from_color(color), None::<&ColorSpace>);
    canvas.draw_simple_text(
        text.as_bytes(),
        TextEncoding::UTF8,
        (x - bounds.center_x(), y - bounds.center_y()),
        font,
        &paint,
    );
}

// Port of: gm/all_bitmap_configs.cpp#L64-L84 (color_wheel_native)
fn color_wheel_native(canvas: &CoreCanvas) {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    canvas.translate((0.5 * SCALE as f32, 0.5 * SCALE as f32));
    let mut white = Paint::default();
    white.set_color(Color::WHITE);
    canvas.draw_circle((0.0, 0.0), SCALE as f32 * 0.5, &white);

    let sqrt_3_over_2 = 0.866_025_403_783_438_7_f64;
    let z = 0.0_f32;
    let d = 0.3_f32 * (SCALE as f32);
    let x = (f64::from(d) * sqrt_3_over_2) as f32;
    let y = d * 0.5_f32;

    let typeface = create_portable_typeface(
        Some("Sans"),
        FontStyle::new(Weight::BOLD, Width::NORMAL, Slant::Upright),
    );
    let mut font = Font::from_typeface(Some(typeface));
    font.set_edging(Edging::Alias);
    font.set_size(0.281_25_f32 * SCALE as f32);
    draw_center_letter('K', &font, Color::BLACK, z, z, canvas);
    draw_center_letter('R', &font, Color::RED, z, d, canvas);
    draw_center_letter('G', &font, Color::GREEN, -x, -y, canvas);
    draw_center_letter('B', &font, Color::BLUE, x, -y, canvas);
    draw_center_letter('C', &font, Color::CYAN, z, -d, canvas);
    draw_center_letter('M', &font, Color::MAGENTA, x, y, canvas);
    draw_center_letter('Y', &font, Color::YELLOW, -x, y, canvas);
}

// Port of: gm/all_bitmap_configs.cpp#L86-L96 (draw)
fn draw(canvas: &CoreCanvas, p: &Paint, font: &Font, src: &Bitmap, text: &str) {
    canvas.draw_image(src.as_image().expect("an image"), (0.0, 0.0), None);
    canvas.draw_simple_text(text.as_bytes(), TextEncoding::UTF8, (0.0, 12.0), font, p);
}

// Port of: gm/all_bitmap_configs.cpp#L98-L127 (DEF_SIMPLE_GM all_bitmap_configs)
crate::def_simple_gm!(all_bitmap_configs, canvas, SCALE, 6 * SCALE, {
    let _acr = AutoCanvasRestore::guard(canvas, true);
    let mut p = Paint::new(Color4f::from_color(Color::BLACK), None::<&ColorSpace>);
    p.set_anti_alias(true);

    let font = default_portable_font();

    draw_checkerboard(canvas, Color::LIGHT_GRAY, Color::WHITE, 8);

    if let Some(bitmap) = get_resource_as_bitmap("images/color_wheel.png") {
        let mut bitmap = bitmap;
        bitmap.set_immutable();
        draw(canvas, &p, &font, &bitmap, "Native 32");

        canvas.translate((0.0, SCALE as f32));
        let copy565 = copy_bitmap(&bitmap, ColorType::RGB565);
        p.set_color(Color::RED);
        draw(canvas, &p, &font, &copy565, "RGB 565");
        p.set_color(Color::BLACK);

        canvas.translate((0.0, SCALE as f32));
        let copy4444 = copy_bitmap(&bitmap, ColorType::ARGB4444);
        draw(canvas, &p, &font, &copy4444, "ARGB 4444");

        canvas.translate((0.0, SCALE as f32));
        let copy_f16 = copy_bitmap(&bitmap, ColorType::RGBAF16);
        draw(canvas, &p, &font, &copy_f16, "RGBA F16");
    } else {
        canvas.translate((0.0, 3.0 * SCALE as f32));
    }

    canvas.translate((0.0, SCALE as f32));
    let bitmap_a8 = make_bitmap(ColorType::Alpha8);
    draw(canvas, &p, &font, &bitmap_a8, "Alpha 8");

    p.set_color(Color::RED);
    canvas.translate((0.0, SCALE as f32));
    let bitmap_g8 = make_bitmap(ColorType::Gray8);
    draw(canvas, &p, &font, &bitmap_g8, "Gray 8");
});

// Port of: gm/all_bitmap_configs.cpp#L129-L144 (make_not_native32_color_wheel)
fn make_not_native32_color_wheel() -> Image {
    let mut n32bitmap = Bitmap::new();
    n32bitmap.alloc_n32_pixels((SCALE, SCALE), false);
    n32bitmap.erase_color(Color::TRANSPARENT);
    {
        let n32canvas = CoreCanvas::from_bitmap(&mut n32bitmap, None).expect("a canvas");
        color_wheel_native(&n32canvas);
    }
    // SK_PMCOLOR_BYTE_ORDER(B,G,R,A) picks RGBA, and the other order picks BGRA.
    let ct = if ColorType::n32() == ColorType::BGRA8888 {
        ColorType::RGBA8888
    } else {
        ColorType::BGRA8888
    };
    let mut not_n32bitmap = Bitmap::new();
    assert!(copy_to(&mut not_n32bitmap, ct, &n32bitmap));
    assert_eq!(not_n32bitmap.color_type(), ct);
    not_n32bitmap.as_image().expect("an image")
}

// Port of: gm/all_bitmap_configs.cpp#L146-L151 (DEF_SIMPLE_GM not_native32_bitmap_config)
crate::def_simple_gm!(not_native32_bitmap_config, canvas, SCALE, SCALE, {
    let not_n32image = make_not_native32_color_wheel();
    draw_checkerboard(canvas, Color::LIGHT_GRAY, Color::WHITE, 8);
    canvas.draw_image(&not_n32image, (0.0, 0.0), None);
});

// Port of: gm/all_bitmap_configs.cpp#L153-L172 (make_pixel)
fn make_pixel(x: i32, y: i32, alpha_type: AlphaType) -> u32 {
    let r = SCALE as f32 / 2.0;

    let mut alpha: u32 = 0x00;

    let fx = x as f32;
    let fy = y as f32;
    if (fx - r) * (fx - r) + (fy - r) * (fy - r) < r * r {
        alpha = 0xFF;
    }

    let component = match alpha_type {
        AlphaType::Premul => alpha,
        AlphaType::Unpremul => 0xFF,
        _ => unreachable!("Should not get here - invalid alpha type"),
    };
    alpha << 24 | component
}

// Port of: gm/all_bitmap_configs.cpp#L174-L187 (make_color_test_bitmap_variant)
fn make_color_test_bitmap_variant(
    color_type: ColorType,
    alpha_type: AlphaType,
    color_space: Option<ColorSpace>,
    bm: &mut Bitmap,
) {
    let info = ImageInfo::new((SCALE, SCALE), color_type, alpha_type, color_space);
    bm.alloc_pixels_info(&info, None);
    for y in 0..SCALE {
        for x in 0..SCALE {
            bm.set_addr32(x, y, make_pixel(x, y, alpha_type));
        }
    }
}

// Port of: gm/all_bitmap_configs.cpp#L189-L213 (DEF_SIMPLE_GM all_variants_8888)
crate::def_simple_gm!(all_variants_8888, canvas, 4 * SCALE + 30, 2 * SCALE + 10, {
    draw_checkerboard(canvas, Color::LIGHT_GRAY, Color::WHITE, 8);

    let color_spaces: [Option<ColorSpace>; 2] = [Some(ColorSpace::new_srgb()), None];
    for color_space in color_spaces {
        canvas.save();
        for alpha_type in [AlphaType::Premul, AlphaType::Unpremul] {
            canvas.save();
            for color_type in [ColorType::RGBA8888, ColorType::BGRA8888] {
                let mut bm = Bitmap::new();
                make_color_test_bitmap_variant(
                    color_type,
                    alpha_type,
                    color_space.clone(),
                    &mut bm,
                );
                canvas.draw_image(bm.as_image().expect("an image"), (0.0, 0.0), None);
                canvas.translate(((SCALE + 10) as f32, 0.0));
            }
            canvas.restore();
            canvas.translate((0.0, (SCALE + 10) as f32));
        }
        canvas.restore();
        canvas.translate((2.0 * (SCALE + 10) as f32, 0.0));
    }
});
