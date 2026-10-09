// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontNamesTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::font_types::FourByteTag;
use skia_rust_core::font_types::set_four_byte_tag;
use skia_rust_core::sfnt::ot_table_name::NameIterator;
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::test_font_mgr;

use crate::{Reporter, def_font_test, reporter_assert};

/// `MAX_FAMILIES`.
// Port of: tests/FontNamesTest.cpp#L157 (chrome/m156)
const MAX_FAMILIES: usize = 1000;

/// A format 0 `name` table with one name, "Test" in en-US (`simpleFormat0NameTable`): the header,
/// one record, and the data with its terminator.
// Port of: tests/FontNamesTest.cpp#L26-L47 (chrome/m156), simpleFormat0NameTable
const SIMPLE_FORMAT0_NAME_TABLE: &[u8] = &[
    // header: format 0, count 1, stringOffset 18
    0x00, 0x00, 0x00, 0x01, 0x00, 0x12,
    // record: Windows, UnicodeBMPUCS2, English_UnitedStates, FontFamilyName, length 8, offset 0
    0x00, 0x03, 0x00, 0x01, 0x04, 0x09, 0x00, 0x01, 0x00, 0x08, 0x00, 0x00,
    // data: "Test" in UTF-16BE, and the terminator of the array
    0x00, b'T', 0x00, b'e', 0x00, b's', 0x00, b't', 0x00,
];

/// A format 1 `name` table whose language is a tag record: `simpleFormat1NameTable`.
// Port of: tests/FontNamesTest.cpp#L49-L77 (chrome/m156), simpleFormat1NameTable
const SIMPLE_FORMAT1_NAME_TABLE: &[u8] = &[
    // header: format 1, count 1, stringOffset 24
    0x00, 0x01, 0x00, 0x01, 0x00, 0x18,
    // record: Windows, UnicodeBMPUCS2, language 0x8000 + 0, FontFamilyName, length 8, offset 0
    0x00, 0x03, 0x00, 0x01, 0x80, 0x00, 0x00, 0x01, 0x00, 0x08, 0x00, 0x00,
    // format1ext: one language tag, length 10, offset 8
    0x00, 0x01, 0x00, 0x0A, 0x00, 0x08,
    // data: "Test", then "en-US" in UTF-16BE, and the terminator of the array
    0x00, b'T', 0x00, b'e', 0x00, b's', 0x00, b't', 0x00, b'e', 0x00, b'n', 0x00, b'-', 0x00, b'U',
    0x00, b'S', 0x00,
];

/// One expected test case of `tests`.
// Port of: tests/FontNamesTest.cpp#L79-L104 (chrome/m156), FontNamesTest
struct FontNamesCase {
    data: &'static [u8],
    name_id: u16,
    name_count: usize,
    names: &'static [(&'static str, &'static str)],
}

/// `tests`: the two synthetic tables, each with the family name "Test" in en-US.
// Port of: tests/FontNamesTest.cpp#L79-L104 (chrome/m156), tests
const TESTS: [FontNamesCase; 2] = [
    FontNamesCase {
        data: SIMPLE_FORMAT0_NAME_TABLE,
        name_id: skia_rust_core::sfnt::ot_table_name::FONT_FAMILY_NAME,
        name_count: 1,
        names: &[("Test", "en-US")],
    },
    FontNamesCase {
        data: SIMPLE_FORMAT1_NAME_TABLE,
        name_id: skia_rust_core::sfnt::ot_table_name::FONT_FAMILY_NAME,
        name_count: 1,
        names: &[("Test", "en-US")],
    },
];

// Port of: tests/FontNamesTest.cpp#L106-L128 (chrome/m156), test_synthetic
fn test_synthetic(reporter: &mut Reporter) {
    for test in &TESTS {
        let mut iter = NameIterator::new(test.data, Some(test.name_id));
        let mut name_index = 0;
        while name_index < test.name_count {
            let Some(record) = iter.next_record() else {
                break;
            };
            reporter_assert!(
                reporter,
                test.names[name_index].0 == record.name,
                "Name did not match."
            );
            reporter_assert!(
                reporter,
                test.names[name_index].1 == record.language,
                "Language did not match."
            );
            name_index += 1;
        }
        reporter_assert!(
            reporter,
            name_index == test.name_count,
            "Fewer names than expected."
        );
        reporter_assert!(
            reporter,
            iter.next_record().is_none(),
            "More names than expected."
        );
    }
}

/// `set->createTypeface(j)` checked and drawn for the names of its `name` table.
// Port of: tests/FontNamesTest.cpp#L160-L240 (chrome/m156), test_systemfonts
fn test_systemfonts(reporter: &mut Reporter) {
    const NAME_TAG: FourByteTag = set_four_byte_tag(b'n', b'a', b'm', b'e');
    let fm = test_font_mgr();
    let count = fm.count_families().min(MAX_FAMILIES);
    for i in 0..count {
        let _family_name = fm.family_name(i);
        let set = fm.create_style_set(i);
        for j in 0..set.count() {
            let (_style, style_name) = set.get_style(j);
            let Some(typeface): Option<Typeface> = set.create_typeface(j) else {
                reporter_assert!(reporter, false, "Could not create {style_name}.");
                continue;
            };
            let _family = typeface.family_name();
            // The localized family names are read through the iterator.
            for _localized in typeface.new_family_name_iterator() {}
            let Some(name_table_size) = typeface.get_table_size(NAME_TAG) else {
                continue;
            };
            if name_table_size == 0 {
                continue;
            }
            let mut name_table = vec![0u8; name_table_size];
            let copied =
                typeface.get_table_data(NAME_TAG, 0, name_table_size, Some(&mut name_table));
            if copied != name_table_size {
                continue;
            }
            let mut family_name_iter = NameIterator::new(
                &name_table,
                Some(skia_rust_core::sfnt::ot_table_name::FONT_FAMILY_NAME),
            );
            while let Some(record) = family_name_iter.next_record() {
                reporter_assert!(
                    reporter,
                    skia_rust_core::sfnt::ot_table_name::FONT_FAMILY_NAME == record.name_id,
                    "Requested family name, got something else."
                );
            }
            let mut style_name_iter = NameIterator::new(
                &name_table,
                Some(skia_rust_core::sfnt::ot_table_name::FONT_SUBFAMILY_NAME),
            );
            while let Some(record) = style_name_iter.next_record() {
                reporter_assert!(
                    reporter,
                    skia_rust_core::sfnt::ot_table_name::FONT_SUBFAMILY_NAME == record.name_id,
                    "Requested subfamily name, got something else."
                );
            }
        }
    }
}

// Port of: tests/FontNamesTest.cpp#L243-L246 (chrome/m156), FontNames
def_font_test!(FontNames, |reporter| {
    test_synthetic(reporter);
    test_systemfonts(reporter);
});
