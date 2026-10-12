// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Mode c's synthetic font callbacks: the formulas of `syn_*` in `cpp/shaper_diff.cc`.
//!
//! The portable `TestTypeface` has no tables, so in every module golden HarfBuzz shaped with an
//! empty face and Skia's callbacks (`docs/design/modules.md` §2.1). The callbacks here are not
//! `TestTypeface`'s numbers (that typeface lives in `skia-rust-tools`, which this crate does not
//! depend on): they are deterministic stand-ins that map most code points, leave every 11th
//! unmapped, and give some glyphs empty extents, so normalization, space and mark fallbacks all
//! run.

use harfrust::GlyphExtents;
use harfrust::GlyphId;
use harfrust::funcs::{BuiltinFontFuncs, FontFuncs};

/// Units per em of the empty face (`TestTypeface.h#L107`).
pub const UPEM: u16 = 2048;

/// The glyph of `cp`, 0 when unmapped.
#[must_use]
pub fn glyph(cp: u32) -> u32 {
    if cp.is_multiple_of(11) {
        0
    } else {
        1 + (cp.wrapping_mul(2_654_435_761) >> 16) % 2999
    }
}

/// The 16.16 advance of glyph `g`.
#[must_use]
pub fn advance(g: u32, subpixel: bool) -> i32 {
    let adv = i32::try_from(((g * 37) % 900 + 300) << 12).unwrap_or(i32::MAX);
    if subpixel {
        adv
    } else {
        ((adv + 32768) >> 16) << 16
    }
}

/// The 16.16 extents of glyph `g`.
#[must_use]
pub fn extents(g: u32) -> GlyphExtents {
    if g.is_multiple_of(5) {
        return GlyphExtents::default();
    }
    let v = |n: u32| i32::try_from(n << 14).unwrap_or(i32::MAX);
    GlyphExtents {
        x_bearing: v(g % 7),
        y_bearing: v((g * 3) % 11 + 5),
        width: v((g * 5) % 9 + 4),
        height: -v((g * 7) % 13 + 4),
    }
}

/// The callbacks as HarfRust font functions.
#[derive(Debug)]
pub struct SyntheticFuncs {
    /// Whether the font is subpixel (advances are not rounded).
    pub subpixel: bool,
}

impl FontFuncs for SyntheticFuncs {
    fn nominal_glyph(&mut self, _: &BuiltinFontFuncs<'_>, c: u32) -> Option<GlyphId> {
        match glyph(c) {
            0 => None,
            g => Some(GlyphId::new(g)),
        }
    }

    fn advance_width(&mut self, _: &BuiltinFontFuncs<'_>, g: GlyphId) -> i32 {
        advance(g.to_u32(), self.subpixel)
    }

    fn extents(&mut self, _: &BuiltinFontFuncs<'_>, g: GlyphId) -> Option<GlyphExtents> {
        Some(extents(g.to_u32()))
    }
}

/// A minimal sfnt that has only a `head` table with `UPEM` (what the synthesized face of M6 will
/// be): no `maxp`, `cmap`, `hhea`, `GSUB`, ...
#[must_use]
pub fn empty_sfnt() -> Vec<u8> {
    let mut out = Vec::new();
    // Offset table: sfntVersion, numTables = 1, searchRange = 16, entrySelector = 0, rangeShift = 0.
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&[0, 1, 0, 16, 0, 0, 0, 0]);
    // Table record: tag, checkSum, offset = 28, length = 54.
    out.extend_from_slice(b"head");
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&28u32.to_be_bytes());
    out.extend_from_slice(&54u32.to_be_bytes());
    // head: version 1.0, fontRevision, checkSumAdjustment, magic, flags, unitsPerEm, created,
    // modified, bbox, macStyle, lowestRecPPEM, fontDirectionHint = 2, indexToLocFormat,
    // glyphDataFormat.
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&0x5F0F_3CF5u32.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&UPEM.to_be_bytes());
    out.extend_from_slice(&[0u8; 16]);
    out.extend_from_slice(&[0u8; 8]);
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&0u16.to_be_bytes());
    out.extend_from_slice(&2i16.to_be_bytes());
    out.extend_from_slice(&0i16.to_be_bytes());
    out.extend_from_slice(&0i16.to_be_bytes());
    debug_assert_eq!(out.len(), 28 + 54);
    out
}
