// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkJpegXmp.cpp and src/codec/SkJpegXmp.h (chrome/m156).

use skia_rust_core::data::Data;
use skia_rust_core::md5::{Digest, Md5};

use crate::xmp::Xmp;

// Port of: src/codec/SkJpegConstants.h#L47-L52 (chrome/m156), `kXMPStandardSig`, including the
// terminating NUL.
const K_XMP_STANDARD_SIG: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";
// Port of: src/codec/SkJpegConstants.h#L50-L53 (chrome/m156), `kXMPExtendedSig`.
const K_XMP_EXTENDED_SIG: &[u8] = b"http://ns.adobe.com/xmp/extension/\0";

const K_GUID_ASCII_SIZE: usize = 32;

// Port of: src/codec/SkJpegXmp.cpp#L19-L35 (chrome/m156), `read_xmp_standard`.
//
// Extract standard XMP metadata. The returned data shares the APP1 segment's bytes.
fn read_xmp_standard(decoder_app1_params: &[Data]) -> Option<Data> {
    let sig_size = K_XMP_STANDARD_SIG.len();
    // Iterate through the image's segments.
    for params in decoder_app1_params {
        let bytes = params.as_bytes();
        // Skip segments that don't have the right signature, or are too small.
        if bytes.len() <= sig_size || !bytes.starts_with(K_XMP_STANDARD_SIG) {
            continue;
        }
        return Some(Data::new_copy(&bytes[sig_size..]));
    }
    None
}

// Port of: src/codec/SkJpegXmp.cpp#L49-L115 (chrome/m156), `read_xmp_extended`.
//
// Extract and validate extended XMP metadata.
//
// See XMP Specification Part 3: Storage in files, Section 1.1.3.1: Extended XMP in JPEG: Each
// chunk is written into the JPEG file within a separate APP1 marker segment. Each ExtendedXMP
// marker segment contains:
//   - A null-terminated signature string
//   - A 128-bit GUID stored as a 32-byte ASCII hex string, capital A-F, no null termination. The
//     GUID is a 128-bit MD5 digest of the full ExtendedXMP serialization.
//   - The full length of the ExtendedXMP serialization as a 32-bit unsigned integer.
//   - The offset of this portion as a 32-bit unsigned integer.
//   - The portion of the ExtendedXMP
fn read_xmp_extended(decoder_app1_params: &[Data], guid_ascii: &[u8]) -> Option<Data> {
    let sig_size = K_XMP_EXTENDED_SIG.len();
    let full_length_size = 4;
    let offset_size = 4;
    let header_size = sig_size + K_GUID_ASCII_SIZE + full_length_size + offset_size;

    // Validate the provided ASCII guid.
    if guid_ascii.len() != K_GUID_ASCII_SIZE {
        // "Invalid ASCII GUID size."
        return None;
    }
    let mut guid_as_digest = Digest { data: [0; 16] };
    for (i, &c) in guid_ascii.iter().enumerate() {
        let digit: u8 = match c {
            b'0'..=b'9' => c - b'0',
            b'A'..=b'F' => c - b'A' + 10,
            _ => {
                // "GUID is not upper-case hex."
                return None;
            }
        };
        if i % 2 == 0 {
            guid_as_digest.data[i / 2] = 16 * digit;
        } else {
            guid_as_digest.data[i / 2] += digit;
        }
    }

    // Iterate through the image's segments.
    let mut full_length: u32 = 0;
    let mut parts: Vec<(u32, &[u8])> = Vec::new();
    for params in decoder_app1_params {
        let bytes = params.as_bytes();
        // Skip segments that don't have the right signature, or are too small.
        if bytes.len() <= header_size || !bytes.starts_with(K_XMP_EXTENDED_SIG) {
            continue;
        }
        // Ignore parts that do not match the expected GUID.
        let part_guid_ascii = &bytes[sig_size..sig_size + K_GUID_ASCII_SIZE];
        if part_guid_ascii != guid_ascii {
            // "Ignoring unexpected GUID."
            continue;
        }
        // Read the full length and the offset for this part (both big-endian).
        let length_at = sig_size + K_GUID_ASCII_SIZE;
        let part_full_length = u32::from_be_bytes(bytes[length_at..length_at + 4].try_into().ok()?);
        let part_offset = u32::from_be_bytes(bytes[length_at + 4..length_at + 8].try_into().ok()?);

        // If this is the first part, set our global full length size.
        if parts.is_empty() {
            full_length = part_full_length;
        }
        // Ensure all parts agree on the full length.
        if part_full_length != full_length {
            // "Multiple parts had different total lengths."
            return None;
        }
        // Add it to the list.
        parts.push((part_offset, &bytes[header_size..]));
    }

    if parts.is_empty() || full_length == 0 {
        return None;
    }

    // Sort the list of parts by offset.
    parts.sort_by_key(|&(offset, _)| offset);

    // Stitch the parts together. Fail if we find that they are not contiguous.
    let full_length = full_length as usize;
    let mut xmp_extended = vec![0u8; full_length];
    let mut current: usize = 0;
    for &(part_offset, part_data) in &parts {
        // Make sure the data is contiguous and doesn't overflow the buffer.
        if part_offset as usize != current {
            // "XMP extension parts not contiguous"
            return None;
        }
        if part_data.len() > full_length - current {
            // "XMP extension parts overflow"
            return None;
        }
        xmp_extended[current..current + part_data.len()].copy_from_slice(part_data);
        current += part_data.len();
    }

    // Make sure we wrote the full buffer.
    if current != full_length {
        // "XMP extension did not match full length."
        return None;
    }

    // Make sure the MD5 hash of the extended data matched the GUID.
    let mut md5 = Md5::new();
    md5.write_bytes(&xmp_extended);
    if md5.finish() != guid_as_digest {
        // "XMP extension did not hash to GUID."
        return None;
    }
    Some(Data::new_from_vec(xmp_extended))
}

// Port of: src/codec/SkJpegXmp.cpp#L173-L202 (chrome/m156), `SkJpegMakeXmp`.
//
// Find and parse all XMP metadata, given a list of all APP1 segment parameters.
#[doc(alias = "SkJpegMakeXmp")]
#[must_use]
pub fn make_xmp(decoder_app1_params: &[Data]) -> Option<Xmp> {
    let xmp_standard = read_xmp_standard(decoder_app1_params)?;
    let xmp = Xmp::make(&xmp_standard)?;

    // Extract the GUID (the MD5 hash) of the extended metadata.
    let Some(extended_guid) = xmp.extended_xmp_guid() else {
        return Some(xmp);
    };

    // Extract and validate the extended metadata from the JPEG structure.
    let Some(xmp_extended) = read_xmp_extended(decoder_app1_params, extended_guid.as_bytes())
    else {
        // "Extended XMP was indicated but failed to read or validate."
        return Some(xmp);
    };
    Xmp::make_with_extended(&xmp_standard, &xmp_extended)
}
