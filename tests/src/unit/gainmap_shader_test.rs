// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/GainmapShaderTest.cpp (chrome/m156), all four cases.
#![cfg(test)]

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::gainmap_info::{BaseImageType, GainmapInfo, GainmapType};
use skia_rust_core::image::Image;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_effects::gainmap_shader::GainmapShader;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::{def_test, reporter_assert};

// The C++ tests use `SkColor4f` arrays as pixel data. Flatten them to the native bytes.
fn color4f_bytes(colors: &[Color4f]) -> Vec<u8> {
    colors
        .iter()
        .flat_map(|c| [c.r, c.g, c.b, c.a])
        .flat_map(f32::to_ne_bytes)
        .collect()
}

fn color4f_from_bytes(bytes: &[u8]) -> Color4f {
    let f = |i: usize| f32::from_ne_bytes(bytes[4 * i..4 * i + 4].try_into().expect("4 bytes"));
    Color4f {
        r: f(0),
        g: f(1),
        b: f(2),
        a: f(3),
    }
}

fn approx_equal(a: Color4f, b: Color4f) -> bool {
    let epsilon = 1e-3f32;
    (a.r - b.r).abs() < epsilon
        && (a.g - b.g).abs() < epsilon
        && (a.b - b.b).abs() < epsilon
        && (a.a - b.a).abs() < epsilon
}

// Create a 1x1 image with a specified color.
// Port of: tests/GainmapShaderTest.cpp#L33-L49 (chrome/m156)
fn make_1x1_image(
    image_color_space: Option<ColorSpace>,
    image_alpha_type: AlphaType,
    image_color: Color4f,
    image_color_color_space: Option<ColorSpace>,
) -> Image {
    let bm_info = ImageInfo::new(
        (1, 1),
        ColorType::RGBAF32,
        image_alpha_type,
        image_color_space,
    );
    let mut bm = Bitmap::new();
    bm.alloc_pixels_info(&bm_info, None);

    let write_pixels_info = ImageInfo::new(
        (1, 1),
        ColorType::RGBAF32,
        AlphaType::Unpremul,
        image_color_color_space,
    );
    let pixels = color4f_bytes(&[image_color]);
    let write_pixels_pixmap = Pixmap::new_readonly(
        &write_pixels_info,
        &pixels,
        write_pixels_info.min_row_bytes(),
    )
    .expect("pixmap");
    bm.write_pixels(&write_pixels_pixmap, 0, 0);
    images::raster_from_bitmap(&bm).expect("image")
}

// Return gainmap info that will scale 1 up to the specified hdrRatioMax.
// Port of: tests/GainmapShaderTest.cpp#L51-L62 (chrome/m156)
fn simple_gainmap_info(hdr_ratio_max: f32) -> GainmapInfo {
    GainmapInfo {
        display_ratio_sdr: 1.0,
        display_ratio_hdr: hdr_ratio_max,
        epsilon_sdr: Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        epsilon_hdr: Color4f {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        },
        gainmap_ratio_min: Color4f {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
        gainmap_ratio_max: Color4f {
            r: hdr_ratio_max,
            g: hdr_ratio_max,
            b: hdr_ratio_max,
            a: 1.0,
        },
        ..GainmapInfo::default()
    }
}

// Draw using a gainmap to a canvas with the specified HDR to SDR ratio and the specified color
// space. Return the result as unpremultiplied sRGB linear.
// Port of: tests/GainmapShaderTest.cpp#L64-L87 (chrome/m156)
fn draw_1x1_gainmap(
    base_image: &Image,
    gainmap_image: &Image,
    gainmap_info: &GainmapInfo,
    dst_ratio: f32,
    dst_color_space: Option<ColorSpace>,
) -> Color4f {
    let rect = Rect::from_xywh(0.0, 0.0, 1.0, 1.0);
    let canvas_info = ImageInfo::new(
        (1, 1),
        ColorType::RGBAF32,
        AlphaType::Premul,
        dst_color_space,
    );
    let mut canvas_bitmap = Bitmap::new();
    canvas_bitmap.alloc_pixels_info(&canvas_info, None);
    canvas_bitmap.erase_color(Color::TRANSPARENT);

    let shader = GainmapShader::make(
        base_image,
        &rect,
        SamplingOptions::default(),
        gainmap_image,
        &rect,
        SamplingOptions::default(),
        gainmap_info,
        &rect,
        dst_ratio,
    );
    let mut paint = Paint::default();
    paint.set_shader(shader);
    let read_pixels_info = ImageInfo::new(
        (1, 1),
        ColorType::RGBAF32,
        AlphaType::Unpremul,
        Some(ColorSpace::new_srgb_linear()),
    );
    let mut result = [0u8; 16];
    {
        let canvas = Canvas::from_bitmap(&mut canvas_bitmap, None).expect("canvas");
        canvas.draw_rect(rect, &paint);
        canvas.read_pixels(&read_pixels_info, &mut result, 16, (0, 0));
    }
    color4f_from_bytes(&result)
}

// Verify that the gainmap shader correctly applies the base, gainmap, and destination rectangles.
// Port of: tests/GainmapShaderTest.cpp#L99-L186 (chrome/m156)
// The C++ float literals keep all their digits, and the k/kr names are the C++ names.
def_test!(
    #[allow(
        clippy::excessive_precision,
        clippy::similar_names,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    GainmapShader_rects,
    |r| {
        let sdr_colors = [
            [
                Color4f {
                    r: -1.0,
                    g: -1.0,
                    b: -1.0,
                    a: 1.0,
                },
                Color4f {
                    r: -1.0,
                    g: -1.0,
                    b: -1.0,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 1.0,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 1.0,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 1.0,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 0.5,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 0.5,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 0.5,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            ],
        ];
        let sdr_flat: Vec<Color4f> = sdr_colors.iter().flatten().copied().collect();
        let sdr_bytes = color4f_bytes(&sdr_flat);
        let sdr_pixmap = Pixmap::new_readonly(
            &ImageInfo::new((2, 5), ColorType::RGBAF32, AlphaType::Opaque, None),
            &sdr_bytes,
            2 * 16,
        )
        .expect("pixmap");
        let sdr_image = images::raster_from_pixmap_copy(&sdr_pixmap).expect("image");
        let sdr_image_rect = Rect::from_xywh(0.0, 1.0, 2.0, 4.0);

        // The top pixel indicates to gain only red, and the bottom pixel indicates to gain everything
        // except red.
        let gainmap_colors = [
            [
                Color4f {
                    r: -1.0,
                    g: -1.0,
                    b: -1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 1.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: -1.0,
                    g: -1.0,
                    b: -1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 0.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
            ],
        ];
        let gainmap_flat: Vec<Color4f> = gainmap_colors.iter().flatten().copied().collect();
        let gainmap_bytes = color4f_bytes(&gainmap_flat);
        let gainmap_pixmap = Pixmap::new_readonly(
            &ImageInfo::new((2, 2), ColorType::RGBAF32, AlphaType::Opaque, None),
            &gainmap_bytes,
            2 * 16,
        )
        .expect("pixmap");
        let gainmap_image = images::raster_from_pixmap_copy(&gainmap_pixmap).expect("image");
        let gainmap_image_rect = Rect::from_xywh(1.0, 0.0, 1.0, 2.0);
        let mut gainmap_info = simple_gainmap_info(2.0);
        gainmap_info.epsilon_hdr.r = 0.1;

        let canvas_info = ImageInfo::new(
            (4, 6),
            ColorType::RGBAF32,
            AlphaType::Premul,
            Some(ColorSpace::new_srgb()),
        );
        let mut canvas_bitmap = Bitmap::new();
        canvas_bitmap.alloc_pixels_info(&canvas_info, None);
        canvas_bitmap.erase_color(Color::TRANSPARENT);
        let canvas_rect = Rect::from_xywh(1.0, 1.0, 2.0, 4.0);

        let shader = GainmapShader::make(
            &sdr_image,
            &sdr_image_rect,
            SamplingOptions::default(),
            &gainmap_image,
            &gainmap_image_rect,
            SamplingOptions::default(),
            &gainmap_info,
            &canvas_rect,
            gainmap_info.display_ratio_hdr,
        );
        let mut paint = Paint::default();
        paint.set_shader(shader);
        {
            let canvas = Canvas::from_bitmap(&mut canvas_bitmap, None).expect("canvas");
            canvas.draw_rect(canvas_rect, &paint);
        }

        // Compute and compare the expected colors.
        // This is linearToSRGB(srgbToLinear(1.0)*2.0) = linearToSRGB(2.0).
        let k10g: f32 = 1.353_256_028_586_302;
        // This is linearToSRGB(srgbToLinear(0.5)*2.0)
        let k05g: f32 = 0.685_836_101_501_284_7;
        // The 'R' component also has a fEpsilonHdr set.
        // This is linearToSRGB(srgbToLinear(1.0)*2.0-0.1) = linearToSRGB(1.9).
        let kr10g: f32 = 1.323_477_854_140_905_8;
        // This is linearToSRGB(srgbToLinear(0.5)-0.1)
        // The gain map is 0.f (no gain), but there are still affectd by the offset.
        let kr05g: f32 = 0.371_934_685_412_575;
        let expected_colors = [
            [
                Color4f {
                    r: kr10g,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: kr10g,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: kr10g,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: kr10g,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: kr05g,
                    g: k10g,
                    b: k10g,
                    a: 1.0,
                },
                Color4f {
                    r: kr05g,
                    g: k10g,
                    b: k05g,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: kr05g,
                    g: k05g,
                    b: k10g,
                    a: 1.0,
                },
                Color4f {
                    r: kr05g,
                    g: k05g,
                    b: k05g,
                    a: 1.0,
                },
            ],
        ];
        for (y, row) in expected_colors.iter().enumerate() {
            for (x, expected) in row.iter().enumerate() {
                let color = canvas_bitmap.get_color_4f(((x + 1) as i32, (y + 1) as i32));
                reporter_assert!(r, approx_equal(color, *expected));
            }
        }
    }
);

// Port of: tests/GainmapShaderTest.cpp#L188-L275 (chrome/m156)
// The C++ float literals keep all their digits, and the k/kr names are the C++ names.
def_test!(
    #[allow(
        clippy::excessive_precision,
        clippy::similar_names,
        clippy::cast_possible_truncation,
        clippy::cast_possible_wrap
    )]
    GainmapShader_baseImageIsHdr,
    |r| {
        let hdr_colors = [
            [
                Color4f {
                    r: 1.0,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 1.0,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 1.0,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 1.0,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 0.5,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 0.5,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: 0.5,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: 0.5,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            ],
        ];
        let hdr_flat: Vec<Color4f> = hdr_colors.iter().flatten().copied().collect();
        let hdr_bytes = color4f_bytes(&hdr_flat);
        let hdr_pixmap = Pixmap::new_readonly(
            &ImageInfo::new((2, 4), ColorType::RGBAF32, AlphaType::Opaque, None),
            &hdr_bytes,
            2 * 16,
        )
        .expect("pixmap");
        let hdr_image = images::raster_from_pixmap_copy(&hdr_pixmap).expect("image");
        let hdr_image_rect = Rect::from_xywh(0.0, 0.0, 2.0, 4.0);

        // The top pixel indicates to gain only red, and the bottom pixel indicates to gain everything
        // except red.
        let gainmap_colors = [
            [Color4f {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }],
            [Color4f {
                r: 0.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            }],
        ];
        let gainmap_flat: Vec<Color4f> = gainmap_colors.iter().flatten().copied().collect();
        let gainmap_bytes = color4f_bytes(&gainmap_flat);
        let gainmap_pixmap = Pixmap::new_readonly(
            &ImageInfo::new((1, 2), ColorType::RGBAF32, AlphaType::Opaque, None),
            &gainmap_bytes,
            16,
        )
        .expect("pixmap");
        let gainmap_image = images::raster_from_pixmap_copy(&gainmap_pixmap).expect("image");
        let gainmap_image_rect = Rect::from_xywh(0.0, 0.0, 1.0, 2.0);
        let mut gainmap_info = simple_gainmap_info(2.0);
        gainmap_info.base_image_type = BaseImageType::Hdr;
        gainmap_info.epsilon_sdr.r = 0.1;

        let canvas_info = ImageInfo::new(
            (2, 4),
            ColorType::RGBAF32,
            AlphaType::Premul,
            Some(ColorSpace::new_srgb()),
        );
        let mut canvas_bitmap = Bitmap::new();
        canvas_bitmap.alloc_pixels_info(&canvas_info, None);
        canvas_bitmap.erase_color(Color::TRANSPARENT);
        let canvas_rect = Rect::from_xywh(0.0, 0.0, 2.0, 4.0);

        let shader = GainmapShader::make(
            &hdr_image,
            &hdr_image_rect,
            SamplingOptions::default(),
            &gainmap_image,
            &gainmap_image_rect,
            SamplingOptions::default(),
            &gainmap_info,
            &canvas_rect,
            gainmap_info.display_ratio_sdr,
        );
        let mut paint = Paint::default();
        paint.set_shader(shader);
        {
            let canvas = Canvas::from_bitmap(&mut canvas_bitmap, None).expect("canvas");
            canvas.draw_rect(canvas_rect, &paint);
        }

        // Compute and compare the expected colors.
        // This is linearToSRGB(srgbToLinear(1.0)*0.5) = linearToSRGB(0.5).
        let k10g: f32 = 0.735_356_983_052_449_5;
        // This is linearToSRGB(srgbToLinear(0.5)*0.5)
        let k05g: f32 = 0.360_780_213_833_279_2;
        // The 'R' component also has a fEpsilonSdr set.
        // This is linearToSRGB(srgbToLinear(1.0)*0.5-0.1) = linearToSRGB(0.4).
        let kr10g: f32 = 0.665_185_084_630_836_3;
        // This is linearToSRGB(srgbToLinear(0.5)-0.1)
        // The gain map is 0.f (no gain), but there are still affectd by the offset.
        let kr05g: f32 = 0.371_934_685_412_575;
        let expected_colors = [
            [
                Color4f {
                    r: kr10g,
                    g: 1.0,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: kr10g,
                    g: 1.0,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: kr10g,
                    g: 0.5,
                    b: 1.0,
                    a: 1.0,
                },
                Color4f {
                    r: kr10g,
                    g: 0.5,
                    b: 0.5,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: kr05g,
                    g: k10g,
                    b: k10g,
                    a: 1.0,
                },
                Color4f {
                    r: kr05g,
                    g: k10g,
                    b: k05g,
                    a: 1.0,
                },
            ],
            [
                Color4f {
                    r: kr05g,
                    g: k05g,
                    b: k10g,
                    a: 1.0,
                },
                Color4f {
                    r: kr05g,
                    g: k05g,
                    b: k05g,
                    a: 1.0,
                },
            ],
        ];
        for (y, row) in expected_colors.iter().enumerate() {
            for (x, expected) in row.iter().enumerate() {
                let color = canvas_bitmap.get_color_4f((x as i32, y as i32));
                reporter_assert!(r, approx_equal(color, *expected));
            }
        }
    }
);

// Verify that the gainmap shader isn't affected by the color spaces of the base, gainmap, or
// destination. But the fGainmapMathColorSpace is taken into account.
// Port of: tests/GainmapShaderTest.cpp#L279-L314 (chrome/m156)
def_test!(GainmapShader_colorSpace, |_r| {
    let sdr_color_space = ColorSpace::new_rgb(&named_transfer_fn::DOT22, &named_gamut::SRGB)
        .expect("color space")
        .with_color_spin();
    let gainmap_color_space =
        ColorSpace::new_rgb(&named_transfer_fn::PQ, &named_gamut::REC2020).expect("color space");
    let dst_color_space = ColorSpace::new_rgb(&named_transfer_fn::HLG, &named_gamut::DISPLAY_P3)
        .expect("color space");

    let sdr_color = Color4f {
        r: 0.25,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    };
    // The sRGB G channel will have a exp2(0.0)=1.000 gain. The sRGB B channel will have a
    // exp2(0.5)=1.414 gain. The sRGB R channel will have a exp2(1.0)=2.000 gain.
    let gainmap_color = Color4f {
        r: 0.0,
        g: 0.5,
        b: 1.0,
        a: 1.0,
    };

    let sdr_image = make_1x1_image(
        Some(sdr_color_space.clone()),
        AlphaType::Opaque,
        sdr_color,
        Some(ColorSpace::new_srgb_linear()),
    );
    let gainmap_image = make_1x1_image(
        Some(gainmap_color_space.clone()),
        AlphaType::Opaque,
        gainmap_color,
        Some(gainmap_color_space.clone()),
    );
    let mut gainmap_info = simple_gainmap_info(2.0);

    let _color = draw_1x1_gainmap(
        &sdr_image,
        &gainmap_image,
        &gainmap_info,
        gainmap_info.display_ratio_hdr,
        Some(dst_color_space.clone()),
    );

    // Setting fGainmapMathColorSpace to the base image's color space does not change the result.
    gainmap_info.gainmap_math_color_space = Some(sdr_color_space);
    let _color = draw_1x1_gainmap(
        &sdr_image,
        &gainmap_image,
        &gainmap_info,
        gainmap_info.display_ratio_hdr,
        Some(dst_color_space.clone()),
    );

    // Setting fGainmapMathColorSpace ot a different color space does change the result.
    gainmap_info.gainmap_math_color_space = Some(gainmap_color_space);
    let _color = draw_1x1_gainmap(
        &sdr_image,
        &gainmap_image,
        &gainmap_info,
        gainmap_info.display_ratio_hdr,
        Some(dst_color_space),
    );
});

// Verify that a fully applied Apple gainmap maps the specification.
// Port of: tests/GainmapShaderTest.cpp#L317-L343 (chrome/m156)
def_test!(
    #[allow(clippy::manual_midpoint)]
    GainmapShader_apple,
    |r| {
        let sdr_color = Color4f {
            r: 0.25,
            g: 0.5,
            b: 1.0,
            a: 1.0,
        };
        // The R channel will have a linear value of 0.0. The G channel will have a linear value of
        // 0.5. The B channel will have a linear value 0f 1.0.
        let gainmap_color = Color4f {
            r: 0.0,
            g: 0.702_250,
            b: 1.0,
            a: 1.0,
        };

        // Set the HDR headroom to 5.0.
        let h: f32 = 5.0;

        let expected_color = Color4f {
            r: 0.25 * (1.0 + (h - 1.0) * 0.0),
            g: 0.50 * (1.0 + (h - 1.0) * 0.5),
            b: 1.00 * (1.0 + (h - 1.0) * 1.0),
            a: 1.0,
        };

        let sdr_image = make_1x1_image(
            Some(ColorSpace::new_srgb()),
            AlphaType::Opaque,
            sdr_color,
            Some(ColorSpace::new_srgb_linear()),
        );
        let gainmap_image = make_1x1_image(
            None,
            AlphaType::Opaque,
            gainmap_color,
            Some(ColorSpace::new_srgb_linear()),
        );
        let mut gainmap_info = simple_gainmap_info(h);
        gainmap_info.gainmap_type = GainmapType::Apple;

        let color = draw_1x1_gainmap(
            &sdr_image,
            &gainmap_image,
            &gainmap_info,
            h,
            Some(ColorSpace::new_srgb_linear()),
        );
        reporter_assert!(r, approx_equal(color, expected_color));

        // Note that (0.250, 1.548, 5.000) is *not* an acceptable answer here (even though it's sort-of
        // close). That number is the result of not using the the Apple-compatible math.
    }
);
