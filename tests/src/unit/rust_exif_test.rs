// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/RustExifTest.cpp (chrome/m156), the Rust EXIF parser's tests. The byte builders are
// the file's own helpers. The equivalence test compares the Rust parser with the `SkExif::Parse`
// port, on the EXIF resource files that ExifTest.cpp uses.

#![cfg(test)]
// The byte builders truncate each field on purpose, as the C++ helpers do.
#![allow(clippy::cast_possible_truncation)]

use skia_rust_codec::exif;
use skia_rust_codec::rust_exif::{ExifMetadata, parse_exif, to_sk_exif_metadata};
use skia_rust_core::encoded_origin::EncodedOrigin;

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_test, reporter_assert};

// Helpers: build minimal EXIF blobs (big- and little-endian) for unit tests.

// Port of: tests/RustExifTest.cpp#L20-L23 (write_u16_be)
fn write_u16_be(buf: &mut [u8], off: usize, v: u16) {
    buf[off] = (v >> 8) as u8;
    buf[off + 1] = v as u8;
}

// Port of: tests/RustExifTest.cpp#L26-L31 (write_u32_be)
fn write_u32_be(buf: &mut [u8], off: usize, v: u32) {
    buf[off] = (v >> 24) as u8;
    buf[off + 1] = (v >> 16) as u8;
    buf[off + 2] = (v >> 8) as u8;
    buf[off + 3] = v as u8;
}

// Port of: tests/RustExifTest.cpp#L34-L37 (write_u16_le)
fn write_u16_le(buf: &mut [u8], off: usize, v: u16) {
    buf[off] = v as u8;
    buf[off + 1] = (v >> 8) as u8;
}

// Port of: tests/RustExifTest.cpp#L40-L45 (write_u32_le)
fn write_u32_le(buf: &mut [u8], off: usize, v: u32) {
    buf[off] = v as u8;
    buf[off + 1] = (v >> 8) as u8;
    buf[off + 2] = (v >> 16) as u8;
    buf[off + 3] = (v >> 24) as u8;
}

// Port of: tests/RustExifTest.cpp#L48-L54 (tiff_header_be)
fn tiff_header_be(ifd_offset: u32) -> [u8; 8] {
    [
        b'M',
        b'M',
        0x00,
        0x2a,
        (ifd_offset >> 24) as u8,
        (ifd_offset >> 16) as u8,
        (ifd_offset >> 8) as u8,
        ifd_offset as u8,
    ]
}

// Port of: tests/RustExifTest.cpp#L57-L63 (tiff_header_le)
fn tiff_header_le(ifd_offset: u32) -> [u8; 8] {
    [
        b'I',
        b'I',
        0x2a,
        0x00,
        ifd_offset as u8,
        (ifd_offset >> 8) as u8,
        (ifd_offset >> 16) as u8,
        (ifd_offset >> 24) as u8,
    ]
}

// Port of: tests/RustExifTest.cpp#L66-L77 (make_orientation_exif_le)
fn make_orientation_exif_le(orientation: u16) -> Vec<u8> {
    let mut buf = vec![0u8; 26];
    buf[..8].copy_from_slice(&tiff_header_le(8));
    write_u16_le(&mut buf, 8, 1); // IFD entry count = 1
    write_u16_le(&mut buf, 10, 0x0112); // tag: Orientation
    write_u16_le(&mut buf, 12, 3); // type: SHORT
    write_u32_le(&mut buf, 14, 1); // count
    write_u16_le(&mut buf, 18, orientation); // value
    write_u32_le(&mut buf, 22, 0); // next IFD offset
    buf
}

// Port of: tests/RustExifTest.cpp#L80-L96 (make_orientation_exif_be)
fn make_orientation_exif_be(orientation: u16) -> Vec<u8> {
    // Header (8) + entry count (2) + 1 entry (12) + next IFD offset (4) = 26 bytes.
    let mut buf = vec![0u8; 26];
    buf[..8].copy_from_slice(&tiff_header_be(8));
    write_u16_be(&mut buf, 8, 1);
    write_u16_be(&mut buf, 10, 0x0112);
    write_u16_be(&mut buf, 12, 3);
    write_u32_be(&mut buf, 14, 1);
    write_u16_be(&mut buf, 18, orientation);
    write_u32_be(&mut buf, 22, 0);
    buf
}

// Port of: tests/RustExifTest.cpp#L99-L141 (make_resolution_exif_be)
fn make_resolution_exif_be(xres: f32, yres: f32, unit: u16) -> Vec<u8> {
    // The RATIONAL values (8 bytes each) are stored after the IFD: header 8, count 2, entries
    // 3 x 12, next IFD 4, so the data is at offset 52. XResolution at 52, YResolution at 60.
    const XRES_OFF: u32 = 52;
    const YRES_OFF: u32 = 60;
    let mut buf = vec![0u8; 68];
    buf[..8].copy_from_slice(&tiff_header_be(8));
    write_u16_be(&mut buf, 8, 3);
    // Entry 0: ResolutionUnit (tag 0x0128, SHORT, 1, unit).
    write_u16_be(&mut buf, 10, 0x0128);
    write_u16_be(&mut buf, 12, 3);
    write_u32_be(&mut buf, 14, 1);
    write_u16_be(&mut buf, 18, unit);
    // Entry 1: XResolution (tag 0x011a, RATIONAL, 1, offset).
    write_u16_be(&mut buf, 22, 0x011a);
    write_u16_be(&mut buf, 24, 5);
    write_u32_be(&mut buf, 26, 1);
    write_u32_be(&mut buf, 30, XRES_OFF);
    // Entry 2: YResolution (tag 0x011b, RATIONAL, 1, offset).
    write_u16_be(&mut buf, 34, 0x011b);
    write_u16_be(&mut buf, 36, 5);
    write_u32_be(&mut buf, 38, 1);
    write_u32_be(&mut buf, 42, YRES_OFF);
    write_u32_be(&mut buf, 46, 0);
    // The resolutions as rationals with denominator 1000.
    // Port of the C++ `static_cast<uint32_t>(xres * 1000.0f + 0.5f)`.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors the C++ cast
    let xnum = (xres * 1000.0 + 0.5) as u32;
    write_u32_be(&mut buf, XRES_OFF as usize, xnum);
    write_u32_be(&mut buf, XRES_OFF as usize + 4, 1000);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // mirrors the C++ cast
    let ynum = (yres * 1000.0 + 0.5) as u32;
    write_u32_be(&mut buf, YRES_OFF as usize, ynum);
    write_u32_be(&mut buf, YRES_OFF as usize + 4, 1000);
    buf
}

// Port of: tests/RustExifTest.cpp#L229-L252 (make_pixel_dimensions_exif_be)
fn make_pixel_dimensions_exif_be(width: u16, height: u16) -> Vec<u8> {
    // Header 8, count 2, entries 2 x 12, next IFD 4: 38 bytes.
    let mut buf = vec![0u8; 38];
    buf[..8].copy_from_slice(&tiff_header_be(8));
    write_u16_be(&mut buf, 8, 2);
    // Entry 0: PixelXDimension (tag 0xa002, SHORT, 1, width).
    write_u16_be(&mut buf, 10, 0xa002);
    write_u16_be(&mut buf, 12, 3);
    write_u32_be(&mut buf, 14, 1);
    write_u16_be(&mut buf, 18, width);
    // Entry 1: PixelYDimension (tag 0xa003, SHORT, 1, height).
    write_u16_be(&mut buf, 22, 0xa003);
    write_u16_be(&mut buf, 24, 3);
    write_u32_be(&mut buf, 26, 1);
    write_u16_be(&mut buf, 30, height);
    write_u32_be(&mut buf, 34, 0);
    buf
}

// Port of: tests/RustExifTest.cpp#L145-L149 (RustExif_empty_data_returns_false, chrome/m156)
def_test!(RustExif_empty_data_returns_false, |r| {
    let mut meta = ExifMetadata::default();
    let ok = parse_exif(&[], &mut meta);
    reporter_assert!(r, !ok);
});

// Port of: tests/RustExifTest.cpp#L151-L157 (RustExif_invalid_data_returns_false, chrome/m156)
def_test!(RustExif_invalid_data_returns_false, |r| {
    let mut meta = ExifMetadata::default();
    let garbage = b"not exif";
    let ok = parse_exif(garbage, &mut meta);
    reporter_assert!(r, !ok);
});

// Port of: tests/RustExifTest.cpp#L159-L175 (RustExif_orientation_parsing, chrome/m156)
def_test!(RustExif_orientation_parsing, |r| {
    for orient in 1u16..=8 {
        let blob = make_orientation_exif_be(orient);
        let mut meta = ExifMetadata::default();
        let ok = parse_exif(&blob, &mut meta);
        reporter_assert!(r, ok);
        reporter_assert!(r, meta.origin.is_some());
        reporter_assert!(r, meta.origin == Some(u32::from(orient)));
        // Verify that to_sk_exif_metadata maps it to the right EncodedOrigin.
        let mut sk_meta = exif::Metadata::default();
        to_sk_exif_metadata(&meta, &mut sk_meta);
        reporter_assert!(r, sk_meta.origin.is_some());
        reporter_assert!(r, sk_meta.origin == Some(origin_of(orient)));
    }
});

/// The `SkEncodedOrigin` with the given EXIF orientation value (1 to 8).
fn origin_of(orient: u16) -> EncodedOrigin {
    match orient {
        1 => EncodedOrigin::TopLeft,
        2 => EncodedOrigin::TopRight,
        3 => EncodedOrigin::BottomRight,
        4 => EncodedOrigin::BottomLeft,
        5 => EncodedOrigin::LeftTop,
        6 => EncodedOrigin::RightTop,
        7 => EncodedOrigin::RightBottom,
        8 => EncodedOrigin::LeftBottom,
        _ => panic!("orientation {orient} is not an origin"),
    }
}

// Port of: tests/RustExifTest.cpp#L177-L188 (RustExif_orientation_out_of_range_ignored,
// chrome/m156)
def_test!(RustExif_orientation_out_of_range_ignored, |r| {
    // Orientation value 0 is invalid (the EXIF spec says 1 to 8).
    let blob = make_orientation_exif_be(0);
    let mut meta = ExifMetadata::default();
    parse_exif(&blob, &mut meta);
    reporter_assert!(r, meta.origin.is_none());

    // Orientation value 9 is out of range.
    let blob = make_orientation_exif_be(9);
    parse_exif(&blob, &mut meta);
    reporter_assert!(r, meta.origin.is_none());
});

// Port of: tests/RustExifTest.cpp#L190-L202 (RustExif_little_endian_parsing, chrome/m156)
def_test!(RustExif_little_endian_parsing, |r| {
    // EXIF supports both big-endian ('M','M') and little-endian ('I','I') byte order.
    for orient in 1u16..=8 {
        let blob = make_orientation_exif_le(orient);
        let mut meta = ExifMetadata::default();
        let ok = parse_exif(&blob, &mut meta);
        reporter_assert!(r, ok);
        reporter_assert!(r, meta.origin.is_some());
        reporter_assert!(r, meta.origin == Some(u32::from(orient)));
    }
});

// Port of: tests/RustExifTest.cpp#L204-L227 (RustExif_resolution_parsing, chrome/m156)
def_test!(RustExif_resolution_parsing, |r| {
    const EPSILON: f32 = 0.0001;
    let blob = make_resolution_exif_be(72.0, 72.0, 2 /* inch */);
    let mut meta = ExifMetadata::default();
    let ok = parse_exif(&blob, &mut meta);
    reporter_assert!(r, ok);
    reporter_assert!(r, meta.resolution_unit == Some(2));
    reporter_assert!(
        r,
        meta.x_resolution
            .is_some_and(|x| (x - 72.0).abs() < EPSILON)
    );
    reporter_assert!(
        r,
        meta.y_resolution
            .is_some_and(|y| (y - 72.0).abs() < EPSILON)
    );

    // Verify the to_sk_exif_metadata conversion.
    let mut sk_meta = exif::Metadata::default();
    to_sk_exif_metadata(&meta, &mut sk_meta);
    reporter_assert!(r, sk_meta.resolution_unit == Some(2));
    reporter_assert!(
        r,
        sk_meta
            .x_resolution
            .is_some_and(|x| (x - 72.0).abs() < EPSILON)
    );
    reporter_assert!(
        r,
        sk_meta
            .y_resolution
            .is_some_and(|y| (y - 72.0).abs() < EPSILON)
    );
});

// Port of: tests/RustExifTest.cpp#L254-L271 (RustExif_pixel_dimensions_parsing, chrome/m156)
def_test!(RustExif_pixel_dimensions_parsing, |r| {
    let blob = make_pixel_dimensions_exif_be(1920, 1080);
    let mut meta = ExifMetadata::default();
    let ok = parse_exif(&blob, &mut meta);
    reporter_assert!(r, ok);
    reporter_assert!(r, meta.pixel_x_dimension == Some(1920));
    reporter_assert!(r, meta.pixel_y_dimension == Some(1080));

    let mut sk_meta = exif::Metadata::default();
    to_sk_exif_metadata(&meta, &mut sk_meta);
    reporter_assert!(r, sk_meta.pixel_x_dimension == Some(1920));
    reporter_assert!(r, sk_meta.pixel_y_dimension == Some(1080));
});

/// Port of `approx_eq_f`.
// Port of: tests/RustExifTest.cpp#L275-L277 (approx_eq_f)
fn approx_eq_f(a: f32, b: f32, eps: f32) -> bool {
    (a - b).abs() < eps
}

/// Compares a Rust-parsed `ExifMetadata` (converted to `SkExif::Metadata`) against the reference
/// `SkExif::Parse` result for the given resource EXIF file.
// Port of: tests/RustExifTest.cpp#L281-L405 (check_equivalence, chrome/m156)
fn check_equivalence(r: &mut Reporter, resource_path: &str) {
    const EPSILON: f32 = 0.0001;

    let Some(data) = get_resource_as_data(resource_path) else {
        // The resource is not available: skip, as the C++ test does.
        return;
    };

    // The C++ reference parse.
    let mut cpp_meta = exif::Metadata::default();
    exif::parse(&mut cpp_meta, Some(&data));

    // The Rust parse, and its conversion.
    let mut rust_raw = ExifMetadata::default();
    let rust_ok = parse_exif(&data, &mut rust_raw);
    let mut rust_meta = exif::Metadata::default();
    if rust_ok {
        to_sk_exif_metadata(&rust_raw, &mut rust_meta);
    }

    reporter_assert!(
        r,
        cpp_meta.origin.is_some() == rust_meta.origin.is_some(),
        "[{resource_path}] fOrigin presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.origin, rust_meta.origin) {
        reporter_assert!(
            r,
            cpp == rust,
            "[{resource_path}] fOrigin value mismatch: cpp={cpp:?} rust={rust:?}"
        );
    }

    reporter_assert!(
        r,
        cpp_meta.hdr_headroom.is_some() == rust_meta.hdr_headroom.is_some(),
        "[{resource_path}] fHdrHeadroom presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.hdr_headroom, rust_meta.hdr_headroom) {
        reporter_assert!(
            r,
            approx_eq_f(cpp, rust, EPSILON),
            "[{resource_path}] fHdrHeadroom value mismatch: cpp={cpp} rust={rust}"
        );
    }

    reporter_assert!(
        r,
        cpp_meta.resolution_unit.is_some() == rust_meta.resolution_unit.is_some(),
        "[{resource_path}] fResolutionUnit presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.resolution_unit, rust_meta.resolution_unit) {
        reporter_assert!(
            r,
            cpp == rust,
            "[{resource_path}] fResolutionUnit value mismatch: cpp={cpp} rust={rust}"
        );
    }

    reporter_assert!(
        r,
        cpp_meta.x_resolution.is_some() == rust_meta.x_resolution.is_some(),
        "[{resource_path}] fXResolution presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.x_resolution, rust_meta.x_resolution) {
        reporter_assert!(
            r,
            approx_eq_f(cpp, rust, EPSILON),
            "[{resource_path}] fXResolution value mismatch: cpp={cpp} rust={rust}"
        );
    }

    reporter_assert!(
        r,
        cpp_meta.y_resolution.is_some() == rust_meta.y_resolution.is_some(),
        "[{resource_path}] fYResolution presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.y_resolution, rust_meta.y_resolution) {
        reporter_assert!(
            r,
            approx_eq_f(cpp, rust, EPSILON),
            "[{resource_path}] fYResolution value mismatch: cpp={cpp} rust={rust}"
        );
    }

    reporter_assert!(
        r,
        cpp_meta.pixel_x_dimension.is_some() == rust_meta.pixel_x_dimension.is_some(),
        "[{resource_path}] fPixelXDimension presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.pixel_x_dimension, rust_meta.pixel_x_dimension) {
        reporter_assert!(
            r,
            cpp == rust,
            "[{resource_path}] fPixelXDimension value mismatch: cpp={cpp} rust={rust}"
        );
    }

    reporter_assert!(
        r,
        cpp_meta.pixel_y_dimension.is_some() == rust_meta.pixel_y_dimension.is_some(),
        "[{resource_path}] fPixelYDimension presence mismatch"
    );
    if let (Some(cpp), Some(rust)) = (cpp_meta.pixel_y_dimension, rust_meta.pixel_y_dimension) {
        reporter_assert!(
            r,
            cpp == rust,
            "[{resource_path}] fPixelYDimension value mismatch: cpp={cpp} rust={rust}"
        );
    }
}

// Port of: tests/RustExifTest.cpp#L407-L413 (RustExif_equivalence_with_resource_files,
// chrome/m156)
def_test!(RustExif_equivalence_with_resource_files, |r| {
    // The same EXIF files that ExifTest.cpp uses.
    check_equivalence(r, "images/test0-hdr.exif");
    check_equivalence(r, "images/test1-pixel32.exif");
    check_equivalence(r, "images/test2-nonuniform.exif");
    check_equivalence(r, "images/test3-little-endian.exif");
});
