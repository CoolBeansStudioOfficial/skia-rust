// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: experimental/rust_jpeg/ffi/jpeg_metadata.rs (chrome/m156), the ICC profile and EXIF
// extraction from the scanned APP segments of a JPEG, for `SkJpegRustCodec`.

//! JPEG metadata extraction from scanned segments.
//!
//! Extracts ICC profiles and EXIF data from JPEG APP marker segments that have been parsed by the
//! segment scanner.

use crate::jpeg_segment_scan::{ScannedSegment, get_segment_params, jpeg_marker};

/// JPEG ICC profile signature: `ICC_PROFILE` followed by a NUL.
// Port of: experimental/rust_jpeg/ffi/jpeg_metadata.rs (ICC_SIG, chrome/m156)
const ICC_SIG: &[u8] = b"ICC_PROFILE\0";

/// Size of the ICC marker header: signature (12) + sequence number (1) + total count (1).
// Port of: experimental/rust_jpeg/ffi/jpeg_metadata.rs (ICC_HEADER_SIZE, chrome/m156)
const ICC_HEADER_SIZE: usize = 14;

/// EXIF APP1 signature: `Exif` followed by two NULs.
// Port of: experimental/rust_jpeg/ffi/jpeg_metadata.rs (EXIF_SIG, chrome/m156)
const EXIF_SIG: &[u8] = b"Exif\0\0";

/// Extract and reassemble a multi-segment ICC profile from scanned APP2 segments.
// Port of: experimental/rust_jpeg/ffi/jpeg_metadata.rs#L17-L72 (extract_icc_profile, chrome/m156)
pub(crate) fn extract_icc_profile(raw_data: &[u8], segments: &[ScannedSegment]) -> Vec<u8> {
    let mut parts: Vec<Option<Vec<u8>>> = Vec::new();
    let mut found_part_count = 0usize;
    let mut expected_part_count = 0u8;

    for seg in segments {
        if seg.marker != jpeg_marker::APP0 + 2 {
            continue;
        }
        let param_data = get_segment_params(raw_data, seg);
        if param_data.len() <= ICC_HEADER_SIZE {
            continue;
        }
        if &param_data[..ICC_SIG.len()] != ICC_SIG {
            continue;
        }

        let seq_num = param_data[12];
        let part_count = param_data[13];
        if part_count == 0 || seq_num == 0 || seq_num > part_count {
            return Vec::new();
        }
        if expected_part_count == 0 {
            expected_part_count = part_count;
            parts.resize_with(usize::from(expected_part_count), || None);
        } else if part_count != expected_part_count {
            return Vec::new();
        }

        let part_index = usize::from(seq_num - 1);
        if parts[part_index].is_some() {
            return Vec::new();
        }
        let payload = &param_data[ICC_HEADER_SIZE..];
        parts[part_index] = Some(payload.to_vec());
        found_part_count += 1;
        if found_part_count == usize::from(expected_part_count) {
            break;
        }
    }

    if expected_part_count == 0 || found_part_count != usize::from(expected_part_count) {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(parts.iter().flatten().map(Vec::len).sum());
    for part in parts {
        let Some(data) = part else {
            return Vec::new();
        };
        result.extend_from_slice(&data);
    }
    result
}

/// Extract raw EXIF data (the TIFF portion after the "Exif\0\0" prefix) from the first APP1
/// segment with an EXIF signature.
// Port of: experimental/rust_jpeg/ffi/jpeg_metadata.rs#L74-L88 (extract_exif_data, chrome/m156)
pub(crate) fn extract_exif_data(raw_data: &[u8], segments: &[ScannedSegment]) -> Vec<u8> {
    for seg in segments {
        if seg.marker != jpeg_marker::APP0 + 1 {
            continue;
        }
        let param_data = get_segment_params(raw_data, seg);
        if param_data.len() < EXIF_SIG.len() {
            continue;
        }
        if &param_data[..EXIF_SIG.len()] == EXIF_SIG {
            return param_data[EXIF_SIG.len()..].to_vec();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jpeg_segment_scan::SegmentScanner;

    fn append_icc_segment(data: &mut Vec<u8>, seq_num: u8, part_count: u8, payload: &[u8]) {
        data.extend_from_slice(&[0xFF, 0xE2]); // APP2
        let param_len = u16::try_from(ICC_HEADER_SIZE + payload.len() + 2).unwrap();
        data.extend_from_slice(&param_len.to_be_bytes());
        data.extend_from_slice(ICC_SIG);
        data.push(seq_num);
        data.push(part_count);
        data.extend_from_slice(payload);
    }

    fn build_icc_jpeg(parts: &[(u8, u8, &[u8])]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8]); // SOI
        for (seq_num, part_count, payload) in parts {
            append_icc_segment(&mut data, *seq_num, *part_count, payload);
        }
        data
    }

    #[test]
    fn test_extract_icc_profile_basic() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8]); // SOI
        append_icc_segment(&mut data, 1, 1, b"test_icc_data");

        let mut scanner = SegmentScanner::new(jpeg_marker::SOS);
        scanner.on_bytes(&data);
        let icc = extract_icc_profile(&data, &scanner.segments);
        assert_eq!(icc, b"test_icc_data");
    }

    #[test]
    fn test_extract_exif_data_basic() {
        let mut data = Vec::new();
        data.extend_from_slice(&[0xFF, 0xD8]); // SOI
        data.extend_from_slice(&[0xFF, 0xE1]); // APP1
        let tiff_data = b"some_tiff_content";
        let param_len = u16::try_from(EXIF_SIG.len() + tiff_data.len() + 2).unwrap();
        data.extend_from_slice(&param_len.to_be_bytes());
        data.extend_from_slice(EXIF_SIG);
        data.extend_from_slice(tiff_data);

        let mut scanner = SegmentScanner::new(jpeg_marker::SOS);
        scanner.on_bytes(&data);
        let exif = extract_exif_data(&data, &scanner.segments);
        assert_eq!(exif, tiff_data);
    }

    #[test]
    fn test_extract_icc_profile_multi_segment() {
        // Two APP2 ICC segments, out of order, that must be reassembled by sequence number.
        let data = build_icc_jpeg(&[(2, 2, b"SECOND_CHUNK"), (1, 2, b"FIRST_CHUNK_")]);

        let mut scanner = SegmentScanner::new(jpeg_marker::SOS);
        scanner.on_bytes(&data);
        let icc = extract_icc_profile(&data, &scanner.segments);
        assert_eq!(icc, b"FIRST_CHUNK_SECOND_CHUNK");
    }

    #[test]
    fn test_extract_icc_profile_rejects_invalid_multipart_sets() {
        for data in [
            build_icc_jpeg(&[(1, 0, b"part")]),
            build_icc_jpeg(&[(0, 1, b"part")]),
            build_icc_jpeg(&[(2, 1, b"part")]),
            build_icc_jpeg(&[(1, 2, b"one"), (1, 2, b"duplicate")]),
            build_icc_jpeg(&[(1, 2, b"one"), (2, 3, b"two")]),
            build_icc_jpeg(&[(1, 2, b"one")]),
        ] {
            let mut scanner = SegmentScanner::new(jpeg_marker::SOS);
            scanner.on_bytes(&data);
            let icc: Vec<u8> = extract_icc_profile(&data, &scanner.segments);
            assert_eq!(icc, Vec::<u8>::new());
        }
    }
}
