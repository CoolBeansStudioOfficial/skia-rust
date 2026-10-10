// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/skunicode/tests/SkUnicodeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_unicode::unicode::{CodeUnitFlags, convert_utf16_to_utf8};
use skia_rust_unicode::unicode_client;

use crate::{def_test, reporter_assert};

// Port of: modules/skunicode/tests/SkUnicodeTest.cpp#L73-L84 (chrome/m156)
def_test!(SkUnicode_Client, |reporter| {
    // u"\U000f2008", the code point U+F2008 as a surrogate pair.
    let text: Vec<u16> = "\u{f2008}".encode_utf16().collect();
    let utf8 = convert_utf16_to_utf8(&text);
    let utf8 = utf8.as_bytes();
    let client = unicode_client::make(utf8, Vec::new(), Vec::new(), Vec::new());
    // The C++ test passes one buffer to Make and to computeCodeUnitFlags. computeCodeUnitFlags
    // may rewrite tabs in its buffer, and Rust cannot also borrow that buffer as the client's
    // text, so the flags are computed on a copy. This test has no tabs, so the copy is exact.
    let mut flags_input = utf8.to_vec();
    let mut results: Vec<CodeUnitFlags> = Vec::new();
    client.compute_code_unit_flags_utf8(&mut flags_input, false, &mut results);

    for flag in results {
        reporter_assert!(reporter, !flag.has_part_of_white_space_break_flag());
    }
});
