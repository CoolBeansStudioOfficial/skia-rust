// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/sfnt/SkOTTable_name.{h,cpp} (chrome/m156), the iterator over the 'name'
// table (`SkOTTableName::Iterator`).

//! The `name` table of an sfnt font, read by [`NameIterator`] (`SkOTTableName::Iterator`).
//!
//! The table is big-endian on disk, as in C++ (`SkEndian_SwapBE16`). The values the iterator
//! compares and returns (platform, encoding, language and name ids) are decoded to host order
//! here, so the predefined ids below are plain numbers.

use super::ot_table_name_tables::{BCP47_FROM_LANGUAGE_ID, UNICODE_FROM_MAC_ROMAN};

/// `SkOTTableName::format_1`: names may carry language tag records.
// Port of: src/sfnt/SkOTTable_name.h#L27 (chrome/m156)
const FORMAT_1: u16 = 1;

/// `SkOTTableName::Record::PlatformID`.
// Port of: src/sfnt/SkOTTable_name.h#L37-L46 (chrome/m156)
pub mod platform_id {
    /// `Unicode`.
    pub const UNICODE: u16 = 0;
    /// `Macintosh`.
    pub const MACINTOSH: u16 = 1;
    /// `ISO`: deprecated, use Unicode.
    pub const ISO: u16 = 2;
    /// `Windows`.
    pub const WINDOWS: u16 = 3;
    /// `Custom`: never appears in a `name` table.
    pub const CUSTOM: u16 = 4;
}

/// `SkOTTableName::Record::EncodingID::Windows` values the iterator reads.
// Port of: src/sfnt/SkOTTable_name.h#L118-L127 (chrome/m156)
pub mod windows_encoding {
    /// `Symbol`.
    pub const SYMBOL: u16 = 0;
    /// `UnicodeBMPUCS2`.
    pub const UNICODE_BMP_UCS2: u16 = 1;
    /// `UnicodeUCS4`.
    pub const UNICODE_UCS4: u16 = 10;
}

/// `SkOTTableName::Record::EncodingID::Macintosh::Roman`.
// Port of: src/sfnt/SkOTTable_name.h#L69 (chrome/m156)
pub const MACINTOSH_ROMAN: u16 = 0;

/// `SkOTTableName::Record::NameID::Predefined::FontFamilyName`.
// Port of: src/sfnt/SkOTTable_name.h#L489 (chrome/m156)
pub const FONT_FAMILY_NAME: u16 = 1;
/// `SkOTTableName::Record::NameID::Predefined::FontSubfamilyName`.
// Port of: src/sfnt/SkOTTable_name.h#L490 (chrome/m156)
pub const FONT_SUBFAMILY_NAME: u16 = 2;

/// `sizeof(SkOTTableName)`: format, count and string offset.
const HEADER_SIZE: usize = 6;
/// `sizeof(SkOTTableName::Record)`: six `u16` fields.
const RECORD_SIZE: usize = 12;
/// `sizeof(SkOTTableName::Format1Ext)`: the language tag count.
const FORMAT1_EXT_SIZE: usize = 2;
/// `sizeof(SkOTTableName::Format1Ext::LangTagRecord)`: length and offset.
const LANG_TAG_RECORD_SIZE: usize = 4;

/// One name that [`NameIterator::next_record`] found (`SkOTTableName::Iterator::Record`).
// Port of: src/sfnt/SkOTTable_name.h#L557-L561 (chrome/m156)
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct NameRecord {
    /// The name in UTF-8. Empty when the record's encoding is not one the iterator decodes.
    pub name: String,
    /// The language as a BCP 47 code, or `"und"` when it is unknown.
    pub language: String,
    /// The name id of the record (`record.type`).
    pub name_id: u16,
}

/// Iterates the names of a `name` table with one name id, or all of them (`SkOTTableName::Iterator`).
// Port of: src/sfnt/SkOTTable_name.h#L544-L570 (chrome/m156)
#[doc(alias = "SkOTTableName::Iterator")]
#[derive(Clone, Debug)]
pub struct NameIterator<'a> {
    name_table: &'a [u8],
    /// `fIndex`: the next name record to look at.
    index: usize,
    /// `fType`: the name id to return, or `None` for every name id (`-1`).
    ty: Option<u16>,
}

impl<'a> NameIterator<'a> {
    /// `Iterator(nameTable, size, type)`: the names with the id `ty`, or all of them if `None`.
    // Port of: src/sfnt/SkOTTable_name.h#L546-L550 (chrome/m156)
    #[must_use]
    pub fn new(name_table: &'a [u8], ty: Option<u16>) -> Self {
        Self {
            name_table,
            index: 0,
            ty,
        }
    }

    /// `reset(type)`: starts again with the name id `ty`.
    // Port of: src/sfnt/SkOTTable_name.h#L552-L555 (chrome/m156)
    pub fn reset(&mut self, ty: Option<u16>) {
        self.index = 0;
        self.ty = ty;
    }

    /// `next(record)`: the next name that matches, or `None` at the end or for a table that is
    /// malformed (C++ returns false).
    // Port of: src/sfnt/SkOTTable_name.cpp#L459-L585 (chrome/m156)
    #[must_use]
    pub fn next_record(&mut self) -> Option<NameRecord> {
        let table = self.name_table;
        if table.len() < HEADER_SIZE {
            return None;
        }
        let format = be_u16(table, 0);
        let count = usize::from(be_u16(table, 2));
        let string_table_offset = usize::from(be_u16(table, 4));
        let name_records = &table[HEADER_SIZE..];
        let name_records_size = name_records.len();
        if table.len() < string_table_offset {
            return None;
        }
        let string_table = &table[string_table_offset..];
        let name_records_max = count.min(name_records_size / RECORD_SIZE);

        // Find the next record which matches the requested type.
        let (name_id, platform, encoding, name_offset, name_length, language_id) = loop {
            if self.index >= name_records_max {
                return None;
            }
            let start = RECORD_SIZE * self.index;
            let raw = &name_records[start..start + RECORD_SIZE];
            self.index += 1;
            let fields = (
                be_u16(raw, 6),
                be_u16(raw, 0),
                be_u16(raw, 2),
                be_u16(raw, 10),
                be_u16(raw, 8),
                be_u16(raw, 4),
            );
            if self.ty.is_none_or(|ty| ty == fields.0) {
                break fields;
            }
        };
        // `record.type = nameRecord.nameID.fontSpecific`.
        let mut record = NameRecord {
            name: String::new(),
            language: String::new(),
            name_id,
        };

        // Decode the name into UTF-8.
        let name_offset = usize::from(name_offset);
        let name_length = usize::from(name_length);
        if string_table.len() < name_offset + name_length {
            return None; // continue?
        }
        let name_string = &string_table[name_offset..name_offset + name_length];
        match platform {
            platform_id::WINDOWS
                if matches!(
                    encoding,
                    windows_encoding::UNICODE_BMP_UCS2
                        | windows_encoding::UNICODE_UCS4
                        | windows_encoding::SYMBOL
                ) =>
            {
                record.name = string_from_utf16be(name_string);
            }
            platform_id::UNICODE | platform_id::ISO => {
                record.name = string_from_utf16be(name_string);
            }
            platform_id::MACINTOSH if encoding == MACINTOSH_ROMAN => {
                // TODO (C++): need better decoding, especially on Mac.
                record.name = string_from_mac_roman(name_string);
            }
            // Custom: these should never appear in a 'name' table.
            _ => {}
        }

        // Determine the language. Format 1 languages are in the table itself.
        if format == FORMAT_1 && language_id >= 0x8000 {
            let language_tag_record_index = language_id - 0x8000;
            if name_records_size < RECORD_SIZE * count {
                return None; //"und" or break?
            }
            let format1ext_data = &name_records[RECORD_SIZE * count..];
            let format1ext_size = format1ext_data.len();
            if format1ext_size < FORMAT1_EXT_SIZE {
                return None; // "und" or break?
            }
            let lang_tag_count = usize::from(be_u16(format1ext_data, 0));
            let language_tag_records = &format1ext_data[FORMAT1_EXT_SIZE..];
            let language_tag_records_size = format1ext_size - FORMAT1_EXT_SIZE;
            if usize::from(language_tag_record_index) < lang_tag_count {
                let index = usize::from(language_tag_record_index);
                if language_tag_records_size < LANG_TAG_RECORD_SIZE * (index + 1) {
                    return None; // "und"?
                }
                let lang_tag = &language_tag_records[LANG_TAG_RECORD_SIZE * index..];
                let language_length = usize::from(be_u16(lang_tag, 0));
                let language_offset = usize::from(be_u16(lang_tag, 2));
                if table.len() < string_table_offset + language_offset + language_length {
                    return None; // "und"?
                }
                let language_string =
                    &string_table[language_offset..language_offset + language_length];
                record.language = string_from_utf16be(language_string);
                return Some(record);
            }
        }

        // Handle format 0 languages, translating them into BCP 47.
        record.language = match sk_tsearch_language(language_id) {
            Some(index) => BCP47_FROM_LANGUAGE_ID[index].1.to_owned(),
            // Unknown language, return the BCP 47 code 'und' for 'undetermined'.
            None => "und".to_owned(),
        };
        Some(record)
    }
}

/// `SkTSearch` over `BCP47FromLanguageID` for a language id. The search is the C++ one, so a
/// language id that appears twice in the table gives the same entry as C++.
// Port of: src/core/SkTSearch.h (SkTSearch, chrome/m156), with the less-than of
// src/sfnt/SkOTTable_name.cpp#L454-L456
fn sk_tsearch_language(key: u16) -> Option<usize> {
    let base = &BCP47_FROM_LANGUAGE_ID;
    let count = base.len();
    if count == 0 {
        return None;
    }
    let mut lo = 0;
    let mut hi = count - 1;
    while lo < hi {
        let mid = lo + ((hi - lo) >> 1);
        if base[mid].0 < key {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    if base[hi].0 < key || key < base[hi].0 {
        None
    } else {
        Some(hi)
    }
}

/// A big-endian `u16` at `at`, or 0 past the end (the callers have checked their lengths).
fn be_u16(bytes: &[u8], at: usize) -> u16 {
    match bytes.get(at..at + 2) {
        Some(&[hi, lo]) => u16::from_be_bytes([hi, lo]),
        _ => 0,
    }
}

/// `next_unichar_UTF16BE`: the next code point of big-endian UTF-16, advancing `src`.
// Port of: src/sfnt/SkOTTable_name.cpp#L17-L51 (chrome/m156)
fn next_unichar_utf16be(src: &mut &[u8]) -> u32 {
    if src.len() < 2 {
        *src = &[];
        return 0xFFFD;
    }
    let leading = u32::from(be_u16(src, 0));
    *src = &src[2..];
    let mut c = leading;
    if (0xDC00..=0xDFFF).contains(&c) {
        return 0xFFFD;
    }
    if (0xD800..=0xDBFF).contains(&c) {
        if src.len() < 2 {
            *src = &[];
            return 0xFFFD;
        }
        let c2 = u32::from(be_u16(src, 0));
        if !(0xDC00..=0xDFFF).contains(&c2) {
            return 0xFFFD;
        }
        *src = &src[2..];
        // C++ wraps the constant part of this sum in unsigned arithmetic, and so do we.
        c = (c << 10)
            .wrapping_add(c2)
            .wrapping_add(0x10000u32.wrapping_sub(0xD800 << 10).wrapping_sub(0xDC00));
    }
    c
}

/// `SkString_from_UTF16BE`: the UTF-8 text of big-endian UTF-16.
// Port of: src/sfnt/SkOTTable_name.cpp#L53-L60 (chrome/m156)
fn string_from_utf16be(mut utf16be: &[u8]) -> String {
    let mut utf8 = String::new();
    while !utf16be.is_empty() {
        let unichar = next_unichar_utf16be(&mut utf16be);
        utf8.push(char::from_u32(unichar).unwrap_or('\u{FFFD}'));
    }
    utf8
}

/// `SkStringFromMacRoman`: the UTF-8 text of `MacRoman` bytes.
// Port of: src/sfnt/SkOTTable_name.cpp#L87-L96 (chrome/m156)
fn string_from_mac_roman(mac_roman: &[u8]) -> String {
    mac_roman
        .iter()
        .map(|&byte| {
            let unichar = if byte < 0x80 {
                u32::from(byte)
            } else {
                u32::from(UNICODE_FROM_MAC_ROMAN[usize::from(byte) - 0x80])
            };
            char::from_u32(unichar).unwrap_or('\u{FFFD}')
        })
        .collect()
}
