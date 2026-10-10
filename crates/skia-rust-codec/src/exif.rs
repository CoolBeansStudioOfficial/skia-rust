// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: include/private/SkExif.h (Metadata, Parse, WriteExif) and src/codec/SkExif.cpp
// (chrome/m156). The Rust-backed parser (`SK_EXIF_PARSE_WITH_RUST`) is not ported.
//
// Also: src/codec/SkParseEncodedOrigin.cpp (chrome/m156), which reads the orientation through
// `SkExif::Parse`.

use std::sync::Arc;

use skia_rust_core::data::Data;
use skia_rust_core::encoded_origin::EncodedOrigin;

use crate::tiff_utility::{
    self, ImageFileDirectory, TYPE_UNSIGNED_LONG, TYPE_UNSIGNED_RATIONAL, TYPE_UNSIGNED_SHORT,
};

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

/// Port of `SkTiff::kEndianSize` for the big-endian header: `MM`, then 42 (`MM\0*`).
// Port of: src/codec/SkTiffUtility.h#L22 (kEndianBig)
const ENDIAN_BIG: [u8; 4] = [b'M', b'M', 0, 42];
/// Port of `SkTiff::kSizeShort`.
// Port of: src/codec/SkTiffUtility.h#L40 (kSizeShort)
const SIZE_SHORT: u32 = 2;
/// Port of `SkTiff::kSizeLong`.
// Port of: src/codec/SkTiffUtility.h#L41 (kSizeLong)
const SIZE_LONG: u32 = 4;
/// Port of `SkTiff::kSizeEntry`.
// Port of: src/codec/SkTiffUtility.h#L39 (kSizeEntry)
const SIZE_ENTRY: u32 = 12;
/// Port of `kOffset` in `WriteExif`: where IFD0 starts, just after the header.
// Port of: src/codec/SkExif.cpp#L292 (kOffset)
const OFFSET: u32 = 8;

// Port of: src/codec/SkExif.cpp#L203-L256 (chrome/m156), write_entry: writes one IFD entry to
// `stream`, and the values that do not fit in four bytes to `buffer`, which goes after the IFDs.
// Returns false for a tag it cannot write.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature, which takes the same state
fn write_entry(
    tag: u16,
    entry_type: u16,
    count: u32,
    value: u32,
    end_of_data: &mut u32,
    stream: &mut Vec<u8>,
    buffer: &mut Vec<u8>,
) -> bool {
    stream.extend_from_slice(&tag.to_be_bytes());
    stream.extend_from_slice(&entry_type.to_be_bytes());
    stream.extend_from_slice(&count.to_be_bytes());
    match tag {
        ORIGIN_TAG | RESOLUTION_UNIT_TAG => {
            // The value is a short, so it is stored in the first two bytes of the four.
            // The C++ passes a value that fits in a short here.
            #[allow(clippy::cast_possible_truncation)]
            let short = value as u16;
            stream.extend_from_slice(&short.to_be_bytes());
            stream.extend_from_slice(&0u16.to_be_bytes()); // Complete the IFD entry.
            true
        }
        PIXEL_X_DIMENSION_TAG | PIXEL_Y_DIMENSION_TAG => {
            stream.extend_from_slice(&value.to_be_bytes());
            true
        }
        X_RESOLUTION_TAG | Y_RESOLUTION_TAG => {
            // A rational is eight bytes, so the entry holds the offset of the value at the end
            // of the data, and the numerator and the denominator go to `buffer`.
            stream.extend_from_slice(&end_of_data.to_be_bytes());
            *end_of_data += 8;
            buffer.extend_from_slice(&value.to_be_bytes()); // Numerator
            buffer.extend_from_slice(&1u32.to_be_bytes()); // Denominator
            true
        }
        SUB_IFD_OFFSET_TAG => {
            // This does not write the subIFD itself, just the IFD0 entry that points to where it
            // is located.
            stream.extend_from_slice(&value.to_be_bytes());
            true
        }
        _ => false,
    }
}

/// Port of `SkExif::WriteExif`: the Exif data (a TIFF header, IFD0 and the Exif sub-IFD) for the
/// metadata. Returns `None` when the metadata has an HDR headroom, since its maker note cannot be
/// written, and when an entry cannot be written.
// Port of: src/codec/SkExif.cpp#L258-L389 (chrome/m156), WriteExif
#[must_use]
#[doc(alias = "WriteExif")]
// One function, as the C++ is: the IFD layout is written in order and the offsets depend on it.
#[allow(clippy::too_many_lines)]
pub fn write_exif(metadata: &Metadata) -> Option<Data> {
    // Cannot write an IFD entry for MakerNote from the HDR Headroom. Information about maker48 and
    // maker33 is lost in encode.
    if metadata.hdr_headroom.is_some() {
        return None;
    }

    let mut stream: Vec<u8> = Vec::new();
    // This buffer will hold the values that are more than 4 bytes and will be appended to the end
    // of the data after going through all available fields.
    let mut buffer_for_larger_values: Vec<u8> = Vec::new();

    // Write the IFD header.
    stream.extend_from_slice(&ENDIAN_BIG);
    // Offset of index IFD.
    stream.extend_from_slice(&OFFSET.to_be_bytes());

    // Count the number of valid metadata entries.
    let mut num_tags: u16 = 0;
    let mut num_sub_ifd_tags: u16 = 0;
    if metadata.origin.is_some() {
        num_tags += 1;
    }
    if metadata.resolution_unit.is_some() {
        num_tags += 1;
    }
    if metadata.x_resolution.is_some() {
        num_tags += 1;
    }
    if metadata.y_resolution.is_some() {
        num_tags += 1;
    }
    if metadata.pixel_x_dimension.is_some() {
        num_sub_ifd_tags += 1;
    }
    if metadata.pixel_y_dimension.is_some() {
        num_sub_ifd_tags += 1;
    }
    // If there exists metadata that belongs in a subIFD, we will write that to a separate stream
    // and append it to the end of the data, before |bufferForLargerValues|.
    let sub_ifd_exists = num_sub_ifd_tags > 0;
    if sub_ifd_exists {
        num_tags += 1;
    }

    // Offset that represents where data will be appended.
    let mut end_of_data = OFFSET
        + SIZE_SHORT          // Number of tags
        + SIZE_ENTRY * u32::from(num_tags) // Entries
        + SIZE_LONG; // Next IFD offset
    // Offset that represents where the subIFD will start if it exists.
    let sub_ifd_offset = end_of_data;
    if sub_ifd_exists {
        end_of_data += SIZE_SHORT                         // Number of subIFD tags
            + SIZE_ENTRY * u32::from(num_sub_ifd_tags)   // SubIFD entries
            + SIZE_LONG; // SubIFD next offset
    }

    // Write the number of tags in the IFD.
    stream.extend_from_slice(&num_tags.to_be_bytes());

    // Write the IFD entries.
    if let Some(origin) = metadata.origin {
        // The discriminants are the SkEncodedOrigin values 1 to 8, which are positive.
        #[allow(clippy::cast_sign_loss)]
        let value = origin as u32;
        if !write_entry(
            ORIGIN_TAG,
            TYPE_UNSIGNED_SHORT,
            1,
            value,
            &mut end_of_data,
            &mut stream,
            &mut buffer_for_larger_values,
        ) {
            return None;
        }
    }
    if let Some(unit) = metadata.resolution_unit
        && !write_entry(
            RESOLUTION_UNIT_TAG,
            TYPE_UNSIGNED_SHORT,
            1,
            u32::from(unit),
            &mut end_of_data,
            &mut stream,
            &mut buffer_for_larger_values,
        )
    {
        return None;
    }
    if let Some(x) = metadata.x_resolution {
        // The C++ passes the float as the uint32_t value: it truncates toward zero.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = x as u32;
        if !write_entry(
            X_RESOLUTION_TAG,
            TYPE_UNSIGNED_RATIONAL,
            1,
            value,
            &mut end_of_data,
            &mut stream,
            &mut buffer_for_larger_values,
        ) {
            return None;
        }
    }
    if let Some(y) = metadata.y_resolution {
        // The C++ passes the float as the uint32_t value: it truncates toward zero.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let value = y as u32;
        if !write_entry(
            Y_RESOLUTION_TAG,
            TYPE_UNSIGNED_RATIONAL,
            1,
            value,
            &mut end_of_data,
            &mut stream,
            &mut buffer_for_larger_values,
        ) {
            return None;
        }
    }
    if sub_ifd_exists
        && !write_entry(
            SUB_IFD_OFFSET_TAG,
            TYPE_UNSIGNED_LONG,
            1,
            sub_ifd_offset,
            &mut end_of_data,
            &mut stream,
            &mut buffer_for_larger_values,
        )
    {
        return None;
    }

    // Next IFD offset (0 for no next IFD).
    stream.extend_from_slice(&0u32.to_be_bytes());

    // After all IFD0 data has been written, then write the SubIFD (ExifIFD).
    if sub_ifd_exists {
        // Write the number of tags in the subIFD.
        stream.extend_from_slice(&num_sub_ifd_tags.to_be_bytes());

        if let Some(x) = metadata.pixel_x_dimension
            && !write_entry(
                PIXEL_X_DIMENSION_TAG,
                TYPE_UNSIGNED_LONG,
                1,
                x,
                &mut end_of_data,
                &mut stream,
                &mut buffer_for_larger_values,
            )
        {
            return None;
        }
        if let Some(y) = metadata.pixel_y_dimension
            && !write_entry(
                PIXEL_Y_DIMENSION_TAG,
                TYPE_UNSIGNED_LONG,
                1,
                y,
                &mut end_of_data,
                &mut stream,
                &mut buffer_for_larger_values,
            )
        {
            return None;
        }

        // Write the SubIFD next offset (0).
        stream.extend_from_slice(&0u32.to_be_bytes());
    }

    // Append the data buffer to the end of the stream.
    stream.extend_from_slice(&buffer_for_larger_values);

    Some(Data::new_from_vec(stream))
}
