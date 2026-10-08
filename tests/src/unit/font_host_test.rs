// Copyright 2011 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontHostTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::font::Font;
use skia_rust_core::font_stream::{count_ttc_entries, get_table_size, get_table_tags};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FontHinting, FourByteTag, TextEncoding, set_four_byte_tag};
use skia_rust_core::stream::{MemoryStream, StreamRewindable};
use skia_rust_core::typeface::Typeface;
use skia_rust_tools::font_tool_utils::{create_test_typeface, create_typeface_from_resource};

use crate::resources::get_resource_as_data;
use crate::{Reporter, def_font_test, errorf, reporter_assert};

// Port of: tests/FontHostTest.cpp#L22-L24 (chrome/m156)
const FONT_TABLE_TAG_HEAD: FourByteTag = set_four_byte_tag(b'h', b'e', b'a', b'd');
const FONT_TABLE_TAG_HHEA: FourByteTag = set_four_byte_tag(b'h', b'h', b'e', b'a');
const FONT_TABLE_TAG_MAXP: FourByteTag = set_four_byte_tag(b'm', b'a', b'x', b'p');

/// `gKnownTableSizes`: the tags and sizes of the tables that have a fixed size.
// Port of: tests/FontHostTest.cpp#L47-L52 (chrome/m156)
const KNOWN_TABLE_SIZES: [(FourByteTag, usize); 2] =
    [(FONT_TABLE_TAG_HEAD, 54), (FONT_TABLE_TAG_HHEA, 36)];

/// `GetResourceAsStream(path)`: the resource as a memory stream, or `None` if it is missing.
fn resource_stream(path: &str) -> Option<Box<MemoryStream>> {
    let data = get_resource_as_data(path)?;
    Some(MemoryStream::make_copy(&data))
}

/// `ToolUtils::CreateTypefaceFromResource(path)` (`None` in the portable configuration).
fn create_typeface_from_resource_path(path: &str) -> Option<Typeface> {
    let stream = resource_stream(path)?;
    create_typeface_from_resource(Some(stream), 0)
}

/// `SkEndian_SwapBE16` of the two bytes that `get_table_data` copied out.
fn be16(bytes: [u8; 2]) -> i32 {
    i32::from(u16::from_be_bytes(bytes))
}

// Test that getUnitsPerEm() agrees with a direct lookup in the 'head' table
// (if that table is available).
// Port of: tests/FontHostTest.cpp#L54-L71 (chrome/m156)
fn test_units_per_em(reporter: &mut Reporter, face: &Typeface) {
    let native_upem = face.units_per_em().unwrap_or(0);
    let mut table_upem = -1;
    let size = face.get_table_size(FONT_TABLE_TAG_HEAD).unwrap_or(0);
    if size != 0 {
        // unitsPerEm is at offset 18 into the 'head' table.
        let mut raw_upem = [0u8; 2];
        let _ = face.get_table_data(FONT_TABLE_TAG_HEAD, 18, 2, Some(&mut raw_upem));
        table_upem = be16(raw_upem);
    }
    if table_upem >= 0 {
        reporter_assert!(reporter, table_upem == native_upem);
    }
}

// Test that countGlyphs() agrees with a direct lookup in the 'maxp' table
// (if that table is available).
// Port of: tests/FontHostTest.cpp#L73-L88 (chrome/m156)
fn test_count_glyphs(reporter: &mut Reporter, face: &Typeface) {
    let native_glyphs = face.count_glyphs();
    let mut table_glyphs = -1;
    let size = face.get_table_size(FONT_TABLE_TAG_MAXP).unwrap_or(0);
    if size != 0 {
        // glyphs is at offset 4 into the 'maxp' table.
        let mut raw_glyphs = [0u8; 2];
        let _ = face.get_table_data(FONT_TABLE_TAG_MAXP, 4, 2, Some(&mut raw_glyphs));
        table_glyphs = be16(raw_glyphs);
    }
    if table_glyphs >= 0 {
        reporter_assert!(reporter, table_glyphs == native_glyphs);
    }
}

// Port of: tests/FontHostTest.cpp#L90-L114 (chrome/m156)
fn test_fontstream_entry(
    reporter: &mut Reporter,
    stream: &mut dyn StreamRewindable,
    tt_index: i32,
) {
    let n = get_table_tags(stream, tt_index, &mut []);
    let mut array = vec![0; usize::try_from(n).unwrap_or(0)];
    let n2 = get_table_tags(stream, tt_index, &mut array);
    reporter_assert!(reporter, n == n2);
    for &tag in &array {
        let size = get_table_size(stream, tt_index, tag);
        for (known_tag, known_size) in KNOWN_TABLE_SIZES {
            if known_tag == tag {
                reporter_assert!(reporter, known_size == size);
            }
        }
    }
}

// Port of: tests/FontHostTest.cpp#L116-L131 (chrome/m156)
fn test_fontstream(reporter: &mut Reporter) {
    let Some(mut stream) = resource_stream("fonts/test.ttc") else {
        eprintln!("Skipping FontHostTest::test_fontstream");
        return;
    };
    let count = count_ttc_entries(&mut *stream);
    for i in 0..count {
        test_fontstream_entry(reporter, &mut *stream, i);
    }
}

// Exercise this rare cmap format (platform 3, encoding 0)
// Port of: tests/FontHostTest.cpp#L133-L143 (chrome/m156)
fn test_symbolfont(reporter: &mut Reporter) {
    if let Some(tf) = create_typeface_from_resource_path("fonts/SpiderSymbol.ttf") {
        let c = 0xf021;
        let g = Font::from_typeface(Some(tf)).unichar_to_glyph(c);
        reporter_assert!(reporter, g == 3);
    } else {
        // not all platforms support data fonts, so we just note that failure
        eprintln!("Skipping FontHostTest::test_symbolfont");
    }
}

// Port of: tests/FontHostTest.cpp#L145-L187 (chrome/m156)
fn test_tables_of(reporter: &mut Reporter, face: &Typeface) {
    let count = face.count_tables();
    let mut tags = vec![0; count];
    let count2 = face.read_table_tags(&mut tags);
    reporter_assert!(reporter, count2 == count);
    for &tag in &tags {
        let size = face.get_table_size(tag).unwrap_or(0);
        reporter_assert!(reporter, size > 0);
        for (known_tag, known_size) in KNOWN_TABLE_SIZES {
            if known_tag == tag {
                reporter_assert!(reporter, known_size == size);
            }
        }
        let mut data = vec![0u8; size];
        let size2 = face.get_table_data(tag, 0, size, Some(&mut data));
        reporter_assert!(reporter, size2 == size);
        match face.copy_table_data(tag) {
            Some(data2) => {
                reporter_assert!(reporter, size == data2.size());
                reporter_assert!(reporter, data == data2.as_bytes());
            }
            None => errorf!(reporter, "copyTableData returned null for a present table"),
        }
    }
}

// Port of: tests/FontHostTest.cpp#L189-L218 (chrome/m156)
fn test_tables(reporter: &mut Reporter) {
    const NAMES: [Option<&str>; 11] = [
        None, // default font
        Some("Helvetica"),
        Some("Arial"),
        Some("Times"),
        Some("Times New Roman"),
        Some("Courier"),
        Some("Courier New"),
        Some("Terminal"),
        Some("MS Sans Serif"),
        Some("Hiragino Mincho ProN"),
        Some("MS PGothic"),
    ];
    for name in NAMES {
        // The C++ test skips a null typeface; CreateTestTypeface never returns one.
        let face = create_test_typeface(name, FontStyle::default());
        test_tables_of(reporter, &face);
        test_units_per_em(reporter, &face);
        test_count_glyphs(reporter, &face);
    }
}

/// `SkFontHinting` and the linear/subpixel settings of one `test_advances` row.
// Port of: tests/FontHostTest.cpp#L220-L232 (chrome/m156)
struct Settings {
    hinting: FontHinting,
    linear: bool,
    subpixel: bool,
}

/// The `fScaleX` and `fSkewX` of one `test_advances` row.
// Port of: tests/FontHostTest.cpp#L234-L238 (chrome/m156)
struct ScaleRec {
    scale_x: f32,
    skew_x: f32,
}

/// Verifies that the advance values returned by various methods match.
// Port of: tests/FontHostTest.cpp#L220-L291 (chrome/m156)
// The C++ compares the widths with `==`: they come from the same computation.
#[allow(clippy::float_cmp)]
fn test_advances(reporter: &mut Reporter) {
    const FACES: [Option<&str>; 9] = [
        None, // default font
        Some("Arial"),
        Some("Times"),
        Some("Times New Roman"),
        Some("Helvetica"),
        Some("Courier"),
        Some("Courier New"),
        Some("Verdana"),
        Some("monospace"),
    ];
    const SETTINGS: [Settings; 9] = [
        Settings {
            hinting: FontHinting::None,
            linear: false,
            subpixel: false,
        },
        Settings {
            hinting: FontHinting::None,
            linear: true,
            subpixel: false,
        },
        Settings {
            hinting: FontHinting::None,
            linear: false,
            subpixel: true,
        },
        Settings {
            hinting: FontHinting::Slight,
            linear: false,
            subpixel: false,
        },
        Settings {
            hinting: FontHinting::Slight,
            linear: true,
            subpixel: false,
        },
        Settings {
            hinting: FontHinting::Slight,
            linear: false,
            subpixel: true,
        },
        Settings {
            hinting: FontHinting::Normal,
            linear: false,
            subpixel: false,
        },
        Settings {
            hinting: FontHinting::Normal,
            linear: true,
            subpixel: false,
        },
        Settings {
            hinting: FontHinting::Normal,
            linear: false,
            subpixel: true,
        },
    ];
    const SCALE_REC: [ScaleRec; 4] = [
        ScaleRec {
            scale_x: 1.0,
            skew_x: 0.0,
        },
        ScaleRec {
            scale_x: 1.0 / 2.0,
            skew_x: 0.0,
        },
        ScaleRec {
            scale_x: 1.0,
            skew_x: -1.0 / 4.0,
        },
        ScaleRec {
            scale_x: 1.0 / 2.0,
            skew_x: -1.0 / 4.0,
        },
    ];
    let mut font = Font::default();
    let txt: &[u8] = b"long.text.with.lots.of.dots.";
    for face in FACES {
        font.set_typeface(Some(create_test_typeface(face, FontStyle::default())));
        for setting in &SETTINGS {
            font.set_hinting(setting.hinting);
            font.set_linear_metrics(setting.linear);
            font.set_subpixel(setting.subpixel);
            for scale in &SCALE_REC {
                font.set_scale_x(scale.scale_x);
                font.set_skew_x(scale.skew_x);
                // `measureText` without and with bounds: one call returns both here.
                let (width1, _) = font.measure_text(txt, TextEncoding::UTF8, None);
                let (width2, _) = font.measure_text(txt, TextEncoding::UTF8, None);
                reporter_assert!(reporter, width1 == width2);
            }
        }
    }
}

// Port of: tests/FontHostTest.cpp#L293-L298 (chrome/m156)
def_font_test!(FontHost, |reporter| {
    test_tables(reporter);
    test_fontstream(reporter);
    test_advances(reporter);
    test_symbolfont(reporter);
});
