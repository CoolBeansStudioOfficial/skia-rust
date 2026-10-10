// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/CodecExactReadTest.cpp (chrome/m156), the cases for the PNG, WBMP and BMP decoders.

use std::sync::{Arc, Mutex};

use skia_rust_codec::Result;
use skia_rust_codec::codecs;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::stream::{MemoryStream, Stream};

use crate::resources::get_resource_as_data;
use crate::{def_test, reporter_assert};

// The C++ test wraps one SkMemoryStream in an UnowningStream for every codec, so each codec starts
// where the previous one stopped. This handle shares the stream the same way, and forwards every
// call to it.
pub(crate) struct SharedStream(pub(crate) Arc<Mutex<MemoryStream>>);

impl Stream for SharedStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        self.0.lock().expect("stream lock").read(buffer)
    }

    fn skip(&mut self, size: usize) -> usize {
        self.0.lock().expect("stream lock").skip(size)
    }

    fn peek(&mut self, buffer: &mut [u8]) -> usize {
        self.0.lock().expect("stream lock").peek(buffer)
    }

    fn is_at_end(&self) -> bool {
        self.0.lock().expect("stream lock").is_at_end()
    }

    fn rewind(&mut self) -> bool {
        self.0.lock().expect("stream lock").rewind()
    }

    fn has_position(&self) -> bool {
        self.0.lock().expect("stream lock").has_position()
    }

    fn get_position(&self) -> usize {
        self.0.lock().expect("stream lock").get_position()
    }

    fn seek(&mut self, position: usize) -> bool {
        self.0.lock().expect("stream lock").seek(position)
    }

    fn move_by(&mut self, offset: i64) -> bool {
        self.0.lock().expect("stream lock").move_by(offset)
    }

    fn has_length(&self) -> bool {
        self.0.lock().expect("stream lock").has_length()
    }

    fn get_length(&self) -> usize {
        self.0.lock().expect("stream lock").get_length()
    }
}

// Port of: tests/CodecExactReadTest.cpp#L53-L94 (chrome/m156)
def_test!(Codec_end, |r| {
    const NUM_IMAGES: usize = 2;
    for path in [
        "images/plane.png",
        "images/yellow_rose.png",
        "images/plane_interlaced.png",
        "images/mandrill.wbmp",
        "images/randPixels.bmp",
    ] {
        let Some(data) = get_resource_as_data(path) else {
            // Like `skip_missing_resource!`, but one file at a time, as the C++ `continue` does.
            eprintln!("todo: skipping, missing Skia resource {path}");
            continue;
        };

        let mut multi_data = Vec::with_capacity(data.len() * NUM_IMAGES);
        for _ in 0..NUM_IMAGES {
            multi_data.extend_from_slice(&data);
        }
        drop(data);

        let stream = Arc::new(Mutex::new(*MemoryStream::make_copy(&multi_data)));
        for i in 0..NUM_IMAGES {
            let codec = codecs::make_codec_from_stream(Box::new(SharedStream(stream.clone())));
            let Ok(mut codec) = codec else {
                reporter_assert!(
                    r,
                    false,
                    "Failed to create a codec from {path}, iteration {i}"
                );
                continue;
            };

            let info = codec.info().with_color_type(ColorType::N32);
            let row_bytes = info.min_row_bytes();
            let mut pixels = vec![0u8; info.compute_byte_size(row_bytes)];
            let result = codec.get_pixels(&info, &mut pixels, row_bytes, None);
            if result != Result::Success {
                reporter_assert!(
                    r,
                    false,
                    "Failed to getPixels from {path}, iteration {i} error {}",
                    result.as_str()
                );
            }
        }
    }
});
