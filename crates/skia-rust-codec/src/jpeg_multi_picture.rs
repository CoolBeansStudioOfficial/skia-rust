// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegMultiPicture.cpp and src/codec/SkJpegMultiPicture.h (chrome/m156), the
// Multi-Picture Format (MPF) index: parsing it from an APP2 segment, and serializing it again.
//
// The `SkCodecPrintf` diagnostics are not ported: they are debug output only.

use std::sync::Arc;

use skia_rust_core::data::Data;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::stream_priv::{write_u16_be, write_u32_be};

use crate::jpeg_constants::{JPEG_MARKER_CODE_SIZE, JPEG_SEGMENT_PARAMETER_LENGTH_SIZE, MPF_SIG};
use crate::tiff_utility::{
    ENDIAN_SIZE, ImageFileDirectory, TYPE_UNDEFINED, TYPE_UNSIGNED_LONG, get_endian_int,
    parse_header,
};

// Port of: src/codec/SkJpegMultiPicture.cpp#L15-L36 (chrome/m156), the MPF tag and value
// constants.
const VERSION_TAG: u16 = 0xB000;
const VERSION_COUNT: u32 = 4;
const VERSION_SIZE: usize = 4;
const VERSION_EXPECTED: &[u8; VERSION_SIZE] = b"0100";

const NUMBER_OF_IMAGES_TAG: u16 = 0xB001;
const NUMBER_OF_IMAGES_COUNT: u32 = 1;

const MP_ENTRY_TAG: u16 = 0xB002;
const MP_ENTRY_SIZE: usize = 16;
const MP_ENTRY_ATTRIBUTE_FORMAT_MASK: u32 = 0x700_0000;
const MP_ENTRY_ATTRIBUTE_FORMAT_JPEG: u32 = 0x000_0000;
const MP_ENTRY_ATTRIBUTE_TYPE_MASK: u32 = 0xFF_FFFF;
const MP_ENTRY_ATTRIBUTE_TYPE_PRIMARY: u32 = 0x03_0000;

const INDIVIDUAL_IMAGE_UNIQUE_ID_TAG: u16 = 0xB003;
const INDIVIDUAL_IMAGE_UNIQUE_ID_SIZE: usize = 33;

const TOTAL_NUMBER_CAPTURED_FRAMES_TAG: u16 = 0xB004;

// Port of: src/codec/SkTiffUtility.h#L13-L16 (chrome/m156), `kEndianBig`.
const ENDIAN_BIG: [u8; ENDIAN_SIZE] = [b'M', b'M', 0, 42];

/// One image in an MPF index (`SkJpegMultiPictureParameters::Image`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MultiPictureImage {
    /// The size of the image's data, in bytes.
    pub size: u32,
    /// The offset of the image's data, from the MPF header (0 for the primary image).
    pub data_offset: u32,
}

/// The parameters of a Multi-Picture Format index (`SkJpegMultiPictureParameters`).
#[doc(alias = "SkJpegMultiPictureParameters")]
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MultiPictureParameters {
    /// The images, the primary image first.
    pub images: Vec<MultiPictureImage>,
}

impl MultiPictureParameters {
    /// Parameters for `number_of_images` images, all zero (`SkJpegMultiPictureParameters(size_t)`).
    // Port of: src/codec/SkJpegMultiPicture.h#L14-L16 (chrome/m156)
    #[must_use]
    pub fn new(number_of_images: usize) -> Self {
        Self {
            images: vec![MultiPictureImage::default(); number_of_images],
        }
    }

    /// Parses the parameters of an MPF segment, which starts with the MPF signature
    /// (`SkJpegMultiPictureParameters::Make`).
    // Port of: src/codec/SkJpegMultiPicture.cpp#L38-L143 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    #[allow(clippy::too_many_lines)] // one function, as in C++
    pub fn make(segment_parameters: &Data) -> Option<Self> {
        let bytes = segment_parameters.as_bytes();
        if bytes.len() < MPF_SIG.len() || &bytes[..MPF_SIG.len()] != MPF_SIG {
            return None;
        }
        let ifd_data: Arc<[u8]> = Arc::from(&bytes[MPF_SIG.len()..]);

        let (little_endian, ifd_offset) = parse_header(&ifd_data)?;
        let ifd = ImageFileDirectory::make_from_offset(
            Arc::clone(&ifd_data),
            little_endian,
            ifd_offset,
            false,
        )?;

        let tag_count = ifd.num_entries();
        let mut number_of_images: u32 = 0;
        let mut mp_entries_data: Option<Vec<u8>> = None;
        let mut previous_tag: u16 = 0;
        for entry_index in 0..tag_count {
            let tag = ifd.entry_tag(entry_index);
            // "MPF tags not in order."
            if previous_tag >= tag {
                return None;
            }
            previous_tag = tag;
            match tag {
                VERSION_TAG => {
                    // The version must be an undefined value of 4 bytes, "0100".
                    let data = ifd.entry_undefined_data(entry_index)?;
                    if data.len() != VERSION_SIZE || data != VERSION_EXPECTED {
                        return None;
                    }
                }
                NUMBER_OF_IMAGES_TAG => {
                    // A failure here is logged, and leaves the count at zero.
                    if let Some(values) = ifd.entry_unsigned_long(entry_index, 1) {
                        number_of_images = values[0];
                    }
                    // "Invalid number of images."
                    if number_of_images < 1 {
                        return None;
                    }
                }
                MP_ENTRY_TAG => {
                    let data = ifd.entry_undefined_data(entry_index)?;
                    let expected_size = MP_ENTRY_SIZE.checked_mul(number_of_images as usize)?;
                    // "MP entries data should be 16x<count> bytes."
                    if data.len() != expected_size {
                        return None;
                    }
                    mp_entries_data = Some(data.to_vec());
                }
                INDIVIDUAL_IMAGE_UNIQUE_ID_TAG => {
                    let data = ifd.entry_undefined_data(entry_index)?;
                    // "Invalid Image Unique ID count."
                    if data.len() != INDIVIDUAL_IMAGE_UNIQUE_ID_SIZE * number_of_images as usize {
                        return None;
                    }
                }
                TOTAL_NUMBER_CAPTURED_FRAMES_TAG => {
                    // A failure here is logged only.
                    let _ = ifd.entry_unsigned_long(entry_index, 1);
                }
                _ => return None,
            }
        }

        // "Number of images must be greater than zero."
        if number_of_images == 0 {
            return None;
        }
        let mp_entries_data = mp_entries_data?;

        let mut result = Self::new(number_of_images as usize);
        for (i, image) in result.images.iter_mut().enumerate() {
            let mp_entry = &mp_entries_data[MP_ENTRY_SIZE * i..MP_ENTRY_SIZE * (i + 1)];
            let attribute = get_endian_int(&mp_entry[0..4], little_endian);
            let size = get_endian_int(&mp_entry[4..8], little_endian);
            let data_offset = get_endian_int(&mp_entry[8..12], little_endian);

            let is_primary =
                (attribute & MP_ENTRY_ATTRIBUTE_TYPE_MASK) == MP_ENTRY_ATTRIBUTE_TYPE_PRIMARY;
            let is_jpeg =
                (attribute & MP_ENTRY_ATTRIBUTE_FORMAT_MASK) == MP_ENTRY_ATTRIBUTE_FORMAT_JPEG;
            // "Image must be primary iff it is the first image."
            if is_primary != (i == 0) {
                return None;
            }
            // "Image format must be 0 (JPEG)."
            if !is_jpeg {
                return None;
            }
            // "First individual Image offset must be NULL."
            if i == 0 && data_offset != 0 {
                return None;
            }
            image.data_offset = data_offset;
            image.size = size;
        }
        Some(result)
    }

    /// Serializes the index for the image `individual_image_number` (0 is the index itself)
    /// (`SkJpegMultiPictureParameters::serialize`).
    // Port of: src/codec/SkJpegMultiPicture.cpp#L145-L203 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // the counts and offsets are uint32_t in C++
    pub fn serialize(&self, individual_image_number: u32) -> Data {
        let mut s = DynamicMemoryWStream::new();
        let number_of_images = self.images.len() as u32;
        s.write(MPF_SIG);
        s.write(&ENDIAN_BIG);
        let first_ifd_offset = (ENDIAN_BIG.len() + std::mem::size_of::<u32>()) as u32;
        write_u32_be(&mut s, first_ifd_offset);

        if individual_image_number == 0 {
            let mp_index_ifd_number_of_tags: u16 = 3;
            write_u16_be(&mut s, mp_index_ifd_number_of_tags);
        } else {
            let mp_attribute_ifd_number_of_tags: u16 = 1;
            write_u16_be(&mut s, mp_attribute_ifd_number_of_tags);
        }
        write_u16_be(&mut s, VERSION_TAG);
        write_u16_be(&mut s, TYPE_UNDEFINED);
        write_u32_be(&mut s, VERSION_COUNT);
        s.write(VERSION_EXPECTED);

        if individual_image_number == 0 {
            write_u16_be(&mut s, NUMBER_OF_IMAGES_TAG);
            write_u16_be(&mut s, TYPE_UNSIGNED_LONG);
            write_u32_be(&mut s, NUMBER_OF_IMAGES_COUNT);
            write_u32_be(&mut s, number_of_images);
            write_u16_be(&mut s, MP_ENTRY_TAG);
            write_u16_be(&mut s, TYPE_UNDEFINED);
            let mp_entries_size = MP_ENTRY_SIZE as u32 * number_of_images;
            write_u32_be(&mut s, mp_entries_size);
            // The bytes written so far, excluding the MPF signature, plus the 4 bytes for this
            // offset and the 4 bytes for the attribute IFD offset.
            let mp_entry_offset = (s.bytes_written() - MPF_SIG.len()
                + std::mem::size_of::<u32>()
                + std::mem::size_of::<u32>()) as u32;
            write_u32_be(&mut s, mp_entry_offset);
            write_u32_be(&mut s, 0);

            for (i, image) in self.images.iter().enumerate() {
                let mut attribute = MP_ENTRY_ATTRIBUTE_FORMAT_JPEG;
                if i == 0 {
                    attribute |= MP_ENTRY_ATTRIBUTE_TYPE_PRIMARY;
                }
                write_u32_be(&mut s, attribute);
                write_u32_be(&mut s, image.size);
                write_u32_be(&mut s, image.data_offset);
                write_u16_be(&mut s, 0);
                write_u16_be(&mut s, 0);
            }
        } else {
            write_u32_be(&mut s, 0);
        }
        s.detach_as_data()
    }

    /// The offset of the image data, from the start of the file, for an image at `data_offset`
    /// in the MPF segment at `mp_segment_offset` (`GetImageAbsoluteOffset`).
    // Port of: src/codec/SkJpegMultiPicture.cpp#L205-L215 (chrome/m156)
    #[must_use]
    pub fn get_image_absolute_offset(data_offset: u32, mp_segment_offset: usize) -> usize {
        if data_offset == 0 {
            return 0;
        }
        mp_header_absolute_offset(mp_segment_offset) + data_offset as usize
    }

    /// The offset of an image in the MPF header, from its offset in the file
    /// (`GetImageDataOffset`).
    // Port of: src/codec/SkJpegMultiPicture.cpp#L217-L225 (chrome/m156)
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // the C++ casts the offset to uint32_t
    pub fn get_image_data_offset(image_absolute_offset: usize, mp_segment_offset: usize) -> u32 {
        if image_absolute_offset == 0 {
            return 0;
        }
        (image_absolute_offset - mp_header_absolute_offset(mp_segment_offset)) as u32
    }
}

// Port of: src/codec/SkJpegMultiPicture.cpp#L205-L211 (chrome/m156), `mp_header_absolute_offset`.
fn mp_header_absolute_offset(mp_segment_offset: usize) -> usize {
    mp_segment_offset        // The offset to the segment's marker
        + JPEG_MARKER_CODE_SIZE  // The marker itself
        + JPEG_SEGMENT_PARAMETER_LENGTH_SIZE // The segment parameter length
        + MPF_SIG.len() // The {'M','P','F',0} signature
}
