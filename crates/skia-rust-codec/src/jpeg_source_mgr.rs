// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegSourceMgr.cpp and src/codec/SkJpegSourceMgr.h (chrome/m156), the
// segment and subset access of the source manager (`getAllSegments`, `getSubsetData`,
// `getSegmentParameters`).
//
// Not ported here: the libjpeg data-source hooks (`initSource`, `fillInputBuffer`,
// `skipInputBytes`). The JPEG decoder keeps the encoded bytes in its own memory source
// (`jpeg_codec`), so libjpeg never reads through this type.
//
// The C++ `wasCopied` out-parameter is not reported: the data returned here is always owned.

use skia_rust_core::data::Data;
use skia_rust_core::stream::Stream;

use crate::jpeg_constants::{
    JPEG_MARKER_CODE_SIZE, JPEG_MARKER_END_OF_IMAGE, JPEG_SEGMENT_PARAMETER_LENGTH_SIZE,
};
use crate::jpeg_segment_scan::{JpegSegment, JpegSegmentScanner};

/// Runs `f` with the stream rewound, then restores its position (`ScopedSkStreamRestorer`).
// Port of: src/codec/SkJpegSourceMgr.cpp#L19-L40 (chrome/m156), `ScopedSkStreamRestorer`.
fn with_rewound<T>(stream: &mut dyn Stream, f: impl FnOnce(&mut dyn Stream) -> T) -> T {
    let position = stream.get_position();
    // Failures are only logged by Skia ("Failed to rewind decoder stream.").
    let _ = stream.rewind();
    let result = f(stream);
    // Failures are only logged by Skia ("Failed to restore decoder stream.").
    let _ = stream.seek(position);
    result
}

/// How the bytes of the stream are accessed (the subclasses of `SkJpegSourceMgr`).
enum Kind {
    /// The whole stream is in memory (`SkJpegMemorySourceMgr`). The bytes are copied once.
    Memory(Vec<u8>),
    /// A seekable stream read through a buffer (`SkJpegBufferedSourceMgr`).
    Buffered,
    /// A stream that cannot seek. The data is scanned as it is read
    /// (`SkJpegUnseekableSourceMgr`).
    Unseekable(UnseekableState),
}

/// The state of an unseekable stream: the last buffer read, and where it starts.
struct UnseekableState {
    buffer: Vec<u8>,
    last_read_size: usize,
    last_read_offset: usize,
}

/// Gives the segments and the bytes of a JPEG file, read from a stream
/// (`SkJpegSourceMgr`).
#[doc(alias = "SkJpegSourceMgr")]
pub struct JpegSourceMgr<'a> {
    stream: &'a mut dyn Stream,
    kind: Kind,
    buffer_size: usize,
    scanner: Option<JpegSegmentScanner>,
}

impl std::fmt::Debug for JpegSourceMgr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JpegSourceMgr")
            .field("buffer_size", &self.buffer_size)
            .finish_non_exhaustive()
    }
}

impl<'a> JpegSourceMgr<'a> {
    /// The source for `stream`, reading through a buffer of `buffer_size` bytes when needed
    /// (`SkJpegSourceMgr::Make`).
    // Port of: src/codec/SkJpegSourceMgr.cpp#L425-L435 (chrome/m156), `SkJpegSourceMgr::Make`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(stream: &'a mut dyn Stream, buffer_size: usize) -> Self {
        let memory = if stream.has_position() && stream.has_length() {
            stream.get_memory_base().map(<[u8]>::to_vec)
        } else {
            None
        };
        let kind = if !stream.has_position() {
            Kind::Unseekable(UnseekableState {
                buffer: vec![0; buffer_size],
                last_read_size: 0,
                last_read_offset: 0,
            })
        } else if let Some(memory) = memory {
            Kind::Memory(memory)
        } else {
            Kind::Buffered
        };
        let scanner = matches!(kind, Kind::Unseekable(_))
            .then(|| JpegSegmentScanner::new(JPEG_MARKER_END_OF_IMAGE));
        Self {
            stream,
            kind,
            buffer_size,
            scanner,
        }
    }

    /// All the marker segments up to `EndOfImage` (`getAllSegments`).
    // Port of: src/codec/SkJpegSourceMgr.cpp#L62-L72 (Memory), #L121-L137 (Buffered) and
    // #L220-L226 (Unseekable) (chrome/m156).
    pub fn get_all_segments(&mut self) -> &[JpegSegment] {
        match &mut self.kind {
            Kind::Memory(memory) => {
                if self.scanner.is_none() {
                    let mut scanner = JpegSegmentScanner::new(JPEG_MARKER_END_OF_IMAGE);
                    scanner.on_bytes(memory);
                    self.scanner = Some(scanner);
                }
            }
            Kind::Buffered => {
                if self.scanner.is_none() {
                    let stream = &mut *self.stream;
                    let scanner = with_rewound(stream, |stream| {
                        let mut scanner = JpegSegmentScanner::new(JPEG_MARKER_END_OF_IMAGE);
                        let mut buffer = [0u8; 1024];
                        while !scanner.is_done() && !scanner.had_error() {
                            let bytes_read = stream.read(&mut buffer);
                            if bytes_read == 0 {
                                // "Unexpected EOF."
                                break;
                            }
                            scanner.on_bytes(&buffer[..bytes_read]);
                        }
                        scanner
                    });
                    self.scanner = Some(scanner);
                }
            }
            Kind::Unseekable(state) => {
                let scanner = self.scanner.as_mut();
                if let Some(scanner) = scanner {
                    while !scanner.is_done() && !scanner.had_error() {
                        if !read_to_buffer_and_scan(
                            &mut *self.stream,
                            state,
                            scanner,
                            self.buffer_size,
                        ) {
                            // "Failure finishing unseekable input buffer."
                            break;
                        }
                    }
                }
            }
        }
        self.scanner
            .as_ref()
            .map_or(&[][..], JpegSegmentScanner::segments)
    }

    /// `size` bytes at `offset` from the start of the stream (`getSubsetData`).
    // Port of: src/codec/SkJpegSourceMgr.cpp#L74-L86 (Memory), #L139-L152 (Buffered) and
    // #L229-L290 (Unseekable) (chrome/m156).
    pub fn get_subset_data(&mut self, offset: usize, size: usize) -> Option<Data> {
        match &mut self.kind {
            Kind::Memory(memory) => {
                if offset > memory.len() || size > memory.len() - offset {
                    return None;
                }
                Some(Data::new_copy(&memory[offset..offset + size]))
            }
            Kind::Buffered => {
                let stream = &mut *self.stream;
                with_rewound(stream, |stream| {
                    if !stream.seek(offset) {
                        // "Failed to seek to subset stream position."
                        return None;
                    }
                    let mut bytes = vec![0u8; size];
                    if stream.read(&mut bytes) != size {
                        // "Failed to read subset stream data."
                        return None;
                    }
                    Some(Data::new_from_vec(bytes))
                })
            }
            // Skia logs "getSubsetData is prematurely terminating scan." when the scan is not done.
            Kind::Unseekable(state) => unseekable_subset(&mut *self.stream, state, offset, size),
        }
    }

    /// The parameters of `segment`, without its marker and length (`getSegmentParameters`).
    // Port of: src/codec/SkJpegSourceMgr.cpp#L88-L110 (Memory), #L154-L186 (Buffered) and
    // #L288-L290 (Unseekable) (chrome/m156).
    pub fn get_segment_parameters(&mut self, segment: &JpegSegment) -> Option<Data> {
        let parameter_length = usize::from(segment.parameter_length);
        match &mut self.kind {
            Kind::Memory(memory) => {
                if parameter_length <= JPEG_SEGMENT_PARAMETER_LENGTH_SIZE {
                    return None;
                }
                let start =
                    segment.offset + JPEG_MARKER_CODE_SIZE + JPEG_SEGMENT_PARAMETER_LENGTH_SIZE;
                let end = segment.offset + parameter_length + JPEG_MARKER_CODE_SIZE;
                memory.get(start..end).map(Data::new_copy)
            }
            Kind::Buffered => {
                if parameter_length <= JPEG_SEGMENT_PARAMETER_LENGTH_SIZE {
                    return None;
                }
                let stream = &mut *self.stream;
                with_rewound(stream, |stream| {
                    if !stream.seek(segment.offset) {
                        // "Failed to seek to segment"
                        return None;
                    }
                    let mut marker_code = [0u8; JPEG_MARKER_CODE_SIZE];
                    if stream.read(&mut marker_code) != JPEG_MARKER_CODE_SIZE {
                        // "Failed to read segment marker code"
                        return None;
                    }
                    let mut length_bytes = [0u8; JPEG_SEGMENT_PARAMETER_LENGTH_SIZE];
                    if stream.read(&mut length_bytes) != JPEG_SEGMENT_PARAMETER_LENGTH_SIZE {
                        // "Failed to read parameter length"
                        return None;
                    }
                    let size_to_read = parameter_length - JPEG_SEGMENT_PARAMETER_LENGTH_SIZE;
                    let mut result = vec![0u8; size_to_read];
                    if stream.read(&mut result) != size_to_read {
                        return None;
                    }
                    Some(Data::new_from_vec(result))
                })
            }
            Kind::Unseekable(_) => None,
        }
    }
}

/// Reads up to `bytes_to_read` bytes into the buffer and scans them
/// (`readToBufferAndScan`).
// Port of: src/codec/SkJpegSourceMgr.cpp#L360-L372 (chrome/m156), `readToBufferAndScan`.
fn read_to_buffer_and_scan(
    stream: &mut dyn Stream,
    state: &mut UnseekableState,
    scanner: &mut JpegSegmentScanner,
    bytes_to_read: usize,
) -> bool {
    state.last_read_offset += state.last_read_size;
    let len = bytes_to_read.min(state.buffer.len());
    state.last_read_size = stream.read(&mut state.buffer[..len]);
    if state.last_read_size == 0 {
        // "Hit end of file reading an unseekable stream."
        return false;
    }
    scanner.on_bytes(&state.buffer[..state.last_read_size]);
    true
}

/// `getSubsetData` of the unseekable source: the bytes can only come forward, so the data already
/// read is reused and the rest is skipped or read.
// Port of: src/codec/SkJpegSourceMgr.cpp#L229-L286 (chrome/m156), the unseekable `getSubsetData`.
fn unseekable_subset(
    stream: &mut dyn Stream,
    state: &mut UnseekableState,
    offset: usize,
    size: usize,
) -> Option<Data> {
    if offset < state.last_read_offset {
        // "Requested that is gone."
        return None;
    }
    let mut subset = vec![0u8; size];
    let mut current = 0usize;
    let mut size_left = size;
    let offset_into_buffer = offset - state.last_read_offset;
    if offset_into_buffer >= state.last_read_size {
        state.last_read_offset += state.last_read_size;
        state.last_read_size = 0;
        let mut bytes_to_skip = offset - state.last_read_offset;
        while bytes_to_skip > 0 {
            let bytes_skipped = stream.skip(bytes_to_skip);
            if bytes_skipped == 0 {
                // "Failed to skip bytes before subset."
                return None;
            }
            bytes_to_skip -= bytes_skipped;
            state.last_read_offset += bytes_skipped;
        }
    } else {
        let bytes_to_read_from_buffer = (state.last_read_size - offset_into_buffer).min(size_left);
        subset[..bytes_to_read_from_buffer].copy_from_slice(
            &state.buffer[offset_into_buffer..offset_into_buffer + bytes_to_read_from_buffer],
        );
        size_left -= bytes_to_read_from_buffer;
        current += bytes_to_read_from_buffer;
        if size_left == 0 {
            return Some(Data::new_from_vec(subset));
        }
        state.last_read_offset += state.last_read_size;
        state.last_read_size = 0;
    }
    while size_left > 0 {
        let bytes_read = stream.read(&mut subset[current..current + size_left]);
        if bytes_read == 0 {
            // "Failed to read subset stream data."
            return None;
        }
        size_left -= bytes_read;
        current += bytes_read;
        state.last_read_offset += bytes_read;
    }
    Some(Data::new_from_vec(subset))
}
