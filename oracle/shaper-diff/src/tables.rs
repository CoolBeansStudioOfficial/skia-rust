// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Modes b and c's inputs for the C++ driver: what Skia's font functions
//! (`SkShaper_harfbuzz.cpp#L72-L238`) return for a [`Font`], as tables.
//! `shaper-diff tables` writes them; the C++ driver installs them as `hb_font_funcs`, so both
//! sides shape with identical inputs. The numbers are those of `skia-rust-shaper`'s
//! [`SkiaFontFuncs`], which the Rust side uses itself, over the real `Font` of a Fontations
//! typeface (mode b) or of the portable `TestTypeface` (mode c).

use std::path::Path;

use skia_rust_core::font::Font;
use skia_rust_shaper::harfbuzz::SkiaFontFuncs;

/// Skia's font functions for one font at one size, as plain tables.
#[derive(Debug, Default)]
pub struct FontTable {
    /// Per glyph: the horizontal advance (16.16).
    pub advance: Vec<i32>,
    /// Per glyph: x bearing, y bearing, width, height (16.16, y up).
    pub extents: Vec<[i32; 4]>,
    /// Nominal glyph mapping: `(code point, glyph)`, sorted, glyph 0 left out.
    pub cmap: Vec<(u32, u32)>,
}

impl FontTable {
    /// The nominal glyph of every Unicode code point under `font` (`skhb_nominal_glyph`), sorted,
    /// glyph 0 left out. It does not depend on the size, so one mapping serves every config.
    #[must_use]
    pub fn cmap_of(font: &Font) -> Vec<(u32, u32)> {
        let funcs = SkiaFontFuncs { font: font.clone() };
        (0..=0x0010_FFFF_u32)
            .filter_map(|cp| funcs.nominal_glyph_of(cp).map(|g| (cp, u32::from(g))))
            .collect()
    }

    /// The table of `font` with the mapping `cmap` (from [`FontTable::cmap_of`]): the advance and
    /// the extents of every glyph of its typeface.
    #[must_use]
    pub fn from_font(font: &Font, cmap: &[(u32, u32)]) -> FontTable {
        let funcs = SkiaFontFuncs { font: font.clone() };
        let glyph_count = u16::try_from(font.typeface().count_glyphs()).unwrap_or(0);
        let mut table = FontTable::default();
        for glyph in 0..glyph_count {
            table.advance.push(funcs.advance_of_glyph(glyph));
            let e = funcs.extents_of_glyph(glyph);
            table
                .extents
                .push([e.x_bearing, e.y_bearing, e.width, e.height]);
        }
        table.cmap = cmap.to_vec();
        table
    }

    /// Writes the table in the format `cpp/shaper_diff.cc` reads: `SDT1`, glyph count, advances,
    /// extents, mapping count, `(code point, glyph)` pairs sorted by code point; little-endian.
    ///
    /// # Errors
    /// If the file cannot be written.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let mut out = Vec::new();
        out.extend_from_slice(b"SDT1");
        let n = u32::try_from(self.advance.len()).map_err(|e| e.to_string())?;
        out.extend_from_slice(&n.to_le_bytes());
        for a in &self.advance {
            out.extend_from_slice(&a.to_le_bytes());
        }
        for e in &self.extents {
            for v in e {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
        out.extend_from_slice(
            &u32::try_from(self.cmap.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        for (cp, gid) in &self.cmap {
            out.extend_from_slice(&cp.to_le_bytes());
            out.extend_from_slice(&gid.to_le_bytes());
        }
        std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
    }
}
