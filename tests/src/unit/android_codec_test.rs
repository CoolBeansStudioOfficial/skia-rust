// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/AndroidCodecTest.cpp (chrome/m156), the cases that need the output colour space of
// SkAndroidCodec and the PNG decoder. AndroidCodec_computeSampleSize needs the JPEG, WebP and GIF
// decoders and sampled decoding, and is not ported yet.

#![cfg(test)]

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::codecs;
use skia_rust_core::color_space::{ColorSpace, named_gamut};
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert, skip_missing_resource};

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
