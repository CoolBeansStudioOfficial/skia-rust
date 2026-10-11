// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/JpegGainmapTest.cpp (chrome/m156), the segment scan, Multi-Picture Format,
// source-manager, gainmap decode, gainmap encode and transcode cases. The cases that need the
// gainmap encoder use `SkJpegGainmapEncoder::EncodeHDRGM` (`encode::jpeg_gainmap_encoder`).
#![cfg(test)]
// The C++ test literals are kept as written (digit groups, float digits).
#![allow(clippy::unreadable_literal, clippy::excessive_precision)]

use std::sync::Arc;

use skia_rust_codec::android_codec::AndroidCodec;
use skia_rust_codec::codec::Result as CodecResult;
use skia_rust_codec::codecs;
use skia_rust_codec::encode::jpeg_encoder::Options as JpegEncoderOptions;
use skia_rust_codec::encode::jpeg_gainmap_encoder;
use skia_rust_codec::jpeg_codec;
use skia_rust_codec::jpeg_constants::{JPEG_MARKER_START_OF_SCAN, MPF_MARKER, MPF_SIG};
use skia_rust_codec::jpeg_multi_picture::MultiPictureParameters;
use skia_rust_codec::jpeg_segment_scan::JpegSegment;
use skia_rust_codec::jpeg_source_mgr::JpegSourceMgr;
use skia_rust_codec::tiff_utility::{ImageFileDirectory, parse_header};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::canvas::Canvas;
use skia_rust_core::color::{Color, Color4f};
use skia_rust_core::color_space::{ColorSpace, named_gamut, named_transfer_fn};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::data::Data;
use skia_rust_core::gainmap_info::{BaseImageType, GainmapInfo, GainmapType};
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::images;
use skia_rust_core::paint::Paint;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_core::rect::Rect;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::size::ISize;
use skia_rust_core::stream::{MemoryStream, Stream, StreamAsset};
use skia_rust_effects::gainmap_shader::GainmapShader;
use skia_rust_raster::raster_canvas::RasterCanvas;

use crate::gainmap_test_common::{approx_eq_color, expect_approx_eq_info};
use crate::resources::{get_resource_as_data, resource_dir};
use crate::{Reporter, def_test, reporter_assert};

/// The stream of a resource file, as `GetResourceAsStream` gives it (a seekable file stream).
fn get_resource_as_stream(path: &str) -> Option<Box<dyn StreamAsset>> {
    let mut full = resource_dir()?;
    for component in path.split('/') {
        full.push(component);
    }
    skia_rust_core::stream::make_from_file(full)
}

// A test stream to stress the different SkJpegSourceMgr sub-classes.
// Port of: tests/JpegGainmapTest.cpp#L37-L112 (chrome/m156), `TestStream`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamType {
    /// `SkJpegUnseekableSourceMgr`.
    Unseekable,
    /// `SkJpegBufferedSourceMgr`.
    Seekable,
    /// `SkJpegMemorySourceMgr`.
    MemoryMapped,
}

struct TestStream<'a> {
    stream: &'a mut dyn Stream,
    seekable: bool,
    memory_mapped: bool,
}

impl<'a> TestStream<'a> {
    fn new(stream_type: StreamType, stream: &'a mut dyn Stream) -> Self {
        Self {
            stream,
            seekable: stream_type != StreamType::Unseekable,
            memory_mapped: stream_type == StreamType::MemoryMapped,
        }
    }
}

impl Stream for TestStream<'_> {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.stream.read(buffer)
    }

    fn is_at_end(&self) -> bool {
        self.stream.is_at_end()
    }

    fn rewind(&mut self) -> bool {
        if !self.seekable {
            return false;
        }
        self.stream.rewind()
    }

    fn has_position(&self) -> bool {
        if !self.seekable {
            return false;
        }
        self.stream.has_position()
    }

    // The C++ returns `fStream->hasPosition()` here, not the position. The test relies on the
    // source managers' use of the position only to restore it, so the value is kept as written.
    fn get_position(&self) -> usize {
        if !self.seekable {
            return 0;
        }
        usize::from(self.stream.has_position())
    }

    fn seek(&mut self, position: usize) -> bool {
        if !self.seekable {
            return false;
        }
        self.stream.seek(position)
    }

    fn move_by(&mut self, offset: i64) -> bool {
        if !self.seekable {
            return false;
        }
        self.stream.move_by(offset)
    }

    fn has_length(&self) -> bool {
        if !self.memory_mapped {
            return false;
        }
        self.stream.has_length()
    }

    fn get_length(&self) -> usize {
        if !self.memory_mapped {
            return 0;
        }
        self.stream.get_length()
    }

    fn get_memory_base(&self) -> Option<&[u8]> {
        if !self.memory_mapped {
            return None;
        }
        self.stream.get_memory_base()
    }
}

// Port of: tests/JpegGainmapTest.cpp#L116-L175 (chrome/m156), `Codec_jpegSegmentScan`.
def_test!(Codec_jpegSegmentScan, |r| {
    // (path, sos segment count, eoi segment count, test segment index, test segment marker,
    //  test segment offset, test segment parameter length)
    let recs: [(&str, usize, usize, usize, u8, usize, u16); 20] = [
        (
            "images/wide_gamut_yellow_224_224_64.jpeg",
            11,
            15,
            10,
            0xda,
            9768,
            12,
        ),
        ("images/CMYK.jpg", 7, 8, 1, 0xee, 2, 14),
        ("images/b78329453.jpeg", 10, 23, 3, 0xe2, 154, 540),
        ("images/brickwork-texture.jpg", 8, 28, 12, 0xc4, 34183, 42),
        (
            "images/brickwork_normal-map.jpg",
            8,
            28,
            27,
            0xd9,
            180612,
            0,
        ),
        (
            "images/cmyk_yellow_224_224_32.jpg",
            19,
            23,
            2,
            0xed,
            854,
            2828,
        ),
        ("images/color_wheel.jpg", 10, 11, 2, 0xdb, 20, 67),
        ("images/cropped_mandrill.jpg", 10, 11, 4, 0xc0, 158, 17),
        ("images/dog.jpg", 10, 11, 5, 0xc4, 177, 28),
        ("images/ducky.jpg", 12, 13, 10, 0xc4, 3718, 181),
        ("images/exif-orientation-2-ur.jpg", 11, 12, 2, 0xe1, 20, 130),
        ("images/flutter_logo.jpg", 9, 27, 21, 0xda, 5731, 8),
        ("images/grayscale.jpg", 6, 16, 9, 0xda, 327, 8),
        ("images/icc-v2-gbr.jpg", 12, 25, 24, 0xd9, 43832, 0),
        ("images/mandrill_512_q075.jpg", 10, 11, 7, 0xc4, 393, 31),
        ("images/mandrill_cmyk.jpg", 19, 35, 16, 0xdd, 574336, 4),
        ("images/mandrill_h1v1.jpg", 10, 11, 1, 0xe0, 2, 16),
        ("images/mandrill_h2v1.jpg", 10, 11, 0, 0xd8, 0, 0),
        ("images/randPixels.jpg", 10, 11, 6, 0xc4, 200, 30),
        (
            "images/wide_gamut_yellow_224_224_64.jpeg",
            11,
            15,
            10,
            0xda,
            9768,
            12,
        ),
    ];

    for (
        path,
        sos_segment_count,
        eoi_segment_count,
        test_segment_index,
        test_segment_marker,
        test_segment_offset,
        test_segment_parameter_length,
    ) in recs
    {
        let Some(mut stream) = get_resource_as_stream(path) else {
            eprintln!("todo: skipping, missing Skia resource {path}");
            continue;
        };

        // Scan all the way to EndOfImage.
        let mut source_mgr = JpegSourceMgr::make(&mut *stream, 1024);
        let segments: Vec<JpegSegment> = source_mgr.get_all_segments().to_vec();

        // Verify we got the expected number of segments at EndOfImage.
        reporter_assert!(r, eoi_segment_count == segments.len());

        // Verify we got the expected number of segments before StartOfScan.
        for (i, segment) in segments.iter().enumerate() {
            if segment.marker == JPEG_MARKER_START_OF_SCAN {
                reporter_assert!(r, sos_segment_count == i + 1);
                break;
            }
        }

        // Verify the values for a randomly pre-selected segment index.
        let segment = segments[test_segment_index];
        reporter_assert!(r, test_segment_marker == segment.marker);
        reporter_assert!(r, test_segment_offset == segment.offset);
        reporter_assert!(r, test_segment_parameter_length == segment.parameter_length);
    }
});

// Port of: tests/JpegGainmapTest.cpp#L177-L196 (chrome/m156), `find_mp_params_segment`.
fn find_mp_params_segment(
    stream: &mut dyn Stream,
) -> Option<(MultiPictureParameters, JpegSegment)> {
    let mut source_mgr = JpegSourceMgr::make(stream, 1024);
    let segments: Vec<JpegSegment> = source_mgr.get_all_segments().to_vec();
    for segment in segments {
        if u32::from(segment.marker) != MPF_MARKER {
            continue;
        }
        let Some(parameter_data) = source_mgr.get_segment_parameters(&segment) else {
            continue;
        };
        if let Some(mp_params) = MultiPictureParameters::make(&parameter_data) {
            return Some((mp_params, segment));
        }
    }
    None
}

// Port of: tests/JpegGainmapTest.cpp#L198-L334 (chrome/m156), `Codec_multiPictureParams`.
def_test!(Codec_multiPictureParams, |r| {
    // Little-endian test.
    {
        let bytes: &[u8] = &[
            0x4d, 0x50, 0x46, 0x00, 0x49, 0x49, 0x2a, 0x00, 0x08, 0x00, 0x00, 0x00, 0x03, 0x00,
            0x00, 0xb0, 0x07, 0x00, 0x04, 0x00, 0x00, 0x00, 0x30, 0x31, 0x30, 0x30, 0x01, 0xb0,
            0x04, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0x00, 0x00, 0x00, 0x02, 0xb0, 0x07, 0x00,
            0x20, 0x00, 0x00, 0x00, 0x32, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x03, 0x00, 0x20, 0xcf, 0x49, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0xee, 0x28, 0x01, 0x00, 0xf9, 0xb7, 0x3c, 0x00, 0x00, 0x00,
            0x00, 0x00,
        ];
        let mp_params = MultiPictureParameters::make(&Data::new_copy(bytes));
        reporter_assert!(r, mp_params.is_some());
        let mp_params = mp_params.expect("checked above");
        reporter_assert!(r, mp_params.images.len() == 2);
        reporter_assert!(r, mp_params.images[0].data_offset == 0);
        reporter_assert!(r, mp_params.images[0].size == 4837152);
        reporter_assert!(r, mp_params.images[1].data_offset == 3979257);
        reporter_assert!(r, mp_params.images[1].size == 76014);
    }
    // Big-endian test.
    {
        let bytes: &[u8] = &[
            0x4d, 0x50, 0x46, 0x00, 0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x03,
            0xb0, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x04, 0x30, 0x31, 0x30, 0x30, 0xb0, 0x01,
            0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x02, 0xb0, 0x02, 0x00, 0x07,
            0x00, 0x00, 0x00, 0x20, 0x00, 0x00, 0x00, 0x32, 0x00, 0x00, 0x00, 0x00, 0x20, 0x03,
            0x00, 0x00, 0x00, 0x56, 0xda, 0x2f, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x14, 0xc6, 0x01, 0x00, 0x55, 0x7c, 0x1f, 0x00, 0x00,
            0x00, 0x00,
        ];
        let mp_params = MultiPictureParameters::make(&Data::new_copy(bytes));
        reporter_assert!(r, mp_params.is_some());
        let mp_params = mp_params.expect("checked above");
        reporter_assert!(r, mp_params.images.len() == 2);
        reporter_assert!(r, mp_params.images[0].data_offset == 0);
        reporter_assert!(r, mp_params.images[0].size == 5691951);
        reporter_assert!(r, mp_params.images[1].data_offset == 5602335);
        reporter_assert!(r, mp_params.images[1].size == 1361409);
    }
    // Three entry test.
    {
        let bytes: &[u8] = &[
            0x4d, 0x50, 0x46, 0x00, 0x4d, 0x4d, 0x00, 0x2a, 0x00, 0x00, 0x00, 0x08, 0x00, 0x03,
            0xb0, 0x00, 0x00, 0x07, 0x00, 0x00, 0x00, 0x04, 0x30, 0x31, 0x30, 0x30, 0xb0, 0x01,
            0x00, 0x04, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x03, 0xb0, 0x02, 0x00, 0x07,
            0x00, 0x00, 0x00, 0x30, 0x00, 0x00, 0x00, 0x32, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03,
            0x00, 0x00, 0x00, 0x1f, 0x1c, 0xc2, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x05, 0xb0, 0x00, 0x1f, 0x12, 0xec, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x96, 0x6b, 0x00, 0x22, 0x18, 0x9c,
            0x00, 0x00, 0x00, 0x00,
        ];
        let mp_params = MultiPictureParameters::make(&Data::new_copy(bytes));
        reporter_assert!(r, mp_params.is_some());
        let mp_params = mp_params.expect("checked above");
        reporter_assert!(r, mp_params.images.len() == 3);
        reporter_assert!(r, mp_params.images[0].data_offset == 0);
        reporter_assert!(r, mp_params.images[0].size == 2038978);
        reporter_assert!(r, mp_params.images[1].data_offset == 2036460);
        reporter_assert!(r, mp_params.images[1].size == 198064);
        reporter_assert!(r, mp_params.images[2].data_offset == 2234524);
        reporter_assert!(r, mp_params.images[2].size == 38507);
    }
    // Inserting various corrupt values.
    {
        let bytes: [u8; 86] = [
            0x4d, 0x50, 0x46, 0x00, // 0: {'M', 'P', 'F',   0} signature
            0x4d, 0x4d, 0x00, 0x2a, // 4: {'M', 'M',   0, '*'} big-endian
            0x00, 0x00, 0x00, 0x08, // 8: Index IFD offset
            0x00, 0x03, // 12: Number of tags
            0xb0, 0x00, // 14: Version tag
            0x00, 0x07, // 16: Undefined type
            0x00, 0x00, 0x00, 0x04, // 18: Size
            0x30, 0x31, 0x30, 0x30, // 22: Value
            0xb0, 0x01, // 26: Number of images
            0x00, 0x04, // 28: Unsigned long type
            0x00, 0x00, 0x00, 0x01, // 30: Count
            0x00, 0x00, 0x00, 0x02, // 34: Value
            0xb0, 0x02, // 38: MP entry tag
            0x00, 0x07, // 40: Undefined type
            0x00, 0x00, 0x00, 0x20, // 42: Size
            0x00, 0x00, 0x00, 0x32, // 46: Value (offset)
            0x00, 0x00, 0x00, 0x00, // 50: Next IFD offset (null)
            0x20, 0x03, 0x00, 0x00, // 54: MP Entry 0 attributes
            0x00, 0x56, 0xda, 0x2f, // 58: MP Entry 0 size (5691951)
            0x00, 0x00, 0x00, 0x00, // 62: MP Entry 0 offset (0)
            0x00, 0x00, 0x00, 0x00, // 66: MP Entry 0 dependencies
            0x00, 0x00, 0x00, 0x00, // 70: MP Entry 1 attributes.
            0x00, 0x14, 0xc6, 0x01, // 74: MP Entry 1 size (1361409)
            0x00, 0x55, 0x7c, 0x1f, // 78: MP Entry 1 offset (5602335)
            0x00, 0x00, 0x00, 0x00, // 82: MP Entry 1 dependencies
        ];
        // Verify the offsets labeled above.
        reporter_assert!(r, bytes[22] == 0x30);
        reporter_assert!(r, bytes[26] == 0xb0);
        reporter_assert!(r, bytes[38] == 0xb0);
        reporter_assert!(r, bytes[54] == 0x20);
        reporter_assert!(r, bytes[81] == 0x1f);
        {
            // Change the version to {'0', '1', '0', '1'}.
            let mut bytes_invalid = bytes.to_vec();
            reporter_assert!(r, bytes[25] == b'0');
            bytes_invalid[25] = b'1';
            reporter_assert!(
                r,
                MultiPictureParameters::make(&Data::new_copy(&bytes_invalid)).is_none()
            );
        }
        {
            // Change the number of images to be undefined type instead of unsigned long type.
            let mut bytes_invalid = bytes.to_vec();
            reporter_assert!(r, bytes[29] == 0x04);
            bytes_invalid[29] = 0x07;
            reporter_assert!(
                r,
                MultiPictureParameters::make(&Data::new_copy(&bytes_invalid)).is_none()
            );
        }
        {
            // Make the MP entries point off of the end of the buffer.
            let mut bytes_invalid = bytes.to_vec();
            reporter_assert!(r, bytes[49] == 0x32);
            bytes_invalid[49] = 0xFE;
            reporter_assert!(
                r,
                MultiPictureParameters::make(&Data::new_copy(&bytes_invalid)).is_none()
            );
        }
        {
            // Make the MP entries too small.
            let mut bytes_invalid = bytes.to_vec();
            reporter_assert!(r, bytes[45] == 0x20);
            bytes_invalid[45] = 0x1F;
            reporter_assert!(
                r,
                MultiPictureParameters::make(&Data::new_copy(&bytes_invalid)).is_none()
            );
        }
    }
});

// Port of: tests/JpegGainmapTest.cpp#L336-L426 (chrome/m156), `Codec_jpegMultiPicture`.
def_test!(
    #[allow(clippy::needless_range_loop)]
    Codec_jpegMultiPicture,
    |r| {
        let path = "images/iphone_13_pro.jpeg";
        let Some(mut stream) = get_resource_as_stream(path) else {
            reporter_assert!(r, false);
            return;
        };
        // Search and parse the MPF header.
        let Some((mp_params, mp_params_segment)) = find_mp_params_segment(&mut *stream) else {
            reporter_assert!(r, false);
            return;
        };

        // Verify that we get the same parameters when we re-serialize and de-serialize them.
        {
            let mp_params_serialized = mp_params.serialize(0);
            let mp_params_round_tripped = MultiPictureParameters::make(&mp_params_serialized);
            reporter_assert!(r, mp_params_round_tripped.is_some());
            let mp_params_round_tripped = mp_params_round_tripped.expect("checked above");
            reporter_assert!(
                r,
                mp_params_round_tripped.images.len() == mp_params.images.len()
            );
            for (round_tripped, original) in
                mp_params_round_tripped.images.iter().zip(&mp_params.images)
            {
                reporter_assert!(r, round_tripped.size == original.size);
                reporter_assert!(r, round_tripped.data_offset == original.data_offset);
            }
        }

        let recs = [
            (StreamType::MemoryMapped, false, 1024usize),
            (StreamType::MemoryMapped, true, 1024),
            (StreamType::Seekable, false, 1024),
            (StreamType::Seekable, true, 1024),
            (StreamType::Seekable, false, 7),
            (StreamType::Seekable, true, 13),
            (StreamType::Seekable, true, 1024 * 1024 * 16),
            (StreamType::Unseekable, false, 1024),
            (StreamType::Unseekable, true, 1024),
            (StreamType::Unseekable, false, 1),
            (StreamType::Unseekable, true, 1),
            (StreamType::Unseekable, false, 7),
            (StreamType::Unseekable, true, 13),
            (StreamType::Unseekable, false, 1024 * 1024 * 16),
            (StreamType::Unseekable, true, 1024 * 1024 * 16),
        ];

        for (stream_type, skip_first_image, buffer_size) in recs {
            stream.rewind();
            let mut test_stream = TestStream::new(stream_type, &mut *stream);
            let mut source_mgr = JpegSourceMgr::make(&mut test_stream, buffer_size);

            // Decode the images into bitmaps.
            let number_of_images = mp_params.images.len();
            let mut bitmaps: Vec<Option<(ImageInfo, Vec<u8>)>> = vec![None; number_of_images];
            for i in 0..number_of_images {
                if i == 0 {
                    reporter_assert!(r, mp_params.images[i].data_offset == 0);
                    continue;
                }
                if i == 1 && skip_first_image {
                    continue;
                }
                let image_absolute_offset = MultiPictureParameters::get_image_absolute_offset(
                    mp_params.images[i].data_offset,
                    mp_params_segment.offset,
                );
                let Some(image_data) = source_mgr
                    .get_subset_data(image_absolute_offset, mp_params.images[i].size as usize)
                else {
                    reporter_assert!(r, false);
                    continue;
                };
                let codec = codecs::make_codec_from_stream(MemoryStream::make(Some(image_data)));
                let Ok(mut codec) = codec else {
                    reporter_assert!(r, false);
                    continue;
                };
                let info = codec.info().clone();
                let row_bytes = info.min_row_bytes();
                let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
                reporter_assert!(
                    r,
                    CodecResult::Success == codec.get_pixels(&info, &mut pixels, row_bytes, None)
                );
                bitmaps[i] = Some((info, pixels));
            }

            // Spot-check the image size and pixels.
            let pixmap_of = |index: usize| {
                bitmaps[index].as_ref().and_then(|(info, pixels)| {
                    Pixmap::new_readonly(info, pixels, info.min_row_bytes())
                })
            };
            if !skip_first_image {
                let Some(first) = pixmap_of(1) else {
                    reporter_assert!(r, false);
                    continue;
                };
                reporter_assert!(r, first.info().dimensions() == ISize::new(1512, 2016));
                reporter_assert!(
                    r,
                    first.get_color((0, 0)) == Color::from_argb(0xFF, 0x3B, 0x3B, 0x3B)
                );
                reporter_assert!(
                    r,
                    first.get_color((1511, 2015)) == Color::from_argb(0xFF, 0x10, 0x10, 0x10)
                );
            }
            let Some(second) = pixmap_of(2) else {
                reporter_assert!(r, false);
                continue;
            };
            reporter_assert!(r, second.info().dimensions() == ISize::new(576, 768));
            reporter_assert!(
                r,
                second.get_color((0, 0)) == Color::from_argb(0xFF, 0x01, 0x01, 0x01)
            );
            reporter_assert!(
                r,
                second.get_color((575, 767)) == Color::from_argb(0xFF, 0xB5, 0xB5, 0xB5)
            );
        }
    }
);

// The ISO 21496-1 version and parameters that the next test parses.
// Port of: tests/JpegGainmapTest.cpp#L991-L1011 (chrome/m156), `versionData` and `data`.
const VERSION_DATA: &[u8] = &[0x00, 0x00, 0x00, 0x00];
const DATA: &[u8] = &[
    0x00, 0x00, 0x00, 0x00, 0xc0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x01, 0x45,
    0x3e, 0x00, 0x00, 0x80, 0x00, 0xfc, 0x23, 0x05, 0x14, 0x40, 0x00, 0x00, 0x00, 0x00, 0x01, 0x1f,
    0xe1, 0x00, 0x00, 0x80, 0x00, 0x10, 0x4b, 0x9f, 0x0a, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0xfd, 0xdb, 0x68,
    0x04, 0x40, 0x00, 0x00, 0x00, 0x00, 0x01, 0x11, 0x68, 0x00, 0x00, 0x80, 0x00, 0x10, 0x28, 0xf9,
    0x53, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x40, 0x00, 0x00, 0x00, 0xf7, 0x16, 0x7b, 0x90, 0x40, 0x00, 0x00, 0x00, 0x00, 0x01, 0x0f,
    0x9a, 0x00, 0x00, 0x80, 0x00, 0x12, 0x95, 0xa8, 0x3f, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00,
    0x00, 0x40, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x40, 0x00, 0x00, 0x00,
];

// Port of: tests/JpegGainmapTest.cpp#L991-L1068 (chrome/m156), `AndroidCodec_gainmapInfoParse`.
def_test!(AndroidCodec_gainmapInfoParse, |r| {
    let expected_info = GainmapInfo {
        gainmap_ratio_min: Color4f {
            r: 0.959023,
            g: 0.977058,
            b: 0.907989,
            a: 1.0,
        },
        gainmap_ratio_max: Color4f {
            r: 4.753710,
            g: 4.395375,
            b: 4.352630,
            a: 1.0,
        },
        gainmap_gamma: Color4f {
            r: 3.927490,
            g: 3.960382,
            b: 3.443712,
            a: 1.0,
        },
        epsilon_sdr: Color4f {
            r: 0.015625,
            g: 0.015625,
            b: 0.015625,
            a: 1.0,
        },
        epsilon_hdr: Color4f {
            r: 0.015625,
            g: 0.015625,
            b: 0.015625,
            a: 1.0,
        },
        display_ratio_sdr: 1.000000,
        display_ratio_hdr: 5.819739,
        base_image_type: BaseImageType::Sdr,
        gainmap_type: GainmapType::Default,
        gainmap_math_color_space: None,
    };
    let single_channel_info = GainmapInfo {
        gainmap_ratio_min: Color4f {
            r: 0.1234567e-4,
            g: 0.1234567e-4,
            b: 0.1234567e-4,
            a: 1.0,
        },
        gainmap_ratio_max: Color4f {
            r: 0.2345678e-4,
            g: 0.2345678e-4,
            b: 0.2345678e-4,
            a: 1.0,
        },
        gainmap_gamma: Color4f {
            r: 0.1234567e0,
            g: 0.1234567e0,
            b: 0.1234567e0,
            a: 1.0,
        },
        epsilon_sdr: Color4f {
            r: 0.1234567e4,
            g: 0.1234567e4,
            b: 0.1234567e4,
            a: 1.0,
        },
        epsilon_hdr: Color4f {
            r: 0.1234567e4,
            g: 0.1234567e4,
            b: 0.1234567e4,
            a: 1.0,
        },
        display_ratio_sdr: 1.0,
        display_ratio_hdr: 4.0,
        base_image_type: BaseImageType::Hdr,
        gainmap_type: GainmapType::Default,
        gainmap_math_color_space: Some(ColorSpace::new_srgb()),
    };

    // Verify the version from data.
    reporter_assert!(
        r,
        GainmapInfo::parse_version(Some(&Data::new_static(VERSION_DATA)))
    );
    // Verify the GainmapInfo from data.
    let mut info = GainmapInfo::default();
    reporter_assert!(
        r,
        GainmapInfo::parse(Some(&Data::new_static(DATA)), &mut info)
    );
    expect_approx_eq_info(r, &info, &expected_info);
    // Verify the parsed version.
    reporter_assert!(
        r,
        GainmapInfo::parse_version(Some(&GainmapInfo::serialize_version()))
    );
    // Verify the round-trip GainmapInfo.
    let data_info = info.serialize();
    let mut info_round_trip = GainmapInfo::default();
    reporter_assert!(
        r,
        GainmapInfo::parse(Some(&data_info), &mut info_round_trip)
    );
    expect_approx_eq_info(r, &info, &info_round_trip);
    // Serialize a single-channel GainmapInfo. The serialized data should be smaller.
    let data_single_channel_info = single_channel_info.serialize();
    reporter_assert!(r, data_single_channel_info.size() < data_info.size());
    let mut single_channel_info_round_trip = GainmapInfo::default();
    reporter_assert!(
        r,
        GainmapInfo::parse(
            Some(&data_single_channel_info),
            &mut single_channel_info_round_trip
        )
    );
    expect_approx_eq_info(r, &single_channel_info_round_trip, &single_channel_info);
});

/// Reads the whole of a test stream, as the JPEG decoder does when it is made.
fn read_all(stream: &mut dyn Stream) -> Vec<u8> {
    let mut out = Vec::new();
    let mut chunk = [0u8; 4096];
    loop {
        let n = stream.read(&mut chunk);
        if n == 0 {
            break;
        }
        out.extend_from_slice(&chunk[..n]);
    }
    out
}

/// A colour with alpha 1, as the `{r, g, b, 1.f}` initializers of the C++ test give it.
fn opaque(r: f32, g: f32, b: f32) -> Color4f {
    Color4f { r, g, b, a: 1.0 }
}

/// The decoded base image, the decoded gainmap and the gainmap parameters (the outputs of the
/// C++ `decode_all`).
struct DecodedJpeg {
    /// The base image's info and pixels.
    base: Option<(ImageInfo, Vec<u8>)>,
    /// The gainmap image's info and pixels.
    gainmap: Option<(ImageInfo, Vec<u8>)>,
    /// The gainmap parameters.
    info: GainmapInfo,
}

// Decode an image and its gainmap.
// Port of: tests/JpegGainmapTest.cpp#L423-L457 (chrome/m156), `decode_all`. The JPEG decoder reads
// its whole stream into memory when it is made, so the test stream is read the same way here.
fn decode_all(r: &mut Reporter, stream: &mut dyn Stream) -> DecodedJpeg {
    let mut result = DecodedJpeg {
        base: None,
        gainmap: None,
        info: GainmapInfo::default(),
    };
    let data = read_all(stream);

    // Decode the base bitmap.
    let Ok(mut base_codec) =
        jpeg_codec::make_from_stream(MemoryStream::make(Some(Data::new_copy(&data))))
    else {
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
    result.base = Some((base_info, base_pixels));

    let Some(mut android_codec) = AndroidCodec::make_from_codec(base_codec) else {
        reporter_assert!(r, false);
        return result;
    };

    // Extract the gainmap info and codec.
    let mut gainmap_codec = None;
    let found =
        android_codec.get_gainmap_android_codec(Some(&mut result.info), Some(&mut gainmap_codec));
    reporter_assert!(r, found);
    let Some(mut gainmap_codec) = gainmap_codec else {
        reporter_assert!(r, false);
        return result;
    };

    // Decode the gainmap bitmap.
    let gainmap_info = gainmap_codec.info();
    let gainmap_row_bytes = gainmap_info.min_row_bytes();
    let mut gainmap_pixels = vec![0u8; gainmap_info.compute_byte_size(gainmap_row_bytes)];
    reporter_assert!(
        r,
        CodecResult::Success
            == gainmap_codec.get_android_pixels(
                &gainmap_info,
                &mut gainmap_pixels,
                gainmap_row_bytes,
                None
            )
    );
    result.gainmap = Some((gainmap_info, gainmap_pixels));
    result
}

// Port of: tests/JpegGainmapTest.cpp#L463-L572 (chrome/m156), `AndroidCodec_jpegGainmapDecode`.
def_test!(AndroidCodec_jpegGainmapDecode, |r| {
    struct Rec {
        path: &'static str,
        dimensions: ISize,
        origin_color: u32,
        far_corner_color: u32,
        info: GainmapInfo,
    }
    let recs = [
        Rec {
            path: "images/iphone_13_pro.jpeg",
            dimensions: ISize::new(1512, 2016),
            origin_color: 0xFF3B3B3B,
            far_corner_color: 0xFF101010,
            info: GainmapInfo {
                gainmap_ratio_min: opaque(1.0, 1.0, 1.0),
                gainmap_ratio_max: opaque(3.482202, 3.482202, 3.482202),
                gainmap_gamma: opaque(1.0, 1.0, 1.0),
                epsilon_sdr: opaque(0.0, 0.0, 0.0),
                epsilon_hdr: opaque(0.0, 0.0, 0.0),
                display_ratio_sdr: 1.0,
                display_ratio_hdr: 3.482202,
                base_image_type: BaseImageType::Sdr,
                gainmap_type: GainmapType::Apple,
                gainmap_math_color_space: None,
            },
        },
        Rec {
            path: "images/iphone_15.jpeg",
            dimensions: ISize::new(2016, 1512),
            origin_color: 0xFF5C5C5C,
            far_corner_color: 0xFF656565,
            info: GainmapInfo {
                gainmap_ratio_min: opaque(1.0, 1.0, 1.0),
                gainmap_ratio_max: opaque(3.755272, 3.755272, 3.755272),
                gainmap_gamma: opaque(1.0, 1.0, 1.0),
                epsilon_sdr: opaque(0.0, 0.0, 0.0),
                epsilon_hdr: opaque(0.0, 0.0, 0.0),
                display_ratio_sdr: 1.0,
                display_ratio_hdr: 3.755272,
                base_image_type: BaseImageType::Sdr,
                gainmap_type: GainmapType::Apple,
                gainmap_math_color_space: None,
            },
        },
        Rec {
            path: "images/gainmap_gcontainer_only.jpg",
            dimensions: ISize::new(32, 32),
            origin_color: 0xffffffff,
            far_corner_color: 0xffffffff,
            info: GainmapInfo {
                gainmap_ratio_min: opaque(25.0, 0.5, 1.0),
                gainmap_ratio_max: opaque(2.0, 4.0, 8.0),
                gainmap_gamma: opaque(0.5, 1.0, 2.0),
                epsilon_sdr: opaque(0.01, 0.001, 0.0001),
                epsilon_hdr: opaque(0.0001, 0.001, 0.01),
                display_ratio_sdr: 2.0,
                display_ratio_hdr: 4.0,
                base_image_type: BaseImageType::Sdr,
                gainmap_type: GainmapType::Default,
                gainmap_math_color_space: None,
            },
        },
        Rec {
            path: "images/gainmap_iso21496_1_adobe_gcontainer.jpg",
            dimensions: ISize::new(32, 32),
            origin_color: 0xffffffff,
            far_corner_color: 0xff000000,
            info: GainmapInfo {
                gainmap_ratio_min: opaque(25.0, 0.5, 1.0),
                gainmap_ratio_max: opaque(2.0, 4.0, 8.0),
                gainmap_gamma: opaque(0.5, 1.0, 2.0),
                epsilon_sdr: opaque(0.01, 0.001, 0.0001),
                epsilon_hdr: opaque(0.0001, 0.001, 0.01),
                display_ratio_sdr: 2.0,
                display_ratio_hdr: 4.0,
                base_image_type: BaseImageType::Sdr,
                gainmap_type: GainmapType::Default,
                gainmap_math_color_space: None,
            },
        },
        Rec {
            path: "images/gainmap_iso21496_1.jpg",
            dimensions: ISize::new(32, 32),
            origin_color: 0xffffffff,
            far_corner_color: 0xff000000,
            info: GainmapInfo {
                gainmap_ratio_min: opaque(25.0, 0.5, 1.0),
                gainmap_ratio_max: opaque(2.0, 4.0, 8.0),
                gainmap_gamma: opaque(0.5, 1.0, 2.0),
                epsilon_sdr: opaque(0.01, 0.001, 0.0001),
                epsilon_hdr: opaque(0.0001, 0.001, 0.01),
                display_ratio_sdr: 2.0,
                display_ratio_hdr: 4.0,
                base_image_type: BaseImageType::Hdr,
                gainmap_type: GainmapType::Default,
                gainmap_math_color_space: ColorSpace::new_rgb(
                    &named_transfer_fn::SRGB,
                    &named_gamut::REC2020,
                ),
            },
        },
    ];

    // The stream types of the C++ test. The memory-mapped stream is not a file stream.
    let stream_types = [
        StreamType::Unseekable,
        StreamType::Seekable,
        StreamType::MemoryMapped,
    ];
    for stream_type in stream_types {
        let use_file_stream = stream_type != StreamType::MemoryMapped;
        for rec in &recs {
            let mut file_stream;
            let mut memory_stream;
            let stream: &mut dyn Stream = if use_file_stream {
                let Some(stream) = get_resource_as_stream(rec.path) else {
                    eprintln!("todo: skipping, missing Skia resource {}", rec.path);
                    continue;
                };
                file_stream = stream;
                &mut *file_stream
            } else {
                let Some(data) = get_resource_as_data(rec.path) else {
                    eprintln!("todo: skipping, missing Skia resource {}", rec.path);
                    continue;
                };
                memory_stream = MemoryStream::make(Some(Data::new_copy(&data)));
                &mut *memory_stream
            };
            let mut test_stream = TestStream::new(stream_type, stream);

            let decoded = decode_all(r, &mut test_stream);

            // Spot-check the image size and pixels.
            let Some((gainmap_info, gainmap_pixels)) = decoded.gainmap else {
                reporter_assert!(r, false);
                continue;
            };
            let Some(gainmap_pixmap) =
                Pixmap::new_readonly(&gainmap_info, &gainmap_pixels, gainmap_info.min_row_bytes())
            else {
                reporter_assert!(r, false);
                continue;
            };
            reporter_assert!(r, gainmap_info.dimensions() == rec.dimensions);
            reporter_assert!(
                r,
                gainmap_pixmap.get_color((0, 0)) == Color::from(rec.origin_color)
            );
            reporter_assert!(
                r,
                gainmap_pixmap.get_color((rec.dimensions.width - 1, rec.dimensions.height - 1))
                    == Color::from(rec.far_corner_color)
            );

            // Verify the gainmap rendering parameters.
            expect_approx_eq_info(r, &rec.info, &decoded.info);
        }
    }
});

// Port of: tests/JpegGainmapTest.cpp#L574-L611 (chrome/m156), `AndroidCodec_jpegNoGainmap`.
def_test!(AndroidCodec_jpegNoGainmap, |r| {
    // This test image has a large APP16 segment that will stress the various SkJpegSourceMgrs'
    // data skipping paths.
    let path = "images/icc-v2-gbr.jpg";
    let stream_types = [
        StreamType::Unseekable,
        StreamType::Seekable,
        StreamType::MemoryMapped,
    ];
    for stream_type in stream_types {
        let use_file_stream = stream_type != StreamType::MemoryMapped;
        let mut file_stream;
        let mut memory_stream;
        let stream: &mut dyn Stream = if use_file_stream {
            let Some(stream) = get_resource_as_stream(path) else {
                eprintln!("todo: skipping, missing Skia resource {path}");
                continue;
            };
            file_stream = stream;
            &mut *file_stream
        } else {
            let Some(data) = get_resource_as_data(path) else {
                eprintln!("todo: skipping, missing Skia resource {path}");
                continue;
            };
            memory_stream = MemoryStream::make(Some(Data::new_copy(&data)));
            &mut *memory_stream
        };
        let mut test_stream = TestStream::new(stream_type, stream);
        let data = read_all(&mut test_stream);

        // Decode the base bitmap.
        let Ok(mut base_codec) =
            jpeg_codec::make_from_stream(MemoryStream::make(Some(Data::new_copy(&data))))
        else {
            reporter_assert!(r, false);
            continue;
        };
        let base_info = base_codec.info().clone();
        let base_row_bytes = base_info.min_row_bytes();
        let mut base_pixels = vec![0u8; base_info.compute_byte_size(base_row_bytes)];
        reporter_assert!(
            r,
            CodecResult::Success
                == base_codec.get_pixels(&base_info, &mut base_pixels, base_row_bytes, None)
        );

        let Some(android_codec) = AndroidCodec::make_from_codec(base_codec) else {
            reporter_assert!(r, false);
            continue;
        };

        // Try to extract the gainmap info and stream. It should fail.
        let mut gainmap_info = GainmapInfo::default();
        let mut gainmap_stream = None;
        reporter_assert!(
            r,
            !android_codec.get_android_gainmap(Some(&mut gainmap_info), Some(&mut gainmap_stream))
        );
    }
});

/// An image's info and zeroed pixels, with the minimum row bytes.
fn zeroed_image(info: ImageInfo) -> (ImageInfo, Vec<u8>) {
    let row_bytes = info.min_row_bytes();
    let pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    (info, pixels)
}

// Port of: tests/JpegGainmapTest.cpp#L615-L702 (chrome/m156), `AndroidCodec_gainmapInfoEncode`.
def_test!(AndroidCodec_gainmapInfoEncode, |r| {
    let base = zeroed_image(ImageInfo::new(
        (16, 16),
        ColorType::N32,
        AlphaType::Premul,
        None,
    ));
    let gainmaps = [
        zeroed_image(ImageInfo::new(
            (16, 16),
            ColorType::N32,
            AlphaType::Premul,
            None,
        )),
        zeroed_image(ImageInfo::new(
            (8, 8),
            ColorType::N32,
            AlphaType::Premul,
            None,
        )),
        zeroed_image(ImageInfo::new(
            (4, 4),
            ColorType::Alpha8,
            AlphaType::Premul,
            None,
        )),
        zeroed_image(ImageInfo::new(
            (8, 8),
            ColorType::Gray8,
            AlphaType::Premul,
            None,
        )),
    ];
    let display_p3 = || ColorSpace::new_rgb(&named_transfer_fn::SRGB, &named_gamut::DISPLAY_P3);
    let infos = [
        // Multi-channel, UltraHDR-compatible.
        GainmapInfo {
            gainmap_ratio_min: opaque(1.0, 2.0, 4.0),
            gainmap_ratio_max: opaque(8.0, 16.0, 32.0),
            gainmap_gamma: opaque(64.0, 128.0, 256.0),
            epsilon_sdr: opaque(1.0 / 10.0, 1.0 / 11.0, 1.0 / 12.0),
            epsilon_hdr: opaque(1.0 / 13.0, 1.0 / 14.0, 1.0 / 15.0),
            display_ratio_sdr: 4.0,
            display_ratio_hdr: 32.0,
            base_image_type: BaseImageType::Sdr,
            gainmap_type: GainmapType::Default,
            gainmap_math_color_space: None,
        },
        // Multi-channel, not UltraHDR-compatible.
        GainmapInfo {
            gainmap_ratio_min: opaque(1.0, 2.0, 4.0),
            gainmap_ratio_max: opaque(8.0, 16.0, 32.0),
            gainmap_gamma: opaque(64.0, 128.0, 256.0),
            epsilon_sdr: opaque(1.0 / 10.0, 1.0 / 11.0, 1.0 / 12.0),
            epsilon_hdr: opaque(1.0 / 13.0, 1.0 / 14.0, 1.0 / 15.0),
            display_ratio_sdr: 4.0,
            display_ratio_hdr: 32.0,
            base_image_type: BaseImageType::Sdr,
            gainmap_type: GainmapType::Default,
            gainmap_math_color_space: display_p3(),
        },
        // Single-channel, UltraHDR-compatible.
        GainmapInfo {
            gainmap_ratio_min: opaque(1.0, 1.0, 1.0),
            gainmap_ratio_max: opaque(8.0, 8.0, 8.0),
            gainmap_gamma: opaque(64.0, 64.0, 64.0),
            epsilon_sdr: opaque(1.0 / 128.0, 1.0 / 128.0, 1.0 / 128.0),
            epsilon_hdr: opaque(1.0 / 256.0, 1.0 / 256.0, 1.0 / 256.0),
            display_ratio_sdr: 4.0,
            display_ratio_hdr: 32.0,
            base_image_type: BaseImageType::Sdr,
            gainmap_type: GainmapType::Default,
            gainmap_math_color_space: None,
        },
        // Single-channel, not UltraHDR-compatible.
        GainmapInfo {
            gainmap_ratio_min: opaque(1.0, 1.0, 1.0),
            gainmap_ratio_max: opaque(8.0, 8.0, 8.0),
            gainmap_gamma: opaque(64.0, 64.0, 64.0),
            epsilon_sdr: opaque(1.0 / 128.0, 1.0 / 128.0, 1.0 / 128.0),
            epsilon_hdr: opaque(1.0 / 256.0, 1.0 / 256.0, 1.0 / 256.0),
            display_ratio_sdr: 4.0,
            display_ratio_hdr: 32.0,
            base_image_type: BaseImageType::Hdr,
            gainmap_type: GainmapType::Default,
            gainmap_math_color_space: display_p3(),
        },
    ];

    let base_pixmap = Pixmap::new_readonly(&base.0, &base.1, base.0.min_row_bytes());
    for (gainmap, info) in gainmaps.iter().zip(infos.iter()) {
        // Encode `info`.
        let gainmap_pixmap =
            Pixmap::new_readonly(&gainmap.0, &gainmap.1, gainmap.0.min_row_bytes());
        let (Some(base_pixmap), Some(gainmap_pixmap)) = (&base_pixmap, &gainmap_pixmap) else {
            reporter_assert!(r, false);
            return;
        };
        let mut encoded = Vec::new();
        let encode_result = jpeg_gainmap_encoder::encode_hdrgm(
            &mut encoded,
            base_pixmap,
            &JpegEncoderOptions::default(),
            gainmap_pixmap,
            &JpegEncoderOptions::default(),
            info,
        );
        reporter_assert!(r, encode_result);

        // Decode into `decoded_gainmap_info`.
        let mut decode_stream = MemoryStream::make(Some(Data::new_copy(&encoded)));
        let decoded = decode_all(r, &mut *decode_stream);
        expect_approx_eq_info(r, info, &decoded.info);
    }
});

/// Renders one pixel of a gainmap applied to a base image (`render_gainmap`, then the pixel of its
/// 1x1 result). The sizes are converted to float, as in the C++.
// Port of: tests/JpegGainmapTest.cpp#L705-L752 (chrome/m156), render_gainmap and
// render_gainmap_pixel
#[allow(clippy::cast_precision_loss)] // the image sizes are converted to float, as in the C++
fn render_gainmap_pixel(
    render_hdr_ratio: f32,
    base: &(ImageInfo, Vec<u8>),
    gainmap: &(ImageInfo, Vec<u8>),
    gainmap_info: &GainmapInfo,
    x: f32,
    y: f32,
) -> Option<Color4f> {
    let (base_info, base_pixels) = base;
    let (gainmap_image_info, gainmap_pixels) = gainmap;
    let base_rect = Rect::from_xywh(x, y, 1.0, 1.0);
    let scale_x = gainmap_image_info.width() as f32 / base_info.width() as f32;
    let scale_y = gainmap_image_info.height() as f32 / base_info.height() as f32;
    let gainmap_rect = Rect::from_xywh(
        base_rect.x() * scale_x,
        base_rect.y() * scale_y,
        base_rect.width() * scale_x,
        base_rect.height() * scale_y,
    );
    let dst_rect = Rect::from_xywh(0.0, 0.0, 1.0, 1.0);

    let base_image = images::raster_from_pixmap_copy(&Pixmap::new_readonly(
        base_info,
        base_pixels,
        base_info.min_row_bytes(),
    )?)?;
    let gainmap_image = images::raster_from_pixmap_copy(&Pixmap::new_readonly(
        gainmap_image_info,
        gainmap_pixels,
        gainmap_image_info.min_row_bytes(),
    )?)?;
    let shader = GainmapShader::make(
        &base_image,
        &base_rect,
        SamplingOptions::default(),
        &gainmap_image,
        &gainmap_rect,
        SamplingOptions::default(),
        gainmap_info,
        &dst_rect,
        render_hdr_ratio,
    )?;
    let mut paint = Paint::default();
    paint.set_shader(shader);

    let render_info = ImageInfo::new(
        (1, 1),
        ColorType::RGBAF16,
        AlphaType::Premul,
        Some(ColorSpace::new_srgb()),
    );
    let mut canvas_bitmap = Bitmap::new();
    canvas_bitmap.alloc_pixels_info(&render_info, None);
    canvas_bitmap.erase_color(Color::TRANSPARENT);
    {
        let canvas = Canvas::from_bitmap(&mut canvas_bitmap, None)?;
        canvas.draw_rect(dst_rect, &paint);
    }
    Some(canvas_bitmap.get_color_4f((0, 0)))
}

/// One pixel to render in the transcode test (the C++ `Rec`).
struct TranscodeRec {
    x: f32,
    y: f32,
    hdr_ratio: f32,
    /// The colour the C++ record expects. The C++ test does not compare it, and neither does this.
    #[allow(dead_code)]
    expected_color: Color4f,
    /// The colour type the gainmap is forced to, or `None` for its own.
    forced_color_type: Option<ColorType>,
}

// Port of: tests/JpegGainmapTest.cpp#L765-L839 (chrome/m156), `AndroidCodec_jpegGainmapTranscode`.
def_test!(AndroidCodec_jpegGainmapTranscode, |r| {
    let path = "images/iphone_13_pro.jpeg";
    let epsilon = 1e-2f32;

    // Decode an MPF-based gainmap image.
    let Some(mut stream) = get_resource_as_stream(path) else {
        eprintln!("todo: skipping, missing Skia resource {path}");
        return;
    };
    let mut decoded = vec![decode_all(r, &mut *stream)];

    // This test was written before SkGainmapShader added support for kApple type. Strip the type.
    decoded[0].info.gainmap_type = GainmapType::Default;

    // Transcode to UltraHDR.
    let (Some(base), Some(gainmap)) = (&decoded[0].base, &decoded[0].gainmap) else {
        reporter_assert!(r, false);
        return;
    };
    let base_pixmap = Pixmap::new_readonly(&base.0, &base.1, base.0.min_row_bytes());
    let gainmap_pixmap = Pixmap::new_readonly(&gainmap.0, &gainmap.1, gainmap.0.min_row_bytes());
    let (Some(base_pixmap), Some(gainmap_pixmap)) = (&base_pixmap, &gainmap_pixmap) else {
        reporter_assert!(r, false);
        return;
    };
    let mut encoded = Vec::new();
    let encode_result = jpeg_gainmap_encoder::encode_hdrgm(
        &mut encoded,
        base_pixmap,
        &JpegEncoderOptions::default(),
        gainmap_pixmap,
        &JpegEncoderOptions::default(),
        &decoded[0].info,
    );
    reporter_assert!(r, encode_result);

    // Decode the just-encoded image.
    let mut decode_stream = MemoryStream::make(Some(Data::new_copy(&encoded)));
    decoded.push(decode_all(r, &mut *decode_stream));

    // HDRGM will have the same rendering parameters.
    expect_approx_eq_info(r, &decoded[0].info, &decoded[1].info);

    // Render a few pixels and verify that they come out the same.
    let recs = [
        TranscodeRec {
            x: 1446.0,
            y: 1603.0,
            hdr_ratio: 1.05,
            expected_color: opaque(0.984375, 1.004883, 1.008789),
            forced_color_type: None,
        },
        TranscodeRec {
            x: 1446.0,
            y: 1603.0,
            hdr_ratio: 100.0,
            expected_color: opaque(1.147461, 1.170898, 1.174805),
            forced_color_type: None,
        },
        TranscodeRec {
            x: 1446.0,
            y: 1603.0,
            hdr_ratio: 100.0,
            expected_color: opaque(1.147461, 1.170898, 1.174805),
            forced_color_type: Some(ColorType::Gray8),
        },
        TranscodeRec {
            x: 1446.0,
            y: 1603.0,
            hdr_ratio: 100.0,
            expected_color: opaque(1.147461, 1.170898, 1.174805),
            forced_color_type: Some(ColorType::Alpha8),
        },
        TranscodeRec {
            x: 1446.0,
            y: 1603.0,
            hdr_ratio: 100.0,
            expected_color: opaque(1.147461, 1.170898, 1.174805),
            forced_color_type: Some(ColorType::R8UNorm),
        },
    ];

    let (Some(base0), Some(gainmap0)) = (&decoded[0].base, &decoded[0].gainmap) else {
        reporter_assert!(r, false);
        return;
    };
    let (Some(base1), Some(gainmap1)) = (&decoded[1].base, &decoded[1].gainmap) else {
        reporter_assert!(r, false);
        return;
    };
    for rec in &recs {
        // Force various different single-channel formats, to ensure that they all work. When the
        // colour type is forced to Alpha8 the shader reads (0,0,0,1) for an opaque alpha type.
        let forced_gainmap0 = match rec.forced_color_type {
            None => gainmap0.clone(),
            Some(color_type) => (
                gainmap0
                    .0
                    .with_color_type(color_type)
                    .with_alpha_type(AlphaType::Premul),
                gainmap0.1.clone(),
            ),
        };
        let p0 = render_gainmap_pixel(
            rec.hdr_ratio,
            base0,
            &forced_gainmap0,
            &decoded[0].info,
            rec.x,
            rec.y,
        );
        let p1 = render_gainmap_pixel(
            rec.hdr_ratio,
            base1,
            gainmap1,
            &decoded[1].info,
            rec.x,
            rec.y,
        );
        let (Some(p0), Some(p1)) = (p0, p1) else {
            reporter_assert!(r, false);
            continue;
        };
        reporter_assert!(r, approx_eq_color(p0, p1, epsilon));
    }
});

/// The image file directory of the segment with `marker` whose parameters begin with `sig`, after
/// `pad` bytes (`get_ifd`).
// Port of: tests/JpegGainmapTest.cpp#L857-L883 (chrome/m156), get_ifd
fn get_ifd(image: &[u8], marker: u32, sig: &[u8], pad: usize) -> Option<ImageFileDirectory> {
    let mut stream = MemoryStream::make(Some(Data::new_copy(image)));
    let mut source_mgr = JpegSourceMgr::make(&mut *stream, 1024);
    let segments = source_mgr.get_all_segments().to_vec();
    for segment in segments {
        if u32::from(segment.marker) != marker {
            continue;
        }
        let Some(parameter_data) = source_mgr.get_segment_parameters(&segment) else {
            continue;
        };
        let bytes = parameter_data.as_bytes();
        if bytes.len() < sig.len() || &bytes[..sig.len()] != sig {
            continue;
        }
        let ifd_data = &bytes[sig.len() + pad..];
        let (little_endian, ifd_offset) = parse_header(ifd_data)?;
        return ImageFileDirectory::make_from_offset(
            Arc::from(ifd_data),
            little_endian,
            ifd_offset,
            false,
        );
    }
    None
}

/// The Multi-Picture directory of `image` (`get_mpf_ifd`).
// Port of: tests/JpegGainmapTest.cpp#L885-L887 (chrome/m156), get_mpf_ifd
fn get_mpf_ifd(image: &[u8]) -> Option<ImageFileDirectory> {
    get_ifd(image, MPF_MARKER, MPF_SIG, 0)
}

// Port of: src/codec/SkJpegConstants.h#L55-L56 (chrome/m156), `kExifMarker`, `kExifSig`
const EXIF_MARKER: u32 = 0xE0 + 1;
const EXIF_SIG: &[u8] = b"Exif\0";

/// The Exif directory of `image` (`get_exif_ifd`).
// Port of: tests/JpegGainmapTest.cpp#L889-L891 (chrome/m156), get_exif_ifd
fn get_exif_ifd(image: &[u8]) -> Option<ImageFileDirectory> {
    get_ifd(image, EXIF_MARKER, EXIF_SIG, 1)
}

/// The bytes of image `image_number` of the Multi-Picture file `image` (`get_mp_image`).
// Port of: tests/JpegGainmapTest.cpp#L841-L856 (chrome/m156), get_mp_image
fn get_mp_image(image: &[u8], image_number: usize) -> Option<Vec<u8>> {
    let mut stream = MemoryStream::make(Some(Data::new_copy(image)));
    let (mp_params, mp_params_segment) = find_mp_params_segment(&mut *stream)?;
    let entry = mp_params.images.get(image_number)?;
    let offset = MultiPictureParameters::get_image_absolute_offset(
        entry.data_offset,
        mp_params_segment.offset,
    );
    let size = usize::try_from(entry.size).ok()?;
    image
        .get(offset..offset.checked_add(size)?)
        .map(<[u8]>::to_vec)
}

// Port of: tests/JpegGainmapTest.cpp#L893-L989 (chrome/m156), `AndroidCodec_mpfParse`.
def_test!(AndroidCodec_mpfParse, |r| {
    let path = "images/iphone_13_pro.jpeg";
    let Some(mut input_data) = get_resource_as_data(path) else {
        eprintln!("todo: skipping, missing Skia resource {path}");
        return;
    };

    {
        // The MPF in iPhone images has 3 entries: version, image count, and the MP entries.
        let Some(ifd) = get_mpf_ifd(&input_data) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, ifd.num_entries() == 3);
        reporter_assert!(r, ifd.entry_tag(0) == 0xB000);
        reporter_assert!(r, ifd.entry_tag(1) == 0xB001);
        reporter_assert!(r, ifd.entry_tag(2) == 0xB002);

        // There is no attribute IFD.
        reporter_assert!(r, ifd.next_ifd_offset() == 0);
    }

    {
        // The gainmap images have version and image count.
        let Some(image) = get_mp_image(&input_data, 1) else {
            reporter_assert!(r, false);
            return;
        };
        let Some(ifd) = get_mpf_ifd(&image) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, ifd.num_entries() == 2);
        reporter_assert!(r, ifd.entry_tag(0) == 0xB000);
        reporter_assert!(r, ifd.entry_tag(1) == 0xB001);
        reporter_assert!(r, ifd.entry_unsigned_long(1, 1) == Some(vec![3]));

        // There is no further IFD.
        reporter_assert!(r, ifd.next_ifd_offset() == 0);
    }

    // Replace |input_data| with its transcoded version.
    {
        let Some(mut stream) = get_resource_as_stream(path) else {
            reporter_assert!(r, false);
            return;
        };
        let mut decoded = decode_all(r, &mut *stream);
        decoded.info.gainmap_type = GainmapType::Default;
        let (Some(base), Some(gainmap)) = (&decoded.base, &decoded.gainmap) else {
            reporter_assert!(r, false);
            return;
        };
        let base_pixmap = Pixmap::new_readonly(&base.0, &base.1, base.0.min_row_bytes());
        let gainmap_pixmap =
            Pixmap::new_readonly(&gainmap.0, &gainmap.1, gainmap.0.min_row_bytes());
        let (Some(base_pixmap), Some(gainmap_pixmap)) = (&base_pixmap, &gainmap_pixmap) else {
            reporter_assert!(r, false);
            return;
        };
        let mut encoded = Vec::new();
        let encode_result = jpeg_gainmap_encoder::encode_hdrgm(
            &mut encoded,
            base_pixmap,
            &JpegEncoderOptions::default(),
            gainmap_pixmap,
            &JpegEncoderOptions::default(),
            &decoded.info,
        );
        reporter_assert!(r, encode_result);
        input_data = encoded;
    }

    {
        // Exif should be present and valid.
        let Some(ifd) = get_exif_ifd(&input_data) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, ifd.num_entries() == 1);
        // The sub-IFD offset tag.
        reporter_assert!(r, ifd.entry_tag(0) == 0x8769);
    }

    {
        // The MPF in encoded images has 3 entries: version, image count, and the MP entries.
        let Some(ifd) = get_mpf_ifd(&input_data) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, ifd.num_entries() == 3);
        reporter_assert!(r, ifd.entry_tag(0) == 0xB000);
        reporter_assert!(r, ifd.entry_tag(1) == 0xB001);
        reporter_assert!(r, ifd.entry_tag(2) == 0xB002);

        // There is no attribute IFD.
        reporter_assert!(r, ifd.next_ifd_offset() == 0);
    }

    {
        // The MPF in encoded gainmap images has 2 entries: Version and number of images.
        let Some(image) = get_mp_image(&input_data, 1) else {
            reporter_assert!(r, false);
            return;
        };
        let Some(ifd) = get_mpf_ifd(&image) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, ifd.num_entries() == 1);
        reporter_assert!(r, ifd.entry_tag(0) == 0xB000);

        // Verify the version data (don't verify the version in the primary image, because if that
        // were broken all MPF images would be broken).
        let Some(version_data) = ifd.entry_undefined_data(0) else {
            reporter_assert!(r, false);
            return;
        };
        reporter_assert!(r, version_data[0] == b'0');
        reporter_assert!(r, version_data[1] == b'1');
        reporter_assert!(r, version_data[2] == b'0');
        reporter_assert!(r, version_data[3] == b'0');

        // There is no further IFD.
        reporter_assert!(r, ifd.next_ifd_offset() == 0);
    }
});
