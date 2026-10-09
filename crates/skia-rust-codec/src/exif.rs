// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/private/SkExif.h (Metadata, Parse) and src/codec/SkExif.cpp (chrome/m156), the
// parsing half. `WriteExif` and the Rust-backed parser (`SK_EXIF_PARSE_WITH_RUST`) are not ported.
//
// Also: src/codec/SkParseEncodedOrigin.cpp (chrome/m156), which reads the orientation through
// `SkExif::Parse`.

use std::sync::Arc;

use skia_rust_core::encoded_origin::EncodedOrigin;

use crate::tiff_utility::{self, ImageFileDirectory};

/// Port of `SkExif::kOriginTag`: the orientation (Exif tag 0x112).
// Port of: include/private/SkExif.h#L23 (kOriginTag)
const ORIGIN_TAG: u16 = 0x0112;
/// Port of `SkExif::kResolutionUnitTag`.
// Port of: include/private/SkExif.h#L24 (kResolutionUnitTag)
const RESOLUTION_UNIT_TAG: u16 = 0x0128;
/// Port of `SkExif::kXResolutionTag`.
// Port of: include/private/SkExif.h#L25 (kXResolutionTag)
const X_RESOLUTION_TAG: u16 = 0x011a;
/// Port of `SkExif::kYResolutionTag`.
// Port of: include/private/SkExif.h#L26 (kYResolutionTag)
const Y_RESOLUTION_TAG: u16 = 0x011b;
/// Port of `SkExif::kPixelXDimensionTag`.
// Port of: include/private/SkExif.h#L27 (kPixelXDimensionTag)
const PIXEL_X_DIMENSION_TAG: u16 = 0xa002;
/// Port of `SkExif::kPixelYDimensionTag`.
// Port of: include/private/SkExif.h#L28 (kPixelYDimensionTag)
const PIXEL_Y_DIMENSION_TAG: u16 = 0xa003;
/// Port of `kSubIFDOffsetTag`.
// Port of: src/codec/SkExif.cpp#L28 (kSubIFDOffsetTag)
const SUB_IFD_OFFSET_TAG: u16 = 0x8769;
/// Port of `kMarkerNoteTag`.
// Port of: src/codec/SkExif.cpp#L29 (kMarkerNoteTag)
const MARKER_NOTE_TAG: u16 = 0x927c;

/// The metadata that `SkExif::Parse` reads (`SkExif::Metadata`). Each field is `None` when the
/// data does not contain it.
// Port of: include/private/SkExif.h#L30-L46 (Metadata)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Metadata {
    /// The image orientation.
    pub origin: Option<EncodedOrigin>,
    /// The HDR headroom property (Apple's maker note).
    pub hdr_headroom: Option<f32>,
    /// Resolution unit.
    pub resolution_unit: Option<u16>,
    /// Horizontal resolution.
    pub x_resolution: Option<f32>,
    /// Vertical resolution.
    pub y_resolution: Option<f32>,
    /// Size in pixels.
    pub pixel_x_dimension: Option<u32>,
    /// Size in pixels.
    pub pixel_y_dimension: Option<u32>,
}

/// Port of `SkEncodedOrigin` from its Exif value (1 to 8). `None` for other values.
// Port of: include/codec/SkEncodedOrigin.h (the enum's values), with the range check of
// src/codec/SkExif.cpp#L112 (`0 < value && value <= kLast_SkEncodedOrigin`)
fn origin_from_value(value: u16) -> Option<EncodedOrigin> {
    Some(match value {
        1 => EncodedOrigin::TopLeft,
        2 => EncodedOrigin::TopRight,
        3 => EncodedOrigin::BottomRight,
        4 => EncodedOrigin::BottomLeft,
        5 => EncodedOrigin::LeftTop,
        6 => EncodedOrigin::RightTop,
        7 => EncodedOrigin::RightBottom,
        8 => EncodedOrigin::LeftBottom,
        _ => return None,
    })
}

/// The single value of entry `i` as an unsigned short, if the entry has one.
fn first_u16(ifd: &ImageFileDirectory, i: u16) -> Option<u16> {
    ifd.entry_unsigned_short(i, 1)?.first().copied()
}

/// The single value of entry `i` as an unsigned long, if the entry has one.
fn first_u32(ifd: &ImageFileDirectory, i: u16) -> Option<u32> {
    ifd.entry_unsigned_long(i, 1)?.first().copied()
}

/// The single value of entry `i` as a signed rational, if the entry has one.
fn first_rational(ifd: &ImageFileDirectory, i: u16) -> Option<f32> {
    ifd.entry_signed_rational(i, 1)?.first().copied()
}

/// Port of `get_maker_note_hdr_headroom`: the HDR headroom from Apple's maker note (tags 33 and
/// 48), or `None` when the note does not have the Apple signature.
// Port of: src/codec/SkExif.cpp#L37-L97 (get_maker_note_hdr_headroom)
fn get_maker_note_hdr_headroom(data: &[u8]) -> Option<f32> {
    // No little endian images that specify this data have been observed. Do not add speculative
    // support.
    const LITTLE_ENDIAN: bool = false;
    const SIG: [u8; 14] = [
        b'A', b'p', b'p', b'l', b'e', b' ', b'i', b'O', b'S', 0, 0, 1, b'M', b'M',
    ];
    if data.len() < SIG.len() || data[..SIG.len()] != SIG {
        return None;
    }
    // The signature is 14 bytes, so the directory starts at offset 14.
    let ifd = ImageFileDirectory::make_from_offset(Arc::from(data), LITTLE_ENDIAN, 14, false)?;

    // See documentation at:
    // https://developer.apple.com/documentation/appkit/images_and_pdf/applying_apple_hdr_effect_to_your_photos
    let mut has_maker33 = false;
    let mut has_maker48 = false;
    let mut maker33 = 0.0f32;
    let mut maker48 = 0.0f32;
    for i in 0..ifd.num_entries() {
        match ifd.entry_tag(i) {
            33 if !has_maker33 => {
                if let Some(v) = first_rational(&ifd, i) {
                    maker33 = v;
                    has_maker33 = true;
                }
            }
            48 if !has_maker48 => {
                if let Some(v) = first_rational(&ifd, i) {
                    maker48 = v;
                    has_maker48 = true;
                }
            }
            _ => {}
        }
    }

    // Many images have a maker33 but not a maker48. Treat them as having maker48 of 0.
    if !has_maker33 {
        return None;
    }

    let stops = if maker33 < 1.0 {
        if maker48 <= 0.01 {
            -20.0f32 * maker48 + 1.8f32
        } else {
            -0.101f32 * maker48 + 1.601f32
        }
    } else if maker48 <= 0.01 {
        -70.0f32 * maker48 + 3.0f32
    } else {
        -0.303f32 * maker48 + 2.303f32
    };
    // Port of `std::pow(2.f, std::max(stops, 0.f))`.
    Some(2.0f32.powf(stops.max(0.0)))
}

/// Port of `parse_ifd`: reads the fields of one directory into `exif`. The sub-directory is read
/// only from the root directory.
// Port of: src/codec/SkExif.cpp#L99-L192 (parse_ifd)
fn parse_ifd(
    exif: &mut Metadata,
    data: &Arc<[u8]>,
    ifd: Option<ImageFileDirectory>,
    little_endian: bool,
    is_root: bool,
) {
    let Some(ifd) = ifd else {
        return;
    };
    for i in 0..ifd.num_entries() {
        match ifd.entry_tag(i) {
            ORIGIN_TAG => {
                if exif.origin.is_none()
                    && let Some(value) = first_u16(&ifd, i)
                    && let Some(origin) = origin_from_value(value)
                {
                    exif.origin = Some(origin);
                }
            }
            MARKER_NOTE_TAG if exif.hdr_headroom.is_none() => {
                if let Some(maker_note) = ifd.entry_undefined_data(i) {
                    exif.hdr_headroom = get_maker_note_hdr_headroom(maker_note);
                }
            }
            SUB_IFD_OFFSET_TAG if is_root => {
                if let Some(sub_ifd_offset) = first_u32(&ifd, i) {
                    let sub_ifd = ImageFileDirectory::make_from_offset(
                        Arc::clone(data),
                        little_endian,
                        sub_ifd_offset,
                        true,
                    );
                    parse_ifd(exif, data, sub_ifd, little_endian, false);
                }
            }
            X_RESOLUTION_TAG => {
                if exif.x_resolution.is_none() {
                    exif.x_resolution = ifd
                        .entry_unsigned_rational(i, 1)
                        .and_then(|v| v.first().copied());
                }
            }
            Y_RESOLUTION_TAG => {
                if exif.y_resolution.is_none() {
                    exif.y_resolution = ifd
                        .entry_unsigned_rational(i, 1)
                        .and_then(|v| v.first().copied());
                }
            }
            RESOLUTION_UNIT_TAG => {
                if exif.resolution_unit.is_none() {
                    exif.resolution_unit = ifd
                        .entry_unsigned_short(i, 1)
                        .and_then(|v| v.first().copied());
                }
            }
            PIXEL_X_DIMENSION_TAG => {
                // The type can be unsigned short or unsigned long (Exif 2.3, CIPA DC-008-2012).
                if exif.pixel_x_dimension.is_none() {
                    exif.pixel_x_dimension = ifd
                        .entry_unsigned_short(i, 1)
                        .and_then(|v| v.first().copied())
                        .map(u32::from);
                }
                if exif.pixel_x_dimension.is_none() {
                    exif.pixel_x_dimension = ifd
                        .entry_unsigned_long(i, 1)
                        .and_then(|v| v.first().copied());
                }
            }
            PIXEL_Y_DIMENSION_TAG => {
                if exif.pixel_y_dimension.is_none() {
                    exif.pixel_y_dimension = ifd
                        .entry_unsigned_short(i, 1)
                        .and_then(|v| v.first().copied())
                        .map(u32::from);
                }
                if exif.pixel_y_dimension.is_none() {
                    exif.pixel_y_dimension = ifd
                        .entry_unsigned_long(i, 1)
                        .and_then(|v| v.first().copied());
                }
            }
            _ => {}
        }
    }
}

/// Port of `SkExif::Parse`: reads the metadata in `data` (an Exif TIFF block) into `metadata`.
/// Truncated input is read as far as it goes.
// Port of: src/codec/SkExif.cpp#L194-L215 (Parse, the C++ parser)
pub fn parse(metadata: &mut Metadata, data: Option<&[u8]>) {
    let Some(data) = data else {
        return;
    };
    if let Some((little_endian, ifd_offset)) = tiff_utility::parse_header(data) {
        let data: Arc<[u8]> = Arc::from(data);
        let ifd = ImageFileDirectory::make_from_offset(
            Arc::clone(&data),
            little_endian,
            ifd_offset,
            true,
        );
        parse_ifd(metadata, &data, ifd, little_endian, true);
    }
}

/// Port of `SkParseEncodedOrigin`: the orientation in Exif `data`, if it has one.
// Port of: src/codec/SkParseEncodedOrigin.cpp#L11-L25 (SkParseEncodedOrigin)
#[must_use]
pub fn parse_encoded_origin(data: &[u8]) -> Option<EncodedOrigin> {
    let mut exif = Metadata::default();
    parse(&mut exif, Some(data));
    exif.origin
}
