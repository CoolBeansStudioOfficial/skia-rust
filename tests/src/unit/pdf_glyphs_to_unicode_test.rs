// Copyright 2010 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/PDFGlyphsToUnicodeTest.cpp (chrome/m156)

use skia_rust_core::checksum::GoodHash;
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::utf::Unichar;
use skia_rust_pdf::glyph_use::PdfGlyphUse;
use skia_rust_pdf::to_unicode_cmap::{GlyphToUnicodeEx, append_cmap_sections};

use crate::{def_test, reporter_assert};

/// `kMaximumGlyphIndex`.
const MAXIMUM_GLYPH_INDEX: GlyphId = u16::MAX;

// Port of: tests/PDFGlyphsToUnicodeTest.cpp#L27-L41 (chrome/m156)
fn stream_equals(stream: &mut DynamicMemoryWStream, buffer: &str) -> bool {
    let stream_size = stream.bytes_written();
    let mut data = vec![0u8; stream_size];
    stream.copy_to(&mut data);

    if stream_size != buffer.len() {
        return false;
    }
    data == buffer.as_bytes()
}

/// `THashMap<SkGlyphID, SkString>`, hashed with `SkGoodHash`.
fn new_glyph_to_unicode_ex() -> GlyphToUnicodeEx {
    GlyphToUnicodeEx::new(GoodHash::good_hash)
}

// Port of: tests/PDFGlyphsToUnicodeTest.cpp#L43-L215 (chrome/m156)
// The pushes of the same value and the `std::min` with 0xFFFF mirror the C++.
def_test!(
    #[allow(clippy::same_item_push, clippy::unnecessary_min_or_max)]
    SkPDF_ToUnicode,
    |reporter| {
        let mut glyph_to_unicode: Vec<Unichar> = Vec::new();
        let mut glyph_to_unicode_ex = new_glyph_to_unicode_ex();
        let mut glyphs_in_subset: Vec<GlyphId> = Vec::new();
        let mut subset = PdfGlyphUse::new(1, MAXIMUM_GLYPH_INDEX);

        glyph_to_unicode.push(0); // 0
        glyph_to_unicode.push(0); // 1
        glyph_to_unicode.push(0); // 2
        glyphs_in_subset.push(3);
        glyph_to_unicode.push(0x20); // 3
        glyphs_in_subset.push(4);
        glyph_to_unicode.push(0x25); // 4
        glyphs_in_subset.push(5);
        glyph_to_unicode.push(0x27); // 5
        glyphs_in_subset.push(6);
        glyph_to_unicode.push(0x28); // 6
        glyphs_in_subset.push(7);
        glyph_to_unicode.push(0x29); // 7
        glyphs_in_subset.push(8);
        glyph_to_unicode.push(0x2F); // 8
        glyphs_in_subset.push(9);
        glyph_to_unicode.push(0x33); // 9
        glyph_to_unicode.push(0); // 10
        glyphs_in_subset.push(11);
        glyph_to_unicode.push(0x35); // 11
        glyphs_in_subset.push(12);
        glyph_to_unicode.push(0x36); // 12
        glyphs_in_subset.push(13);
        glyph_to_unicode.push(0x37); // 13
        for _ in 14u16..0xFE {
            glyph_to_unicode.push(0); // Zero from index 14 to 0xFD
        }
        glyphs_in_subset.push(0xFE);
        glyph_to_unicode.push(0x1010);
        glyphs_in_subset.push(0xFF);
        glyph_to_unicode.push(0x1011);
        glyphs_in_subset.push(0x100);
        glyph_to_unicode.push(0x1012);
        glyphs_in_subset.push(0x101);
        glyph_to_unicode.push(0x1013);

        glyph_to_unicode_ex.set(14, b"ffi".to_vec());
        glyph_to_unicode_ex.set(0xFC, b"st".to_vec());

        let mut last_glyph_id = GlyphId::try_from(glyph_to_unicode.len() - 1).expect("SkToU16");

        let mut buffer = DynamicMemoryWStream::new();
        for &gid in &glyphs_in_subset {
            subset.set(gid);
        }
        let mut ex_glyphs = Vec::new();
        glyph_to_unicode_ex.foreach(|&gid, _| ex_glyphs.push(gid));
        for gid in ex_glyphs {
            subset.set(gid);
        }
        append_cmap_sections(
            &glyph_to_unicode,
            &glyph_to_unicode_ex,
            Some(&subset),
            &mut buffer,
            true,
            0,
            0xFFFF.min(last_glyph_id),
        );

        let expected_result = "4 beginbfchar\n\
<0003> <0020>\n\
<0004> <0025>\n\
<0008> <002F>\n\
<0009> <0033>\n\
endbfchar\n\
2 beginbfchar\n\
<00FC> <00730074>\n\
<000E> <006600660069>\n\
endbfchar\n\
4 beginbfrange\n\
<0005> <0007> <0027>\n\
<000B> <000D> <0035>\n\
<00FE> <00FF> <1010>\n\
<0100> <0101> <1012>\n\
endbfrange\n";

        reporter_assert!(reporter, stream_equals(&mut buffer, expected_result));

        // Remove characters and ranges.
        buffer.reset();

        append_cmap_sections(
            &glyph_to_unicode,
            &glyph_to_unicode_ex,
            Some(&subset),
            &mut buffer,
            true,
            8,
            0x00FF.min(last_glyph_id),
        );

        let expected_result_chop1 = "2 beginbfchar\n\
<0008> <002F>\n\
<0009> <0033>\n\
endbfchar\n\
2 beginbfchar\n\
<00FC> <00730074>\n\
<000E> <006600660069>\n\
endbfchar\n\
2 beginbfrange\n\
<000B> <000D> <0035>\n\
<00FE> <00FF> <1010>\n\
endbfrange\n";

        reporter_assert!(reporter, stream_equals(&mut buffer, expected_result_chop1));

        // Remove characters from range to downdrade it to one char.
        buffer.reset();

        append_cmap_sections(
            &glyph_to_unicode,
            &glyph_to_unicode_ex,
            Some(&subset),
            &mut buffer,
            true,
            0x00D,
            0x00FE.min(last_glyph_id),
        );

        let expected_result_chop2 = "2 beginbfchar\n\
<000D> <0037>\n\
<00FE> <1010>\n\
endbfchar\n\
2 beginbfchar\n\
<00FC> <00730074>\n\
<000E> <006600660069>\n\
endbfchar\n";

        reporter_assert!(reporter, stream_equals(&mut buffer, expected_result_chop2));

        buffer.reset();

        append_cmap_sections(
            &glyph_to_unicode,
            &glyph_to_unicode_ex,
            Some(&subset),
            &mut buffer,
            false,
            0xFC,
            0x110.min(last_glyph_id),
        );

        let expected_result_single_bytes = "1 beginbfchar\n\
<01> <00730074>\n\
endbfchar\n\
1 beginbfrange\n\
<03> <06> <1010>\n\
endbfrange\n";

        reporter_assert!(
            reporter,
            stream_equals(&mut buffer, expected_result_single_bytes)
        );

        glyph_to_unicode.clear();
        glyph_to_unicode_ex.reset();
        glyphs_in_subset.clear();
        let mut subset2 = PdfGlyphUse::new(1, MAXIMUM_GLYPH_INDEX);

        // Test mapping:
        //           I  n  s  t  a  l
        // Glyph id 2c 51 56 57 44 4f
        // Unicode  49 6e 73 74 61 6c
        for i in 0..100 {
            glyph_to_unicode.push(i + 29);
        }
        last_glyph_id = GlyphId::try_from(glyph_to_unicode.len() - 1).expect("SkToU16");

        glyphs_in_subset.push(0x2C);
        glyphs_in_subset.push(0x44);
        glyphs_in_subset.push(0x4F);
        glyphs_in_subset.push(0x51);
        glyphs_in_subset.push(0x56);
        glyphs_in_subset.push(0x57);

        let mut buffer2 = DynamicMemoryWStream::new();
        for &v in &glyphs_in_subset {
            subset2.set(v);
        }
        append_cmap_sections(
            &glyph_to_unicode,
            &glyph_to_unicode_ex,
            Some(&subset2),
            &mut buffer2,
            true,
            0,
            0xFFFF.min(last_glyph_id),
        );

        let expected_result2 = "4 beginbfchar\n\
<002C> <0049>\n\
<0044> <0061>\n\
<004F> <006C>\n\
<0051> <006E>\n\
endbfchar\n\
1 beginbfrange\n\
<0056> <0057> <0073>\n\
endbfrange\n";

        reporter_assert!(reporter, stream_equals(&mut buffer2, expected_result2));
    }
);
