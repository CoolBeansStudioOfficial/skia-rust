// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: client_utils/android/FrontBufferedStream.h, client_utils/android/FrontBufferedStream.cpp

//! `FrontBufferedStream`: a stream that buffers the first X bytes of another stream, so that
//! it can be rewound (and peeked) within them.

use crate::stream::{Stream, StreamRewindable};

/// `SkCodec::MinBufferedBytesNeeded()`.
// Port of: include/codec/SkCodec.h#L72 (chrome/m156)
const MIN_BUFFERED_BYTES_NEEDED: usize = 32;

/// `FrontBufferedStream::kStorageSize`: buffers up to this size live inside the stream object
/// in Skia, larger ones on the heap (and are freed once no rewind is possible any more).
// Port of: client_utils/android/FrontBufferedStream.cpp#L40 (chrome/m156)
const STORAGE_SIZE: usize = MIN_BUFFERED_BYTES_NEEDED;

/// Specialized stream that buffers the first X bytes of a stream, where X is passed in by the
/// user. Note that unlike some buffered stream APIs, once more bytes than can fit in the
/// buffer are read, no more buffering is done. This stream is designed for a use case where
/// the caller knows that rewind will only be called from within X bytes (inclusive), and the
/// wrapped stream is not necessarily able to rewind at all.
// Port of: client_utils/android/FrontBufferedStream.h#L21-L40 (chrome/m156)
#[derive(Debug)]
pub struct FrontBufferedStream;

impl FrontBufferedStream {
    /// Creates a new stream that wraps and buffers a [`Stream`].
    ///
    /// `stream` is expected to be owned by the returned stream, so it should no longer be used
    /// directly. Returns a stream that can buffer at least `buffer_size` bytes, or `None` if
    /// `stream` is `None` (or the buffer cannot be allocated).
    // Port of: client_utils/android/FrontBufferedStream.cpp#L72-L85 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        stream: Option<Box<dyn Stream>>,
        buffer_size: usize,
    ) -> Option<Box<dyn StreamRewindable>> {
        let stream = stream?;
        let front_buffered_stream = Inner::new(stream, buffer_size)?;
        Some(Box::new(front_buffered_stream))
    }
}

// Port of: client_utils/android/FrontBufferedStream.cpp#L17-L69 (chrome/m156)
struct Inner {
    stream: Box<dyn Stream>,
    has_length: bool,
    length: usize,
    // Current offset into the stream. Always >= 0.
    offset: usize,
    // Amount that has been buffered by calls to read. Will always be less than buffer_size.
    buffered_so_far: usize,
    // Total size of the buffer.
    buffer_size: usize,
    // Empty once the buffer is no longer needed (see `read_directly_from_stream`).
    buffer: Vec<u8>,
}

impl std::fmt::Debug for Inner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FrontBufferedStream")
            .field("offset", &self.offset)
            .field("buffered_so_far", &self.buffered_so_far)
            .field("buffer_size", &self.buffer_size)
            .finish_non_exhaustive()
    }
}

impl Inner {
    // Port of: client_utils/android/FrontBufferedStream.cpp#L87-L94 (chrome/m156)
    fn new(stream: Box<dyn Stream>, buffer_size: usize) -> Option<Self> {
        let has_length = stream.has_position() && stream.has_length();
        let length = stream.get_length().wrapping_sub(stream.get_position());
        let mut buffer = Vec::new();
        buffer.try_reserve_exact(buffer_size).ok()?;
        buffer.resize(buffer_size, 0);
        Some(Self {
            stream,
            has_length,
            length,
            offset: 0,
            buffered_so_far: 0,
            buffer_size,
            buffer,
        })
    }

    /// Reads up to `size` bytes from already buffered data, and copies them to `dst`, if
    /// present. Updates `offset`. Assumes that `offset` is less than `buffered_so_far`.
    // Port of: client_utils/android/FrontBufferedStream.cpp#L120-L137 (chrome/m156)
    fn read_from_buffer(&mut self, dst: Option<&mut [u8]>, size: usize) -> usize {
        debug_assert!(self.offset < self.buffered_so_far);
        // Some data has already been copied to buffer. Read up to the lesser of the size
        // requested and the remainder of the buffered data.
        let bytes_to_copy = size.min(self.buffered_so_far - self.offset);
        if let Some(dst) = dst {
            dst[..bytes_to_copy]
                .copy_from_slice(&self.buffer[self.offset..self.offset + bytes_to_copy]);
        }

        // Update offset to the new position. It is guaranteed to be within the buffered data.
        self.offset += bytes_to_copy;
        debug_assert!(self.offset <= self.buffered_so_far);

        bytes_to_copy
    }

    /// Buffers up to `size` bytes from the stream, and copies them to `dst` if present.
    /// Updates `offset` and `buffered_so_far`. Assumes that `offset` is at least
    /// `buffered_so_far`, and `size` is greater than 0.
    // Port of: client_utils/android/FrontBufferedStream.cpp#L139-L159 (chrome/m156)
    fn buffer_and_write_to(&mut self, dst: Option<&mut [u8]>, size: usize) -> usize {
        debug_assert!(size > 0);
        debug_assert!(self.offset >= self.buffered_so_far);
        // Data needs to be buffered. Buffer up to the lesser of the size requested and the
        // remainder of the max buffer size.
        let bytes_to_buffer = size.min(self.buffer_size - self.buffered_so_far);
        let start = self.offset;
        let buffered = self
            .stream
            .read(&mut self.buffer[start..start + bytes_to_buffer]);

        self.buffered_so_far += buffered;
        self.offset = self.buffered_so_far;
        debug_assert!(self.buffered_so_far <= self.buffer_size);

        // Copy the buffer to the destination buffer and update the amount read.
        if let Some(dst) = dst {
            dst[..buffered].copy_from_slice(&self.buffer[start..start + buffered]);
        }

        buffered
    }

    /// Reads up to `size` bytes directly from the stream and into `dst` if present. Updates
    /// `offset`. Assumes `offset` is at or beyond the buffered data, and `size` is greater
    /// than 0.
    // Port of: client_utils/android/FrontBufferedStream.cpp#L161-L178 (chrome/m156)
    fn read_directly_from_stream(&mut self, dst: Option<&mut [u8]>, size: usize) -> usize {
        debug_assert!(size > 0);
        // If we get here, we have buffered all that can be buffered.
        debug_assert!(self.buffer_size == self.buffered_so_far && self.offset >= self.buffer_size);

        let bytes_read_directly = match dst {
            Some(dst) => self.stream.read(&mut dst[..size]),
            None => self.stream.skip(size),
        };
        self.offset += bytes_read_directly;

        // If we have read past the end of the buffer, rewinding is no longer supported, so we
        // can go ahead and free the memory.
        if bytes_read_directly > 0 && self.buffer_size > STORAGE_SIZE {
            self.buffer = Vec::new();
        }

        bytes_read_directly
    }

    /// `read(dst, size)`, where `dst` may be absent (skip).
    // Port of: client_utils/android/FrontBufferedStream.cpp#L198-L242 (chrome/m156)
    fn read_or_skip(&mut self, dst: Option<&mut [u8]>, size: usize) -> usize {
        let mut size = size;
        let mut dst = dst;
        let total_size = size;
        let start = self.offset;

        // First, read any data that was previously buffered.
        if self.offset < self.buffered_so_far {
            let bytes_copied = self.read_from_buffer(dst.as_deref_mut(), size);

            // Update the remaining number of bytes needed to read and the destination buffer.
            size -= bytes_copied;
            debug_assert_eq!(size + (self.offset - start), total_size);
            if let Some(d) = dst.take() {
                dst = Some(&mut d[bytes_copied..]);
            }
        }

        // Buffer any more data that should be buffered, and copy it to the destination.
        if size > 0 && self.buffered_so_far < self.buffer_size && !self.stream.is_at_end() {
            let buffered = self.buffer_and_write_to(dst.as_deref_mut(), size);

            // Update the remaining number of bytes needed to read and the destination buffer.
            size -= buffered;
            debug_assert_eq!(size + (self.offset - start), total_size);
            if let Some(d) = dst.take() {
                dst = Some(&mut d[buffered..]);
            }
        }

        if size > 0 && !self.stream.is_at_end() {
            let bytes_read_directly = self.read_directly_from_stream(dst, size);
            size -= bytes_read_directly;
            debug_assert_eq!(size + (self.offset - start), total_size);
        }
        let _ = size;

        self.offset - start
    }
}

// Port of: client_utils/android/FrontBufferedStream.cpp#L96-L242 (chrome/m156)
impl Stream for Inner {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let size = buffer.len();
        self.read_or_skip(Some(buffer), size)
    }

    fn skip(&mut self, size: usize) -> usize {
        self.read_or_skip(None, size)
    }

    fn peek(&mut self, dst: &mut [u8]) -> usize {
        // Keep track of the offset so we can return to it.
        let start = self.offset;

        if start >= self.buffer_size {
            // This stream is not able to buffer.
            return 0;
        }

        let size = dst.len().min(self.buffer_size - start);
        let bytes_read = self.read(&mut dst[..size]);
        self.offset = start;
        bytes_read
    }

    fn is_at_end(&self) -> bool {
        if self.offset < self.buffered_so_far {
            // Even if the underlying stream is at the end, this stream has been rewound after
            // buffering, so it is not at the end.
            return false;
        }

        self.stream.is_at_end()
    }

    fn rewind(&mut self) -> bool {
        // Only allow a rewind if we have not exceeded the buffer.
        if self.offset <= self.buffer_size {
            self.offset = 0;
            return true;
        }
        false
    }

    fn has_length(&self) -> bool {
        self.has_length
    }

    fn get_length(&self) -> usize {
        self.length
    }
}

impl StreamRewindable for Inner {}
