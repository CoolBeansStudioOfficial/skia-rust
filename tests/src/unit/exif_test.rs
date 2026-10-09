// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
#![cfg(test)]

// Port of: tests/ExifTest.cpp (chrome/m156), the cases for the JPEG orientation and the EXIF parser
// (ExifOrientation, ExifOrientationInExif, ExifOrientationInSubIFD, ExifParse, ExifTruncate).
// GetImageRespectsExif needs the WebP decoder, and ExifWrite* needs SkExif's WriteExif, which is
// not ported.

use skia_rust_codec::codecs;
use skia_rust_codec::exif::{Metadata, parse};
use skia_rust_core::encoded_origin::EncodedOrigin;
use skia_rust_core::stream::MemoryStream;

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert, skip_missing_resource};

fn approx_eq(x: f32, y: f32, epsilon: f32) -> bool {
    (x - y).abs() < epsilon
}

// Port of: tests/ExifTest.cpp#L24-L40 (chrome/m156)
def_test!(ExifOrientation, |r| {
    let path = "images/exif-orientation-2-ur.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let origin = codec.origin();
    reporter_assert!(r, origin == EncodedOrigin::TopRight);

    let path = "images/mandrill_512_q075.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let origin = codec.origin();
    reporter_assert!(r, origin == EncodedOrigin::TopLeft);
});

// Port of: tests/ExifTest.cpp#L65-L72 (chrome/m156)
def_test!(ExifOrientationInExif, |r| {
    let path = "images/orientation/exif.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let origin = codec.origin();
    reporter_assert!(r, origin == EncodedOrigin::LeftBottom);
});

// Port of: tests/ExifTest.cpp#L74-L82 (chrome/m156)
def_test!(ExifOrientationInSubIFD, |r| {
    let path = "images/orientation/subifd.jpg";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    let Ok(codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&data)) else {
        reporter_assert!(r, false);
        return;
    };
    let origin = codec.origin();
    reporter_assert!(r, origin == EncodedOrigin::LeftBottom);
});

// Port of: tests/ExifTest.cpp#L85-L191 (chrome/m156)
def_test!(ExifParse, |r| {
    let k_epsilon = 0.001f32;
    {
        let path = "images/test0-hdr.exif";
        let data = skip_missing_resource!(get_resource_as_data(path), path);
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data));
        reporter_assert!(r, exif.hdr_headroom.is_some());
        reporter_assert!(
            r,
            approx_eq(exif.hdr_headroom.unwrap_or(0.0), 3.755_296, k_epsilon)
        );
        reporter_assert!(r, exif.resolution_unit.is_some_and(|v| v != 0));
        reporter_assert!(r, exif.resolution_unit == Some(2));
        reporter_assert!(r, exif.x_resolution == Some(72.0));
        reporter_assert!(r, exif.y_resolution == Some(72.0));
        reporter_assert!(r, exif.pixel_x_dimension == Some(4032));
        reporter_assert!(r, exif.pixel_y_dimension == Some(3024));
    }
    {
        let path = "images/test1-pixel32.exif";
        let data = skip_missing_resource!(get_resource_as_data(path), path);
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data));
        reporter_assert!(r, exif.hdr_headroom.is_none());
        reporter_assert!(r, exif.resolution_unit.is_some_and(|v| v != 0));
        reporter_assert!(r, exif.resolution_unit == Some(2));
        reporter_assert!(r, exif.x_resolution == Some(72.0));
        reporter_assert!(r, exif.y_resolution == Some(72.0));
        reporter_assert!(r, exif.pixel_x_dimension == Some(200));
        reporter_assert!(r, exif.pixel_y_dimension == Some(100));
    }
    {
        let path = "images/test2-nonuniform.exif";
        let data = skip_missing_resource!(get_resource_as_data(path), path);
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data));
        reporter_assert!(r, exif.hdr_headroom.is_none());
        reporter_assert!(r, exif.resolution_unit.is_some_and(|v| v != 0));
        reporter_assert!(r, exif.resolution_unit == Some(2));
        reporter_assert!(r, exif.x_resolution == Some(144.0));
        reporter_assert!(r, exif.y_resolution == Some(36.0));
        reporter_assert!(r, exif.pixel_x_dimension == Some(50));
        reporter_assert!(r, exif.pixel_y_dimension == Some(100));
    }
    {
        let path = "images/test3-little-endian.exif";
        let data = skip_missing_resource!(get_resource_as_data(path), path);
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data));
        reporter_assert!(r, exif.hdr_headroom.is_none());
        reporter_assert!(r, exif.resolution_unit.is_some_and(|v| v != 0));
        reporter_assert!(r, exif.resolution_unit == Some(2));
        reporter_assert!(r, exif.x_resolution == Some(350.0));
        reporter_assert!(r, exif.y_resolution == Some(350.0));
        reporter_assert!(r, exif.pixel_x_dimension.is_none());
        reporter_assert!(r, exif.pixel_y_dimension.is_none());
    }
    {
        let path = "images/test0-hdr.exif";
        let mut data = skip_missing_resource!(get_resource_as_data(path), path);
        // Zero out the denominators of signed and unsigned rationals, to verify that we do not
        // divide by zero.
        data[186..190].fill(0);
        data[2171..2175].fill(0);
        data[2240..2244].fill(0);
        // Parse the corrupted Exif.
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data));
        // HDR headroom signed denominators are destroyed.
        reporter_assert!(r, exif.hdr_headroom.is_some());
        reporter_assert!(
            r,
            approx_eq(exif.hdr_headroom.unwrap_or(0.0), 3.482_202, k_epsilon)
        );
        // The X resolution should be zero.
        reporter_assert!(r, exif.x_resolution == Some(0.0));
        reporter_assert!(r, exif.y_resolution == Some(72.0));
    }
});

// Port of: tests/ExifTest.cpp#L193-L229 (chrome/m156)
def_test!(ExifTruncate, |r| {
    let path = "images/test0-hdr.exif";
    let data = skip_missing_resource!(get_resource_as_data(path), path);
    // At 545 bytes, we do not have either value yet.
    {
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data[..545]));
        reporter_assert!(r, exif.pixel_x_dimension.is_none());
        reporter_assert!(r, exif.pixel_y_dimension.is_none());
    }
    // At 546 bytes, we have one.
    {
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data[..546]));
        reporter_assert!(r, exif.pixel_x_dimension.is_some());
        reporter_assert!(r, exif.pixel_y_dimension.is_none());
    }
    // At 558 bytes (12 bytes later, one tag), we have both.
    {
        let mut exif = Metadata::default();
        parse(&mut exif, Some(&data[..558]));
        reporter_assert!(r, exif.pixel_x_dimension.is_some());
        reporter_assert!(r, exif.pixel_y_dimension.is_some());
    }
});
