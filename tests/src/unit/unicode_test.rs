// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/UnicodeTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::font_priv;
use skia_rust_core::font_types::{GlyphId, TextEncoding};
use skia_rust_core::utf::Unichar;
use skia_rust_tools::font_tool_utils::default_font;

use crate::{def_test, reporter_assert};

// Port of: tests/UnicodeTest.cpp#L22-L51 (chrome/m156), Unicode_textencodings
def_test!(Unicode_textencodings, |reporter| {
    let text8: &[u8] = b"ABCDEFGabcdefg0123456789";
    let len8 = text8.len();
    // expand our 8bit chars to 16 and 32 (the native-endian bytes of each element, as C++ reads
    // the `uint16_t` and `int32_t` arrays)
    let bytes16: Vec<u8> = text8
        .iter()
        .flat_map(|&c| u16::from(c).to_ne_bytes())
        .collect();
    let bytes32: Vec<u8> = text8
        .iter()
        .flat_map(|&c| i32::from(c).to_ne_bytes())
        .collect();

    let mut glyphs8 = vec![GlyphId::default(); len8];
    let mut glyphs16 = vec![GlyphId::default(); len8];
    let mut glyphs32 = vec![GlyphId::default(); len8];
    let font = default_font();
    let count8 = font.text_to_glyphs(text8, TextEncoding::UTF8, &mut glyphs8);
    let count16 = font.text_to_glyphs(&bytes16, TextEncoding::UTF16, &mut glyphs16);
    let count32 = font.text_to_glyphs(&bytes32, TextEncoding::UTF32, &mut glyphs32);
    reporter_assert!(reporter, len8 == count8);
    reporter_assert!(reporter, len8 == count16);
    reporter_assert!(reporter, len8 == count32);
    reporter_assert!(reporter, glyphs8[..count8] == glyphs16[..count8]);
    reporter_assert!(reporter, glyphs8[..count8] == glyphs32[..count8]);
});

// Port of: tests/UnicodeTest.cpp#L53-L66 (chrome/m156), glyphs_to_unichars
def_test!(glyphs_to_unichars, |reporter| {
    const N: usize = 52;
    let font = default_font();
    let mut uni: [Unichar; N] = [0; N];
    for (i, c) in (b'A'..=b'Z').enumerate() {
        uni[i] = Unichar::from(c);
    }
    for (i, c) in (b'a'..=b'z').enumerate() {
        uni[i + 26] = Unichar::from(c);
    }
    let uni_bytes: Vec<u8> = uni.iter().flat_map(|v| v.to_ne_bytes()).collect();
    let mut glyphs = [GlyphId::default(); N];
    font.text_to_glyphs(&uni_bytes, TextEncoding::UTF32, &mut glyphs);
    let mut uni2 = [0; N];
    font_priv::glyphs_to_unichars(&font, &glyphs, &mut uni2);
    reporter_assert!(reporter, uni == uni2);
});
