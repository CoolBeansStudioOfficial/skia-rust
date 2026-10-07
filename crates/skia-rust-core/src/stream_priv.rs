// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkStreamPriv.h, src/core/SkStream.cpp (SkStreamPriv namespace)

//! `SkStreamPriv`: stream helpers that Skia keeps out of its public headers.

use crate::data::Data;
use crate::stream::{DynamicMemoryWStream, Stream, WStream};

/// `SkStreamPriv::kBufferSize`.
// Port of: src/core/SkStream.cpp#L940 (chrome/m156)
const BUFFER_SIZE: usize = 4096;

/// Copies the provided stream to a [`Data`].
///
/// Note: assumes the stream is at the beginning. If it has a length, but is not at the
/// beginning, this call will fail (return `None`).
// Port of: src/core/SkStream.cpp#L942-L956 (chrome/m156)
#[doc(alias = "SkStreamPriv::CopyStreamToData")]
#[doc(alias = "CopyStreamToData")]
pub fn copy_stream_to_data(stream: &mut dyn Stream) -> Option<Data> {
    if stream.has_length() {
        let length = stream.get_length();
        return Data::from_stream(stream, length);
    }

    let mut temp_stream = DynamicMemoryWStream::new();
    let mut buffer = [0u8; BUFFER_SIZE];
    loop {
        let bytes_read = stream.read(&mut buffer);
        let _ = temp_stream.write(&buffer[..bytes_read]);
        if stream.is_at_end() {
            break;
        }
    }
    Some(temp_stream.detach_as_data())
}

/// Copies the input stream from the current position to the end. Does not rewind the input
/// stream.
// Port of: src/core/SkStream.cpp#L958-L978 (chrome/m156)
#[doc(alias = "SkStreamPriv::Copy")]
#[doc(alias = "Copy")]
pub fn copy(out: &mut dyn WStream, input: &mut dyn Stream) -> bool {
    if input.has_position()
        && input.has_length()
        && let Some(base) = input.get_memory_base()
    {
        // Shortcut that avoids the while loop.
        let position = input.get_position();
        let length = input.get_length();
        debug_assert!(length >= position);
        return out.write(&base[position..length]);
    }
    let mut scratch = [0u8; BUFFER_SIZE];
    loop {
        let count = input.read(&mut scratch);
        if 0 == count {
            return true;
        }
        if !out.write(&scratch[..count]) {
            return false;
        }
    }
}

/// A [`WStream`] that writes all output to stderr (`SkDebugf`), for debugging purposes.
// Port of: src/core/SkStreamPriv.h#L38-L45 (chrome/m156)
#[doc(alias = "SkStreamPriv::DebugfStream")]
#[derive(Debug, Default)]
pub struct DebugfStream {
    bytes_written: usize,
}

// Port of: src/core/SkStream.cpp#L994-L1000 (chrome/m156)
impl WStream for DebugfStream {
    fn write(&mut self, buffer: &[u8]) -> bool {
        eprint!("{}", String::from_utf8_lossy(buffer));
        self.bytes_written += buffer.len();
        true
    }

    fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

/// Writes `value` in big-endian order.
// Port of: src/core/SkStreamPriv.h#L50-L53 (chrome/m156)
#[doc(alias = "WriteU16BE")]
pub fn write_u16_be(s: &mut dyn WStream, value: u16) -> bool {
    s.write(&value.to_be_bytes())
}

/// Writes `value` in big-endian order.
// Port of: src/core/SkStreamPriv.h#L55-L58 (chrome/m156)
#[doc(alias = "WriteU32BE")]
pub fn write_u32_be(s: &mut dyn WStream, value: u32) -> bool {
    s.write(&value.to_be_bytes())
}

/// Writes `value` in big-endian order.
// Port of: src/core/SkStreamPriv.h#L60-L63 (chrome/m156)
#[doc(alias = "WriteS32BE")]
pub fn write_s32_be(s: &mut dyn WStream, value: i32) -> bool {
    s.write(&value.to_be_bytes())
}

/// Reads a big-endian `u16`.
// Port of: src/core/SkStreamPriv.h#L65-L71 (chrome/m156)
#[doc(alias = "ReadU16BE")]
pub fn read_u16_be(s: &mut dyn Stream) -> Option<u16> {
    s.read_u16().map(u16::swap_bytes)
}

/// Reads a big-endian `u32`.
// Port of: src/core/SkStreamPriv.h#L73-L79 (chrome/m156)
#[doc(alias = "ReadU32BE")]
pub fn read_u32_be(s: &mut dyn Stream) -> Option<u32> {
    s.read_u32().map(u32::swap_bytes)
}

/// Reads a big-endian `i32`.
// Port of: src/core/SkStreamPriv.h#L81-L87 (chrome/m156)
#[doc(alias = "ReadS32BE")]
pub fn read_s32_be(s: &mut dyn Stream) -> Option<i32> {
    s.read_s32().map(i32::swap_bytes)
}

/// If the stream supports identifying the current position and total length, this returns true
/// if there are not enough bytes in the stream to fulfill a read of the given length.
/// Otherwise, it returns false.
///
/// False does *not* mean a read will succeed of the given length, but true means we are
/// certain it will fail.
// Port of: src/core/SkStream.cpp#L980-L992 (chrome/m156)
#[doc(alias = "SkStreamPriv::RemainingLengthIsBelow")]
#[doc(alias = "RemainingLengthIsBelow")]
pub fn remaining_length_is_below(stream: &dyn Stream, len: usize) -> bool {
    if stream.has_length() {
        if stream.has_position() {
            let remaining_bytes = stream.get_length() - stream.get_position();
            return remaining_bytes < len;
        }
        // We don't know the position, but we can still return true if the stream's entire
        // length is shorter than the requested length.
        return stream.get_length() < len;
    }
    false
}
