// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkStream.h, src/core/SkStream.cpp

//! Streams: sources of bytes ([`Stream`] and its refinements) and sinks of bytes ([`WStream`]),
//! with memory, file and growable-memory implementations.
//!
//! In Skia, `SkStream` is one class that declares every capability (`rewind`, `seek`,
//! `getLength`, `getMemoryBase`, ...) with a "not supported" default, and `SkStreamRewindable`,
//! `SkStreamSeekable`, `SkStreamAsset` and `SkStreamMemory` are refinements that make the
//! capabilities of the same name mandatory. The same shape is kept here: [`Stream`] has all the
//! methods with the same defaults, and the refinements are sub-traits that an implementation
//! declares to promise the capability.

use std::fs::File;
use std::io::{Seek, Write};
use std::path::Path;
use std::sync::Arc;

use crate::data::Data;
use crate::safe_math::SafeMath;
use crate::scalar::scalar;
use crate::string::{str_append_hex, str_append_s32, str_append_scalar, str_append_u64};

const SK_MAX_BYTE_FOR_U8: usize = 0xFD;
const SK_BYTE_SENTINEL_FOR_U16: u8 = 0xFE;
const SK_BYTE_SENTINEL_FOR_U32: u8 = 0xFF;

/// A source of bytes. Implementations can be backed by memory, or a file, or something else.
///
/// Differences from `SkStream::read(void* buffer, size_t size)`: [`Stream::read`] takes a slice
/// (the C++ `buffer == nullptr` "skip" mode is the separate [`Stream::skip`]), and
/// [`Stream::peek`] takes `&mut self` because some streams (`FrontBufferedStream`) buffer as
/// they peek.
// Port of: include/core/SkStream.h#L31-L156 (chrome/m156)
#[doc(alias = "SkStream")]
pub trait Stream {
    /// Reads `buffer.len()` bytes into `buffer`, returning how many were actually read (fewer
    /// if the stream ends first).
    fn read(&mut self, buffer: &mut [u8]) -> usize;

    /// Skips `size` bytes. Returns the actual number of bytes that could be skipped.
    ///
    /// Skia's `skip` is `read(nullptr, size)`; the default here reads into scratch memory,
    /// and streams that can do better override it.
    // Port of: include/core/SkStream.h#L53-L55 (chrome/m156)
    fn skip(&mut self, size: usize) -> usize {
        let mut scratch = [0u8; 4096];
        let mut skipped = 0;
        while skipped < size {
            let want = (size - skipped).min(scratch.len());
            let got = self.read(&mut scratch[..want]);
            skipped += got;
            if got < want {
                break;
            }
        }
        skipped
    }

    /// Attempts to peek at `buffer.len()` bytes.
    ///
    /// If this stream supports peeking, copies min(size, peekable bytes) into `buffer` and
    /// returns the number of bytes copied. If the stream does not support peeking, or cannot
    /// peek any bytes, returns 0 and leaves `buffer` unchanged. The stream is guaranteed to be
    /// in the same visible state after this call, regardless of success or failure.
    // Port of: include/core/SkStream.h#L70 (chrome/m156)
    fn peek(&mut self, _buffer: &mut [u8]) -> usize {
        0
    }

    /// Returns true when all the bytes in the stream have been read.
    ///
    /// As a `Stream` represents synchronous I/O, this returns false when the final stream
    /// length isn't known yet, even when all the bytes available so far have been read. This
    /// may return true early (when there are no more bytes to be read) or late (after the
    /// first unsuccessful read).
    // Port of: include/core/SkStream.h#L79 (chrome/m156)
    #[doc(alias = "isAtEnd")]
    fn is_at_end(&self) -> bool;

    /// Reads an `i8`; `None` if the stream is exhausted.
    // Port of: src/core/SkStream.cpp#L32-L34 (chrome/m156)
    #[doc(alias = "readS8")]
    fn read_s8(&mut self) -> Option<i8> {
        let mut bytes = [0; 1];
        (self.read(&mut bytes) == bytes.len()).then(|| i8::from_ne_bytes(bytes))
    }

    /// Reads a native-endian `i16`; `None` if the stream is exhausted.
    // Port of: src/core/SkStream.cpp#L36-L38 (chrome/m156)
    #[doc(alias = "readS16")]
    fn read_s16(&mut self) -> Option<i16> {
        let mut bytes = [0; 2];
        (self.read(&mut bytes) == bytes.len()).then(|| i16::from_ne_bytes(bytes))
    }

    /// Reads a native-endian `i32`; `None` if the stream is exhausted.
    // Port of: src/core/SkStream.cpp#L40-L42 (chrome/m156)
    #[doc(alias = "readS32")]
    fn read_s32(&mut self) -> Option<i32> {
        let mut bytes = [0; 4];
        (self.read(&mut bytes) == bytes.len()).then(|| i32::from_ne_bytes(bytes))
    }

    /// Reads a native-endian `i64`; `None` if the stream is exhausted.
    // Port of: src/core/SkStream.cpp#L44-L46 (chrome/m156)
    #[doc(alias = "readS64")]
    fn read_s64(&mut self) -> Option<i64> {
        let mut bytes = [0; 8];
        (self.read(&mut bytes) == bytes.len()).then(|| i64::from_ne_bytes(bytes))
    }

    /// Reads a `u8`; `None` if the stream is exhausted.
    // Port of: include/core/SkStream.h#L86 (chrome/m156)
    #[doc(alias = "readU8")]
    fn read_u8(&mut self) -> Option<u8> {
        self.read_s8().map(|i| i.to_ne_bytes()[0])
    }

    /// Reads a native-endian `u16`; `None` if the stream is exhausted.
    // Port of: include/core/SkStream.h#L87 (chrome/m156)
    #[doc(alias = "readU16")]
    fn read_u16(&mut self) -> Option<u16> {
        self.read_s16().map(|i| u16::from_ne_bytes(i.to_ne_bytes()))
    }

    /// Reads a native-endian `u32`; `None` if the stream is exhausted.
    // Port of: include/core/SkStream.h#L88 (chrome/m156)
    #[doc(alias = "readU32")]
    fn read_u32(&mut self) -> Option<u32> {
        self.read_s32().map(|i| u32::from_ne_bytes(i.to_ne_bytes()))
    }

    /// Reads a native-endian `u64`; `None` if the stream is exhausted.
    // Port of: include/core/SkStream.h#L89 (chrome/m156)
    #[doc(alias = "readU64")]
    fn read_u64(&mut self) -> Option<u64> {
        self.read_s64().map(|i| u64::from_ne_bytes(i.to_ne_bytes()))
    }

    /// Reads a bool (any non-zero byte is true); `None` if the stream is exhausted.
    // Port of: include/core/SkStream.h#L91-L96 (chrome/m156)
    #[doc(alias = "readBool")]
    fn read_bool(&mut self) -> Option<bool> {
        self.read_u8().map(|i| i != 0)
    }

    /// Reads a native-endian scalar; `None` if the stream is exhausted.
    // Port of: src/core/SkStream.cpp#L48-L50 (chrome/m156)
    #[doc(alias = "readScalar")]
    fn read_scalar(&mut self) -> Option<scalar> {
        let mut bytes = [0; 4];
        (self.read(&mut bytes) == bytes.len()).then(|| scalar::from_ne_bytes(bytes))
    }

    /// Reads a value written by [`WStream::write_packed_uint`]; `None` if the stream is
    /// exhausted.
    // Port of: src/core/SkStream.cpp#L56-L73 (chrome/m156)
    #[doc(alias = "readPackedUInt")]
    fn read_packed_uint(&mut self) -> Option<usize> {
        let mut byte = [0u8; 1];
        if self.read(&mut byte) == 0 {
            return None;
        }
        let byte = byte[0];
        if SK_BYTE_SENTINEL_FOR_U16 == byte {
            Some(usize::from(self.read_u16()?))
        } else if SK_BYTE_SENTINEL_FOR_U32 == byte {
            // A 32-bit value always fits `usize` on the 32/64-bit targets Skia supports.
            Some(usize::try_from(self.read_u32()?).expect("u32 fits in usize"))
        } else {
            Some(usize::from(byte))
        }
    }

    // ---- SkStreamRewindable ----

    /// Rewinds to the beginning of the stream. Returns true if the stream is known to be at
    /// the beginning after this call returns.
    // Port of: include/core/SkStream.h#L104 (chrome/m156)
    fn rewind(&mut self) -> bool {
        false
    }

    /// Duplicates this stream. If this cannot be done, returns `None`. The returned stream
    /// will be positioned at the beginning of its data.
    // Port of: include/core/SkStream.h#L109-L111 (chrome/m156)
    fn duplicate(&self) -> Option<Box<dyn Stream>> {
        None
    }

    /// Duplicates this stream. If this cannot be done, returns `None`. The returned stream
    /// will be positioned the same as this stream.
    // Port of: include/core/SkStream.h#L115-L117 (chrome/m156)
    fn fork(&self) -> Option<Box<dyn Stream>> {
        None
    }

    // ---- SkStreamSeekable ----

    /// Returns true if this stream can report its current position.
    // Port of: include/core/SkStream.h#L121 (chrome/m156)
    #[doc(alias = "hasPosition")]
    fn has_position(&self) -> bool {
        false
    }

    /// Returns the current position in the stream. If this cannot be done, returns 0.
    // Port of: include/core/SkStream.h#L123 (chrome/m156)
    #[doc(alias = "getPosition")]
    fn get_position(&self) -> usize {
        0
    }

    /// Seeks to an absolute position in the stream. If this cannot be done, returns false. If
    /// an attempt is made to seek past the end of the stream, the position will be set to the
    /// end of the stream.
    // Port of: include/core/SkStream.h#L129 (chrome/m156)
    fn seek(&mut self, _position: usize) -> bool {
        false
    }

    /// Seeks to a relative offset in the stream. If this cannot be done, returns false. If an
    /// attempt is made to move to a position outside the stream, the position will be set to
    /// the closest point within the stream (beginning or end).
    ///
    /// (`SkStream::move` takes a C `long`; this takes an `i64`, as on 64-bit Linux and macOS.)
    // Port of: include/core/SkStream.h#L135 (chrome/m156)
    #[doc(alias = "move")]
    fn move_by(&mut self, _offset: i64) -> bool {
        false
    }

    // ---- SkStreamAsset ----

    /// Returns true if this stream can report its total length.
    // Port of: include/core/SkStream.h#L139 (chrome/m156)
    #[doc(alias = "hasLength")]
    fn has_length(&self) -> bool {
        false
    }

    /// Returns the total length of the stream. If this cannot be done, returns 0.
    // Port of: include/core/SkStream.h#L141 (chrome/m156)
    #[doc(alias = "getLength")]
    fn get_length(&self) -> usize {
        0
    }

    // ---- SkStreamMemory ----

    /// Returns the bytes of the stream if they all live in one contiguous block of memory.
    /// If this cannot be done, returns `None`.
    // Port of: include/core/SkStream.h#L145 (chrome/m156)
    #[doc(alias = "getMemoryBase")]
    fn get_memory_base(&self) -> Option<&[u8]> {
        None
    }

    /// Returns the data this stream reads from, if it has one.
    // Port of: include/core/SkStream.h#L146 (chrome/m156)
    #[doc(alias = "getData")]
    fn get_data(&self) -> Option<Data> {
        None
    }
}

/// A [`Stream`] for which `rewind` and `duplicate` are required.
// Port of: include/core/SkStream.h#L159-L167 (chrome/m156)
#[doc(alias = "SkStreamRewindable")]
pub trait StreamRewindable: Stream {}

/// A [`StreamRewindable`] for which `get_position`, `seek`, `move_by` and `fork` are required.
// Port of: include/core/SkStream.h#L170-L187 (chrome/m156)
#[doc(alias = "SkStreamSeekable")]
pub trait StreamSeekable: StreamRewindable {}

/// A [`StreamSeekable`] for which `get_length` is required.
// Port of: include/core/SkStream.h#L190-L204 (chrome/m156)
#[doc(alias = "SkStreamAsset")]
pub trait StreamAsset: StreamSeekable {
    /// `SkStreamAsset::duplicate`: like [`Stream::duplicate`], but always succeeds and keeps
    /// the asset type.
    fn duplicate_asset(&self) -> Box<dyn StreamAsset>;

    /// `SkStreamAsset::fork`: like [`Stream::fork`], but always succeeds and keeps the asset
    /// type.
    fn fork_asset(&self) -> Box<dyn StreamAsset>;
}

/// A [`StreamAsset`] for which `get_memory_base` is required.
// Port of: include/core/SkStream.h#L207-L220 (chrome/m156)
#[doc(alias = "SkStreamMemory")]
pub trait StreamMemory: StreamAsset {}

/// Attempts to open the specified file as a stream, returns `None` on failure.
// Port of: src/core/SkStream.cpp#L913-L936 (chrome/m156)
#[doc(alias = "SkStream::MakeFromFile")]
#[doc(alias = "MakeFromFile")]
pub fn make_from_file(path: impl AsRef<Path>) -> Option<Box<dyn StreamAsset>> {
    let path = path.as_ref();
    if let Some(data) = Data::from_filename(path) {
        return Some(Box::new(MemoryStream::from_data(Some(data))));
    }

    // If we get here, then our attempt at using mmap failed, so try normal file access.
    let stream = FileStream::new(path);
    if !stream.is_valid() {
        return None;
    }
    Some(Box::new(stream))
}

// ---------------------------------------------------------------------------------------------

/// The number of bytes [`WStream::write_packed_uint`] uses to store `value`.
// Port of: src/core/SkStream.cpp#L117-L124 (chrome/m156)
#[doc(alias = "SkWStream::SizeOfPackedUInt")]
#[doc(alias = "SizeOfPackedUInt")]
#[must_use]
pub fn size_of_packed_uint(value: usize) -> usize {
    if value <= SK_MAX_BYTE_FOR_U8 {
        1
    } else if value <= 0xFFFF {
        3
    } else {
        5
    }
}

/// A sink of bytes.
// Port of: include/core/SkStream.h#L222-L281 (chrome/m156)
#[doc(alias = "SkWStream")]
pub trait WStream {
    /// Called to write bytes to the stream. Returns true on success.
    fn write(&mut self, buffer: &[u8]) -> bool;

    /// Flushes any buffered bytes.
    // Port of: src/core/SkStream.cpp#L81-L83 (chrome/m156)
    fn flush(&mut self) {}

    /// The number of bytes written so far.
    #[doc(alias = "bytesWritten")]
    fn bytes_written(&self) -> usize;

    /// Writes one byte.
    // Port of: include/core/SkStream.h#L239-L242 (chrome/m156)
    fn write8(&mut self, value: u8) -> bool {
        self.write(&[value])
    }

    /// Writes a native-endian `u16`.
    // Port of: include/core/SkStream.h#L243-L246 (chrome/m156)
    fn write16(&mut self, value: u16) -> bool {
        self.write(&value.to_ne_bytes())
    }

    /// Writes a native-endian `u32`.
    // Port of: include/core/SkStream.h#L247-L249 (chrome/m156)
    fn write32(&mut self, value: u32) -> bool {
        self.write(&value.to_ne_bytes())
    }

    /// Writes a native-endian `u64`.
    // Port of: include/core/SkStream.h#L250-L252 (chrome/m156)
    fn write64(&mut self, value: u64) -> bool {
        self.write(&value.to_ne_bytes())
    }

    /// Writes the text's bytes (without a terminator).
    // Port of: include/core/SkStream.h#L254-L257 (chrome/m156)
    #[doc(alias = "writeText")]
    fn write_text(&mut self, text: &str) -> bool {
        self.write(text.as_bytes())
    }

    /// Writes `"\n"`.
    // Port of: include/core/SkStream.h#L259 (chrome/m156)
    fn newline(&mut self) -> bool {
        self.write(b"\n")
    }

    /// Writes `dec` as decimal text.
    // Port of: src/core/SkStream.cpp#L85-L90 (chrome/m156)
    #[doc(alias = "writeDecAsText")]
    fn write_dec_as_text(&mut self, dec: i32) -> bool {
        let mut text = String::new();
        str_append_s32(&mut text, dec);
        self.write(text.as_bytes())
    }

    /// Writes `dec` as decimal text, zero-padded to at least `min_digits` digits. (As in Skia,
    /// the value is formatted as unsigned.)
    // Port of: src/core/SkStream.cpp#L92-L97 (chrome/m156)
    #[doc(alias = "writeBigDecAsText")]
    #[allow(clippy::cast_sign_loss)] // mirrors passing an int64_t to SkStrAppendU64
    fn write_big_dec_as_text(&mut self, dec: i64, min_digits: i32) -> bool {
        let mut text = String::new();
        str_append_u64(&mut text, dec as u64, min_digits);
        self.write(text.as_bytes())
    }

    /// Writes `hex` as upper-case hexadecimal text, zero-padded to at least `digits` digits.
    // Port of: src/core/SkStream.cpp#L99-L104 (chrome/m156)
    #[doc(alias = "writeHexAsText")]
    fn write_hex_as_text(&mut self, hex: u32, digits: i32) -> bool {
        let mut text = String::new();
        str_append_hex(&mut text, hex, digits);
        self.write(text.as_bytes())
    }

    /// Writes `value` as decimal text (`%.8g`).
    // Port of: src/core/SkStream.cpp#L106-L111 (chrome/m156)
    #[doc(alias = "writeScalarAsText")]
    fn write_scalar_as_text(&mut self, value: scalar) -> bool {
        let mut text = String::new();
        str_append_scalar(&mut text, value);
        self.write(text.as_bytes())
    }

    /// Writes a bool as one byte.
    // Port of: include/core/SkStream.h#L266 (chrome/m156)
    #[doc(alias = "writeBool")]
    fn write_bool(&mut self, v: bool) -> bool {
        self.write8(u8::from(v))
    }

    /// Writes a native-endian scalar.
    // Port of: src/core/SkStream.cpp#L113-L115 (chrome/m156)
    #[doc(alias = "writeScalar")]
    fn write_scalar(&mut self, value: scalar) -> bool {
        self.write(&value.to_ne_bytes())
    }

    /// Writes `value` in 1, 3 or 5 bytes, depending on its magnitude (read it back with
    /// [`Stream::read_packed_uint`]).
    ///
    /// # Panics
    /// In debug builds, if `value` does not fit in 32 bits (`SkToU32`).
    // Port of: src/core/SkStream.cpp#L126-L144 (chrome/m156)
    #[doc(alias = "writePackedUInt")]
    fn write_packed_uint(&mut self, value: usize) -> bool {
        let mut data = [0u8; 5];
        let len = if value <= SK_MAX_BYTE_FOR_U8 {
            data[0] = u8::try_from(value).expect("checked above");
            1
        } else if value <= 0xFFFF {
            let value16 = u16::try_from(value).expect("checked above");
            data[0] = SK_BYTE_SENTINEL_FOR_U16;
            data[1..3].copy_from_slice(&value16.to_ne_bytes());
            3
        } else {
            let value32 = crate::to::to_u32(value);
            data[0] = SK_BYTE_SENTINEL_FOR_U32;
            data[1..5].copy_from_slice(&value32.to_ne_bytes());
            5
        };
        self.write(&data[..len])
    }

    /// Copies `length` bytes from `stream`. As in Skia, a short read from `stream` is not
    /// detected: the (stale) scratch bytes are written for the missing part.
    // Port of: src/core/SkStream.cpp#L146-L162 (chrome/m156)
    #[doc(alias = "writeStream")]
    fn write_stream(&mut self, stream: &mut dyn Stream, mut length: usize) -> bool {
        let mut scratch = [0u8; 1024];
        let max = scratch.len();

        while length != 0 {
            let n = length.min(max);
            let _ = stream.read(&mut scratch[..n]);
            if !self.write(&scratch[..n]) {
                return false;
            }
            length -= n;
        }
        true
    }
}

/// A [`WStream`] that discards the bytes, only counting them.
// Port of: include/core/SkStream.h#L283-L293 (chrome/m156)
#[doc(alias = "SkNullWStream")]
#[derive(Debug, Default)]
pub struct NullWStream {
    bytes_written: usize,
}

impl NullWStream {
    /// A stream that has written nothing yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl WStream for NullWStream {
    fn write(&mut self, buffer: &[u8]) -> bool {
        self.bytes_written += buffer.len();
        true
    }

    fn flush(&mut self) {}

    fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

// ---------------------------------------------------------------------------------------------

/// `sk_qread`: reads from `file` at `offset` without relying on (or caring about) the file's
/// current position. `None` on error.
// Port of: src/ports/SkOSFile_posix.cpp#L119-L129 (chrome/m156)
fn qread(file: &File, buffer: &mut [u8], offset: usize) -> Option<usize> {
    let offset = u64::try_from(offset).ok()?;
    #[cfg(unix)]
    {
        std::os::unix::fs::FileExt::read_at(file, buffer, offset).ok()
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::FileExt::seek_read(file, buffer, offset).ok()
    }
    #[cfg(not(any(unix, windows)))]
    {
        use std::io::{Read, SeekFrom};
        let mut file = file;
        file.seek(SeekFrom::Start(offset)).ok()?;
        file.read(buffer).ok()
    }
}

/// `sk_fgetsize`: the size of the file.
fn fgetsize(file: &File) -> usize {
    file.metadata()
        .ok()
        .and_then(|metadata| usize::try_from(metadata.len()).ok())
        .unwrap_or(0)
}

/// A stream that reads a file. Duplicates and forks share the open file.
// Port of: include/core/SkStream.h#L298-L366 (chrome/m156)
#[doc(alias = "SkFILEStream")]
#[derive(Debug)]
pub struct FileStream {
    file: Option<Arc<File>>,
    // These are seek positions in the underlying file, not offsets into the stream.
    end: usize,
    start: usize,
    current: usize,
}

impl FileStream {
    // Port of: src/core/SkStream.cpp#L166-L174 (chrome/m156)
    fn with_current(file: Option<Arc<File>>, end: usize, start: usize, current: usize) -> Self {
        let start = start.min(end);
        Self {
            file,
            end,
            start,
            current: current.clamp(start, end),
        }
    }

    // Port of: src/core/SkStream.cpp#L176-L178 (chrome/m156)
    fn with_start(file: Option<Arc<File>>, end: usize, start: usize) -> Self {
        Self::with_current(file, end, start, start)
    }

    /// Opens the file at `path` for reading. Check [`FileStream::is_valid`].
    // Port of: src/core/SkStream.cpp#L194-L196 (chrome/m156)
    pub fn new(path: impl AsRef<Path>) -> Self {
        match File::open(path) {
            Ok(file) => Self::from_file(file),
            Err(_) => Self::with_start(None, 0, 0),
        }
    }

    /// Initializes the stream with an existing file. The current position of the file will be
    /// considered the beginning of the stream and the current end of the file will be the
    /// end. The file is closed when the stream is dropped.
    // Port of: src/core/SkStream.cpp#L188-L192 (chrome/m156)
    #[must_use]
    pub fn from_file(mut file: File) -> Self {
        let start = file
            .stream_position()
            .ok()
            .and_then(|position| usize::try_from(position).ok())
            .unwrap_or(0);
        let end = fgetsize(&file);
        Self::with_start(Some(Arc::new(file)), end, start)
    }

    /// Initializes the stream with an existing file. The current position of the file will be
    /// considered the beginning of the stream and `size` bytes later will be the end. The
    /// file is closed when the stream is dropped.
    // Port of: src/core/SkStream.cpp#L180-L186 (chrome/m156)
    #[must_use]
    pub fn from_file_with_size(mut file: File, size: usize) -> Self {
        let start = file
            .stream_position()
            .ok()
            .and_then(|position| usize::try_from(position).ok())
            .unwrap_or(0);
        Self::with_start(
            Some(Arc::new(file)),
            SafeMath::saturating_add(start, size),
            start,
        )
    }

    /// Opens the file at `path`, or returns `None` if it cannot be opened.
    // Port of: include/core/SkStream.h#L321-L324 (chrome/m156)
    #[doc(alias = "Make")]
    pub fn make(path: impl AsRef<Path>) -> Option<Box<FileStream>> {
        let stream = Self::new(path);
        stream.is_valid().then(|| Box::new(stream))
    }

    /// Returns true if the file could be opened.
    // Port of: include/core/SkStream.h#L327 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.file.is_some()
    }

    /// Closes this stream's reference to the file.
    // Port of: src/core/SkStream.cpp#L202-L207 (chrome/m156)
    pub fn close(&mut self) {
        self.file = None;
        self.end = 0;
        self.start = 0;
        self.current = 0;
    }

    /// Duplicates this stream, positioned at its beginning.
    // Port of: src/core/SkStream.cpp#L236-L238 (chrome/m156)
    #[must_use]
    pub fn duplicate(&self) -> Box<FileStream> {
        Box::new(Self::with_current(
            self.file.clone(),
            self.end,
            self.start,
            self.start,
        ))
    }

    /// Duplicates this stream, positioned where this stream is.
    // Port of: src/core/SkStream.cpp#L270-L272 (chrome/m156)
    #[must_use]
    pub fn fork(&self) -> Box<FileStream> {
        Box::new(Self::with_current(
            self.file.clone(),
            self.end,
            self.start,
            self.current,
        ))
    }
}

// Port of: src/core/SkStream.cpp#L209-L276 (chrome/m156)
impl Stream for FileStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let size = buffer.len().min(self.end - self.current);
        let Some(file) = &self.file else { return 0 };
        let Some(bytes_read) = qread(file, &mut buffer[..size], self.current) else {
            return 0;
        };
        self.current += bytes_read;
        bytes_read
    }

    fn skip(&mut self, size: usize) -> usize {
        let size = size.min(self.end - self.current);
        self.current += size;
        size
    }

    fn is_at_end(&self) -> bool {
        if self.current == self.end {
            return true;
        }
        self.current >= self.file.as_deref().map_or(0, fgetsize)
    }

    fn rewind(&mut self) -> bool {
        self.current = self.start;
        true
    }

    fn duplicate(&self) -> Option<Box<dyn Stream>> {
        Some(FileStream::duplicate(self))
    }

    fn fork(&self) -> Option<Box<dyn Stream>> {
        Some(FileStream::fork(self))
    }

    fn has_position(&self) -> bool {
        true
    }

    fn get_position(&self) -> usize {
        debug_assert!(self.current >= self.start);
        self.current - self.start
    }

    fn seek(&mut self, position: usize) -> bool {
        self.current = SafeMath::saturating_add(position, self.start).min(self.end);
        true
    }

    fn move_by(&mut self, offset: i64) -> bool {
        if offset < 0 {
            // `-offset` always fits in a u64; Skia also guards `long` min and `size_t` width.
            let back = offset.unsigned_abs();
            if usize::try_from(back).map_or(true, |back| back >= self.get_position()) {
                self.current = self.start;
            } else {
                self.current -= usize::try_from(back).expect("checked above");
            }
        } else if let Ok(forward) = usize::try_from(offset) {
            self.current = SafeMath::saturating_add(self.current, forward).min(self.end);
        } else {
            self.current = self.end;
        }

        debug_assert!(self.current >= self.start && self.current <= self.end);
        true
    }

    fn has_length(&self) -> bool {
        true
    }

    fn get_length(&self) -> usize {
        self.end - self.start
    }
}

impl StreamRewindable for FileStream {}
impl StreamSeekable for FileStream {}
impl StreamAsset for FileStream {
    fn duplicate_asset(&self) -> Box<dyn StreamAsset> {
        FileStream::duplicate(self)
    }

    fn fork_asset(&self) -> Box<dyn StreamAsset> {
        FileStream::fork(self)
    }
}

// ---------------------------------------------------------------------------------------------

/// A read-only view into a block of memory.
///
/// Skia's `SkMemoryStream(const void* data, size_t length, bool copyData)` constructors that
/// borrow the memory become [`MemoryStream::make_direct`] (memory that lives forever) and the
/// copying ones; to share memory use [`MemoryStream::from_data`].
// Port of: include/core/SkStream.h#L369-L440 (chrome/m156)
#[doc(alias = "SkMemoryStream")]
#[derive(Debug, Clone)]
pub struct MemoryStream {
    data: Data,
    offset: usize,
}

impl Default for MemoryStream {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStream {
    /// An empty stream.
    // Port of: src/core/SkStream.cpp#L288-L291 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self {
            data: Data::new_empty(),
            offset: 0,
        }
    }

    /// A stream over `length` bytes it allocates (zeroed; Skia leaves them uninitialised and
    /// lets the caller write through `getMemoryBase()`).
    // Port of: src/core/SkStream.cpp#L293-L296 (chrome/m156)
    #[must_use]
    pub fn with_length(length: usize) -> Self {
        Self {
            data: Data::new_uninitialized(length),
            offset: 0,
        }
    }

    /// A stream with a copy of `data`.
    // Port of: src/core/SkStream.cpp#L310-L312 (chrome/m156)
    #[doc(alias = "MakeCopy")]
    #[must_use]
    pub fn make_copy(data: &[u8]) -> Box<MemoryStream> {
        Box::new(Self::from_data(Some(Data::new_copy(data))))
    }

    /// A stream over memory that lives for the whole program, without a copy.
    // Port of: src/core/SkStream.cpp#L314-L316 (chrome/m156)
    #[doc(alias = "MakeDirect")]
    #[must_use]
    pub fn make_direct(data: &'static [u8]) -> Box<MemoryStream> {
        Box::new(Self::from_data(Some(Data::new_static(data))))
    }

    /// A stream with a shared reference to `data` (`None` is the same as empty data).
    // Port of: src/core/SkStream.cpp#L318-L320 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(data: Option<Data>) -> Box<MemoryStream> {
        Box::new(Self::from_data(data))
    }

    /// A stream that reads from the specified data (`None` is the same as empty data).
    // Port of: src/core/SkStream.cpp#L303-L308 (chrome/m156)
    #[must_use]
    pub fn from_data(data: Option<Data>) -> Self {
        Self {
            data: data.unwrap_or_default(),
            offset: 0,
        }
    }

    /// Resets the stream to a copy of `data`.
    // Port of: src/core/SkStream.cpp#L327-L330 (chrome/m156)
    #[doc(alias = "setMemory")]
    pub fn set_memory_copy(&mut self, data: &[u8]) {
        self.data = Data::new_copy(data);
        self.offset = 0;
    }

    /// Resets the stream to memory that lives for the whole program, without a copy.
    // Port of: src/core/SkStream.cpp#L327-L330 (chrome/m156)
    #[doc(alias = "setMemory")]
    pub fn set_memory_static(&mut self, data: &'static [u8]) {
        self.data = Data::new_static(data);
        self.offset = 0;
    }

    /// Replaces any memory buffer with the specified (owned) buffer.
    // Port of: src/core/SkStream.cpp#L322-L325 (chrome/m156)
    #[doc(alias = "setMemoryOwned")]
    pub fn set_memory_owned(&mut self, data: Vec<u8>) {
        self.data = Data::new_from_vec(data);
        self.offset = 0;
    }

    /// Resets the stream to read from `data` (`None` is the same as empty data).
    // Port of: src/core/SkStream.cpp#L332-L339 (chrome/m156)
    #[doc(alias = "setData")]
    pub fn set_data(&mut self, data: Option<Data>) {
        self.data = data.unwrap_or_default();
        self.offset = 0;
    }

    /// The data this stream reads from.
    // Port of: include/core/SkStream.h#L403 (chrome/m156)
    #[doc(alias = "getData")]
    #[must_use]
    pub fn data(&self) -> &Data {
        &self.data
    }

    /// The bytes from the current position to the end.
    // Port of: src/core/SkStream.cpp#L407-L409 (chrome/m156)
    #[doc(alias = "getAtPos")]
    #[must_use]
    pub fn get_at_pos(&self) -> &[u8] {
        &self.data.as_bytes()[self.offset..]
    }

    /// Duplicates this stream, positioned at its beginning.
    // Port of: src/core/SkStream.cpp#L374-L376 (chrome/m156)
    #[must_use]
    pub fn duplicate(&self) -> Box<MemoryStream> {
        Box::new(Self::from_data(Some(self.data.clone())))
    }

    /// Duplicates this stream, positioned where this stream is.
    // Port of: src/core/SkStream.cpp#L393-L397 (chrome/m156)
    #[must_use]
    pub fn fork(&self) -> Box<MemoryStream> {
        let mut that = self.duplicate();
        that.seek(self.offset);
        that
    }
}

// Port of: src/core/SkStream.cpp#L341-L409 (chrome/m156)
impl Stream for MemoryStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let data_size = self.data.size();

        debug_assert!(self.offset <= data_size);
        let size = buffer.len().min(data_size - self.offset);
        buffer[..size].copy_from_slice(&self.data.as_bytes()[self.offset..self.offset + size]);
        self.offset += size;
        size
    }

    fn skip(&mut self, size: usize) -> usize {
        let size = size.min(self.data.size() - self.offset);
        self.offset += size;
        size
    }

    fn peek(&mut self, buffer: &mut [u8]) -> usize {
        let current_offset = self.offset;
        let bytes_read = self.read(buffer);
        self.offset = current_offset;
        bytes_read
    }

    fn is_at_end(&self) -> bool {
        self.offset == self.data.size()
    }

    fn rewind(&mut self) -> bool {
        self.offset = 0;
        true
    }

    fn duplicate(&self) -> Option<Box<dyn Stream>> {
        Some(MemoryStream::duplicate(self))
    }

    fn fork(&self) -> Option<Box<dyn Stream>> {
        Some(MemoryStream::fork(self))
    }

    fn has_position(&self) -> bool {
        true
    }

    fn get_position(&self) -> usize {
        self.offset
    }

    fn seek(&mut self, position: usize) -> bool {
        self.offset = position.min(self.data.size());
        true
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // mirrors `fOffset + offset` (long converted to size_t)
    fn move_by(&mut self, offset: i64) -> bool {
        self.seek(self.offset.wrapping_add(offset as usize))
    }

    fn has_length(&self) -> bool {
        true
    }

    fn get_length(&self) -> usize {
        self.data.size()
    }

    // An empty `Data` has no memory base, as `SkData::MakeEmpty()` has a null `data()`.
    fn get_memory_base(&self) -> Option<&[u8]> {
        (!self.data.is_empty()).then(|| self.data.as_bytes())
    }

    fn get_data(&self) -> Option<Data> {
        Some(self.data.clone())
    }
}

impl StreamRewindable for MemoryStream {}
impl StreamSeekable for MemoryStream {}
impl StreamAsset for MemoryStream {
    fn duplicate_asset(&self) -> Box<dyn StreamAsset> {
        MemoryStream::duplicate(self)
    }

    fn fork_asset(&self) -> Box<dyn StreamAsset> {
        MemoryStream::fork(self)
    }
}
impl StreamMemory for MemoryStream {}

// ---------------------------------------------------------------------------------------------

/// A [`WStream`] that writes to a file.
// Port of: include/core/SkStream.h#L444-L462 (chrome/m156)
#[doc(alias = "SkFILEWStream")]
#[derive(Debug)]
pub struct FileWStream {
    file: Option<std::io::BufWriter<File>>,
    bytes_written: usize,
}

impl FileWStream {
    /// Creates (or truncates) the file at `path` for writing. Check [`FileWStream::is_valid`].
    // Port of: src/core/SkStream.cpp#L414-L417 (chrome/m156)
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            file: File::create(path).ok().map(std::io::BufWriter::new),
            bytes_written: 0,
        }
    }

    /// Returns true if the file could be opened (and no write has failed since).
    // Port of: include/core/SkStream.h#L451 (chrome/m156)
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.file.is_some()
    }

    /// Flushes and asks the OS to write the file to disk.
    // Port of: src/core/SkStream.cpp#L453-L459 (chrome/m156)
    pub fn fsync(&mut self) {
        self.flush();
        if let Some(file) = &self.file {
            let _ = file.get_ref().sync_all();
        }
    }
}

// Port of: src/core/SkStream.cpp#L426-L451 (chrome/m156)
impl WStream for FileWStream {
    fn write(&mut self, buffer: &[u8]) -> bool {
        let Some(file) = &mut self.file else {
            return false;
        };

        if file.write_all(buffer).is_err() {
            self.file = None;
            return false;
        }
        self.bytes_written += buffer.len();
        true
    }

    fn flush(&mut self) {
        if let Some(file) = &mut self.file {
            let _ = file.flush();
        }
    }

    fn bytes_written(&self) -> usize {
        self.bytes_written
    }
}

// ---------------------------------------------------------------------------------------------

/// `SkDynamicMemoryWStream_MinBlockSize`.
const DYNAMIC_MEMORY_WSTREAM_MIN_BLOCK_SIZE: usize = 4096;

/// `sizeof(SkDynamicMemoryWStream::Block)`: a next pointer plus the current and stop pointers.
const BLOCK_HEADER_SIZE: usize = 3 * size_of::<usize>();

/// One block of a [`DynamicMemoryWStream`]: `capacity` bytes of room, of which `data.len()`
/// are written.
// Port of: src/core/SkStream.cpp#L473-L495 (chrome/m156)
#[derive(Debug)]
struct Block {
    data: Vec<u8>,
    capacity: usize,
}

impl Block {
    fn avail(&self) -> usize {
        self.capacity - self.data.len()
    }

    fn written(&self) -> usize {
        self.data.len()
    }

    fn append(&mut self, data: &[u8]) {
        debug_assert!(self.avail() >= data.len());
        self.data.extend_from_slice(data);
    }
}

/// A [`WStream`] that grows a list of blocks in memory.
///
/// Skia's linked list of blocks is a `Vec` of blocks, with the same block sizes, so
/// [`DynamicMemoryWStream::bytes_written`], reads and detaches behave identically.
// Port of: include/core/SkStream.h#L464-L522 (chrome/m156)
#[doc(alias = "SkDynamicMemoryWStream")]
#[derive(Debug, Default)]
pub struct DynamicMemoryWStream {
    blocks: Vec<Block>,
    bytes_written_before_tail: usize,
}

impl DynamicMemoryWStream {
    /// An empty stream.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    // Port of: src/core/SkStream.cpp#L734-L753 (chrome/m156)
    fn validate(&self) {
        if cfg!(debug_assertions) {
            let mut bytes = 0;
            if let Some((_, before_tail)) = self.blocks.split_last() {
                bytes = before_tail.iter().map(Block::written).sum();
            }
            debug_assert_eq!(bytes, self.bytes_written_before_tail);
        }
    }

    /// Reads `buffer.len()` bytes starting `offset` bytes into the stream. Returns false, and
    /// writes nothing, if that range is not entirely written.
    // Port of: src/core/SkStream.cpp#L613-L633 (chrome/m156)
    pub fn read(&self, buffer: &mut [u8], mut offset: usize) -> bool {
        let mut count = buffer.len();
        if offset.wrapping_add(count) > self.bytes_written() {
            return false; // test does not partially modify
        }
        let mut written = 0;
        for block in &self.blocks {
            let size = block.written();
            if offset < size {
                let part = if offset + count > size {
                    size - offset
                } else {
                    count
                };
                buffer[written..written + part].copy_from_slice(&block.data[offset..offset + part]);
                if count <= part {
                    return true;
                }
                count -= part;
                written += part;
            }
            offset = offset.saturating_sub(size);
        }
        false
    }

    /// A more efficient version of `read(dst, 0)` for the whole stream: copies the
    /// `bytes_written()` bytes to the front of `dst`.
    ///
    /// # Panics
    /// If `dst` is shorter than `bytes_written()`.
    // Port of: src/core/SkStream.cpp#L635-L644 (chrome/m156)
    #[doc(alias = "copyTo")]
    pub fn copy_to(&self, dst: &mut [u8]) {
        let mut written = 0;
        for block in &self.blocks {
            let size = block.written();
            dst[written..written + size].copy_from_slice(&block.data);
            written += size;
        }
    }

    /// Writes the contents to `dst`.
    // Port of: src/core/SkStream.cpp#L646-L654 (chrome/m156)
    #[doc(alias = "writeToStream")]
    pub fn write_to_stream(&self, dst: &mut dyn WStream) -> bool {
        for block in &self.blocks {
            if !dst.write(&block.data) {
                return false;
            }
        }
        true
    }

    /// Equivalent to [`copy_to`](Self::copy_to) followed by [`reset`](Self::reset), but may
    /// save memory use. With `None`, just resets.
    ///
    /// # Panics
    /// If `dst` is shorter than `bytes_written()`.
    // Port of: src/core/SkStream.cpp#L672-L691 (chrome/m156)
    #[doc(alias = "copyToAndReset")]
    pub fn copy_to_and_reset(&mut self, dst: Option<&mut [u8]>) {
        let Some(dst) = dst else {
            self.reset();
            return;
        };
        // By looping through the source and freeing as we copy, we can reduce real memory use
        // with large streams.
        let mut written = 0;
        for block in std::mem::take(&mut self.blocks) {
            let len = block.written();
            dst[written..written + len].copy_from_slice(&block.data);
            written += len;
        }
        self.bytes_written_before_tail = 0;
    }

    /// Equivalent to [`write_to_stream`](Self::write_to_stream) followed by
    /// [`reset`](Self::reset), but may save memory use.
    // Port of: src/core/SkStream.cpp#L693-L709 (chrome/m156)
    #[doc(alias = "writeToAndReset")]
    pub fn write_to_and_reset(&mut self, dst: &mut dyn WStream) -> bool {
        // By looping through the source and freeing as we copy, we can reduce real memory use
        // with large streams.
        let mut dst_stream_good = true;
        for block in std::mem::take(&mut self.blocks) {
            if dst_stream_good && !dst.write(&block.data) {
                dst_stream_good = false;
            }
        }
        self.bytes_written_before_tail = 0;
        dst_stream_good
    }

    /// Like [`write_to_and_reset`](Self::write_to_and_reset), for a destination that is also a
    /// `DynamicMemoryWStream`: the implementation is constant time.
    // Port of: src/core/SkStream.cpp#L577-L593 (chrome/m156)
    #[doc(alias = "writeToAndReset")]
    pub fn write_to_and_reset_dynamic(&mut self, dst: &mut DynamicMemoryWStream) -> bool {
        if 0 == self.bytes_written() {
            return true;
        }
        if 0 == dst.bytes_written() {
            *dst = std::mem::take(self);
            return true;
        }
        let dst_tail_written = dst.blocks.last().map_or(0, Block::written);
        dst.bytes_written_before_tail += self.bytes_written_before_tail + dst_tail_written;
        dst.blocks.append(&mut self.blocks);
        self.bytes_written_before_tail = 0;
        true
    }

    /// Prepends this stream to `dst`, resetting this.
    // Port of: src/core/SkStream.cpp#L595-L610 (chrome/m156)
    #[doc(alias = "prependToAndReset")]
    pub fn prepend_to_and_reset(&mut self, dst: &mut DynamicMemoryWStream) {
        if 0 == self.bytes_written() {
            return;
        }
        if 0 == dst.bytes_written() {
            *dst = std::mem::take(self);
            return;
        }
        let tail_written = self.blocks.last().map_or(0, Block::written);
        dst.bytes_written_before_tail += self.bytes_written_before_tail + tail_written;
        let mut combined = std::mem::take(&mut self.blocks);
        combined.append(&mut dst.blocks);
        dst.blocks = combined;
        self.bytes_written_before_tail = 0;
    }

    /// Returns the contents as [`Data`], and then resets the stream.
    // Port of: src/core/SkStream.cpp#L711-L719 (chrome/m156)
    #[doc(alias = "detachAsData")]
    pub fn detach_as_data(&mut self) -> Data {
        let size = self.bytes_written();
        if 0 == size {
            return Data::new_empty();
        }
        let mut data = Data::new_uninitialized(size);
        self.copy_to_and_reset(data.writable_data());
        data
    }

    /// Returns the contents as a vector, and then resets the stream.
    // Port of: src/core/SkStream.cpp#L721-L732 (chrome/m156)
    #[doc(alias = "detachAsVector")]
    pub fn detach_as_vector(&mut self) -> Vec<u8> {
        let size = self.bytes_written();
        if 0 == size {
            return Vec::new();
        }

        let mut result = vec![0; size];
        self.copy_to_and_reset(Some(&mut result));
        result
    }

    /// Resets the stream, returning a reader stream with the current content.
    // Port of: src/core/SkStream.cpp#L893-L911 (chrome/m156)
    #[doc(alias = "detachAsStream")]
    pub fn detach_as_stream(&mut self) -> Box<dyn StreamAsset> {
        if self.blocks.is_empty() {
            // no need to reset.
            return MemoryStream::make(None);
        }
        if self.blocks.len() == 1 {
            // one block, may be worth shrinking.
            let block = &mut self.blocks[0];
            block.data.shrink_to_fit();
            block.capacity = block.data.len();
            debug_assert_eq!(0, self.bytes_written_before_tail);
        }
        let size = self.bytes_written();
        let blocks = std::mem::take(&mut self.blocks);
        self.reset();
        Box::new(BlockMemoryStream::new(Arc::new(blocks), size))
    }

    /// Resets the stream to its original, empty, state.
    // Port of: src/core/SkStream.cpp#L519-L528 (chrome/m156)
    pub fn reset(&mut self) {
        self.blocks.clear();
        self.bytes_written_before_tail = 0;
    }

    /// Writes zeros until the entire stream has written a multiple of 4 bytes.
    // Port of: src/core/SkStream.cpp#L656-L669 (chrome/m156)
    #[doc(alias = "padToAlign4")]
    pub fn pad_to_align4(&mut self) {
        // The contract is to write zeros until the entire stream has written a multiple of 4
        // bytes. Our Blocks are guaranteed always be (a) full (except the tail) and (b) a
        // multiple of 4 so it is sufficient to just examine the tail (if present).
        if let Some(tail) = self.blocks.last_mut() {
            let pad_bytes = (4 - tail.written() % 4) % 4;
            if pad_bytes != 0 {
                tail.append(&[0; 3][..pad_bytes]);
            }
        }
    }
}

// Port of: src/core/SkStream.cpp#L530-L575 (chrome/m156)
impl WStream for DynamicMemoryWStream {
    fn write(&mut self, buffer: &[u8]) -> bool {
        let mut buffer = buffer;
        if !buffer.is_empty() {
            if let Some(tail) = self.blocks.last_mut()
                && tail.avail() > 0
            {
                let size = tail.avail().min(buffer.len());
                tail.append(&buffer[..size]);
                buffer = &buffer[size..];
                if buffer.is_empty() {
                    return true;
                }
            }

            let mut size = buffer
                .len()
                .max(DYNAMIC_MEMORY_WSTREAM_MIN_BLOCK_SIZE - BLOCK_HEADER_SIZE);
            size = crate::align::align4(size); // ensure we're always a multiple of 4 (see pad_to_align4())

            let mut data = Vec::new();
            if data.try_reserve_exact(size).is_err() {
                self.validate();
                return false;
            }
            let mut block = Block {
                data,
                capacity: size,
            };
            block.append(buffer);

            if let Some(tail) = self.blocks.last() {
                self.bytes_written_before_tail += tail.written();
            }
            self.blocks.push(block);
            self.validate();
        }
        true
    }

    fn bytes_written(&self) -> usize {
        self.validate();

        if let Some(tail) = self.blocks.last() {
            return self.bytes_written_before_tail + tail.written();
        }
        0
    }
}

/// The reader that [`DynamicMemoryWStream::detach_as_stream`] returns (`SkBlockMemoryStream`):
/// walks the blocks the writer had, which are shared with its duplicates.
// Port of: src/core/SkStream.cpp#L773-L891 (chrome/m156)
#[derive(Debug)]
struct BlockMemoryStream {
    block_memory: Arc<Vec<Block>>,
    /// Index of the current block; `block_memory.len()` plays the role of a null pointer.
    current: usize,
    size: usize,
    offset: usize,
    current_offset: usize,
}

impl BlockMemoryStream {
    fn new(block_memory: Arc<Vec<Block>>, size: usize) -> Self {
        Self {
            block_memory,
            current: 0,
            size,
            offset: 0,
            current_offset: 0,
        }
    }

    fn duplicate_stream(&self) -> BlockMemoryStream {
        Self::new(self.block_memory.clone(), self.size)
    }

    fn fork_stream(&self) -> BlockMemoryStream {
        let mut that = self.duplicate_stream();
        that.current = self.current;
        that.offset = self.offset;
        that.current_offset = self.current_offset;
        that
    }

    /// `read(buffer, count)`, where `buffer` may be absent (skip).
    fn read_or_skip(&mut self, mut buffer: Option<&mut [u8]>, raw_count: usize) -> usize {
        let mut count = raw_count;
        if self.offset.wrapping_add(count) > self.size {
            count = self.size - self.offset;
        }
        let mut bytes_left_to_read = count;
        while let Some(current) = self.block_memory.get(self.current) {
            let bytes_left_in_current = current.written() - self.current_offset;
            let bytes_from_current = bytes_left_to_read.min(bytes_left_in_current);
            if let Some(out) = buffer.take() {
                let (head, tail) = out.split_at_mut(bytes_from_current);
                head.copy_from_slice(
                    &current.data[self.current_offset..self.current_offset + bytes_from_current],
                );
                buffer = Some(tail);
            }
            if bytes_left_to_read <= bytes_from_current {
                self.current_offset += bytes_from_current;
                self.offset += count;
                return count;
            }
            bytes_left_to_read -= bytes_from_current;
            self.current += 1;
            self.current_offset = 0;
        }
        debug_assert!(false);
        0
    }
}

impl Stream for BlockMemoryStream {
    fn read(&mut self, buffer: &mut [u8]) -> usize {
        let size = buffer.len();
        self.read_or_skip(Some(buffer), size)
    }

    fn skip(&mut self, size: usize) -> usize {
        self.read_or_skip(None, size)
    }

    fn is_at_end(&self) -> bool {
        self.offset == self.size
    }

    fn peek(&mut self, buff: &mut [u8]) -> usize {
        let bytes_to_peek = buff.len().min(self.size - self.offset);

        let mut bytes_left_to_peek = bytes_to_peek;
        let mut written = 0;
        let mut current = self.current;
        let mut current_offset = self.current_offset;
        while bytes_left_to_peek != 0 {
            let block = &self.block_memory[current];
            let bytes_from_current = (block.written() - current_offset).min(bytes_left_to_peek);
            buff[written..written + bytes_from_current]
                .copy_from_slice(&block.data[current_offset..current_offset + bytes_from_current]);
            bytes_left_to_peek -= bytes_from_current;
            written += bytes_from_current;
            current += 1;
            current_offset = 0;
        }
        bytes_to_peek
    }

    fn rewind(&mut self) -> bool {
        self.current = 0;
        self.offset = 0;
        self.current_offset = 0;
        true
    }

    fn duplicate(&self) -> Option<Box<dyn Stream>> {
        Some(Box::new(self.duplicate_stream()))
    }

    fn fork(&self) -> Option<Box<dyn Stream>> {
        Some(Box::new(self.fork_stream()))
    }

    fn has_position(&self) -> bool {
        true
    }

    fn get_position(&self) -> usize {
        self.offset
    }

    fn seek(&mut self, position: usize) -> bool {
        // If possible, skip forward.
        if position >= self.offset {
            let skip_amount = position - self.offset;
            return self.skip(skip_amount) == skip_amount;
        }
        // If possible, move backward within the current block.
        let move_back_amount = self.offset - position;
        if move_back_amount <= self.current_offset {
            self.current_offset -= move_back_amount;
            self.offset -= move_back_amount;
            return true;
        }
        // Otherwise rewind and move forward.
        self.rewind() && self.skip(position) == position
    }

    #[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)] // mirrors `fOffset + offset` (long converted to size_t)
    fn move_by(&mut self, offset: i64) -> bool {
        self.seek(self.offset.wrapping_add(offset as usize))
    }

    fn has_length(&self) -> bool {
        true
    }

    fn get_length(&self) -> usize {
        self.size
    }

    fn get_memory_base(&self) -> Option<&[u8]> {
        if let [only] = self.block_memory.as_slice() {
            return Some(&only.data);
        }
        None
    }
}

impl StreamRewindable for BlockMemoryStream {}
impl StreamSeekable for BlockMemoryStream {}
impl StreamAsset for BlockMemoryStream {
    fn duplicate_asset(&self) -> Box<dyn StreamAsset> {
        Box::new(self.duplicate_stream())
    }

    fn fork_asset(&self) -> Box<dyn StreamAsset> {
        Box::new(self.fork_stream())
    }
}
impl StreamMemory for BlockMemoryStream {}
