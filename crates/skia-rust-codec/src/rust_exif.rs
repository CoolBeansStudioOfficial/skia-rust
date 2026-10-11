// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: rust/exif/exif_parse.rs, rust/exif/FFI.rs and rust/exif/FFI.cpp (chrome/m156), the Rust
// EXIF parser of Skia (`SK_EXIF_PARSE_WITH_RUST`). The cxx bridge is not ported: `ExifMetadata`
// is a plain struct, with `Option` fields where the bridge had `has_*` flags, and
// `ToSkExifMetadata` is `to_sk_exif_metadata`.

//! The Rust EXIF parser: the fields of `SkExif::Metadata`, read from raw EXIF (TIFF) bytes.

use crate::exif::{Metadata, origin_from_value};

/// The largest EXIF blob the parser reads: 4 MiB. Prevents unbounded work on malformed input.
// Port of: rust/exif/FFI.rs#L70-L72 (MAX_EXIF_SIZE, chrome/m156)
const MAX_EXIF_SIZE: usize = 4 * 1024 * 1024;

/// Parsed EXIF metadata fields. Each field is `None` when the data does not contain it. Port of
/// `rust_exif::ExifMetadata`.
// Port of: rust/exif/FFI.rs#L14-L44 (ExifMetadata, chrome/m156)
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ExifMetadata {
    /// Image orientation (`SkEncodedOrigin`), values 1-8 per EXIF tag 0x0112.
    pub origin: Option<u32>,
    /// Apple HDR headroom (computed from `MakerNote` tags 33 and 48).
    pub hdr_headroom: Option<f32>,
    /// Resolution unit (EXIF tag 0x0128): 2 = inch, 3 = centimeter.
    pub resolution_unit: Option<u16>,
    /// X resolution (EXIF tag 0x011a).
    pub x_resolution: Option<f32>,
    /// Y resolution (EXIF tag 0x011b).
    pub y_resolution: Option<f32>,
    /// Image width in pixels (EXIF tag 0xa002, can be SHORT or LONG).
    pub pixel_x_dimension: Option<u32>,
    /// Image height in pixels (EXIF tag 0xa003, can be SHORT or LONG).
    pub pixel_y_dimension: Option<u32>,
}

/// Parses raw EXIF bytes into `out`. Returns `false` if the data cannot be parsed at all; returns
/// `true` otherwise, with the fields that were found set. Port of `rust_exif::parse_exif`.
// Port of: rust/exif/FFI.rs#L46-L68 (parse_exif, chrome/m156)
pub fn parse_exif(data: &[u8], out: &mut ExifMetadata) -> bool {
    *out = ExifMetadata::default();
    if data.is_empty() || data.len() > MAX_EXIF_SIZE {
        return false;
    }
    exif_parse::parse(data, out)
}

/// Converts the Rust-parsed fields into `SkExif::Metadata`: each field that was found is copied,
/// and an orientation outside 1-8 is ignored. Port of `rust_exif::ToSkExifMetadata`.
// Port of: rust/exif/FFI.cpp#L14-L44 (ToSkExifMetadata, chrome/m156)
#[doc(alias = "ToSkExifMetadata")]
pub fn to_sk_exif_metadata(rust_meta: &ExifMetadata, out: &mut Metadata) {
    // Orientation: cast the value (1-8) to `SkEncodedOrigin` after the range check.
    if let Some(origin) = rust_meta
        .origin
        .and_then(|v| u16::try_from(v).ok())
        .and_then(origin_from_value)
    {
        out.origin = Some(origin);
    }
    if let Some(h) = rust_meta.hdr_headroom {
        out.hdr_headroom = Some(h);
    }
    if let Some(unit) = rust_meta.resolution_unit {
        out.resolution_unit = Some(unit);
    }
    if let Some(x) = rust_meta.x_resolution {
        out.x_resolution = Some(x);
    }
    if let Some(y) = rust_meta.y_resolution {
        out.y_resolution = Some(y);
    }
    if let Some(w) = rust_meta.pixel_x_dimension {
        out.pixel_x_dimension = Some(w);
    }
    if let Some(h) = rust_meta.pixel_y_dimension {
        out.pixel_y_dimension = Some(h);
    }
}

/// Port of `exif_parse.rs`: the TIFF/EXIF walk behind [`parse_exif`].
// Port of: rust/exif/exif_parse.rs (chrome/m156)
mod exif_parse {
    use super::ExifMetadata;

    #[derive(Clone, Copy)]
    enum Endianness {
        Little,
        Big,
    }

    // Tag numbers parsed by this module.
    const TAG_ORIENTATION: u16 = 0x0112;
    const TAG_X_RESOLUTION: u16 = 0x011a;
    const TAG_Y_RESOLUTION: u16 = 0x011b;
    const TAG_RESOLUTION_UNIT: u16 = 0x0128;
    const TAG_MAKER_NOTE: u16 = 0x927c;
    const TAG_EXIF_IFD: u16 = 0x8769;
    const TAG_PIXEL_X_DIMENSION: u16 = 0xa002;
    const TAG_PIXEL_Y_DIMENSION: u16 = 0xa003;

    // TIFF type codes.
    const TYPE_SHORT: u16 = 3;
    const TYPE_LONG: u16 = 4;
    const TYPE_RATIONAL: u16 = 5;
    const TYPE_UNDEFINED: u16 = 7;

    /// Apple `MakerNote` signature (14 bytes).
    const APPLE_MAKER_NOTE_SIG: &[u8] = b"Apple iOS\x00\x00\x01MM";

    /// The size of one IFD entry.
    const ENTRY_SIZE: usize = 12;

    fn read_u16(data: &[u8], off: usize, endianness: Endianness) -> Option<u16> {
        let bytes: [u8; 2] = data.get(off..off + 2)?.try_into().ok()?;
        Some(match endianness {
            Endianness::Little => u16::from_le_bytes(bytes),
            Endianness::Big => u16::from_be_bytes(bytes),
        })
    }

    fn read_u32(data: &[u8], off: usize, endianness: Endianness) -> Option<u32> {
        let bytes: [u8; 4] = data.get(off..off + 4)?.try_into().ok()?;
        Some(match endianness {
            Endianness::Little => u32::from_le_bytes(bytes),
            Endianness::Big => u32::from_be_bytes(bytes),
        })
    }

    fn read_i32(data: &[u8], off: usize, endianness: Endianness) -> Option<i32> {
        let bytes: [u8; 4] = data.get(off..off + 4)?.try_into().ok()?;
        Some(match endianness {
            Endianness::Little => i32::from_le_bytes(bytes),
            Endianness::Big => i32::from_be_bytes(bytes),
        })
    }

    /// Computes Apple HDR headroom from the raw `MakerNote` IFD bytes. Formula:
    /// <https://developer.apple.com/documentation/appkit/images_and_pdf/applying_apple_hdr_effect_to_your_photos>
    // Port of: rust/exif/exif_parse.rs#L30-L92 (parse_apple_hdr_headroom, chrome/m156)
    fn parse_apple_hdr_headroom(data: &[u8]) -> Option<f32> {
        // The Apple MakerNote is big-endian only. No little-endian images using this data have
        // been observed (matches the comment in SkExif.cpp).
        if !data.starts_with(APPLE_MAKER_NOTE_SIG) {
            return None;
        }
        let ifd_start = APPLE_MAKER_NOTE_SIG.len();
        let entry_count = usize::from(read_u16(data, ifd_start, Endianness::Big)?);
        let entries_start = ifd_start + 2;

        let mut maker33: Option<f32> = None;
        let mut maker48: Option<f32> = None;
        for i in 0..entry_count {
            let off = entries_start + i * ENTRY_SIZE;
            let Some(entry) = data.get(off..off + ENTRY_SIZE) else {
                break;
            };
            let tag = read_u16(entry, 0, Endianness::Big)?;
            let type_ = read_u16(entry, 2, Endianness::Big)?;
            let count = read_u32(entry, 4, Endianness::Big)?;
            // Only tags 33 and 48, of type SRATIONAL (10) and count 1.
            if (tag == 33 || tag == 48) && type_ == 10 && count == 1 {
                let value_off = usize::try_from(read_u32(entry, 8, Endianness::Big)?).ok()?;
                let Some(numer) = read_i32(data, value_off, Endianness::Big) else {
                    continue;
                };
                let Some(denom) = read_i32(data, value_off + 4, Endianness::Big) else {
                    continue;
                };
                // The C++ parser divides as floats, which is what the casts below do.
                #[allow(clippy::cast_precision_loss)] // mirrors the C++ (float) conversions
                let value = if denom != 0 {
                    numer as f32 / denom as f32
                } else {
                    0.0
                };
                match tag {
                    33 if maker33.is_none() => maker33 = Some(value),
                    48 if maker48.is_none() => maker48 = Some(value),
                    _ => {}
                }
            }
        }

        let maker33 = maker33?;
        let maker48 = maker48.unwrap_or(0.0);
        let stops = if maker33 < 1.0 {
            if maker48 <= 0.01 {
                -20.0 * maker48 + 1.8
            } else {
                -0.101 * maker48 + 1.601
            }
        } else if maker48 <= 0.01 {
            -70.0 * maker48 + 3.0
        } else {
            -0.303 * maker48 + 2.303
        };
        Some(f32::powf(2.0, stops.max(0.0)))
    }

    /// Walks one IFD, filling in the fields it finds. Port of `parse_ifd`.
    // Port of: rust/exif/exif_parse.rs#L94-L220 (parse_ifd, chrome/m156)
    fn parse_ifd(
        data: &[u8],
        ifd_off: usize,
        endianness: Endianness,
        is_root: bool,
        out: &mut ExifMetadata,
    ) {
        let Some(count) = read_u16(data, ifd_off, endianness) else {
            return;
        };
        let entries_start = ifd_off + 2;
        let mut followed_exif_ifd = false;

        for i in 0..usize::from(count) {
            let e = entries_start + i * ENTRY_SIZE;
            // Read the four fields of the 12-byte IFD entry.
            let (Some(tag), Some(type_), Some(cnt), Some(val_field)) = (
                read_u16(data, e, endianness),
                read_u16(data, e + 2, endianness),
                read_u32(data, e + 4, endianness),
                read_u32(data, e + 8, endianness),
            ) else {
                break;
            };
            // The last four bytes are an inline value (if the total is at most four bytes), or a
            // file offset to the value. They are read as `u32` and used as an offset only when
            // the total is larger.
            match tag {
                // Orientation: SHORT, count 1, inline at e+8.
                TAG_ORIENTATION if out.origin.is_none() && type_ == TYPE_SHORT && cnt == 1 => {
                    if let Some(v) =
                        read_u16(data, e + 8, endianness).filter(|v| (1..=8).contains(v))
                    {
                        out.origin = Some(u32::from(v));
                    }
                }
                // ResolutionUnit: SHORT, count 1, inline at e+8.
                TAG_RESOLUTION_UNIT
                    if out.resolution_unit.is_none() && type_ == TYPE_SHORT && cnt == 1 =>
                {
                    if let Some(v) = read_u16(data, e + 8, endianness) {
                        out.resolution_unit = Some(v);
                    }
                }
                // XResolution: RATIONAL, count 1, 8 bytes, so the value field is a file offset.
                TAG_X_RESOLUTION
                    if out.x_resolution.is_none() && type_ == TYPE_RATIONAL && cnt == 1 =>
                {
                    out.x_resolution = read_rational(data, val_field, endianness);
                }
                // YResolution: RATIONAL, count 1, 8 bytes, so the value field is a file offset.
                TAG_Y_RESOLUTION
                    if out.y_resolution.is_none() && type_ == TYPE_RATIONAL && cnt == 1 =>
                {
                    out.y_resolution = read_rational(data, val_field, endianness);
                }
                // PixelXDimension: SHORT or LONG per the Exif 2.3 spec, count 1, inline at e+8.
                TAG_PIXEL_X_DIMENSION if out.pixel_x_dimension.is_none() && cnt == 1 => {
                    out.pixel_x_dimension = read_dimension(data, e, type_, endianness);
                }
                // PixelYDimension: SHORT or LONG per the Exif 2.3 spec, count 1, inline at e+8.
                TAG_PIXEL_Y_DIMENSION if out.pixel_y_dimension.is_none() && cnt == 1 => {
                    out.pixel_y_dimension = read_dimension(data, e, type_, endianness);
                }
                // MakerNote: UNDEFINED bytes, typically large, so the value field is a file
                // offset.
                TAG_MAKER_NOTE
                    if out.hdr_headroom.is_none() && type_ == TYPE_UNDEFINED && cnt > 4 =>
                {
                    let off = val_field as usize;
                    if let Some(bytes) = usize::try_from(cnt)
                        .ok()
                        .and_then(|len| data.get(off..off.checked_add(len)?))
                    {
                        out.hdr_headroom = parse_apple_hdr_headroom(bytes);
                    }
                }
                // ExifIFD pointer: LONG, count 1, so the value field is the sub-IFD offset. The
                // Exif specification permits only one in the root IFD. Following duplicates
                // could otherwise cost quadratic work on malformed input.
                TAG_EXIF_IFD if is_root && !followed_exif_ifd && type_ == TYPE_LONG && cnt == 1 => {
                    followed_exif_ifd = true;
                    if let Ok(sub_ifd) = usize::try_from(val_field) {
                        parse_ifd(data, sub_ifd, endianness, false, out);
                    }
                }
                _ => {}
            }
        }
    }

    /// The value of a RATIONAL at `off`, as `numerator / denominator`, or 0 if the denominator is
    /// zero. `None` if the value is out of bounds.
    // Port of: rust/exif/exif_parse.rs (the RATIONAL reads of parse_ifd, chrome/m156)
    fn read_rational(data: &[u8], off: u32, endianness: Endianness) -> Option<f32> {
        let off = usize::try_from(off).ok()?;
        let n = read_u32(data, off, endianness)?;
        let d = read_u32(data, off + 4, endianness)?;
        // The C++ parser divides as floats, which is what the casts below do.
        #[allow(clippy::cast_precision_loss)] // mirrors the C++ (float) conversions
        let value = if d != 0 { n as f32 / d as f32 } else { 0.0 };
        Some(value)
    }

    /// A pixel dimension stored inline as SHORT or LONG, at the entry at `e`.
    // Port of: rust/exif/exif_parse.rs (the PixelX/YDimension reads of parse_ifd, chrome/m156)
    fn read_dimension(data: &[u8], e: usize, type_: u16, endianness: Endianness) -> Option<u32> {
        match type_ {
            TYPE_SHORT => read_u16(data, e + 8, endianness).map(u32::from),
            TYPE_LONG => read_u32(data, e + 8, endianness),
            _ => None,
        }
    }

    /// Parses a raw TIFF-formatted EXIF blob into `out`. Returns `true` if the header is valid;
    /// `false` otherwise. Missing individual fields are left `None`.
    // Port of: rust/exif/exif_parse.rs#L222-L246 (parse, chrome/m156)
    pub(super) fn parse(data: &[u8], out: &mut ExifMetadata) -> bool {
        if data.len() < 8 {
            return false;
        }
        let endianness = match data.get(0..2) {
            Some(b"MM") => Endianness::Big,
            Some(b"II") => Endianness::Little,
            _ => return false,
        };
        // TIFF magic: 42 (0x002A).
        if read_u16(data, 2, endianness) != Some(0x002A) {
            return false;
        }
        let Some(ifd_off) = read_u32(data, 4, endianness).and_then(|off| usize::try_from(off).ok())
        else {
            return false;
        };
        parse_ifd(data, ifd_off, endianness, true, out);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_data_returns_false() {
        let mut out = ExifMetadata::default();
        assert!(!parse_exif(&[], &mut out));
    }

    #[test]
    fn test_invalid_data_returns_false() {
        let mut out = ExifMetadata::default();
        assert!(!parse_exif(b"not valid exif data at all", &mut out));
        assert!(!parse_exif(b"MM\x00\x2a", &mut out));
    }
}
