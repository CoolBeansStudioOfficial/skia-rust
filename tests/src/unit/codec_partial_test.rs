// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecPartialTest.cpp (chrome/m156), the cases that decode PNG only. The other
// cases decode GIF, WebP, JPEG, BMP and WBMP files, and the incremental-stream helpers they use
// (`HaltingStream`, `test_partial`) are ported with them.

use std::sync::{Arc, Mutex};

use skia_rust_codec::{Codec, Options, Result, codecs, decoders};
use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::Reporter;
use crate::resources::get_resource_as_data;
use crate::unit::codec_test::Pixels;
use crate::{def_test, errorf, reporter_assert, skip_missing_resource};

// Port of: tests/CodecPartialTest.cpp#L29-L34 (standardize_info): the N32 premultiplied info of
// the image, without a colour space.
fn standardize_info(codec: &Codec<'_>) -> ImageInfo {
    let dims = codec.dimensions();
    ImageInfo::new(
        dims,
        ColorType::N32,
        AlphaType::Premul,
        None::<skia_rust_core::color_space::ColorSpace>,
    )
}

// Port of: tests/CodecPartialTest.cpp#L452-L474 (chrome/m156)
def_test!(Codec_emptyIDAT, |r| {
    let name = "images/baby_tux.png";
    let file = skip_missing_resource!(get_resource_as_data(name), name);
    // Truncate to the beginning of the IDAT, immediately after the IDAT tag.
    let truncated = &file[..80.min(file.len())];
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(truncated), decoders())
    else {
        reporter_assert!(r, false);
        return;
    };
    let info = standardize_info(&codec);
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
    let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
    reporter_assert!(r, result == Result::IncompleteInput);
});

// Port of: tests/FakeStreams.h#L61-L100 (chrome/m156), HaltingStream: a stream whose readable
// length is `limit`, which the test raises with add_new_data. The C++ test keeps a pointer to the
// stream after handing ownership to the codec, so the state is shared through an Arc.
#[derive(Clone)]
pub(crate) struct HaltingStream(Arc<Mutex<HaltingState>>);

struct HaltingState {
    total_size: usize,
    limit: usize,
    stream: MemoryStream,
}

impl HaltingStream {
    // Port of: HaltingStream(sk_sp<SkData> data, size_t initialLimit)
    pub(crate) fn new(data: &[u8], initial_limit: usize) -> Self {
        Self(Arc::new(Mutex::new(HaltingState {
            total_size: data.len(),
            limit: initial_limit,
            stream: *MemoryStream::make_copy(data),
        })))
    }

    fn with<R>(&self, f: impl FnOnce(&mut HaltingState) -> R) -> R {
        f(&mut self.0.lock().expect("stream lock"))
    }

    // Port of: HaltingStream::addNewData(size_t extra)
    pub(crate) fn add_new_data(&self, extra: usize) {
        self.with(|s| s.limit = s.total_size.min(s.limit + extra));
    }
}

impl Stream for HaltingStream {
    // Port of: HaltingStream::read(void* buffer, size_t size)
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.with(|s| {
            let mut size = buffer.len();
            if s.stream.get_position() + size > s.limit {
                size = s.limit - s.stream.get_position();
            }
            s.stream.read(&mut buffer[..size])
        })
    }

    fn is_at_end(&self) -> bool {
        self.with(|s| s.stream.is_at_end())
    }

    fn has_length(&self) -> bool {
        true
    }

    fn get_length(&self) -> usize {
        self.with(|s| s.limit)
    }

    fn has_position(&self) -> bool {
        true
    }

    fn get_position(&self) -> usize {
        self.with(|s| s.stream.get_position())
    }

    fn rewind(&mut self) -> bool {
        self.with(|s| s.stream.rewind())
    }

    fn move_by(&mut self, offset: i64) -> bool {
        self.with(|s| s.stream.move_by(offset))
    }

    fn seek(&mut self, position: usize) -> bool {
        self.with(|s| s.stream.seek(position))
    }
}

// Port of: tests/CodecPartialTest.cpp#L76-L90 (chrome/m156), compare_bitmaps
fn compare_bitmaps(reporter: &mut Reporter, bm1: &Pixels, bm2: &Pixels) -> bool {
    let info = &bm1.info;
    if *info != bm2.info {
        errorf!(reporter, "Bitmaps have different image infos!");
        return false;
    }
    let row_bytes = info.min_row_bytes();
    for i in 0..info.height() {
        let start = usize::try_from(i).unwrap_or(0) * row_bytes;
        if bm1.data[start..start + row_bytes] != bm2.data[start..start + row_bytes] {
            errorf!(
                reporter,
                "Bitmaps have different pixels, starting on line {}!",
                i
            );
            return false;
        }
    }
    true
}

// Port of: tests/CodecPartialTest.cpp#L227-L326 (chrome/m156)
def_test!(Codec_partialAnim, |r| {
    let path = "images/test640x479.gif";
    let file = skip_missing_resource!(get_resource_as_data(path), path);

    // This stream will be owned by fullCodec, but we hang on to the pointer to determine frame
    // offsets.
    let Ok(mut full_codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&file)) else {
        reporter_assert!(r, false);
        return;
    };
    let info = standardize_info(&full_codec);

    // frameByteCounts stores the number of bytes to decode a particular frame.
    // - [0] is the number of bytes for the header
    // - frames[i] requires frameByteCounts[i+1] bytes to decode
    let frame_byte_counts: [usize; 5] = [455, 69350, 1344, 1346, 1327];
    let mut frames: Vec<Pixels> = Vec::new();
    let mut i: usize = 0;
    loop {
        let mut frame = Pixels::alloc(&info);
        let opts = Options {
            frame_index: i32::try_from(i).unwrap_or(i32::MAX),
            ..Options::default()
        };
        let row_bytes = frame.row_bytes;
        let result = full_codec.get_pixels(&info, &mut frame.data, row_bytes, Some(&opts));
        if result == Result::IncompleteInput || result == Result::InvalidInput {
            // We need to distinguish between a partial frame and no more frames. getFrameInfo lets
            // us do this, since it tells the number of frames not considering whether they are
            // complete.
            if full_codec.frame_infos().len() > i {
                // This is a partial frame.
                frames.push(frame);
            }
            break;
        }
        if result != Result::Success {
            errorf!(r, "Failed to decode frame {} from {}", i, path);
            return;
        }
        frames.push(frame);
        i += 1;
    }

    // Now decode frames partially, then completely, and compare to the original.
    let halting = HaltingStream::new(&file, frame_byte_counts[0]);
    let Ok(mut partial_codec) = codecs::make_codec_from_stream(Box::new(halting.clone())) else {
        errorf!(
            r,
            "Failed to create a partial codec from {} with {} bytes out of {}",
            path,
            frame_byte_counts[0],
            file.len()
        );
        return;
    };

    for (i, expected) in frames.iter().enumerate() {
        let full_frame_bytes = frame_byte_counts[i + 1];
        let first_half = full_frame_bytes / 2;
        let second_half = full_frame_bytes - first_half;
        halting.add_new_data(first_half);
        let frame_info = partial_codec.frame_infos();
        reporter_assert!(r, frame_info.len() == i + 1);
        reporter_assert!(r, !frame_info[i].fully_received);

        let mut frame = Pixels::alloc(&info);
        let row_bytes = frame.row_bytes;
        let opts = Options {
            frame_index: i32::try_from(i).unwrap_or(i32::MAX),
            ..Options::default()
        };
        match partial_codec.start_incremental_decode(&info, &mut frame.data, row_bytes, Some(&opts))
        {
            Ok(mut incremental) => {
                let (result, _) = incremental.incremental_decode();
                reporter_assert!(r, result == Result::IncompleteInput);
                halting.add_new_data(second_half);
                let (result, _) = incremental.incremental_decode();
                reporter_assert!(r, result == Result::Success);
            }
            Err(result) => {
                // The C++ returns here after reporting the failure to start.
                errorf!(
                    r,
                    "Failed to start incremental decode for {} on frame {} with {}",
                    path,
                    i,
                    result.as_str()
                );
                return;
            }
        }
        let frame_info = partial_codec.frame_infos();
        reporter_assert!(r, frame_info.len() == i + 1);
        reporter_assert!(r, frame_info[i].fully_received);

        if !compare_bitmaps(r, expected, &frame) {
            // C++: ERRORF(r, "\tfailure was on frame %zu", i), then write_bm dumps both frames to
            // PNG files for debugging. skia-rust: not expressible in Rust: the file dump is a
            // debugging aid, and the mismatch is reported above.
            errorf!(r, "\tfailure was on frame {}", i);
        }
    }
});

// Verify that when decoding an animated gif byte by byte we report the correct fRequiredFrame as
// soon as getFrameInfo reports the frame.
// Port of: tests/CodecPartialTest.cpp#L176-L225 (chrome/m156)
def_test!(Codec_requiredFrame, |r| {
    let path = "images/colorTables.gif";
    let file = skip_missing_resource!(get_resource_as_data(path), path);

    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(&file)) else {
        errorf!(r, "Failed to create codec from {}", path);
        return;
    };
    let frame_info = codec.frame_infos();
    if frame_info.len() <= 1 {
        errorf!(r, "Test is uninteresting with 0 or 1 frames");
        return;
    }

    // Find the shortest prefix that yields a partial codec.
    let mut i: usize = 0;
    let (halting, mut partial_codec) = loop {
        if file.len() == i {
            errorf!(r, "Should have created a partial codec for {}", path);
            return;
        }
        let halting = HaltingStream::new(&file, i);
        if let Ok(partial_codec) = codecs::make_codec_from_stream(Box::new(halting.clone())) {
            break (halting, partial_codec);
        }
        i += 1;
    };

    let mut frame_to_compare: usize = 0;
    loop {
        let partial_info = partial_codec.frame_infos();
        while frame_to_compare < partial_info.len() {
            reporter_assert!(
                r,
                partial_info[frame_to_compare].required_frame
                    == frame_info[frame_to_compare].required_frame
            );
            frame_to_compare += 1;
        }
        if frame_to_compare == frame_info.len() {
            break;
        }
        if halting.get_length() == file.len() {
            errorf!(r, "Should have found all frames for {}", path);
            return;
        }
        halting.add_new_data(1);
    }
});

// Port of: tests/CodecPartialTest.cpp#L399-L450 (chrome/m156)
def_test!(Codec_GifPreMap, |r| {
    // Modified version of the giflib logo; the global color map is replaced with a local one.
    let data: &[u8] = &GIF_NO_GLOBAL_COLOR_MAP;
    let Ok(mut codec) = codecs::make_codec_from_stream(MemoryStream::make_copy(data)) else {
        errorf!(r, "failed to create codec");
        return;
    };

    let info = standardize_info(&codec);
    let mut truth = Pixels::alloc(&info);
    let row_bytes = truth.row_bytes;
    let result = codec.get_pixels(&info, &mut truth.data, row_bytes, None);
    reporter_assert!(r, result == Result::Success);

    // Truncate to 23 bytes, just before the color map. This should fail to decode.
    //
    // See also Codec_GifTruncated2 in GifTest.cpp for this magic 23.
    let truncated = codecs::make_codec_from_stream(MemoryStream::make_copy(&data[..23]));
    reporter_assert!(r, truncated.is_ok());
    if let Ok(mut codec) = truncated {
        let mut bm = Pixels::alloc(&info);
        let row_bytes = bm.row_bytes;
        let result = codec.get_pixels(&info, &mut bm.data, row_bytes, None);
        reporter_assert!(r, result == Result::IncompleteInput);
    }

    // Again, truncate to 23 bytes, this time for an incremental decode. We cannot start an
    // incremental decode until we have more data. If we did, we would be using the wrong color
    // table.
    let halting = HaltingStream::new(data, 23);
    let codec = codecs::make_codec_from_stream(Box::new(halting.clone()));
    reporter_assert!(r, codec.is_ok());
    if let Ok(mut codec) = codec {
        let mut bm = Pixels::alloc(&info);
        let row_bytes = bm.row_bytes;
        match codec.start_incremental_decode(&info, &mut bm.data, row_bytes, None) {
            Ok(mut incremental) => {
                // Note that this is incrementalDecode, not startIncrementalDecode.
                let (result, _) = incremental.incremental_decode();
                reporter_assert!(r, result == Result::IncompleteInput);
                halting.add_new_data(data.len());
                let (result, _) = incremental.incremental_decode();
                reporter_assert!(r, result == Result::Success);
            }
            Err(result) => reporter_assert!(r, result == Result::Success),
        }
        compare_bitmaps(r, &truth, &bm);
    }
});

// Port of: tests/CodecPartialTest.cpp#L476-L509 (chrome/m156)
def_test!(Codec_incomplete, |r| {
    let names = [
        "images/baby_tux.png",
        "images/baby_tux.webp",
        "images/CMYK.jpg",
        "images/color_wheel.gif",
        "images/google_chrome.ico",
        "images/rle.bmp",
        "images/mandrill.wbmp",
    ];
    for name in names {
        let Some(file) = get_resource_as_data(name) else {
            continue;
        };
        let mut len: usize = 14;
        while len <= file.len() {
            match codecs::make_codec_from_stream(MemoryStream::make_copy(&file[..len])) {
                // A codec was created, so the result is kSuccess (MakeFromStream's outResult).
                Ok(_) => break,
                Err(result) => {
                    if result != Result::IncompleteInput {
                        errorf!(
                            r,
                            "Reported error {} for {} with {} bytes",
                            result.as_str(),
                            name,
                            len
                        );
                        break;
                    }
                }
            }
            len += 5;
        }
    }
});

// Port of: tests/CodecPartialTest.cpp#L377-L396 (gNoGlobalColorMap), the data of Codec_GifPreMap.
// Modified version of the giflib logo, from
// http://giflib.sourceforge.net/whatsinagif/bits_and_bytes.html
// The global color map has been replaced with a local color map.
const GIF_NO_GLOBAL_COLOR_MAP: [u8; 61] = [
    // Header
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, // Logical screen descriptor
    0x0A, 0x00, 0x0A, 0x00, 0x11, 0x00, 0x00, // Image descriptor
    0x2C, 0x00, 0x00, 0x00, 0x00, 0x0A, 0x00, 0x0A, 0x00, 0x81, // Local color table
    0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x00, 0xFF, 0x00, 0x00, 0x00,
    // Image data
    0x02, 0x16, 0x8C, 0x2D, 0x99, 0x87, 0x2A, 0x1C, 0xDC, 0x33, 0xA0, 0x02, 0x75, 0xEC, 0x95, 0xFA,
    0xA8, 0xDE, 0x60, 0x8C, 0x04, 0x91, 0x4C, 0x01, 0x00, // Trailer
    0x3B,
];
