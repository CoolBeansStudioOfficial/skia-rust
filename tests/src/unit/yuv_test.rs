// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/YUVTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_codec::{Codec, decoders};
use skia_rust_core::color_matrix::ColorMatrix;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::image_info::YUVColorSpace;
use skia_rust_core::scalar::{Scalar, scalar};
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;
use skia_rust_core::yuva_info::{PlaneConfig, Siting, Subsampling, YUVAInfo};
use skia_rust_core::yuva_pixmaps::{SupportedDataTypes, YUVAPixmaps, num_channels_and_data_type};

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert};

// Port of: tests/YUVTest.cpp#L26-L58 (chrome/m156), `codec_yuv`
fn codec_yuv(reporter: &mut Reporter, path: &str, expected_info: Option<&YUVAInfo>) {
    let Some(data) = get_resource_as_data(path) else {
        return;
    };
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(&data), decoders()) else {
        reporter_assert!(reporter, false);
        return;
    };

    // Test queryYUVAInfo()
    // skia-rust: not expressible: `queryYUVAInfo(kAllTypes, nullptr)` must fail, but the Rust
    // API returns the info, so there is no null out-parameter to pass.
    let no_types = SupportedDataTypes::default();
    let all_types = SupportedDataTypes::all();

    // Fails when there is no support for YUVA planes.
    reporter_assert!(reporter, codec.query_yuva_info(&no_types).is_none());

    let yuva_pixmap_info = codec.query_yuva_info(&all_types);
    reporter_assert!(
        reporter,
        expected_info.is_some() == yuva_pixmap_info.is_some()
    );
    let Some(yuva_pixmap_info) = yuva_pixmap_info else {
        return;
    };
    reporter_assert!(
        reporter,
        expected_info.is_some_and(|expected| *expected == *yuva_pixmap_info.yuva_info())
    );

    let num_planes = yuva_pixmap_info.num_planes();
    reporter_assert!(reporter, num_planes <= YUVAInfo::MAX_PLANES);
    for i in 0..num_planes {
        let plane_info = yuva_pixmap_info.plane_info(i).expect("plane");
        let plane_ct = plane_info.color_type();
        reporter_assert!(reporter, !plane_info.is_empty());
        reporter_assert!(reporter, plane_ct != ColorType::Unknown);
        reporter_assert!(
            reporter,
            plane_info.valid_row_bytes(yuva_pixmap_info.row_bytes(i).expect("row bytes"))
        );
        // Currently all planes must share a data type, gettable as SkYUVAPixmapInfo::dataType().
        let (_, plane_data_type) = num_channels_and_data_type(plane_ct);
        reporter_assert!(reporter, plane_data_type == yuva_pixmap_info.data_type());
    }
    for i in num_planes..YUVAInfo::MAX_PLANES {
        let plane_info = yuva_pixmap_info.plane_info(i).expect("plane");
        reporter_assert!(reporter, plane_info.dimensions().is_empty());
        reporter_assert!(reporter, plane_info.color_type() == ColorType::Unknown);
        reporter_assert!(reporter, yuva_pixmap_info.row_bytes(i) == Some(0));
    }

    // Allocate the memory for the YUV decode.
    let Some(mut pixmaps) = YUVAPixmaps::allocate(&yuva_pixmap_info) else {
        reporter_assert!(reporter, false);
        return;
    };
    reporter_assert!(reporter, pixmaps.is_valid());

    for i in 0..YUVAPixmaps::MAX_PLANES {
        reporter_assert!(
            reporter,
            pixmaps.plane(i).info() == yuva_pixmap_info.plane_info(i).expect("plane")
        );
    }
    for i in num_planes..YUVAInfo::MAX_PLANES {
        reporter_assert!(reporter, pixmaps.plane(i).row_bytes() == 0);
    }

    // Test getYUVAPlanes()
    reporter_assert!(
        reporter,
        codec.get_yuva_planes(&mut pixmaps) == skia_rust_codec::codec::Result::Success
    );
}

// Port of: tests/YUVTest.cpp#L98-L142 (chrome/m156)
def_test!(Jpeg_YUV_Codec, |reporter| {
    let set_expectations = |dims: ISize, subsampling: Subsampling| {
        YUVAInfo::new(
            dims,
            PlaneConfig::Y_U_V,
            subsampling,
            YUVColorSpace::JPEG,
            EncodedOrigin::TopLeft,
            (Siting::Centered, Siting::Centered),
        )
    };

    let expectations = set_expectations(ISize::new(128, 128), Subsampling::S420);
    codec_yuv(reporter, "images/color_wheel.jpg", expectations.as_ref());

    // H2V2
    let expectations = set_expectations(ISize::new(512, 512), Subsampling::S420);
    codec_yuv(
        reporter,
        "images/mandrill_512_q075.jpg",
        expectations.as_ref(),
    );

    // H1V1
    let expectations = set_expectations(ISize::new(512, 512), Subsampling::S444);
    codec_yuv(reporter, "images/mandrill_h1v1.jpg", expectations.as_ref());

    // H2V1
    let expectations = set_expectations(ISize::new(512, 512), Subsampling::S422);
    codec_yuv(reporter, "images/mandrill_h2v1.jpg", expectations.as_ref());

    // Non-power of two dimensions
    let expectations = set_expectations(ISize::new(439, 154), Subsampling::S420);
    codec_yuv(
        reporter,
        "images/cropped_mandrill.jpg",
        expectations.as_ref(),
    );

    let expectations = set_expectations(ISize::new(8, 8), Subsampling::S420);
    codec_yuv(reporter, "images/randPixels.jpg", expectations.as_ref());

    // Progressive images
    let expectations = set_expectations(ISize::new(512, 512), Subsampling::S444);
    codec_yuv(
        reporter,
        "images/brickwork-texture.jpg",
        expectations.as_ref(),
    );
    codec_yuv(
        reporter,
        "images/brickwork_normal-map.jpg",
        expectations.as_ref(),
    );

    // A CMYK encoded image should fail.
    codec_yuv(reporter, "images/CMYK.jpg", None);
    // A grayscale encoded image should fail.
    codec_yuv(reporter, "images/grayscale.jpg", None);
    // A PNG should fail.
    codec_yuv(reporter, "images/arrow.png", None);
});

// Port of: tests/YUVTest.cpp#L212-L240 (chrome/m156)
def_test!(YUVMath, |reporter| {
    let spaces = [
        YUVColorSpace::JPEGFull,
        YUVColorSpace::Rec601Limited,
        YUVColorSpace::Rec709Full,
        YUVColorSpace::BT2020_8BitFull,
        YUVColorSpace::Identity,
    ];

    // Not sure what the theoretical precision we can hope for is, so pick a big value that
    // passes (when I think we're correct).
    // 1.0f/(1 << 18), exact in f32
    let tolerance: f32 = 1.0 / 262_144.0;

    for cs in spaces {
        let mut r2ym = ColorMatrix::rgb_to_yuv(cs);
        let y2rm = ColorMatrix::yuv_to_rgb(cs);
        r2ym.post_concat(&y2rm);

        let mut tmp = [0.0f32; 20];
        r2ym.get_row_major(&mut tmp);
        for (i, &value) in tmp.iter().enumerate() {
            // diagonal
            let expected = if i % 6 == 0 { 1.0 } else { 0.0 };
            reporter_assert!(reporter, scalar::nearly_equal(value, expected, tolerance));
        }
    }
});

// Port of: tests/YUVTest.cpp#L144-L157 (chrome/m156), `decode_yuva`
fn decode_yuva(reporter: &mut Reporter, data: &[u8]) -> YUVAPixmaps {
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(data), decoders()) else {
        reporter_assert!(reporter, false);
        return YUVAPixmaps::default();
    };
    let all_types = SupportedDataTypes::all();
    let Some(yuva_pixmap_info) = codec.query_yuva_info(&all_types) else {
        reporter_assert!(reporter, false);
        return YUVAPixmaps::default();
    };
    let Some(mut result) = YUVAPixmaps::allocate(&yuva_pixmap_info) else {
        reporter_assert!(reporter, false);
        return YUVAPixmaps::default();
    };
    reporter_assert!(
        reporter,
        codec.get_yuva_planes(&mut result) == skia_rust_codec::codec::Result::Success
    );
    result
}

// Port of: tests/YUVTest.cpp#L159-L179 (chrome/m156), `verify_same`
fn verify_same(reporter: &mut Reporter, a: &YUVAPixmaps, b: &YUVAPixmaps) {
    reporter_assert!(reporter, a.yuva_info() == b.yuva_info());
    reporter_assert!(reporter, a.num_planes() == b.num_planes());
    for plane in 0..a.num_planes() {
        let a_plane = a.plane(plane);
        let b_plane = b.plane(plane);
        reporter_assert!(
            reporter,
            a_plane.compute_byte_size() == b_plane.compute_byte_size()
        );
        let a_bytes = a_plane.addr().unwrap_or(&[]);
        let b_bytes = b_plane.addr().unwrap_or(&[]);
        let bytes_per_pixel = a_plane.info().bytes_per_pixel();
        let width_bytes = usize::try_from(a_plane.info().width()).unwrap_or(0) * bytes_per_pixel;
        for row in 0..usize::try_from(a_plane.info().height()).unwrap_or(0) {
            for col in 0..width_bytes {
                let a_byte = i32::from(a_bytes[row * a_plane.row_bytes() + col]);
                let b_byte = i32::from(b_bytes[row * b_plane.row_bytes() + col]);
                // Allow at most one bit of difference.
                reporter_assert!(reporter, (a_byte - b_byte).abs() <= 1);
            }
        }
    }
}

// Port of: tests/YUVTest.cpp#L181-L208 (chrome/m156)
def_test!(Jpeg_YUV_Encode, |reporter| {
    let paths = [
        "images/color_wheel.jpg",
        "images/mandrill_512_q075.jpg",
        "images/mandrill_h1v1.jpg",
        "images/mandrill_h2v1.jpg",
        "images/cropped_mandrill.jpg",
        "images/randPixels.jpg",
    ];
    for path in paths {
        // A missing resource makes the decode fail, as GetResourceAsStream's null does in Skia.
        let data = get_resource_as_data(path).unwrap_or_default();
        let decoded = decode_yuva(reporter, &data);

        let mut encoded = Vec::new();
        reporter_assert!(
            reporter,
            skia_rust_codec::encode::jpeg_encoder::encode_yuva(
                &decoded,
                None,
                &mut encoded,
                &skia_rust_codec::encode::jpeg_encoder::Options::default(),
            )
        );
        let roundtrip = decode_yuva(reporter, &encoded);
        verify_same(reporter, &decoded, &roundtrip);
    }
});
