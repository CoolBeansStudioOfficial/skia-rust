// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/NotifyInUseTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::canvas::SaveLayerRec;
use skia_rust_core::color::Color;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_gpu::gpu::gpu_types::Mipmapped;
use skia_rust_gpu::graphite::surface_graphite::Surface;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::{def_graphite_adapter_test, reporter_assert};

// Port of: tests/graphite/NotifyInUseTest.cpp#L27-L33 (chrome/m156)
fn colors_are_similar(c1: Color, c2: Color, tolerance: i32) -> bool {
    if (i32::from(c1.a()) - i32::from(c2.a())).abs() > tolerance {
        return false;
    }
    if (i32::from(c1.r()) - i32::from(c2.r())).abs() > tolerance {
        return false;
    }
    if (i32::from(c1.g()) - i32::from(c2.g())).abs() > tolerance {
        return false;
    }
    if (i32::from(c1.b()) - i32::from(c2.b())).abs() > tolerance {
        return false;
    }
    true
}

// `surface->readPixels(bitmap, 0, 0)`: the recorder is snapped inside the read.
fn read_pixels(context: &mut WgpuContext, surface: &Surface, bitmap: &mut Bitmap) -> bool {
    let Some(mut pm) = bitmap.peek_pixels_mut() else {
        return false;
    };
    context.read_surface_pixels(surface, &mut pm, 0, 0)
}

fn paint_of(color: u32) -> Paint {
    let mut paint = Paint::default();
    paint.set_color(Color::new(color));
    paint
}

// Port of: tests/graphite/NotifyInUseTest.cpp#L35-L65 (chrome/m156)
fn layer_test(bitmap: &mut Bitmap, context: &mut WgpuContext, blend_mode: BlendMode) -> bool {
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new((256, 256), ColorType::RGBA8888, AlphaType::Premul, None);
    let Some(surface) = Surface::render_target(&recorder, &info, Mipmapped::No, None, "") else {
        return false;
    };
    let canvas = surface.canvas();

    let mut blend_paint = Paint::default();
    blend_paint.set_blend_mode(blend_mode);

    let blue_paint = paint_of(0xFF00_99FF);
    canvas.clear(Color::TRANSPARENT);
    canvas.draw_rect(Rect::from_xywh(5.0, 5.0, 45.0, 45.0), &blue_paint);

    canvas.save_layer(&SaveLayerRec::default().paint(&blend_paint));
    let green_paint = paint_of(0xCC33_FF99);
    canvas.clear(Color::TRANSPARENT);
    canvas.draw_rect(Rect::from_xywh(30.0, 15.0, 45.0, 45.0), &green_paint);
    canvas.restore();

    canvas.save_layer(&SaveLayerRec::default().paint(&blend_paint));
    let red_paint = paint_of(0xFFFF_3300);
    canvas.clear(Color::TRANSPARENT);
    canvas.draw_rect(Rect::from_xywh(20.0, 25.0, 45.0, 45.0), &red_paint);
    canvas.restore();

    bitmap.alloc_pixels_info(&info, None);
    read_pixels(context, &surface, bitmap)
}

// This test creates a dependency chain between different surfaces, by forcing a
// Device::makeImageCopy through makeImageSnapshot(). See the C++ file for the task orders.
// Port of: tests/graphite/NotifyInUseTest.cpp#L103-L141 (chrome/m156)
def_graphite_adapter_test!(NotifyInUseTestSnapshot, |reporter, context| {
    let recorder = context.make_recorder(None);
    let info = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);

    // "A"
    let surface_a = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    let canvas_a = surface_a.canvas();
    let green_paint = paint_of(Color::GREEN.into());
    canvas_a.draw_rect(Rect::from_irect(info.bounds()), &green_paint);
    let image_a = surface_a.as_image();

    // "B"
    let surface_b = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    let canvas_b = surface_b.canvas();
    canvas_b.clear(Color::BLACK);
    canvas_b.draw_image(&image_a, (0.0, 0.0), None);

    // "C"
    let surface_c = Surface::render_target(&recorder, &info, Mipmapped::No, None, "").unwrap();
    let canvas_c = surface_c.canvas();
    canvas_c.clear(Color::BLACK);
    let snapshot_b = surface_b.make_image_copy(None, Mipmapped::No);
    reporter_assert!(reporter, snapshot_b.is_some());
    let Some(snapshot_b) = snapshot_b else {
        return;
    };
    canvas_c.draw_image(&snapshot_b, (0.0, 0.0), None);

    // Recorder snap inside readPixels
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&info, None);
    reporter_assert!(reporter, read_pixels(context, &surface_c, &mut bitmap));

    let top_left = bitmap.get_color((0, 0));
    reporter_assert!(
        reporter,
        top_left == Color::GREEN,
        "Expected green pixel from surface C, got 0x{:08X}",
        u32::from(top_left)
    );
});

// This test confirms that mixing reads from an image view of a surface (SkSurfaces::AsImage)
// with writes to that surface's canvas order tasks correctly (A1, B1, A2, B2).
// Port of: tests/graphite/NotifyInUseTest.cpp#L182-L222 (chrome/m156)
def_graphite_adapter_test!(NotifyInUseTestAsImage, |reporter, context| {
    let recorder = context.make_recorder(None);
    let a_info = ImageInfo::new((10, 10), ColorType::RGBA8888, AlphaType::Premul, None);
    let b_info = ImageInfo::new((20, 10), ColorType::RGBA8888, AlphaType::Premul, None);

    // "A1"
    let surface_a = Surface::render_target(&recorder, &a_info, Mipmapped::No, None, "").unwrap();
    surface_a.canvas().clear(Color::BLUE);
    let image_a = surface_a.as_image();

    // "B1"
    let surface_b = Surface::render_target(&recorder, &b_info, Mipmapped::No, None, "").unwrap();
    let canvas_b = surface_b.canvas();
    canvas_b.clear(Color::BLACK);
    canvas_b.draw_image(&image_a, (0.0, 0.0), None); // Should see A's blue clear in left half of B

    // "A2"
    surface_a.canvas().clear(Color::RED);

    // "B2"
    canvas_b.draw_image(&image_a, (10.0, 0.0), None); // Should see A's now-red clear in right half of B

    // Recorder snaps inside readPixels
    let mut bitmap = Bitmap::new();
    bitmap.alloc_pixels_info(&b_info, None);
    reporter_assert!(reporter, read_pixels(context, &surface_b, &mut bitmap));

    let left_b = bitmap.get_color((5, 5));
    reporter_assert!(
        reporter,
        left_b == Color::BLUE,
        "Expected blue pixel from surface B's left half, got 0x{:08X}",
        u32::from(left_b)
    );
    let right_b = bitmap.get_color((15, 5));
    reporter_assert!(
        reporter,
        right_b == Color::RED,
        "Expected red pixel from surface B's right half, got 0x{:08X}",
        u32::from(right_b)
    );
});

// These tests replicate the compositing behavior in blink's canvas2d (see the C++ file for the
// task orders).
// Port of: tests/graphite/NotifyInUseTest.cpp#L335-L394 (chrome/m156), DEFINE_LAYER_BLEND_TEST
macro_rules! define_layer_blend_test {
    ($test_name:ident, $blend_mode:expr, $expected_blue_only:expr, $expected_green_only:expr,
     $expected_blue_and_green:expr, $expected_red_only:expr, $expected_all_overlap:expr) => {
        def_graphite_adapter_test!($test_name, |reporter, context| {
            let blend_mode: BlendMode = $blend_mode;
            let mut bitmap = Bitmap::new();
            reporter_assert!(
                reporter,
                layer_test(&mut bitmap, context, blend_mode),
                "Failed to read pixels for BlendMode '{}'",
                blend_mode.name()
            );

            let k_tolerance: i32 = 2;
            let checks: [(i32, i32, u32, &str); 5] = [
                (10, 10, $expected_blue_only, "blue-only"),
                (70, 30, $expected_green_only, "green-only"),
                (40, 20, $expected_blue_and_green, "blue/green"),
                (25, 68, $expected_red_only, "red-only"),
                (40, 40, $expected_all_overlap, "all-overlap"),
            ];
            for (x, y, expected, what) in checks {
                let got = bitmap.get_color((x, y));
                reporter_assert!(
                    reporter,
                    colors_are_similar(got, Color::new(expected), k_tolerance),
                    "Pt({},{}) BlendMode '{}': Expected {} ~0x{:08X}, got 0x{:08X}",
                    x,
                    y,
                    blend_mode.name(),
                    what,
                    expected,
                    u32::from(got)
                );
            }
        });
    };
}

// Basic Porter-Duff blend modes
// Port of: tests/graphite/NotifyInUseTest.cpp#L396-L426 (chrome/m156)
define_layer_blend_test!(
    Clear,
    BlendMode::Clear,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000
);
define_layer_blend_test!(
    Src,
    BlendMode::Src,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xFFFF_3300,
    0xFFFF_3300
);
define_layer_blend_test!(
    Dst,
    BlendMode::Dst,
    0xFF00_99FF,
    0x0000_0000,
    0xFF00_99FF,
    0x0000_0000,
    0xFF00_99FF
);
define_layer_blend_test!(
    SrcOver,
    BlendMode::SrcOver,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF29_EBAD,
    0xFFFF_3300,
    0xFFFF_3300
);
define_layer_blend_test!(
    DstOver,
    BlendMode::DstOver,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_99FF,
    0xFFFD_3302,
    0xFF00_99FF
);
define_layer_blend_test!(
    SrcIn,
    BlendMode::SrcIn,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xCCFF_3300
);
define_layer_blend_test!(
    DstIn,
    BlendMode::DstIn,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xCC00_99FF
);
define_layer_blend_test!(
    SrcOut,
    BlendMode::SrcOut,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xFDFF_3300,
    0xFFFF_3300
);
define_layer_blend_test!(
    DstOut,
    BlendMode::DstOut,
    0xFF00_99FF,
    0x0000_0000,
    0x3300_9BFF,
    0x0000_0000,
    0x0000_0000
);
define_layer_blend_test!(
    SrcATop,
    BlendMode::SrcATop,
    0xFF00_99FF,
    0x0000_0000,
    0xFF29_EBAD,
    0x0000_0000,
    0xFFFF_3300
);
define_layer_blend_test!(
    DstATop,
    BlendMode::DstATop,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xFFFD_3302,
    0xFF33_84CC
);
define_layer_blend_test!(
    Xor,
    BlendMode::Xor,
    0xFF00_99FF,
    0xCC33_FF99,
    0x3300_9BFF,
    0xFDFF_3300,
    0xCCFF_3300
);
define_layer_blend_test!(
    Plus,
    BlendMode::Plus,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF29_FFFF,
    0xFFFF_3302,
    0xFFFF_FFFF
);
define_layer_blend_test!(
    Modulate,
    BlendMode::Modulate,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0x0000_0000,
    0xCC00_1E00
);
define_layer_blend_test!(
    Screen,
    BlendMode::Screen,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF29_EBFF,
    0xFFFF_3302,
    0xFFFF_EFFF
);

// Advanced, non-separable blend modes
// Port of: tests/graphite/NotifyInUseTest.cpp#L428-L448 (chrome/m156)
define_layer_blend_test!(
    Overlay,
    BlendMode::Overlay,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_EBFF,
    0xFFFD_3302,
    0xFF00_DFFF
);
define_layer_blend_test!(
    Darken,
    BlendMode::Darken,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_99AD,
    0xFFFD_3300,
    0xFF00_3300
);
define_layer_blend_test!(
    Lighten,
    BlendMode::Lighten,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF29_EBFF,
    0xFFFF_3302,
    0xFFFF_EBFF
);
define_layer_blend_test!(
    ColorDodge,
    BlendMode::ColorDodge,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_EBFF,
    0xFFFD_3302,
    0xFF00_FFFF
);
define_layer_blend_test!(
    ColorBurn,
    BlendMode::ColorBurn,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_99FF,
    0xFFFD_3302,
    0xFF00_00FF
);
define_layer_blend_test!(
    HardLight,
    BlendMode::HardLight,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_EBFF,
    0xFFFF_3300,
    0xFFFF_5E00
);
define_layer_blend_test!(
    SoftLight,
    BlendMode::SoftLight,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_BDFF,
    0xFFFD_3302,
    0xFF00_A0FF
);
define_layer_blend_test!(
    Difference,
    BlendMode::Difference,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF29_7085,
    0xFFFF_3302,
    0xFFD6_3D85
);
define_layer_blend_test!(
    Exclusion,
    BlendMode::Exclusion,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF29_7085,
    0xFFFF_3302,
    0xFFD6_7685
);
define_layer_blend_test!(
    Multiply,
    BlendMode::Multiply,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_99AD,
    0xFFFD_3300,
    0xFF00_1F00
);

// HSL component blend modes
// Port of: tests/graphite/NotifyInUseTest.cpp#L450-L458 (chrome/m156)
define_layer_blend_test!(
    Hue,
    BlendMode::Hue,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_B17C,
    0xFFFE_3300,
    0xFFDD_4F2C
);
define_layer_blend_test!(
    Saturation,
    BlendMode::Saturation,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF13_93E9,
    0xFFFD_3302,
    0xFF00_99FF
);
define_layer_blend_test!(
    Color,
    BlendMode::Color,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF00_B17C,
    0xFFFE_3300,
    0xFFFF_4314
);
define_layer_blend_test!(
    Luminosity,
    BlendMode::Luminosity,
    0xFF00_99FF,
    0xCC33_FF99,
    0xFF60_BFFF,
    0xFFFE_3302,
    0xFF21_80C0
);
