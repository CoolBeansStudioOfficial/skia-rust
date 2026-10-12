// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Cases of the ICU shim with the values Chromium's ICU 78.2 (d578f2e8) gives, from
//! `oracle/unicode-diff`'s driver (`unicode_diff --dump breaks` over these strings). The full
//! comparison is the unicode-diff replay test; these pin the semantics the shim adds to ICU4X.

use skia_rust_unicode::icu_shim::{
    CaseLocale, UBRK_DONE, UBreakIterator, UBreakIteratorType, u_error_name, u_str_to_upper,
    ucase_get_case_locale, uloc_for_language_tag, uloc_get_default,
};

/// `p:s` for every boundary, as the driver prints it.
fn dump(kind: UBreakIteratorType, text: &str) -> String {
    let mut it = UBreakIterator::open(kind, Some("en")).unwrap();
    it.set_text_utf8(text.as_bytes());
    let mut out = Vec::new();
    let mut p = it.first();
    while p != UBRK_DONE {
        assert_eq!(it.current(), p);
        out.push(format!("{p}:{}", it.get_rule_status()));
        p = it.next();
    }
    out.join(" ")
}

#[test]
fn word_status() {
    use UBreakIteratorType::Word;
    // ExtendNumLet takes the status of the base before it (13a); kana is UBRK_WORD_IDEO.
    assert_eq!(
        dump(Word, "a_ 1_ カ_ __ x"),
        "0:0 2:200 3:0 5:100 6:0 10:400 11:0 13:200 14:0 15:200"
    );
    // A Hangul syllable with a trailing mark is UBRK_WORD_NONE; a regional indicator before an
    // emoji too.
    assert_eq!(
        dump(Word, "a\u{301} 가\u{301} 漢字 \u{1F1F8}\u{1F469}"),
        "0:0 3:200 4:0 9:0 10:0 16:400 17:0 21:0 25:0"
    );
    assert_eq!(
        dump(Word, "a\r\nb\u{2028}c"),
        "0:0 1:200 3:0 4:200 7:0 8:200"
    );
}

#[test]
fn sentence_status() {
    use UBreakIteratorType::Sentence;
    assert_eq!(
        dump(Sentence, "Hello world. Bye\n\nEnd"),
        "0:0 13:0 17:100 18:100 21:100"
    );
    assert_eq!(dump(Sentence, "a\r\nb\u{2028}c"), "0:0 3:100 7:100 8:100");
}

#[test]
fn line_status() {
    use UBreakIteratorType::Line;
    assert_eq!(
        dump(Line, "Hello world. Bye\n\nEnd"),
        "0:0 6:0 13:0 17:100 18:100 21:0"
    );
    assert_eq!(dump(Line, "a\r\nb\u{2028}c"), "0:0 3:100 7:100 8:0");
    // The dictionary boundaries of a Lao run that ends in a line feed take the status of the
    // rule-based range: UBRK_LINE_HARD (docs/design/modules.md §3.2).
    let lao = dump(Line, "ສະບາຍດີ\n");
    for b in lao.split(' ').skip(1) {
        assert!(b.ends_with(":100"), "{lao}");
    }
}

#[test]
fn navigation_inside_code_points() {
    let text: Vec<u16> = "a\u{301} 가\u{301} 漢字 \u{1F1F8}\u{1F469}"
        .encode_utf16()
        .collect();
    let mut it = UBreakIterator::open(UBreakIteratorType::Word, Some("en")).unwrap();
    it.set_text_utf16(&text);
    let mut got = Vec::new();
    for i in 0..=i32::try_from(text.len()).unwrap() {
        let f = it.following(i);
        let p = it.preceding(i);
        got.push(format!("{i}>{f}<{p}"));
    }
    assert_eq!(
        got.join(" "),
        "0>2<-1 1>2<0 2>3<0 3>5<2 4>5<3 5>6<3 6>8<5 7>8<6 8>9<6 9>11<8 10>11<8 11>13<9 12>13<9 \
         13>-1<11"
    );
}

#[test]
fn locales() {
    use UBreakIteratorType::{Line, Title};
    // `ko@lb=strict` names line_cj.brk, which Chromium's data does not have.
    assert_eq!(
        UBreakIterator::open(Line, Some("ko@lb=strict")).err(),
        Some(3)
    );
    assert_eq!(UBreakIterator::open(Title, None).err(), Some(3));
    assert!(UBreakIterator::open(Line, Some("zh_Hans_CN")).is_ok());
    assert_eq!(uloc_get_default(), "en_US");
    let r = uloc_for_language_tag("de-DE-u-co-phonebk");
    assert_eq!(
        (r.locale_id.as_str(), r.parsed_length),
        ("de_DE@collation=phonebook", 18)
    );
    let r = uloc_for_language_tag("en_US");
    assert_eq!((r.locale_id.as_str(), r.parsed_length), ("", 0));
    assert_eq!(u_error_name(0), "U_ZERO_ERROR");
    assert_eq!(u_error_name(-129), "[BOGUS UErrorCode]");
}

#[test]
fn to_upper_locales() {
    assert_eq!(ucase_get_case_locale("tr_TR"), CaseLocale::Turkish);
    assert_eq!(ucase_get_case_locale("AZE"), CaseLocale::Turkish);
    assert_eq!(ucase_get_case_locale("tu"), CaseLocale::Root);
    assert_eq!(ucase_get_case_locale("hyw"), CaseLocale::Root);
    assert_eq!(ucase_get_case_locale("el-GR"), CaseLocale::Greek);
    let up = |s: &str, l: &str| {
        String::from_utf16(&u_str_to_upper(
            &s.encode_utf16().collect::<Vec<_>>(),
            Some(l),
        ))
        .unwrap()
    };
    assert_eq!(up("i", "tr"), "\u{130}");
    assert_eq!(up("i", "en"), "I");
    assert_eq!(up("\u{587}", "hy"), "ԵՎ");
    assert_eq!(up("\u{587}", ""), "ԵՒ");
    // Unpaired surrogates are copied.
    assert_eq!(
        u_str_to_upper(&[0x61, 0xD800, 0x62], None),
        vec![0x41, 0xD800, 0x42]
    );
}
