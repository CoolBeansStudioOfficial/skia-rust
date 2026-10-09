// Copyright 2017 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sksl/SkSLOutputStream.{h,cpp} and src/sksl/SkSLStringStream.h.

//! The byte sinks the `SkSL` generators write to: the [`OutputStream`] trait and its in-memory
//! [`StringStream`].

use crate::string::{Arg, printf};

/// `SkSL::OutputStream`: a sink of bytes and text.
#[doc(alias = "SkSL::OutputStream")]
pub trait OutputStream {
    /// Whether the stream can take output (`isValid`). In-memory streams always can.
    fn is_valid(&self) -> bool {
        true
    }

    /// Writes one byte.
    fn write8(&mut self, b: u8);

    /// Writes a 16-bit value, little-endian.
    fn write16(&mut self, i: u16) {
        for b in i.to_le_bytes() {
            self.write8(b);
        }
    }

    /// Writes a 32-bit value, little-endian.
    fn write32(&mut self, i: u32) {
        for b in i.to_le_bytes() {
            self.write8(b);
        }
    }

    /// Writes text (`writeText(const char*)`).
    fn write_text(&mut self, s: &str) {
        self.write(s.as_bytes());
    }

    /// Writes raw bytes.
    fn write(&mut self, s: &[u8]);

    /// Writes a string (`writeString`).
    fn write_string(&mut self, s: &str) {
        self.write(s.as_bytes());
    }

    /// Formats with [`printf`] and writes the result (`OutputStream::printf`).
    fn printf(&mut self, format: &str, args: &[Arg<'_>]) {
        let text = printf(format, args);
        self.write(text.as_bytes());
    }
}

/// `SkSL::StringStream`: an [`OutputStream`] that collects its bytes into a string.
#[doc(alias = "SkSL::StringStream")]
#[derive(Clone, Debug, Default)]
pub struct StringStream {
    bytes: Vec<u8>,
}

impl StringStream {
    /// An empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of bytes written so far (`bytesWritten`).
    #[must_use]
    pub fn bytes_written(&self) -> usize {
        self.bytes.len()
    }

    /// The bytes written so far.
    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The text written so far (`str()`). Output from the `SkSL` generators is UTF-8; invalid
    /// sequences would be replaced.
    #[must_use]
    pub fn str(&self) -> std::borrow::Cow<'_, str> {
        String::from_utf8_lossy(&self.bytes)
    }

    /// Clears the stream (`reset`).
    pub fn reset(&mut self) {
        self.bytes.clear();
    }
}

impl OutputStream for StringStream {
    fn write8(&mut self, b: u8) {
        self.bytes.push(b);
    }

    fn write(&mut self, s: &[u8]) {
        self.bytes.extend_from_slice(s);
    }
}

#[cfg(test)]
mod tests {
    use super::{OutputStream, StringStream};
    use crate::string::Arg;

    #[test]
    fn string_stream_collects_text() {
        let mut out = StringStream::new();
        out.write_text("a");
        out.write8(b'b');
        out.printf("%d-%s", &[Arg::Int(3), Arg::Str("c")]);
        assert_eq!(out.str(), "ab3-c");
        assert_eq!(out.bytes_written(), 5);
        out.reset();
        assert_eq!(out.bytes_written(), 0);
    }

    #[test]
    fn little_endian_writes() {
        let mut out = StringStream::new();
        out.write16(0x1234);
        out.write32(0xAABB_CCDD);
        assert_eq!(out.bytes(), [0x34, 0x12, 0xDD, 0xCC, 0xBB, 0xAA]);
    }
}
