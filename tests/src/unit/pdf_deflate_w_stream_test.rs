// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFDeflateWStreamTest.cpp (chrome/m156)

use skia_rust_core::random::Random;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_pdf::deflate::DeflateWStream;
use skia_rust_zlib::{Flush, Inflate, ReturnCode};

use crate::{Reporter, def_test, errorf, reporter_assert};

/// Uses the un-deflate compression algorithm to decompress `src`. Returns `None` if an error
/// occurs. (`stream_inflate`, here for an in-memory source.)
// Port of: tests/PDFDeflateWStreamTest.cpp#L44-L118 (chrome/m156)
fn stream_inflate(reporter: &mut Reporter, src: &[u8]) -> Option<Vec<u8>> {
    const BUFFER_SIZE: usize = 1024;
    let Ok(mut z) = Inflate::new_default() else {
        errorf!(reporter, "Zlib: inflateInit failed");
        return None;
    };
    let mut decompressed = Vec::new();
    let mut output_buffer = [0u8; BUFFER_SIZE];
    let mut input = src;
    let mut rc = ReturnCode::Ok;
    // Feed the input until it is used up, or inflate stops making progress.
    while rc == ReturnCode::Ok && !input.is_empty() {
        let inflated = z.inflate(input, &mut output_buffer, Flush::NoFlush);
        input = &input[inflated.consumed..];
        decompressed.extend_from_slice(&output_buffer[..inflated.produced]);
        rc = inflated.ret;
    }
    // Drain whatever zlib still holds.
    while rc == ReturnCode::Ok {
        let inflated = z.inflate(&[], &mut output_buffer, Flush::Finish);
        decompressed.extend_from_slice(&output_buffer[..inflated.produced]);
        rc = inflated.ret;
    }
    if rc != ReturnCode::StreamEnd {
        errorf!(reporter, "Zlib: inflateEnd failed");
        return None;
    }
    Some(decompressed)
}

// Port of: tests/PDFDeflateWStreamTest.cpp#L120-L178 (chrome/m156)
def_test!(SkPDF_DeflateWStream, |r| {
    let mut random = Random::new(123_456);
    for loop_ in 0..50 {
        let size = random.next_u_less_than(10_000) as usize;
        let buffer: Vec<u8> = (0..size).map(|_| (random.next_u() & 0xff) as u8).collect();
        let mut dynamic_memory_w_stream = DynamicMemoryWStream::new();
        {
            let mut deflate_w_stream =
                DeflateWStream::new(Some(&mut dynamic_memory_w_stream), -1, false);
            let mut j = 0;
            while j < size {
                let write_size = (size - j).min(random.next_range_u(1, 400) as usize);
                if !deflate_w_stream.write(&buffer[j..j + write_size]) {
                    errorf!(r, "something went wrong.");
                    return;
                }
                j += write_size;
            }
            reporter_assert!(r, deflate_w_stream.bytes_written() == size);
        }
        let compressed = dynamic_memory_w_stream.detach_as_vector();
        let Some(decompressed) = stream_inflate(r, &compressed) else {
            errorf!(r, "Decompression failed.");
            return;
        };
        if decompressed.len() != size {
            errorf!(
                r,
                "Decompression failed to get right size [{}]. {} != {}",
                loop_,
                decompressed.len(),
                size
            );
            continue;
        }
        let min_length = size.min(decompressed.len());
        for i in 0..min_length {
            if buffer[i] != decompressed[i] {
                errorf!(r, "Decompression failed at byte {}.", i);
                break;
            }
        }
    }
    // `SkDeflateWStream emptyDeflateWStream(nullptr, -1); !writeText("FOO")`
    let mut empty_deflate_w_stream = DeflateWStream::new(None, -1, false);
    reporter_assert!(r, !empty_deflate_w_stream.write_text("FOO"));
});
