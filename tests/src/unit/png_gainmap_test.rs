// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PngGainmapTest.cpp (chrome/m156), the decode cases. The encode case needs the
// PNG encoder's gainmap chunks, which are not ported.
#![cfg(test)]
// The C++ test literals are kept as written.
#![allow(clippy::excessive_precision)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::codec::Result as CodecResult;
use skia_rust_codec::png_codec;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::gainmap_info::{BaseImageType, GainmapInfo, GainmapType};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;

use crate::gainmap_test_common::expect_approx_eq_info;
use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert, skip_missing_resource};

/// The result of decoding a PNG and its gainmap (the locals of `decode_all`).
struct DecodedGainmap {
    /// Whether the image had a gainmap that decoded (`decodedGainmap`).
    decoded: bool,
    /// The gainmap rendering parameters (`gainmapInfo`).
    info: GainmapInfo,
    /// The gainmap's pixels, with their image info (`gainmapBitmap`).
    gainmap: Option<(ImageInfo, Vec<u8>)>,
}

// Decode an image and its gainmap.
// Port of: tests/PngGainmapTest.cpp#L23-L53 (chrome/m156), `decode_all`.
fn decode_all(r: &mut Reporter, data: &[u8]) -> DecodedGainmap {
    let mut result = DecodedGainmap {
        decoded: false,
        info: GainmapInfo::default(),
        gainmap: None,
    };

    // Decode the base bitmap.
    let Ok(mut base_codec) = png_codec::make_from_stream(MemoryStream::make_copy(data)) else {
        reporter_assert!(r, false);
        return result;
    };
    let base_info = base_codec.info().clone();
    let base_row_bytes = base_info.min_row_bytes();
    let mut base_pixels = vec![0u8; base_info.compute_byte_size(base_row_bytes)];
    reporter_assert!(
        r,
        CodecResult::Success
            == base_codec.get_pixels(&base_info, &mut base_pixels, base_row_bytes, None)
    );

    let Some(mut android_codec) = AndroidCodec::make_from_codec(base_codec) else {
        reporter_assert!(r, false);
        return result;
    };

    // Extract the gainmap info and codec.
    let mut gainmap_codec = None;
    result.decoded =
        android_codec.get_gainmap_android_codec(Some(&mut result.info), Some(&mut gainmap_codec));
    if result.decoded {
        let Some(mut gainmap_codec) = gainmap_codec else {
            reporter_assert!(r, false);
            return result;
        };
        // Decode the gainmap bitmap.
        let gainmap_info = gainmap_codec.info();
        let row_bytes = gainmap_info.min_row_bytes();
        let mut pixels = vec![0u8; gainmap_info.compute_byte_size(row_bytes)];
        reporter_assert!(
            r,
            CodecResult::Success
                == gainmap_codec.get_android_pixels(&gainmap_info, &mut pixels, row_bytes, None)
        );
        result.gainmap = Some((gainmap_info, pixels));
    }
    result
}

// Port of: tests/PngGainmapTest.cpp#L71-L117 (chrome/m156), `AndroidCodec_pngGainmapDecode`.
def_test!(AndroidCodec_pngGainmapDecode, |r| {
    let path = "images/gainmap.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let dimensions = ISize::new(32, 32);
    let origin_color = Color::from_argb(0xff, 0xff, 0xff, 0xff);
    let far_corner_color = Color::from_argb(0xff, 0x00, 0x00, 0x00);
    let expected_info = GainmapInfo {
        gainmap_ratio_min: Color4f {
            r: 25.0,
            g: 0.5,
            b: 1.0,
            a: 1.0,
        },
        gainmap_ratio_max: Color4f {
            r: 2.0,
            g: 4.0,
            b: 8.0,
            a: 1.0,
        },
        gainmap_gamma: Color4f {
            r: 0.5,
            g: 1.0,
            b: 2.0,
            a: 1.0,
        },
        epsilon_sdr: Color4f {
            r: 0.01,
            g: 0.001,
            b: 0.0001,
            a: 1.0,
        },
        epsilon_hdr: Color4f {
            r: 0.0001,
            g: 0.001,
            b: 0.01,
            a: 1.0,
        },
        display_ratio_sdr: 2.0,
        display_ratio_hdr: 4.0,
        base_image_type: BaseImageType::Hdr,
        gainmap_type: GainmapType::Default,
        gainmap_math_color_space: None,
    };

    let decoded = decode_all(r, &data);
    reporter_assert!(r, decoded.decoded);
    let Some((gainmap_info, gainmap_pixels)) = decoded.gainmap else {
        reporter_assert!(r, false);
        return;
    };

    // Spot-check the image size and pixels.
    reporter_assert!(r, gainmap_info.dimensions() == dimensions);
    let gainmap_pixmap =
        Pixmap::new_readonly(&gainmap_info, &gainmap_pixels, gainmap_info.min_row_bytes())
            .expect("pixmap");
    reporter_assert!(r, gainmap_pixmap.get_color((0, 0)) == origin_color);
    reporter_assert!(
        r,
        gainmap_pixmap.get_color((dimensions.width - 1, dimensions.height - 1)) == far_corner_color
    );

    // Verify the gainmap rendering parameters.
    expect_approx_eq_info(r, &expected_info, &decoded.info);
});

// Port of: tests/PngGainmapTest.cpp#L119-L138 (chrome/m156), `AndroidCodec_pngGainmapInvalidDecode`.
def_test!(AndroidCodec_pngGainmapInvalidDecode, |r| {
    let paths = [
        "images/gainmap_no_gdat.png",
        "images/gainmap_gdat_no_gmap.png",
    ];
    for path in paths {
        let data = skip_missing_resource!(get_resource_as_data(path), path);
        let decoded = decode_all(r, &data);
        reporter_assert!(r, !decoded.decoded);
    }
});
