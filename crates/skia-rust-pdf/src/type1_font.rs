// Copyright 2018 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/pdf/SkPDFType1Font.{h,cpp} (chrome/m156)

//! `SkPDFType1Font`: embeds Type1 fonts (the font program in a `PFA` or `PFB` file, converted
//! to what PDF wants).
//!
//! No typeface of this port reads Type1 data (they come from `FreeType` in Skia), so this is
//! reached only by a typeface that reports itself as `FontType::Type1`.

use std::rc::Rc;

use skia_rust_core::advanced_typeface_metrics::{AdvancedTypefaceMetrics, FontFlags};
use skia_rust_core::font_types::GlyphId;
use skia_rust_core::scalar::{scalar, scalar_round_to_int};
use skia_rust_core::stream::StreamAsset;
use skia_rust_core::strike_spec::BulkGlyphMetrics;
use skia_rust_core::typeface::Typeface;

use crate::document::DocHandle;
use crate::font::{
    PdfFont, PdfStrikeSpec, get_metrics, get_type1_glyph_names, populate_common_font_descriptor,
};
use crate::types::{PdfArray, PdfDict, PdfIndirectReference};

/// The `strstr` of a NUL-terminated buffer: the offset of `needle` in `haystack`, which ends at
/// its first NUL.
fn strstr(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    let end = haystack
        .iter()
        .position(|&b| b == 0)
        .unwrap_or(haystack.len());
    let haystack = &haystack[..end];
    if needle.is_empty() {
        return Some(0);
    }
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// "A standard Type 1 font program, as described in the Adobe Type 1 Font Format specification,
/// consists of three parts: a clear-text portion (written using PostScript syntax), an encrypted
/// portion, and a fixed-content portion. The fixed-content portion contains 512 ASCII zeros
/// followed by a cleartomark operator, and perhaps followed by additional data. Although the
/// encrypted portion of a standard Type 1 font may be in binary or ASCII hexadecimal format, PDF
/// supports only the binary format."
// Port of: src/pdf/SkPDFType1Font.cpp#L36-L65 (parsePFBSection, chrome/m156)
fn parse_pfb_section(src: &mut &[u8], section_type: u8, size: &mut usize) -> bool {
    // PFB sections have a two or six bytes header. 0x80 and a one byte
    // section type followed by a four byte section length.  Type one is
    // an ASCII section (includes a length), type two is a binary section
    // (includes a length) and type three is an EOF marker with no length.
    let buf = *src;
    if buf.len() < 2 || buf[0] != 0x80 || buf[1] != section_type {
        return false;
    } else if buf[1] == 3 {
        return true;
    } else if buf.len() < 6 {
        return false;
    }

    *size = (buf[2] as usize)
        | ((buf[3] as usize) << 8)
        | ((buf[4] as usize) << 16)
        | ((buf[5] as usize) << 24);
    let consumed = *size + 6;
    if consumed > buf.len() {
        return false;
    }
    *src = &buf[consumed..];
    true
}

// Port of: src/pdf/SkPDFType1Font.cpp#L67-L78 (parsePFB, chrome/m156)
fn parse_pfb(
    src: &[u8],
    header_len: &mut usize,
    data_len: &mut usize,
    trailer_len: &mut usize,
) -> bool {
    let mut src_ptr = src;
    let mut ignored = 0;
    parse_pfb_section(&mut src_ptr, 1, header_len)
        && parse_pfb_section(&mut src_ptr, 2, data_len)
        && parse_pfb_section(&mut src_ptr, 1, trailer_len)
        && parse_pfb_section(&mut src_ptr, 3, &mut ignored)
}

/// `isspace` of the C locale.
fn is_space(c: u8) -> bool {
    matches!(c, b' ' | b'\t' | b'\n' | 0x0B | 0x0C | b'\r')
}

/// The sections of a PFA file are implicitly defined. The body starts after the line containing
/// "eexec," and the trailer starts with 512 literal 0's followed by "cleartomark" (plus arbitrary
/// white space).
///
/// `src` is the data and a NUL, which is not counted in `size`.
// Port of: src/pdf/SkPDFType1Font.cpp#L80-L141 (parsePFA, chrome/m156)
fn parse_pfa(
    src: &[u8],
    size: usize,
    header_len: &mut usize,
    hex_data_len: &mut usize,
    data_len: &mut usize,
    trailer_len: &mut usize,
) -> bool {
    let end = size;

    let Some(eexec) = strstr(src, b"eexec") else {
        return false;
    };
    let mut data_pos = eexec + b"eexec".len();
    while (src[data_pos] == b'\n' || src[data_pos] == b'\r' || src[data_pos] == b' ')
        && data_pos < end
    {
        data_pos += 1;
    }
    *header_len = data_pos;

    let Some(trailer_offset) = strstr(&src[data_pos..], b"cleartomark") else {
        return false;
    };
    let mut trailer_pos = data_pos + trailer_offset;
    let mut zero_count = 0;
    trailer_pos -= 1;
    while trailer_pos > data_pos && zero_count < 512 {
        if src[trailer_pos] == b'\n' || src[trailer_pos] == b'\r' || src[trailer_pos] == b' ' {
            trailer_pos -= 1;
            continue;
        } else if src[trailer_pos] == b'0' {
            zero_count += 1;
        } else {
            return false;
        }
        trailer_pos -= 1;
    }
    if zero_count != 512 {
        return false;
    }

    *hex_data_len = trailer_pos - *header_len;
    *trailer_len = size - *header_len - *hex_data_len;

    // Verify that the data section is hex encoded and count the bytes.
    let mut nibbles = 0;
    while data_pos < trailer_pos {
        let c = src[data_pos];
        data_pos += 1;
        if is_space(c) {
            continue;
        }
        // isxdigit() is locale-sensitive https://bugs.skia.org/8285
        if !c.is_ascii_hexdigit() {
            return false;
        }
        nibbles += 1;
    }
    *data_len = (nibbles + 1) / 2;

    true
}

// Port of: src/pdf/SkPDFType1Font.cpp#L143-L155 (hexToBin, chrome/m156)
fn hex_to_bin(c: u8) -> i8 {
    if !c.is_ascii_hexdigit() {
        -1
    } else if c <= b'9' {
        (c - b'0') as i8
    } else if c <= b'F' {
        (c - b'A' + 10) as i8
    } else if c <= b'f' {
        (c - b'a' + 10) as i8
    } else {
        -1
    }
}

// Port of: src/pdf/SkPDFType1Font.cpp#L157-L246 (convert_type1_font_stream, chrome/m156)
fn convert_type1_font_stream(
    src_stream: Option<&mut dyn StreamAsset>,
    header_len: &mut usize,
    data_len: &mut usize,
    trailer_len: &mut usize,
) -> Option<Vec<u8>> {
    let src_stream = src_stream?;
    let src_len = src_stream.get_length();
    if src_len == 0 {
        return None;
    }
    // Flatten and Nul-terminate the source stream so that we can use
    // strstr() to search it.
    let mut source_buffer = vec![0u8; src_len + 1];
    let n = src_stream.read(&mut source_buffer[..src_len]);
    // (void)srcStream->read(...): a short read leaves the rest zero.
    let _ = n;
    source_buffer[src_len] = 0;
    let src: &[u8] = &source_buffer;

    if parse_pfb(&src[..src_len], header_len, data_len, trailer_len) {
        const PFB_SECTION_HEADER_LENGTH: usize = 6;
        let length = *header_len + *data_len + *trailer_len;
        if length == 0 {
            return None;
        }
        if src_len < length + (2 * PFB_SECTION_HEADER_LENGTH) {
            return None;
        }
        let mut data = vec![0u8; length];

        // There is a six-byte section header before header and data
        // (but not trailer) that we're not going to copy.
        let src_header = PFB_SECTION_HEADER_LENGTH;
        let src_data = src_header + *header_len + PFB_SECTION_HEADER_LENGTH;
        let src_trailer = src_data + *data_len;
        debug_assert_eq!(
            src_trailer + *trailer_len,
            length + (2 * PFB_SECTION_HEADER_LENGTH)
        );

        let dst_data = *header_len;
        let dst_trailer = dst_data + *data_len;
        debug_assert_eq!(dst_trailer + *trailer_len, length);

        data[..*header_len].copy_from_slice(&src[src_header..src_header + *header_len]);
        data[dst_data..dst_data + *data_len].copy_from_slice(&src[src_data..src_data + *data_len]);
        data[dst_trailer..].copy_from_slice(&src[src_trailer..src_trailer + *trailer_len]);

        return Some(data);
    }

    // A PFA has to be converted for PDF.
    let mut hex_data_len = 0;
    if !parse_pfa(
        src,
        src_len,
        header_len,
        &mut hex_data_len,
        data_len,
        trailer_len,
    ) {
        return None;
    }
    let length = *header_len + *data_len + *trailer_len;
    if length == 0 {
        return None;
    }
    let mut buffer = vec![0u8; length];

    buffer[..*header_len].copy_from_slice(&src[..*header_len]);

    let hex_data = &src[*header_len..*header_len + hex_data_len];
    let mut output_offset = 0usize;
    let mut data_byte = 0u8; // To hush compiler.
    let mut high_nibble = true;
    for &c in hex_data {
        let cur_nibble = hex_to_bin(c);
        if cur_nibble < 0 {
            continue;
        }
        if high_nibble {
            data_byte = (cur_nibble as u8) << 4;
            high_nibble = false;
        } else {
            data_byte |= cur_nibble as u8;
            high_nibble = true;
            buffer[*header_len + output_offset] = data_byte;
            output_offset += 1;
        }
    }
    if !high_nibble {
        buffer[*header_len + output_offset] = data_byte;
        output_offset += 1;
    }
    debug_assert_eq!(output_offset, *data_len);

    let result_trailer = *header_len + output_offset;
    buffer[result_trailer..result_trailer + *trailer_len].copy_from_slice(
        &src[*header_len + hex_data_len..*header_len + hex_data_len + *trailer_len],
    );
    Some(buffer)
}

// Port of: src/pdf/SkPDFType1Font.cpp#L248-L250 (can_embed, chrome/m156)
fn can_embed(metrics: &AdvancedTypefaceMetrics) -> bool {
    !metrics.flags.contains(FontFlags::NOT_EMBEDDABLE)
}

// Port of: src/pdf/SkPDFType1Font.cpp#L252-L254 (from_font_units, chrome/m156)
fn from_font_units(scaled: scalar, em_size: u16) -> scalar {
    if em_size == 1000 {
        scaled
    } else {
        scaled * 1000.0 / scalar::from(em_size)
    }
}

// Port of: src/pdf/SkPDFType1Font.cpp#L256-L286 (make_type1_font_descriptor, chrome/m156)
fn make_type1_font_descriptor(
    doc: &DocHandle,
    pdf_strike_spec: &PdfStrikeSpec,
    info: Option<&AdvancedTypefaceMetrics>,
) -> PdfIndirectReference {
    let mut descriptor = PdfDict::new(Some("FontDescriptor"));
    let em_size =
        u16::try_from(scalar_round_to_int(pdf_strike_spec.units_per_em)).expect("SkToU16");
    if let Some(info) = info {
        populate_common_font_descriptor(&mut descriptor, info, em_size, 0);
        if can_embed(info) {
            let mut header = 0;
            let mut data = 0;
            let mut trailer = 0;
            let typeface = pdf_strike_spec.strike_spec.typeface();
            let raw_font_data = typeface.open_stream();
            let font_data = match raw_font_data {
                Some((mut stream, _ttc_index)) => convert_type1_font_stream(
                    Some(stream.as_mut()),
                    &mut header,
                    &mut data,
                    &mut trailer,
                ),
                None => None,
            };
            if let Some(font_data) = font_data {
                let mut dict = PdfDict::new(None);
                dict.insert_int_usize("Length1", header);
                dict.insert_int_usize("Length2", data);
                dict.insert_int_usize("Length3", trailer);
                let font_stream = doc.stream_out(Some(dict), &font_data, true);
                descriptor.insert_ref("FontFile", font_stream);
            }
        }
    }
    doc.emit_new(&descriptor)
}

// Port of: src/pdf/SkPDFType1Font.cpp#L289-L302 (type_1_glyphnames, chrome/m156)
fn type_1_glyphnames(doc: &DocHandle, typeface: &Typeface) -> Rc<Vec<String>> {
    let typeface_id = typeface.unique_id();
    if let Some(names) = doc.with(|d| d.type1_glyph_names.get(&typeface_id).cloned()) {
        return names;
    }
    let mut names = vec![String::new(); usize::try_from(typeface.count_glyphs()).unwrap_or(0)];
    get_type1_glyph_names(typeface, &mut names);
    let names = Rc::new(names);
    doc.with(|d| d.type1_glyph_names.insert(typeface_id, Rc::clone(&names)));
    names
}

// Port of: src/pdf/SkPDFType1Font.cpp#L304-L318 (type1_font_descriptor, chrome/m156)
fn type1_font_descriptor(doc: &DocHandle, pdf_strike_spec: &PdfStrikeSpec) -> PdfIndirectReference {
    let typeface = pdf_strike_spec.strike_spec.typeface();
    let typeface_id = typeface.unique_id();
    if let Some(reference) = doc.with(|d| d.font_descriptors.get(&typeface_id).copied()) {
        return reference;
    }
    let info = get_metrics(typeface, doc);
    let font_descriptor = make_type1_font_descriptor(doc, pdf_strike_spec, info.as_deref());
    doc.with(|d| d.font_descriptors.insert(typeface_id, font_descriptor));
    font_descriptor
}

/// `SkPDFEmitType1Font`: writes a Type1 font.
// Port of: src/pdf/SkPDFType1Font.cpp#L321-L363 (chrome/m156)
#[doc(alias = "SkPDFEmitType1Font")]
pub fn emit_type1_font(pdf_font: &PdfFont, doc: &DocHandle) {
    let strike = pdf_font.strike();
    let typeface = strike.path().strike_spec.typeface().clone();
    let glyph_names = type_1_glyphnames(doc, &typeface);
    let first_glyph_id = pdf_font.first_glyph_id();
    let last_glyph_id = pdf_font.last_glyph_id();

    let mut font = PdfDict::new(Some("Font"));
    font.insert_ref("FontDescriptor", type1_font_descriptor(doc, strike.path()));
    font.insert_name("Subtype", "Type1");
    if let Some(info) = get_metrics(&typeface, doc) {
        font.insert_name_escaped("BaseFont", &info.post_script_name);
    }

    // glyphCount not including glyph 0
    let glyph_count = 1 + u32::from(last_glyph_id) - u32::from(first_glyph_id);
    debug_assert!(glyph_count > 0 && glyph_count <= 255);
    font.insert_int("FirstChar", 0);
    font.insert_int("LastChar", i32::try_from(glyph_count).expect("at most 255"));
    {
        let em_size = strike.path().units_per_em as i32;
        let mut widths = PdfArray::new();

        let glyph_range_size = usize::from(last_glyph_id - first_glyph_id) + 2;
        let mut glyph_ids: Vec<GlyphId> = vec![0; glyph_range_size];
        glyph_ids[0] = 0;
        for g_id in first_glyph_id..=last_glyph_id {
            glyph_ids[usize::from(g_id - first_glyph_id) + 1] = g_id;
        }
        let metrics = BulkGlyphMetrics::new(&strike.path().strike_spec);
        let glyphs = metrics.glyphs(&glyph_ids);
        for glyph in glyphs.iter().take(glyph_range_size) {
            widths.append_scalar(from_font_units(
                glyph.advance_x(),
                u16::try_from(em_size).expect("SkToU16"),
            ));
        }
        font.insert_object("Widths", Box::new(widths));
    }
    let mut enc_diffs = PdfArray::new();
    enc_diffs.reserve(usize::from(last_glyph_id - first_glyph_id) + 3);
    enc_diffs.append_int(0);

    debug_assert!(glyph_names.len() > usize::from(last_glyph_id));
    let unknown = "UNKNOWN";
    let name_or_unknown = |name: &String| -> String {
        if name.is_empty() {
            unknown.to_owned()
        } else {
            name.clone()
        }
    };
    enc_diffs.append_name_escaped(name_or_unknown(&glyph_names[0]));
    for g_id in first_glyph_id..=last_glyph_id {
        enc_diffs.append_name_escaped(name_or_unknown(&glyph_names[usize::from(g_id)]));
    }

    let mut encoding = PdfDict::new(Some("Encoding"));
    encoding.insert_object("Differences", Box::new(enc_diffs));
    font.insert_object("Encoding", Box::new(encoding));

    doc.emit(&font, pdf_font.indirect_reference());
}
