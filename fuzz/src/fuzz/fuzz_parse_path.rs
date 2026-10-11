// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: fuzz/FuzzParsePath.cpp (chrome/m156)

// Most of this is taken from random_parse_path.cpp and adapted to use the Fuzz
// instead of SKRandom

use skia_rust_core::scalar::scalar;
use skia_rust_core::string::str_append_scalar;
use skia_rust_core::utils::parse_path;

use crate::{Fuzz, def_fuzz};

// Port of: fuzz/FuzzParsePath.cpp#L18-L21 (chrome/m156)
struct Legal {
    symbol: u8,
    scalars: i32,
}

// Port of: fuzz/FuzzParsePath.cpp#L23-L34 (chrome/m156)
#[rustfmt::skip] // one row per entry, as in C++
const LEGAL: [Legal; 10] = [
    Legal { symbol: b'M', scalars: 2 },
    Legal { symbol: b'H', scalars: 1 },
    Legal { symbol: b'V', scalars: 1 },
    Legal { symbol: b'L', scalars: 2 },
    Legal { symbol: b'Q', scalars: 4 },
    Legal { symbol: b'T', scalars: 2 },
    Legal { symbol: b'C', scalars: 6 },
    Legal { symbol: b'S', scalars: 4 },
    Legal { symbol: b'A', scalars: 4 },
    Legal { symbol: b'Z', scalars: 0 },
];

// `gEasy` (set to true while debugging to suppress unusual whitespace) is false.
// Port of: fuzz/FuzzParsePath.cpp#L36 (chrome/m156)
const EASY: bool = false;

// mostly do nothing, then bias towards spaces
// Port of: fuzz/FuzzParsePath.cpp#L38-L41 (chrome/m156)
const WHITE_SPACE: [u8; 15] = [
    0, 0, 0, 0, 0, 0, 0, 0, b' ', b' ', b' ', b' ', 0x09, 0x0D, 0x0A,
];

// Port of: fuzz/FuzzParsePath.cpp#L43-L58 (chrome/m156)
fn add_white(fuzz: &mut Fuzz<'_>, atom: &mut String) {
    if EASY {
        atom.push(' ');
        return;
    }
    // Use a uint8_t to conserve bytes.  This makes our "fuzzed bytes footprint"
    // smaller, which leads to more efficient fuzzing.
    let reps: u8 = fuzz.next_range(0, 2);
    for _ in 0..reps {
        #[allow(clippy::cast_possible_truncation)] // `(int) std::size(gWhiteSpace) - 1` fits
        let index: u8 = fuzz.next_range(0, (WHITE_SPACE.len() - 1) as u8);
        if WHITE_SPACE[usize::from(index)] != 0 {
            atom.push(char::from(WHITE_SPACE[usize::from(index)]));
        }
    }
}

// Port of: fuzz/FuzzParsePath.cpp#L60-L64 (chrome/m156)
fn add_some_white(fuzz: &mut Fuzz<'_>, atom: &mut String) {
    for _ in 0..10 {
        add_white(fuzz, atom);
    }
}

// Port of: fuzz/FuzzParsePath.cpp#L66-L77 (chrome/m156)
fn add_comma(fuzz: &mut Fuzz<'_>, atom: &mut String) {
    if EASY {
        atom.push(',');
        return;
    }
    add_white(fuzz, atom);
    let b = fuzz.next_bool();
    if b {
        atom.push(',');
    }
    add_some_white(fuzz, atom);
}

/// `MakeRandomParsePathPiece`: one command of fuzzed SVG path data.
// Port of: fuzz/FuzzParsePath.cpp#L79-L119 (chrome/m156)
#[doc(alias = "MakeRandomParsePathPiece")]
#[must_use]
pub fn make_random_parse_path_piece(fuzz: &mut Fuzz<'_>) -> String {
    let mut atom = String::new();
    #[allow(clippy::cast_possible_truncation)] // `(int) std::size(gLegal) - 1` fits
    let legal_index: u8 = fuzz.next_range(0, (LEGAL.len() - 1) as u8);
    let legal = &LEGAL[usize::from(legal_index)];
    if EASY {
        atom.push('\n');
    } else {
        add_white(fuzz, &mut atom);
    }
    let mut b = fuzz.next_bool();
    let symbol = legal.symbol | if b { 0x20 } else { 0 };
    atom.push(char::from(symbol));
    let reps: u8 = fuzz.next_range(1, 3);
    for rep in 0..i32::from(reps) {
        for index in 0..legal.scalars {
            let coord: scalar = fuzz.next_range_f32(0.0, 100.0);
            add_white(fuzz, &mut atom);
            str_append_scalar(&mut atom, coord);
            if rep < i32::from(reps) - 1 && index < legal.scalars - 1 {
                add_comma(fuzz, &mut atom);
            } else {
                add_some_white(fuzz, &mut atom);
            }
            if b'A' == legal.symbol && 1 == index {
                let s: scalar = fuzz.next_range_f32(-720.0, 720.0);
                str_append_scalar(&mut atom, s);
                add_comma(fuzz, &mut atom);
                b = fuzz.next_bool();
                atom.push_str(&u32::from(b).to_string());
                add_comma(fuzz, &mut atom);
                b = fuzz.next_bool();
                atom.push_str(&u32::from(b).to_string());
                add_comma(fuzz, &mut atom);
            }
        }
    }
    atom
}

// Port of: fuzz/FuzzParsePath.cpp#L121-L132 (chrome/m156)
def_fuzz!(ParsePath, fuzz_ParsePath, |fuzz| {
    let mut spec = String::new();
    let count: u8 = fuzz.next_range(0, 40);
    for _ in 0..count {
        spec.push_str(&make_random_parse_path_piece(fuzz));
    }
    // skia-rust: the `SkDebugf` traces of the spec and of a failed parse are not ported; they are
    // diagnostics with no effect on the fuzzer.
    let _ = parse_path::from_svg(&spec);
});
