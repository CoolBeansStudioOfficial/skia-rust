// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/AndroidCodecTest.cpp (chrome/m156), the cases that need the output colour space of
// SkAndroidCodec, and AndroidCodec_computeSampleSize over every decoder.

#![cfg(test)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::codecs;
use skia_rust_core::color_space::{ColorSpace, named_gamut};
use skia_rust_core::encoded_image_format::EncodedImageFormat;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::MemoryStream;

use crate::resources::{get_resource_as_data, resource_dir};
use crate::{def_test, errorf, reporter_assert, skip_missing_resource};

// Makes the Android codec for the resource at `path`, or None if it does not decode.
fn android_codec_for(data: &[u8]) -> Option<AndroidCodec<'static>> {
    let codec = codecs::make_codec_from_stream(MemoryStream::make_copy(data)).ok()?;
    AndroidCodec::make_from_codec(codec)
}

// Port of: tests/AndroidCodecTest.cpp#L135-L163 (chrome/m156)
def_test!(AndroidCodec_wide, |r| {
    let path = "images/wide-gamut.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Some(codec) = android_codec_for(&data) else {
        reporter_assert!(r, false, "Failed to create codec from {path}");
        return;
    };

    let info = codec.info();
    let Some(cs) = codec.compute_output_color_space(info.color_type(), None) else {
        reporter_assert!(r, false, "{path} should have a color space");
        return;
    };

    // This image has a gamut that is VERY close to sRGB, so SkColorSpace::MakeRGB snaps to sRGB.
    let expected = ColorSpace::new_srgb();
    reporter_assert!(r, ColorSpace::equals(Some(&cs), Some(&expected)));
});

// Port of: tests/AndroidCodecTest.cpp#L165-L202 (chrome/m156)
def_test!(AndroidCodec_P3, |r| {
    let path = "images/purple-displayprofile.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Some(codec) = android_codec_for(&data) else {
        reporter_assert!(r, false, "Failed to create codec from {path}");
        return;
    };

    let info = codec.info();
    let Some(cs) = codec.compute_output_color_space(info.color_type(), None) else {
        reporter_assert!(r, false, "{path} should have a color space");
        return;
    };

    reporter_assert!(r, !cs.is_srgb());
    reporter_assert!(r, cs.gamma_close_to_srgb());
    // The expected matrix is Skia's, written at its precision.
    #[allow(clippy::excessive_precision)]
    let expected = [
        [0.426_254_272_f32, 0.369_018_555, 0.168_914_795],
        [0.226_013_184, 0.685_974_121, 0.088_012_695_3],
        [0.011_672_973_6, 0.095_092_773_4, 0.718_124_39],
    ];
    reporter_assert!(r, cs.to_xyzd50().vals == expected);
});

// Port of: tests/AndroidCodecTest.cpp#L204-L238 (chrome/m156)
def_test!(AndroidCodec_HLG, |r| {
    let path = "images/red-hlg-profile.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Some(codec) = android_codec_for(&data) else {
        reporter_assert!(r, false, "Failed to create codec from {path}");
        return;
    };

    let info = codec.info();
    let Some(cs) = codec.compute_output_color_space(info.color_type(), None) else {
        reporter_assert!(r, false, "{path} should have a color space");
        return;
    };

    let tf = cs.transfer_fn();
    reporter_assert!(r, tf.is_hlgish() || tf.is_hlg());
    reporter_assert!(r, cs.to_xyzd50().vals == named_gamut::REC2020.vals);
});

// Port of: tests/AndroidCodecTest.cpp#L240-L274 (chrome/m156)
def_test!(AndroidCodec_PQ, |r| {
    let path = "images/red-pq-profile.png";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Some(codec) = android_codec_for(&data) else {
        reporter_assert!(r, false, "Failed to create codec from {path}");
        return;
    };

    let info = codec.info();
    let Some(cs) = codec.compute_output_color_space(info.color_type(), None) else {
        reporter_assert!(r, false, "{path} should have a color space");
        return;
    };

    let tf = cs.transfer_fn();
    reporter_assert!(r, tf.is_pqish() || tf.is_pq());
    reporter_assert!(r, cs.to_xyzd50().vals == named_gamut::REC2020.vals);
});

// Port of: tests/AndroidCodecTest.cpp#L19-L21 (chrome/m156), times
#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    reason = "mirrors the (int) cast of the C++ times(), which truncates toward zero"
)]
fn times(size: ISize, factor: f32) -> ISize {
    ISize::new(
        (size.width as f32 * factor) as i32,
        (size.height as f32 * factor) as i32,
    )
}

// Port of: tests/AndroidCodecTest.cpp#L23-L25 (chrome/m156), plus
fn plus(size: ISize, term: i32) -> ISize {
    ISize::new(size.width + term, size.height + term)
}

// Port of: tests/AndroidCodecTest.cpp#L27-L29 (chrome/m156), invalid
fn invalid(size: ISize) -> bool {
    size.width < 1 || size.height < 1
}

// Port of: tests/AndroidCodecTest.cpp#L41-L133 (chrome/m156)
def_test!(
    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        reason = "the scaled sizes are (int) casts of float products, as in the C++ test"
    )]
    AndroidCodec_computeSampleSize,
    |r| {
        if resource_dir().is_none() {
            return;
        }
        let files = [
            "images/color_wheel.webp",
            "images/ship.png",
            "images/dog.jpg",
            "images/color_wheel.gif",
            "images/rle.bmp",
            "images/google_chrome.ico",
            "images/mandrill.wbmp",
        ];
        for file in files {
            let Some(data) = get_resource_as_data(file) else {
                errorf!(r, "Could not get {}", file);
                continue;
            };
            let Ok(codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
                errorf!(r, "Could not create codec for {}", file);
                continue;
            };
            // SkAndroidCodec::MakeFromCodec(SkCodec::MakeFromData(...))
            let Some(codec) = AndroidCodec::make_from_codec(codec) else {
                errorf!(r, "Could not create codec for {}", file);
                continue;
            };
            let dims = codec.info().dimensions();
            let downscales = [
                plus(dims, -1),
                times(dims, 0.15),
                times(dims, 0.6),
                ISize::new(
                    (dims.width as f32 * 0.25) as i32,
                    (dims.height as f32 * 0.75) as i32,
                ),
                ISize::new(1, 1),
                ISize::new(1, 2),
                ISize::new(2, 1),
                ISize::new(0, -1),
                ISize::new(dims.width, dims.height - 1),
            ];
            for requested in downscales {
                let mut size = requested;
                let computed_sample_size = codec.compute_sample_size(&mut size);
                reporter_assert!(r, size.width >= 1 && size.height >= 1);
                if codec.encoded_format() == EncodedImageFormat::WEBP {
                    // WebP supports arbitrary down-scaling.
                    reporter_assert!(r, size == requested || invalid(requested));
                } else if computed_sample_size == 1 {
                    reporter_assert!(r, size == dims);
                } else {
                    reporter_assert!(r, computed_sample_size > 1);
                    if size.width >= dims.width || size.height >= dims.height {
                        errorf!(
                            r,
                            "File {}'s computed sample size ({}) is bigger than original? original: {} x {}\tsampled: {} x {}",
                            file,
                            computed_sample_size,
                            dims.width,
                            dims.height,
                            size.width,
                            size.height
                        );
                    }
                    reporter_assert!(
                        r,
                        size.width >= requested.width && size.height >= requested.height
                    );
                    reporter_assert!(r, size.width < dims.width && size.height < dims.height);
                }
            }

            let upscales = [dims, plus(dims, 5), times(dims, 2.0)];
            for upscale in upscales {
                let mut size = upscale;
                let computed_sample_size = codec.compute_sample_size(&mut size);
                reporter_assert!(r, computed_sample_size == 1);
                reporter_assert!(r, dims == size);
            }

            // This mimics how Android's ImageDecoder uses SkAndroidCodec. A client can choose their
            // dimensions based on calling getSampledDimensions, but the ImageDecoder API takes an
            // arbitrary size. It then uses computeSampleSize to determine the best dimensions and
            // sampleSize. It should return the same dimensions. the sampleSize may be different due to
            // integer division.
            for sample_size in [1, 2, 3, 4, 8, 16, 32] {
                let sampled_dims = codec.get_sampled_dimensions(sample_size);
                let mut size = sampled_dims;
                let computed_sample_size = codec.compute_sample_size(&mut size);
                if sampled_dims != size {
                    errorf!(
                        r,
                        "File '{}'->getSampledDimensions({}) yields computed sample size of {}\n\tsampledDimensions: {} x {}\tcomputed dimensions: {} x {}",
                        file,
                        sample_size,
                        computed_sample_size,
                        sampled_dims.width,
                        sampled_dims.height,
                        size.width,
                        size.height
                    );
                }
            }
        }
    }
);
