// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBuffer.h, src/core/SkBuffer.cpp

//! `RBuffer`: a light weight reader over a memory block.

use crate::safe_math::SafeMath;

/// Light weight class for reading data from a memory block. The `RBuffer` is given the buffer to
/// read from; reading past its end marks it invalid.
// Port of: src/core/SkBuffer.h#L24-L75 (chrome/m156)
#[doc(alias = "SkRBuffer")]
#[derive(Debug, Clone)]
pub struct RBuffer<'a> {
    data: &'a [u8],
    pos: usize,
    valid: bool,
}

impl Default for RBuffer<'_> {
    fn default() -> Self {
        Self::new(&[])
    }
}

impl<'a> RBuffer<'a> {
    /// Initializes the buffer with the data to read from.
    // Port of: src/core/SkBuffer.h#L25-L35 (chrome/m156)
    #[must_use]
    pub fn new(data: &'a [u8]) -> Self {
        Self {
            data,
            pos: 0,
            valid: true,
        }
    }

    /// Returns the number of bytes that have been read from the beginning of the data.
    // Port of: src/core/SkBuffer.h#L40 (chrome/m156)
    #[must_use]
    pub fn pos(&self) -> usize {
        self.pos
    }

    /// Returns the total size of the data.
    // Port of: src/core/SkBuffer.h#L44 (chrome/m156)
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }

    /// Returns true if the buffer has read to the end of the data.
    // Port of: src/core/SkBuffer.h#L49 (chrome/m156)
    #[must_use]
    pub fn eof(&self) -> bool {
        self.pos >= self.data.len()
    }

    /// The number of bytes that can still be read.
    // Port of: src/core/SkBuffer.h#L51 (chrome/m156)
    #[must_use]
    pub fn available(&self) -> usize {
        self.data.len() - self.pos
    }

    /// False once a read or skip has failed.
    // Port of: src/core/SkBuffer.h#L53 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.valid
    }

    /// Reads `buffer.len()` bytes into `buffer`. Returns false (and invalidates the buffer) if
    /// that many bytes are not available.
    // Port of: src/core/SkBuffer.cpp#L19-L25 (chrome/m156)
    pub fn read(&mut self, buffer: &mut [u8]) -> bool {
        if let Some(src) = self.skip(buffer.len()) {
            buffer.copy_from_slice(src);
            return true;
        }
        false
    }

    /// Skips to the next 4-byte boundary of the position within the data. (The C++ aligns the
    /// address; Rust has no stable address, so the offset from the start is aligned.)
    // Port of: src/core/SkBuffer.cpp#L27-L36 (chrome/m156)
    #[doc(alias = "skipToAlign4")]
    pub fn skip_to_align4(&mut self) -> bool {
        let n = crate::align::align4(self.pos) - self.pos;
        if self.valid && n <= self.available() {
            self.pos += n;
            true
        } else {
            self.valid = false;
            false
        }
    }

    /// Reads one byte.
    // Port of: src/core/SkBuffer.h#L62 (chrome/m156)
    #[doc(alias = "readU8")]
    pub fn read_u8(&mut self) -> Option<u8> {
        let mut x = [0; 1];
        self.read(&mut x).then(|| x[0])
    }

    /// Reads a native-endian `i32`.
    // Port of: src/core/SkBuffer.h#L63 (chrome/m156)
    #[doc(alias = "readS32")]
    pub fn read_s32(&mut self) -> Option<i32> {
        let mut x = [0; 4];
        self.read(&mut x).then(|| i32::from_ne_bytes(x))
    }

    /// Reads a native-endian `u32`.
    // Port of: src/core/SkBuffer.h#L64 (chrome/m156)
    #[doc(alias = "readU32")]
    pub fn read_u32(&mut self) -> Option<u32> {
        let mut x = [0; 4];
        self.read(&mut x).then(|| u32::from_ne_bytes(x))
    }

    /// Skips `bytes` bytes and returns them, or `None` (invalidating the buffer) on failure.
    // Port of: src/core/SkBuffer.cpp#L11-L17 (chrome/m156)
    pub fn skip(&mut self, bytes: usize) -> Option<&'a [u8]> {
        if self.valid && bytes <= self.available() {
            let data: &'a [u8] = self.data;
            let pos = &data[self.pos..self.pos + bytes];
            self.pos += bytes;
            return Some(pos);
        }
        self.valid = false;
        None
    }

    /// Skips `count` elements of `size_of_element` bytes each, checking the multiplication for
    /// overflow (`skipCount<T>`).
    // Port of: src/core/SkBuffer.h#L69-L72 (chrome/m156)
    #[doc(alias = "skipCount")]
    pub fn skip_count(&mut self, count: usize, size_of_element: usize) -> Option<&'a [u8]> {
        self.skip(SafeMath::saturating_mul(count, size_of_element))
    }
}
