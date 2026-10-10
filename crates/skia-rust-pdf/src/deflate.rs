// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkDeflate.{h,cpp} (chrome/m156)

//! `SkDeflateWStream`: a write stream that compresses what is written to it with zlib's deflate,
//! for PDF stream objects.
//!
//! The compressor is the Chromium zlib port in `skia-rust-zlib`, which is byte-exact against
//! Skia's zlib (`oracle/codec-diff/zlib`). The gzip framing that `SkDeflateWStream` can also
//! write is not ported: the zlib port has no gzip wrapper.

use skia_rust_core::stream::WStream;
use skia_rust_zlib::{Deflate, Flush};

/// `SKDEFLATEWSTREAM_INPUT_BUFFER_SIZE`.
const INPUT_BUFFER_SIZE: usize = 4096;
/// `SKDEFLATEWSTREAM_OUTPUT_BUFFER_SIZE`: 4096 + 128, usually big enough for a single loop.
const OUTPUT_BUFFER_SIZE: usize = 4224;

/// `Z_DEFLATED`, `Z_DEFAULT_STRATEGY` and `windowBits`/`memLevel` as `SkDeflateWStream` passes
/// them to `deflateInit2`.
const WINDOW_BITS_ZLIB: i32 = 0x0F;
const WINDOW_BITS_GZIP: i32 = 0x1F;
const MEM_LEVEL: i32 = 8;
const Z_DEFAULT_STRATEGY: i32 = 0;

/// `do_deflate`: compresses `input` with `flush`, writing every output chunk to `out`.
// Port of: src/pdf/SkDeflate.cpp#L36-L58 (chrome/m156)
fn do_deflate(flush: Flush, z: &mut Deflate, out: &mut dyn WStream, input: &[u8]) {
    let mut input = input;
    let mut output = [0u8; OUTPUT_BUFFER_SIZE];
    loop {
        let deflated = z.deflate(input, &mut output, flush);
        input = &input[deflated.consumed..];
        out.write(&output[..deflated.produced]);
        // `while (zStream->avail_in || !zStream->avail_out)`: avail_out is zero when the
        // output buffer was filled completely.
        if input.is_empty() && deflated.produced < output.len() {
            break;
        }
    }
}

/// `SkDeflateWStream`: compresses everything written to it into the wrapped stream.
///
/// The destructor finalizes the stream (`finalize`), so the compressed data is complete once the
/// value is dropped.
// Port of: src/pdf/SkDeflate.h#L15-L47, src/pdf/SkDeflate.cpp#L60-L145 (chrome/m156)
#[doc(alias = "SkDeflateWStream")]
pub struct DeflateWStream<'a> {
    /// The destination; `None` once finalized, or when constructed without one.
    out: Option<&'a mut dyn WStream>,
    in_buffer: Box<[u8; INPUT_BUFFER_SIZE]>,
    in_buffer_index: usize,
    /// The compressor; `None` when there is no destination, or zlib refused the parameters.
    z: Option<Deflate>,
}

impl std::fmt::Debug for DeflateWStream<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeflateWStream")
            .field("in_buffer_index", &self.in_buffer_index)
            .field("finished", &self.out.is_none())
            .finish_non_exhaustive()
    }
}

impl<'a> DeflateWStream<'a> {
    /// Wraps `out`. `compression_level` is 1 (best speed) to 9 (best compression), or -1 for
    /// zlib's default. 0 means no compression and is not allowed (Skia asserts on it).
    ///
    /// `gzip` would write a gzip file. It is not supported: the zlib port has no gzip wrapper,
    /// so the stream then refuses every write.
    // Port of: src/pdf/SkDeflate.cpp#L76-L94 (chrome/m156)
    #[doc(alias = "SkDeflateWStream::SkDeflateWStream")]
    #[must_use]
    pub fn new(out: Option<&'a mut dyn WStream>, compression_level: i32, gzip: bool) -> Self {
        debug_assert!(compression_level != 0);
        let mut this = Self {
            out,
            in_buffer: Box::new([0; INPUT_BUFFER_SIZE]),
            in_buffer_index: 0,
            z: None,
        };
        if this.out.is_none() {
            return this;
        }
        let window_bits = if gzip { WINDOW_BITS_GZIP } else { WINDOW_BITS_ZLIB };
        debug_assert!((-1..=9).contains(&compression_level));
        this.z = Deflate::new(compression_level, window_bits, MEM_LEVEL, Z_DEFAULT_STRATEGY).ok();
        this
    }

    /// Writes the end of the compressed stream. Later writes fail, and later calls do nothing.
    // Port of: src/pdf/SkDeflate.cpp#L96-L107 (chrome/m156)
    #[doc(alias = "SkDeflateWStream::finalize")]
    pub fn finalize(&mut self) {
        let Some(out) = self.out.take() else {
            return;
        };
        if let Some(mut z) = self.z.take() {
            let input = &self.in_buffer[..self.in_buffer_index];
            do_deflate(Flush::Finish, &mut z, out, input);
        }
    }
}

impl WStream for DeflateWStream<'_> {
    // Port of: src/pdf/SkDeflate.cpp#L109-L131 (chrome/m156)
    fn write(&mut self, buffer: &[u8]) -> bool {
        let Some(out) = self.out.as_deref_mut() else {
            return false;
        };
        let Some(z) = self.z.as_mut() else {
            return false;
        };
        let mut buffer = buffer;
        while !buffer.is_empty() {
            let tocopy = buffer.len().min(INPUT_BUFFER_SIZE - self.in_buffer_index);
            self.in_buffer[self.in_buffer_index..self.in_buffer_index + tocopy]
                .copy_from_slice(&buffer[..tocopy]);
            buffer = &buffer[tocopy..];
            self.in_buffer_index += tocopy;
            debug_assert!(self.in_buffer_index <= INPUT_BUFFER_SIZE);
            // If the buffer isn't filled, don't call into zlib yet.
            if self.in_buffer_index == INPUT_BUFFER_SIZE {
                do_deflate(
                    Flush::NoFlush,
                    z,
                    out,
                    &self.in_buffer[..self.in_buffer_index],
                );
                self.in_buffer_index = 0;
            }
        }
        true
    }

    // Port of: src/pdf/SkDeflate.cpp#L133-L135 (chrome/m156)
    fn bytes_written(&self) -> usize {
        let total_in = self.z.as_ref().map_or(0, skia_rust_zlib::Deflate::total_in);
        usize::try_from(total_in).unwrap_or(usize::MAX) + self.in_buffer_index
    }
}

impl Drop for DeflateWStream<'_> {
    // `~SkDeflateWStream() { this->finalize(); }`
    fn drop(&mut self) {
        self.finalize();
    }
}
