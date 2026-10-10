// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Checks of the M1 API that the 1:1 test ports do not reach: the UTF helpers, the mapping, the
//! hardcoded character properties and the client break iterator.

use skia_rust_unicode::unicode::{
    BreakType, CodeUnitFlags, LineBreakBefore, LineBreakType, convert_utf8_to_utf16,
    convert_utf16_to_utf8, extract_utf_conversion_mapping, for_each_codepoint_utf8,
    for_each_codepoint_utf16,
};
use skia_rust_unicode::unicode_client;

#[test]
fn utf_round_trip() {
    let text = "a\u{e9}\u{4e2d}\u{1f600}";
    let utf16 = convert_utf8_to_utf16(text.as_bytes());
    assert_eq!(utf16, text.encode_utf16().collect::<Vec<u16>>());
    assert_eq!(convert_utf16_to_utf8(&utf16), text);
}

#[test]
fn invalid_utf8_gives_empty_utf16() {
    assert_eq!(convert_utf8_to_utf16(&[0xFF, b'a']), Vec::<u16>::new());
}

#[test]
fn invalid_utf16_gives_empty_utf8() {
    assert_eq!(convert_utf16_to_utf8(&[0xD800]), "");
}

#[test]
fn utf_conversion_mapping_counts_units() {
    // "a" (1 byte, 1 unit), U+1F600 (4 bytes, 2 units): 5 UTF-8 units, 3 UTF-16 units.
    let text = "a\u{1f600}".as_bytes();
    let mut to8 = Vec::new();
    let mut to16 = Vec::new();
    let ok = extract_utf_conversion_mapping(text, |i| to8.push(i), |i| to16.push(i));
    assert!(ok);
    // One entry per UTF-16 unit (its UTF-8 index), then the UTF-8 length.
    assert_eq!(to8, vec![0, 1, 1, 5]);
    // One entry per UTF-8 unit (its UTF-16 index), then the UTF-16 length.
    assert_eq!(to16, vec![0, 1, 1, 1, 1, 3]);
}

#[test]
fn codepoints_of_utf8_report_their_ranges() {
    let mut seen = Vec::new();
    for_each_codepoint_utf8("a\u{e9}\u{ff}".as_bytes(), |u, before, after, count| {
        seen.push((u, before, after, count));
    });
    assert_eq!(
        seen,
        vec![(0x61, 0, 1, 1), (0xE9, 1, 3, 1), (0xFF, 3, 5, 1)]
    );
}

#[test]
fn invalid_utf8_codepoint_is_replacement() {
    let mut seen = Vec::new();
    for_each_codepoint_utf8(&[0xFF], |u, before, after, _| seen.push((u, before, after)));
    assert_eq!(seen, vec![(0xFFFD, 0, 1)]);
}

#[test]
fn unpaired_surrogate_is_negative_and_ends_the_scan() {
    // A failed decode moves to the end (SkUTF's next_fail), so the 0x61 is not visited.
    let mut seen = Vec::new();
    for_each_codepoint_utf16(&[0xD800, 0x61], |u, before, after| {
        seen.push((u, before, after))
    });
    assert_eq!(seen, vec![(-1, 0, 2)]);
}

#[test]
fn hardcoded_properties() {
    let u = unicode_client::make(b"", Vec::new(), Vec::new(), Vec::new());
    assert!(u.is_space(0x85));
    assert!(!u.is_whitespace(0x85));
    assert!(u.is_whitespace(0x20));
    assert!(u.is_control(0x7f));
    assert!(u.is_tabulation(i32::from(b'\t')));
    assert!(u.is_hard_break(0x2028));
    assert!(u.is_ideographic(0x4e2d));
    // Skia's ranges are half-open: 0x11FF (4607) is not in the Hangul Jamo range.
    assert!(!u.is_ideographic(4607));
    assert!(u.is_ideographic(4606));
}

#[test]
fn flags_helpers() {
    let flags = CodeUnitFlags::TABULATION | CodeUnitFlags::HARD_LINE_BREAK_BEFORE;
    assert!(flags.has_tabulation_flag());
    assert!(flags.has_hard_line_break_flag());
    assert!(!flags.has_soft_line_break_flag());
    assert!(!flags.has_control_flag());
}

#[test]
fn client_flags_mark_breaks_and_graphemes() {
    let text = b"ab c";
    let u = unicode_client::make(
        text,
        Vec::new(),
        vec![0, 2],
        vec![LineBreakBefore::new(2, LineBreakType::SoftLineBreak)],
    );
    let mut input = text.to_vec();
    let mut results = Vec::new();
    assert!(u.compute_code_unit_flags_utf8(&mut input, false, &mut results));
    assert_eq!(results.len(), text.len() + 1);
    assert!(results[0].has_grapheme_start_flag());
    assert!(results[2].has_grapheme_start_flag());
    assert!(results[2].has_soft_line_break_flag());
    // The space at 2 is white space and an intra-word space; the 'c' at 3 is neither.
    assert!(results[2].has_part_of_white_space_break_flag());
    assert!(results[2].contains(CodeUnitFlags::PART_OF_INTRA_WORD_BREAK));
    assert!(!results[3].has_part_of_white_space_break_flag());
}

#[test]
fn client_replaces_tabs_in_place_when_asked() {
    let u = unicode_client::make(b"a\tb", Vec::new(), Vec::new(), Vec::new());
    let mut input = b"a\tb".to_vec();
    let mut results = Vec::new();
    u.compute_code_unit_flags_utf8(&mut input, true, &mut results);
    assert_eq!(input, b"a b");
    assert!(results[1].has_tabulation_flag());
}

#[test]
fn client_break_iterator_walks_line_breaks_as_skia_does() {
    let text = b"hello world";
    let u = unicode_client::make(
        text,
        Vec::new(),
        Vec::new(),
        vec![
            LineBreakBefore::new(6, LineBreakType::SoftLineBreak),
            LineBreakBefore::new(11, LineBreakType::HardLineBreak),
        ],
    );
    let mut it = u
        .make_break_iterator(BreakType::Lines)
        .expect("client breaks");
    assert!(it.set_text_utf8(text));
    assert_eq!(it.first(), 6);
    assert_eq!(it.status(), 4); // kSoftLineBreakBefore
    assert!(!it.is_done());
    // Skia's next() does not advance: it reports the following break every time.
    assert_eq!(it.next(), 11);
    assert_eq!(it.next(), 11);
    assert_eq!(it.current(), 6);
    assert!(!it.is_done());
}

#[test]
fn client_to_upper_returns_the_client_text() {
    let u = unicode_client::make(b"abc", Vec::new(), Vec::new(), Vec::new());
    assert_eq!(u.to_upper("xyz"), "abc");
    assert_eq!(u.to_upper_with_locale("xyz", Some("en")), "abc");
}

#[test]
fn reorder_visual_of_no_levels_is_a_no_op() {
    let u = unicode_client::make(b"", Vec::new(), Vec::new(), Vec::new());
    u.reorder_visual(&[], &mut []);
}
