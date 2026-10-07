// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkUTFTest.cpp (chrome/m156)

use skia_rust_core::utf::{self, Unichar};

use crate::{def_test, errorf, reporter_assert};

// Port of: tests/SkUTFTest.cpp#L13-L29 (chrome/m156)
def_test!(SkUTF_UTF16, |reporter| {
    // Test non-basic-multilingual-plane unicode.
    const G_UNI: [Unichar; 5] = [0x10000, 0x18080, 0x20202, 0xFFFFF, 0x10_1234];
    for uni in G_UNI {
        let mut buf = [0u16; 2];
        let count = utf::to_utf16(uni, Some(&mut buf));
        reporter_assert!(reporter, count == 2);
        // skia-rust: CountUTF16 takes code units, not sizeof(buf) bytes.
        let count2 = utf::count_utf16(&buf);
        reporter_assert!(reporter, count2 == 1);
        let mut ptr: &[u16] = &buf;
        let c = utf::next_utf16(&mut ptr);
        reporter_assert!(reporter, c == uni);
        reporter_assert!(reporter, buf.len() - ptr.len() == 2);
    }
});

// Port of: tests/SkUTFTest.cpp#L31-L58 (chrome/m156)
def_test!(SkUTF_UTF8, |reporter| {
    struct Case {
        f_utf8: &'static [u8],
        f_uni: Unichar,
    }
    let g_test = [
        Case {
            f_utf8: b"a",
            f_uni: 'a' as Unichar,
        },
        Case {
            f_utf8: b"\x7f",
            f_uni: 0x7f,
        },
        Case {
            f_utf8: b"\xC2\x80",
            f_uni: 0x80,
        },
        Case {
            f_utf8: b"\xC3\x83",
            f_uni: (3 << 6) | 3,
        },
        Case {
            f_utf8: b"\xDF\xBF",
            f_uni: 0x7ff,
        },
        Case {
            f_utf8: b"\xE0\xA0\x80",
            f_uni: 0x800,
        },
        Case {
            f_utf8: b"\xE0\xB0\xB8",
            f_uni: 0xC38,
        },
        Case {
            f_utf8: b"\xE3\x83\x83",
            f_uni: (3 << 12) | (3 << 6) | 3,
        },
        Case {
            f_utf8: b"\xEF\xBF\xBF",
            f_uni: 0xFFFF,
        },
        Case {
            f_utf8: b"\xF0\x90\x80\x80",
            f_uni: 0x10000,
        },
        Case {
            f_utf8: b"\xF3\x83\x83\x83",
            f_uni: (3 << 18) | (3 << 12) | (3 << 6) | 3,
        },
    ];
    for test in g_test {
        let mut p: &[u8] = test.f_utf8;
        let n = utf::count_utf8(p);
        let u1 = utf::next_utf8(&mut p);

        reporter_assert!(reporter, n == 1);
        reporter_assert!(reporter, u1 == test.f_uni);
        reporter_assert!(reporter, test.f_utf8.len() - p.len() == test.f_utf8.len());
    }
});

const ASCII_BYTE: &[u8] = b"X";
const CONTINUATION_BYTE: &[u8] = b"\xA1";
const LEADING_TWO_BYTE: &[u8] = b"\xC2";
const LEADING_THREE_BYTE: &[u8] = b"\xE1";
const LEADING_FOUR_BYTE: &[u8] = b"\xF0";
const INVALID_BYTE: &[u8] = b"\xFC";

// Port of: tests/SkUTFTest.cpp#L66-L100 (chrome/m156)
def_test!(SkUTF_CountUTF8, |r| {
    let a = ASCII_BYTE;
    let c = CONTINUATION_BYTE;
    let l2 = LEADING_TWO_BYTE;
    let l3 = LEADING_THREE_BYTE;
    let l4 = LEADING_FOUR_BYTE;
    let inv = INVALID_BYTE;
    let test_cases: Vec<(i32, Vec<u8>)> = vec![
        (0, vec![]),
        (1, [a].concat()),
        (2, [a, a].concat()),
        (1, [l2, c].concat()),
        (2, [a, l2, c].concat()),
        (3, [a, a, l2, c].concat()),
        (1, [l3, c, c].concat()),
        (2, [a, l3, c, c].concat()),
        (3, [a, a, l3, c, c].concat()),
        (1, [l4, c, c, c].concat()),
        (2, [a, l4, c, c, c].concat()),
        (3, [a, a, l4, c, c, c].concat()),
        (-1, [inv].concat()),
        (-1, [inv, c].concat()),
        (-1, [inv, c, c].concat()),
        (-1, [inv, c, c, c].concat()),
        (-1, [l2].concat()),
        (-1, [c].concat()),
        (-1, [c, c].concat()),
        (-1, [l3, c].concat()),
        (-1, [c, c, c].concat()),
        (-1, [l4, c].concat()),
        (-1, [c, c, c, c].concat()),
    ];
    for (expected_count, utf8_string) in &test_cases {
        let s: &[u8] = utf8_string;
        reporter_assert!(r, *expected_count == utf::count_utf8(s));
    }
});

// The length of a C string held in `buf`: bytes up to the first NUL.
fn strlen(buf: &[u8]) -> usize {
    buf.iter().position(|&b| b == 0).unwrap_or(buf.len())
}

// Port of: tests/SkUTFTest.cpp#L102-L142 (chrome/m156)
def_test!(SkUTF_NextUTF8_ToUTF8, |r| {
    let l4_tail: Vec<u8> = [LEADING_FOUR_BYTE, b"\x90\x8C\xB0"].concat();
    let l3 = [LEADING_THREE_BYTE, CONTINUATION_BYTE, CONTINUATION_BYTE].concat();
    let l2 = [LEADING_TWO_BYTE, CONTINUATION_BYTE].concat();
    let test_cases: Vec<(Unichar, &[u8])> = vec![
        (-1, INVALID_BYTE),
        (-1, b""),
        (0x0058, ASCII_BYTE),
        (0x00A1, &l2),
        (0x1861, &l3),
        (0x01_0330, &l4_tail),
    ];
    for (expected, utf8_string) in test_cases {
        let mut str_: &[u8] = utf8_string;
        let uni = utf::next_utf8(&mut str_);
        reporter_assert!(r, str_.is_empty());
        reporter_assert!(r, uni == expected);
        let mut buff = [0u8; 5];
        let len = utf::to_utf8(uni, Some(&mut buff));
        if buff[len] != 0 {
            errorf!(r, "unexpected write");
            continue;
        }
        if uni == -1 {
            reporter_assert!(r, len == 0);
            continue;
        }
        if len == 0 {
            errorf!(r, "unexpected failure.");
            continue;
        }
        if len > 4 {
            errorf!(r, "wrote too much");
            continue;
        }
        let str_ = utf8_string;
        reporter_assert!(r, len == strlen(&buff));
        reporter_assert!(r, len == strlen(str_));
        reporter_assert!(r, str_[..strlen(str_)] == buff[..strlen(&buff)]);
    }
});
