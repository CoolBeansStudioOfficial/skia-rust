// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/codec/SkTiffUtility.h and src/codec/SkTiffUtility.cpp (chrome/m156)
//
// The TIFF image file directory reader that EXIF parsing uses. Truncated input is read as far as it
// goes when `allow_truncated` is set. Reads outside the data are reported as failures, where Skia
// reads out of bounds only when its own bounds checks have already passed.

use std::sync::Arc;

/// Port of `SkTiff::kEndianSize`. The signatures are `MM\0*` (big endian) and `II*\0` (little).
// Port of: src/codec/SkTiffUtility.h#L13-L16 (kEndianSize, kEndianBig, kEndianLittle)
pub const ENDIAN_SIZE: usize = 4;

/// Port of `SkTiff::kTypeUnsignedByte` and the other type constants.
// Port of: src/codec/SkTiffUtility.h#L19-L31 (kType*)
pub const TYPE_UNSIGNED_BYTE: u16 = 1;
pub const TYPE_ASCII_STRING: u16 = 2;
pub const TYPE_UNSIGNED_SHORT: u16 = 3;
pub const TYPE_UNSIGNED_LONG: u16 = 4;
pub const TYPE_UNSIGNED_RATIONAL: u16 = 5;
pub const TYPE_SIGNED_BYTE: u16 = 6;
pub const TYPE_UNDEFINED: u16 = 7;
pub const TYPE_SIGNED_SHORT: u16 = 8;
pub const TYPE_SIGNED_LONG: u16 = 9;
pub const TYPE_SIGNED_RATIONAL: u16 = 10;
pub const TYPE_SINGLE_FLOAT: u16 = 11;
pub const TYPE_DOUBLE_FLOAT: u16 = 12;

/// Port of `SkTiff::kSizeEntry`, `kSizeShort` and `kSizeLong`.
// Port of: src/codec/SkTiffUtility.h#L33-L35 (kSizeEntry, kSizeShort, kSizeLong)
const SIZE_ENTRY: usize = 12;
const SIZE_SHORT: usize = 2;
const SIZE_LONG: usize = 4;

/// Port of `SkCodecPriv::GetEndianShort`.
// Port of: src/codec/SkCodecPriv.h#L279-L285 (GetEndianShort)
fn get_endian_short(data: &[u8], little_endian: bool) -> u16 {
    if little_endian {
        (u16::from(data[1]) << 8) | u16::from(data[0])
    } else {
        (u16::from(data[0]) << 8) | u16::from(data[1])
    }
}

/// Port of `SkCodecPriv::GetEndianInt`.
// Port of: src/codec/SkCodecPriv.h#L287-L293 (GetEndianInt)
fn get_endian_int(data: &[u8], little_endian: bool) -> u32 {
    if little_endian {
        (u32::from(data[3]) << 24)
            | (u32::from(data[2]) << 16)
            | (u32::from(data[1]) << 8)
            | u32::from(data[0])
    } else {
        (u32::from(data[0]) << 24)
            | (u32::from(data[1]) << 16)
            | (u32::from(data[2]) << 8)
            | u32::from(data[3])
    }
}

/// Port of `SkCodecPriv::IsValidEndianMarker`: `II` is little endian, `MM` big endian.
// Port of: src/codec/SkCodecPriv.h#L269-L277 (IsValidEndianMarker)
fn is_valid_endian_marker(data: &[u8]) -> Option<bool> {
    match (data[0], data[1]) {
        (b'I', b'I') => Some(true),
        (b'M', b'M') => Some(false),
        _ => None,
    }
}

/// Port of `SkTiff::ImageFileDirectory::BytesForType`.
// Port of: src/codec/SkTiffUtility.cpp#L16-L44 (BytesForType)
fn bytes_for_type(tag_type: u16) -> usize {
    match tag_type {
        TYPE_UNSIGNED_BYTE | TYPE_ASCII_STRING | TYPE_SIGNED_BYTE | TYPE_UNDEFINED => 1,
        TYPE_UNSIGNED_SHORT | TYPE_SIGNED_SHORT => SIZE_SHORT,
        TYPE_UNSIGNED_LONG | TYPE_SIGNED_LONG | TYPE_SINGLE_FLOAT => SIZE_LONG,
        TYPE_UNSIGNED_RATIONAL | TYPE_SIGNED_RATIONAL | TYPE_DOUBLE_FLOAT => 8,
        _ => 0,
    }
}

/// Port of `SkTiff::ImageFileDirectory::IsValidType`.
// Port of: src/codec/SkTiffUtility.cpp#L14 (IsValidType)
fn is_valid_type(tag_type: u16) -> bool {
    (1..=12).contains(&tag_type)
}

/// Port of `ImageFileDirectory::ParseHeader`: the endianness and the offset of the first IFD.
// Port of: src/codec/SkTiffUtility.cpp#L46-L63 (ParseHeader)
#[must_use]
pub fn parse_header(data: &[u8]) -> Option<(bool, u32)> {
    // Read the endianness (4 bytes) and IFD offset (4 bytes).
    if data.len() < 8 {
        return None;
    }
    let little_endian = is_valid_endian_marker(data)?;
    Some((little_endian, get_endian_int(&data[4..], little_endian)))
}

/// Port of `validate_ifd`: the number of entries and the next IFD's offset, or `None` when the IFD
/// does not fit (unless it may be truncated).
// Port of: src/codec/SkTiffUtility.cpp#L65-L120 (validate_ifd)
fn validate_ifd(
    data: &[u8],
    little_endian: bool,
    ifd_offset: u32,
    allow_truncated: bool,
) -> Option<(u16, u32)> {
    let ifd_offset = ifd_offset as usize;
    // Seek to the IFD offset.
    if data.len() < ifd_offset {
        return None;
    }
    let rest = &data[ifd_offset..];

    // Read the number of entries.
    if rest.len() < SIZE_SHORT {
        return None;
    }
    let num_entries = get_endian_short(rest, little_endian);
    let rest = &rest[SIZE_SHORT..];

    // Check that there is enough space for all entries.
    let entries_bytes = SIZE_ENTRY * usize::from(num_entries);
    if rest.len() < entries_bytes {
        if allow_truncated {
            // The entries that can be fully read, and no next IFD.
            // The C++ stores this count in a uint16_t, so a large count wraps.
            #[allow(clippy::cast_possible_truncation)] // mirrors the implicit narrowing in C++
            let readable = (rest.len() / SIZE_ENTRY) as u16;
            return Some((readable, 0));
        }
        return None;
    }

    // Read the next IFD offset, which follows the entries.
    let rest = &rest[entries_bytes..];
    if rest.len() < SIZE_LONG {
        if allow_truncated {
            return Some((num_entries, 0));
        }
        return None;
    }
    Some((num_entries, get_endian_int(rest, little_endian)))
}

/// An image file directory in shared data (`SkTiff::ImageFileDirectory`).
#[derive(Debug, Clone)]
pub struct ImageFileDirectory {
    data: Arc<[u8]>,
    little_endian: bool,
    offset: u32,
    num_entries: u16,
    next_ifd_offset: u32,
}

/// The raw fields of an entry: its type, count and the bytes of its value.
struct RawEntry<'a> {
    tag_type: u16,
    count: u32,
    data: &'a [u8],
}

impl ImageFileDirectory {
    /// Port of `ImageFileDirectory::MakeFromOffset`: the directory at `ifd_offset` in `data`.
    /// `allow_truncated` reads as many entries as fit.
    // Port of: src/codec/SkTiffUtility.cpp#L122-L132 (MakeFromOffset)
    #[must_use]
    pub fn make_from_offset(
        data: Arc<[u8]>,
        little_endian: bool,
        ifd_offset: u32,
        allow_truncated: bool,
    ) -> Option<Self> {
        let (num_entries, next_ifd_offset) =
            validate_ifd(&data, little_endian, ifd_offset, allow_truncated)?;
        Some(Self {
            data,
            little_endian,
            offset: ifd_offset,
            num_entries,
            next_ifd_offset,
        })
    }

    /// Port of `getNumEntries`.
    // Port of: src/codec/SkTiffUtility.h (getNumEntries)
    #[must_use]
    pub fn num_entries(&self) -> u16 {
        self.num_entries
    }

    /// Port of `nextIfdOffset`.
    // Port of: src/codec/SkTiffUtility.h (nextIfdOffset)
    #[must_use]
    pub fn next_ifd_offset(&self) -> u32 {
        self.next_ifd_offset
    }

    /// The byte offset of entry `entry_index` (`get_entry_address`).
    // Port of: src/codec/SkTiffUtility.cpp#L134-L141 (get_entry_address)
    fn entry_address(&self, entry_index: u16) -> usize {
        self.offset as usize + SIZE_SHORT + SIZE_ENTRY * usize::from(entry_index)
    }

    /// Port of `getEntryTag`. Entries past the end read as tag 0 (Skia reads them without checks).
    // Port of: src/codec/SkTiffUtility.cpp#L185-L189 (getEntryTag)
    #[must_use]
    pub fn entry_tag(&self, entry_index: u16) -> u16 {
        let address = self.entry_address(entry_index);
        match self.data.get(address..address + SIZE_SHORT) {
            Some(bytes) => get_endian_short(bytes, self.little_endian),
            None => 0,
        }
    }

    /// Port of `getEntryRawData`: the entry's tag, type, count and value bytes. `None` if the type
    /// is invalid, or the value lies outside the data.
    // Port of: src/codec/SkTiffUtility.cpp#L191-L232 (getEntryRawData)
    fn entry_raw_data(&self, entry_index: u16) -> Option<RawEntry<'_>> {
        let address = self.entry_address(entry_index);
        let entry = self.data.get(address..address + SIZE_ENTRY)?;
        // Read the tag, the type, and the count.
        let tag_type = get_endian_short(&entry[2..], self.little_endian);
        if !is_valid_type(tag_type) {
            return None;
        }
        let count = get_endian_int(&entry[4..], self.little_endian);
        let value = &entry[8..];

        // If the entry fits in the remaining 4 bytes, use that.
        let entry_data_bytes = bytes_for_type(tag_type) * count as usize;
        let entry_data: &[u8] = if entry_data_bytes <= SIZE_LONG {
            &value[..entry_data_bytes]
        } else {
            // Otherwise, the next 4 bytes give the offset of the data.
            let entry_data_offset = get_endian_int(value, self.little_endian) as usize;
            let end = entry_data_offset.checked_add(entry_data_bytes)?;
            self.data.get(entry_data_offset..end)?
        };
        Some(RawEntry {
            tag_type,
            count,
            data: entry_data,
        })
    }

    /// Port of `getEntryUndefinedData`: the bytes of an entry of type undefined (7).
    // Port of: src/codec/SkTiffUtility.cpp#L234-L246 (getEntryUndefinedData)
    #[must_use]
    pub fn entry_undefined_data(&self, entry_index: u16) -> Option<&[u8]> {
        let raw = self.entry_raw_data(entry_index)?;
        (raw.tag_type == TYPE_UNDEFINED).then_some(raw.data)
    }

    /// Port of `getEntryValuesGeneric` for one requested type and count. The values are read at a
    /// stride of four bytes, as Skia reads them.
    // Port of: src/codec/SkTiffUtility.cpp#L248-L305 (getEntryValuesGeneric)
    fn entry_values(&self, entry_index: u16, requested: u16, count: u32) -> Option<Vec<&[u8]>> {
        let raw = self.entry_raw_data(entry_index)?;
        if requested != raw.tag_type || count != raw.count {
            return None;
        }
        let stride = bytes_for_type(TYPE_UNSIGNED_LONG);
        let width = bytes_for_type(requested);
        let mut values = Vec::with_capacity(count as usize);
        for i in 0..count as usize {
            let start = i * stride;
            values.push(raw.data.get(start..start + width)?);
        }
        Some(values)
    }

    /// Port of `getEntryUnsignedShort`: `count` values of type unsigned short (3).
    // Port of: src/codec/SkTiffUtility.h#L52-L54 (getEntryUnsignedShort)
    #[must_use]
    pub fn entry_unsigned_short(&self, entry_index: u16, count: u32) -> Option<Vec<u16>> {
        let values = self.entry_values(entry_index, TYPE_UNSIGNED_SHORT, count)?;
        Some(
            values
                .iter()
                .map(|v| get_endian_short(v, self.little_endian))
                .collect(),
        )
    }

    /// Port of `getEntryUnsignedLong`: `count` values of type unsigned long (4).
    // Port of: src/codec/SkTiffUtility.h#L56-L58 (getEntryUnsignedLong)
    #[must_use]
    pub fn entry_unsigned_long(&self, entry_index: u16, count: u32) -> Option<Vec<u32>> {
        let values = self.entry_values(entry_index, TYPE_UNSIGNED_LONG, count)?;
        Some(
            values
                .iter()
                .map(|v| get_endian_int(v, self.little_endian))
                .collect(),
        )
    }

    /// Port of `getEntrySignedRational` (type 10): the numerator over the denominator, or zero
    /// for a zero denominator.
    // Port of: src/codec/SkTiffUtility.h#L60-L62 (getEntrySignedRational), and the
    // kTypeSignedRational case of getEntryValuesGeneric (src/codec/SkTiffUtility.cpp#L271-L283)
    #[must_use]
    pub fn entry_signed_rational(&self, entry_index: u16, count: u32) -> Option<Vec<f32>> {
        self.rational_values(entry_index, TYPE_SIGNED_RATIONAL, count)
    }

    /// Port of `getEntryUnsignedRational` (type 5).
    // Port of: src/codec/SkTiffUtility.h#L64-L66 (getEntryUnsignedRational), and the
    // kTypeUnsignedRational case of getEntryValuesGeneric (src/codec/SkTiffUtility.cpp#L284-L296)
    #[must_use]
    pub fn entry_unsigned_rational(&self, entry_index: u16, count: u32) -> Option<Vec<f32>> {
        self.rational_values(entry_index, TYPE_UNSIGNED_RATIONAL, count)
    }

    fn rational_values(&self, entry_index: u16, requested: u16, count: u32) -> Option<Vec<f32>> {
        let values = self.entry_values(entry_index, requested, count)?;
        Some(
            values
                .iter()
                .map(|v| {
                    // Both rationals are two longs: the numerator, then the denominator. Zero
                    // denominators read as zero.
                    let numerator = get_endian_int(v, self.little_endian);
                    let denominator = v
                        .get(SIZE_LONG..)
                        .map_or(0, |d| get_endian_int(d, self.little_endian));
                    if denominator == 0 {
                        0.0
                    } else {
                        // Port of `numerator / static_cast<float>(denominator)`.
                        #[allow(clippy::cast_precision_loss)] // mirrors the C++ float cast
                        let n = numerator as f32;
                        #[allow(clippy::cast_precision_loss)] // mirrors the C++ float cast
                        let d = denominator as f32;
                        n / d
                    }
                })
                .collect(),
        )
    }
}
