// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegConstants.h (chrome/m156), the entries the metadata and segment code
// use. The other markers are listed where the decoder needs them.

/// `kJpegMarkerStartOfImage`.
// Port of: src/codec/SkJpegConstants.h#L16 (chrome/m156)
pub const JPEG_MARKER_START_OF_IMAGE: u8 = 0xD8;
/// `kJpegMarkerEndOfImage`.
// Port of: src/codec/SkJpegConstants.h#L17 (chrome/m156)
pub const JPEG_MARKER_END_OF_IMAGE: u8 = 0xD9;
/// `kJpegMarkerStartOfScan`.
// Port of: src/codec/SkJpegConstants.h#L18 (chrome/m156)
pub const JPEG_MARKER_START_OF_SCAN: u8 = 0xDA;
/// `kJpegMarkerAPP0`.
// Port of: src/codec/SkJpegConstants.h#L19 (chrome/m156)
pub const JPEG_MARKER_APP0: u8 = 0xE0;
/// `kJpegMarkerCodeSize`: the 0xFF and the marker byte.
// Port of: src/codec/SkJpegConstants.h#L20 (chrome/m156)
pub const JPEG_MARKER_CODE_SIZE: usize = 2;
/// `kJpegSegmentParameterLengthSize`: the two bytes of a segment's length.
// Port of: src/codec/SkJpegConstants.h#L21 (chrome/m156)
pub const JPEG_SEGMENT_PARAMETER_LENGTH_SIZE: usize = 2;

/// `kMpfMarker`: the APP2 segment that holds the Multi-Picture Format index.
// Port of: src/codec/SkJpegConstants.h#L59 (chrome/m156)
pub const MPF_MARKER: u32 = JPEG_MARKER_APP0 as u32 + 2;
/// `kMpfSig`, including the terminating NUL.
// Port of: src/codec/SkJpegConstants.h#L60 (chrome/m156)
pub const MPF_SIG: &[u8] = b"MPF\0";
