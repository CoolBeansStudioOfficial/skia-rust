// Copyright 2016 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/FontTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_types::FontHinting;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::typeface::Typeface;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_tools::font_tool_utils::sample_user_typeface;

use crate::{Reporter, def_test, reporter_assert};

/// `serialize_deserialize(font)`: flattens the font with a typeface recorder and reads it back.
// Port of: tests/FontTest.cpp#L8-L30 (chrome/m156), serialize_deserialize
fn serialize_deserialize(reporter: &mut Reporter, font: &Font) -> Font {
    let mut wb = BinaryWriteBuffer::new();
    wb.set_typeface_recorder();
    font.flatten(&mut wb);
    let size = wb.bytes_written();
    let mut storage = vec![0u8; size];
    wb.write_to_memory(&mut storage);
    let count = wb.typeface_recorder().map_or(0, <[Typeface]>::len);

    let mut rb = ReadBuffer::new(&storage);
    if count > 0 {
        // The recorded typeface is the font's own, and the read side gets the same object.
        rb.set_typeface_array(vec![font.typeface().clone()]);
    }
    let mut clone = Font::default();
    reporter_assert!(reporter, clone.unflatten(&mut rb));
    clone
}

// Port of: tests/FontTest.cpp#L32-L45 (chrome/m156), apply_flags
const FORCE_AUTO_HINTING: u32 = 1 << 0;
const EMBEDDED_BITMAPS: u32 = 1 << 1;
const SUBPIXEL: u32 = 1 << 2;
const LINEAR_METRICS: u32 = 1 << 3;
const EMBOLDEN: u32 = 1 << 4;
const BASELINE_SNAP: u32 = 1 << 5;
const ALL_BITS: u32 = 0x3F;

/// `apply_flags(font, flags)`: sets each flag bit on the font.
// Port of: tests/FontTest.cpp#L32-L45 (chrome/m156), apply_flags
fn apply_flags(font: &mut Font, flags: u32) {
    font.set_force_auto_hinting(flags & FORCE_AUTO_HINTING != 0);
    font.set_embedded_bitmaps(flags & EMBEDDED_BITMAPS != 0);
    font.set_subpixel(flags & SUBPIXEL != 0);
    font.set_linear_metrics(flags & LINEAR_METRICS != 0);
    font.set_embolden(flags & EMBOLDEN != 0);
    font.set_baseline_snap(flags & BASELINE_SNAP != 0);
}

// Port of: tests/FontTest.cpp#L74-L108 (chrome/m156), Font_flatten
def_test!(Font_flatten, |reporter| {
    let sizes = [0.0, 0.001, 1.0, 10.0, 10.001, 100_000.01];
    let scales = [-5.0, 0.0, 1.0, 5.0];
    let skews = [-5.0, 0.0, 5.0];
    let edges = [Edging::Alias, Edging::SubpixelAntiAlias];
    let hints = [FontHinting::None, FontHinting::Full];
    let typefaces = [None, Some(sample_user_typeface())];

    let mut font = Font::default();
    for size in sizes {
        font.set_size(size);
        for scale in scales {
            font.set_scale_x(scale);
            for skew in skews {
                font.set_skew_x(skew);
                for edge in edges {
                    font.set_edging(edge);
                    for hint in hints {
                        font.set_hinting(hint);
                        for flag in [
                            FORCE_AUTO_HINTING,
                            EMBEDDED_BITMAPS,
                            SUBPIXEL,
                            LINEAR_METRICS,
                            EMBOLDEN,
                            BASELINE_SNAP,
                            ALL_BITS,
                        ] {
                            apply_flags(&mut font, flag);
                            for typeface in &typefaces {
                                font.set_typeface(typeface.clone());
                                let clone = serialize_deserialize(reporter, &font);
                                reporter_assert!(reporter, font == clone);
                            }
                        }
                    }
                }
            }
        }
    }
});
