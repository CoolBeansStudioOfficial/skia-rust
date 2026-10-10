// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFMakeToUnicodeCmap.{h,cpp} (chrome/m156)

//! `SkPDFMakeToUnicodeCmap`: the `ToUnicode` CMap that maps the glyphs a PDF font used back to
//! text.

use skia_rust_core::font_types::GlyphId;
use skia_rust_core::stream::{DynamicMemoryWStream, WStream};
use skia_rust_core::t_hash::THashMap;
use skia_rust_core::utf::{Unichar, next_utf8};

use crate::glyph_use::PdfGlyphUse;
use crate::utils::{write_uint8, write_uint16_be, write_utf16be_hex};

/// The glyphs that a font maps to text of more than one code point, or to a code point the font
/// has no cmap entry for (`THashMap<SkGlyphID, SkString>`), with their UTF-8 text.
pub type GlyphToUnicodeEx = THashMap<GlyphId, Vec<u8>>;

// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L22-L58 (append_tounicode_header, chrome/m156)
fn append_tounicode_header(cmap: &mut DynamicMemoryWStream, multibyte: bool) {
    // 12 dict begin: 12 is an Adobe-suggested value. Shall not change.
    // It's there to prevent old version Adobe Readers from malfunctioning.
    cmap.write_text(
        "/CIDInit /ProcSet findresource begin\n\
         12 dict begin\n\
         begincmap\n",
    );

    // The /CIDSystemInfo must be consistent to the one in
    // SkPDFFont::populateCIDFont().
    // We can not pass over the system info object here because the format is
    // different. This is not a reference object.
    cmap.write_text(
        "/CIDSystemInfo\n\
         <<  /Registry (Adobe)\n\
         /Ordering (UCS)\n\
         /Supplement 0\n\
         >> def\n",
    );

    // The CMapName must be consistent to /CIDSystemInfo above.
    // /CMapType 2 means ToUnicode.
    // Codespace range just tells the PDF processor the valid range.
    cmap.write_text(
        "/CMapName /Adobe-Identity-UCS def\n\
         /CMapType 2 def\n\
         1 begincodespacerange\n",
    );
    if multibyte {
        cmap.write_text("<0000> <FFFF>\n");
    } else {
        cmap.write_text("<00> <FF>\n");
    }
    cmap.write_text("endcodespacerange\n");
}

// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L60-L68 (append_cmap_footer, chrome/m156)
fn append_cmap_footer(cmap: &mut DynamicMemoryWStream) {
    cmap.write_text(
        "endcmap\n\
         CMapName currentdict /CMap defineresource pop\n\
         end\n\
         end",
    );
}

/// `BFChar`.
#[derive(Clone, Copy)]
struct BfChar {
    glyph_id: GlyphId,
    unicode: Unichar,
}

/// `BFRange`.
#[derive(Clone, Copy, Default)]
struct BfRange {
    start: GlyphId,
    end: GlyphId,
    unicode: Unichar,
}

// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L82-L91 (write_glyph, chrome/m156)
fn write_glyph(cmap: &mut DynamicMemoryWStream, multi_byte: bool, gid: GlyphId) {
    if multi_byte {
        write_uint16_be(cmap, gid);
    } else {
        // SkToU8
        debug_assert!(gid <= 0xFF);
        write_uint8(cmap, (gid & 0xFF) as u8);
    }
}

// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L93-L112 (append_bfchar_section, chrome/m156)
fn append_bfchar_section(bfchar: &[BfChar], multi_byte: bool, cmap: &mut DynamicMemoryWStream) {
    // PDF spec defines that every bf* list can have at most 100 entries.
    for chunk in bfchar.chunks(100) {
        cmap.write_dec_as_text(i32::try_from(chunk.len()).expect("at most 100"));
        cmap.write_text(" beginbfchar\n");
        for entry in chunk {
            cmap.write_text("<");
            write_glyph(cmap, multi_byte, entry.glyph_id);
            cmap.write_text("> <");
            write_utf16be_hex(cmap, entry.unicode);
            cmap.write_text(">\n");
        }
        cmap.write_text("endbfchar\n");
    }
}

// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L114-L162 (append_bfchar_section_ex, chrome/m156)
#[allow(clippy::cast_possible_truncation)] // `glyphId - glyphOffset` is an int passed as SkGlyphID
fn append_bfchar_section_ex(
    glyph_to_unicode_ex: &GlyphToUnicodeEx,
    multi_byte: bool,
    first_glyph_id: GlyphId,
    last_glyph_id: GlyphId,
    cmap: &mut DynamicMemoryWStream,
) {
    let mut glyph_count = 0usize;
    glyph_to_unicode_ex.foreach(|&glyph_id, _| {
        if glyph_id < first_glyph_id || last_glyph_id < glyph_id {
            return;
        }
        glyph_count += 1;
    });

    let mut glyph_offset = 0i32;
    if !multi_byte {
        glyph_offset = i32::from(first_glyph_id) - 1;
    }
    // PDF spec defines that every bf* list can have at most 100 entries.
    let mut i = 0usize;
    glyph_to_unicode_ex.foreach(|&glyph_id, text| {
        if glyph_id < first_glyph_id || last_glyph_id < glyph_id {
            return;
        }
        if i % 100 == 0 {
            let count = (glyph_count - i).min(100);
            cmap.write_dec_as_text(i32::try_from(count).expect("at most 100"));
            cmap.write_text(" beginbfchar\n");
        }

        cmap.write_text("<");
        write_glyph(
            cmap,
            multi_byte,
            (i32::from(glyph_id) - glyph_offset) as GlyphId,
        );
        cmap.write_text("> <");
        let mut remaining: &[u8] = text;
        while !remaining.is_empty() {
            let unichar = next_utf8(&mut remaining);
            write_utf16be_hex(cmap, unichar);
        }
        cmap.write_text(">\n");

        if i % 100 == 99 || i == glyph_count - 1 {
            cmap.write_text("endbfchar\n");
        }
        i += 1;
    });
}

// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L164-L186 (append_bfrange_section, chrome/m156)
fn append_bfrange_section(bfrange: &[BfRange], multi_byte: bool, cmap: &mut DynamicMemoryWStream) {
    // PDF spec defines that every bf* list can have at most 100 entries.
    for chunk in bfrange.chunks(100) {
        cmap.write_dec_as_text(i32::try_from(chunk.len()).expect("at most 100"));
        cmap.write_text(" beginbfrange\n");
        for entry in chunk {
            cmap.write_text("<");
            write_glyph(cmap, multi_byte, entry.start);
            cmap.write_text("> <");
            write_glyph(cmap, multi_byte, entry.end);
            cmap.write_text("> <");
            write_utf16be_hex(cmap, entry.unicode);
            cmap.write_text(">\n");
        }
        cmap.write_text("endbfrange\n");
    }
}

/// `SkPDFAppendCmapSections`: generates the `<bfchar>` and `<bfrange>` tables according to PDF
/// spec 1.4 and Adobe Technote 5014. Exposed for unit testing.
///
/// The current implementation guarantees that bfchar and bfrange entries do not overlap.
///
/// The current implementation does not attempt aggressive optimizations against the following
/// case because the specification is not clear.
///
/// ```text
/// 4 beginbfchar          1 beginbfchar
/// <0003> <0013>          <0020> <0014>
/// <0005> <0015>    to    endbfchar
/// <0007> <0017>          1 beginbfrange
/// <0020> <0014>          <0003> <0007> <0013>
/// endbfchar              endbfrange
/// ```
///
/// Adobe Technote 5014 said: "Code mappings (unlike codespace ranges) may overlap, but
/// succeeding maps supersede preceding maps."
///
/// In case of searching text in PDF, bfrange will have higher precedence so typing char id
/// 0x0014 in search box will get glyph id 0x0004 first. However, the spec does not mention how
/// will this kind of conflict being resolved.
///
/// For the worst case (having 65536 continuous unicode and we use every other one of them), the
/// possible savings by aggressive optimization is 416KB pre-compressed and does not provide
/// enough motivation for implementation.
// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L188-L262 (SkPDFAppendCmapSections, chrome/m156)
#[doc(alias = "SkPDFAppendCmapSections")]
#[allow(clippy::cast_possible_truncation)] // `SkGlyphID gid = i + glyphOffset`, `fEnd = i`
#[allow(clippy::cast_sign_loss)] // the indices are non-negative
pub fn append_cmap_sections(
    glyph_to_unicode: &[Unichar],
    glyph_to_unicode_ex: &GlyphToUnicodeEx,
    subset: Option<&PdfGlyphUse>,
    cmap: &mut DynamicMemoryWStream,
    multi_byte_glyphs: bool,
    first_glyph_id: GlyphId,
    last_glyph_id: GlyphId,
) {
    let mut glyph_offset = 0i32;
    if !multi_byte_glyphs {
        glyph_offset = i32::from(first_glyph_id) - 1;
    }

    let mut bfchar_entries: Vec<BfChar> = Vec::new();
    let mut bfrange_entries: Vec<BfRange> = Vec::new();

    let mut current_range_entry = BfRange::default();
    let mut range_empty = true;
    let limit = i32::from(last_glyph_id) + 1 - glyph_offset;

    let mut i = i32::from(first_glyph_id) - glyph_offset;
    while i < limit + 1 {
        let gid = (i + glyph_offset) as GlyphId;
        let in_subset = i < limit
            && subset.is_none_or(|s| s.has(gid))
            && glyph_to_unicode_ex.find(&gid).is_none();
        if !range_empty {
            // PDF spec requires bfrange not changing the higher byte,
            // e.g. <1035> <10FF> <2222> is ok, but
            //      <1035> <1100> <2222> is no good
            let in_range = i == i32::from(current_range_entry.end) + 1
                && i >> 8 == i32::from(current_range_entry.start) >> 8
                && i < limit
                && glyph_to_unicode[usize::from(gid)]
                    == current_range_entry.unicode + i - i32::from(current_range_entry.start);
            if !in_subset || !in_range {
                if current_range_entry.end > current_range_entry.start {
                    bfrange_entries.push(current_range_entry);
                } else {
                    bfchar_entries.push(BfChar {
                        glyph_id: current_range_entry.start,
                        unicode: current_range_entry.unicode,
                    });
                }
                range_empty = true;
            }
        }
        if in_subset {
            current_range_entry.end = i as GlyphId;
            if range_empty {
                current_range_entry.start = i as GlyphId;
                current_range_entry.unicode = glyph_to_unicode[usize::from(gid)];
                range_empty = false;
            }
        }
        i += 1;
    }

    // The spec requires all bfchar entries for a font must come before bfrange entries.
    append_bfchar_section(&bfchar_entries, multi_byte_glyphs, cmap);
    append_bfchar_section_ex(
        glyph_to_unicode_ex,
        multi_byte_glyphs,
        first_glyph_id,
        last_glyph_id,
        cmap,
    );
    append_bfrange_section(&bfrange_entries, multi_byte_glyphs, cmap);
}

/// `SkPDFMakeToUnicodeCmap`: the `ToUnicode` CMap of the glyphs in `subset` (all of the glyphs
/// when there is none), as a stream.
// Port of: src/pdf/SkPDFMakeToUnicodeCmap.cpp#L264-L279 (chrome/m156)
#[doc(alias = "SkPDFMakeToUnicodeCmap")]
#[must_use]
pub fn make_to_unicode_cmap(
    glyph_to_unicode: &[Unichar],
    glyph_to_unicode_ex: &GlyphToUnicodeEx,
    subset: Option<&PdfGlyphUse>,
    multi_byte_glyphs: bool,
    first_glyph_id: GlyphId,
    last_glyph_id: GlyphId,
) -> Vec<u8> {
    let mut cmap = DynamicMemoryWStream::new();
    append_tounicode_header(&mut cmap, multi_byte_glyphs);
    append_cmap_sections(
        glyph_to_unicode,
        glyph_to_unicode_ex,
        subset,
        &mut cmap,
        multi_byte_glyphs,
        first_glyph_id,
        last_glyph_id,
    );
    append_cmap_footer(&mut cmap);
    cmap.detach_as_vector()
}
