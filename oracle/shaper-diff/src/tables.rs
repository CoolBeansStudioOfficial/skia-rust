// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Mode b's inputs: what Skia's font functions (`SkShaper_harfbuzz.cpp#L72-L238`) return for a
//! font at a size, as tables. `shaper-diff tables` writes them for the C++ driver, which installs
//! them as `hb_font_funcs`; the Rust side shapes with the same tables through HarfRust's
//! `FontFuncs`, so both sides see identical inputs.
//!
//! The numbers are those of Fontations (skrifa, Skia's pin) for the unhinted font at `size`:
//! `GlyphMetrics::advance_width` and `bounds`, which is what `SkFont::getWidth` and `getBounds`
//! reduce to once `Font` (text T9) and the scaler context are ported. Until then this is the stand-in,
//! and the shaper comparison does not depend on it: both sides read the same table.

use std::collections::HashMap;
use std::path::Path;

use harfrust::GlyphExtents;
use harfrust::GlyphId;
use harfrust::funcs::{BuiltinFontFuncs, FontFuncs};
use read_fonts::FontRef;
use read_fonts::TableProvider as _;
use skia_rust_core::scalar::{scalar_ceil_to_int, scalar_floor_to_int, scalar_round_to_int};
use skrifa::MetadataProvider as _;
use skrifa::instance::{LocationRef, Size};
use skrifa::metrics::GlyphMetrics;

/// `skhb_position`: Skia's value as a HarfBuzz 16.16 position.
// Port of: modules/skshaper/src/SkShaper_harfbuzz.cpp#L72-L76 (chrome/m156)
#[must_use]
pub fn skhb_position(value: f32) -> i32 {
    // Treat HarfBuzz hb_position_t as 16.16 fixed-point.
    const HB_POSITION_1: f32 = 65536.0;
    scalar_round_to_int(value * HB_POSITION_1)
}

/// Skia's font functions for one font at one size.
#[derive(Debug, Default)]
pub struct FontTable {
    /// Per glyph: the horizontal advance (16.16).
    pub advance: Vec<i32>,
    /// Per glyph: x bearing, y bearing, width, height (16.16, y up).
    pub extents: Vec<[i32; 4]>,
    /// Nominal glyph mapping.
    pub cmap: HashMap<u32, u32>,
}

impl FontTable {
    /// Builds the table of `font` at `size` (`subpixel` as `SkFont::isSubpixel`).
    #[must_use]
    pub fn build(font: &FontRef<'_>, size: f32, subpixel: bool) -> FontTable {
        let glyph_count = font.maxp().map_or(0, |m| m.num_glyphs());
        let metrics = GlyphMetrics::new(font, Size::new(size), LocationRef::default());
        let mut table = FontTable::default();
        for gid in 0..u32::from(glyph_count) {
            let id = GlyphId::new(gid);
            // skhb_glyph_h_advance (#L129-L142)
            let mut adv = metrics.advance_width(id).unwrap_or(0.0);
            if !subpixel {
                // Exact: the rounded value is an integer far below 2^24.
                #[allow(clippy::cast_precision_loss)]
                {
                    adv = scalar_round_to_int(adv) as f32;
                }
            }
            table.advance.push(skhb_position(adv));
            // skhb_glyph_extents (#L182-L208); Skia is y-down but HarfBuzz is y-up.
            let (mut left, mut top, mut right, mut bottom) =
                metrics.bounds(id).map_or((0.0, 0.0, 0.0, 0.0), |b| {
                    (b.x_min, -b.y_max, b.x_max, -b.y_min)
                });
            if !subpixel {
                // sk_bounds.set(sk_bounds.roundOut())
                #[allow(clippy::cast_precision_loss)]
                {
                    left = scalar_floor_to_int(left) as f32;
                    top = scalar_floor_to_int(top) as f32;
                    right = scalar_ceil_to_int(right) as f32;
                    bottom = scalar_ceil_to_int(bottom) as f32;
                }
            }
            table.extents.push([
                skhb_position(left),
                skhb_position(-top),
                skhb_position(right - left),
                skhb_position(-(bottom - top)),
            ]);
        }
        for (cp, gid) in font.charmap().mappings() {
            // skhb_nominal_glyph: `return *glyph != 0`.
            if gid.to_u32() != 0 {
                table.cmap.insert(cp, gid.to_u32());
            }
        }
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
        let mut pairs: Vec<_> = self.cmap.iter().collect();
        pairs.sort();
        out.extend_from_slice(
            &u32::try_from(pairs.len())
                .map_err(|e| e.to_string())?
                .to_le_bytes(),
        );
        for (cp, gid) in pairs {
            out.extend_from_slice(&cp.to_le_bytes());
            out.extend_from_slice(&gid.to_le_bytes());
        }
        std::fs::write(path, out).map_err(|e| format!("{}: {e}", path.display()))
    }
}

impl FontFuncs for FontTable {
    // skhb_nominal_glyph
    fn nominal_glyph(&mut self, _: &BuiltinFontFuncs<'_>, c: u32) -> Option<GlyphId> {
        self.cmap.get(&c).map(|g| GlyphId::new(*g))
    }

    // skhb_glyph_h_advance: past the table, 0 (the C++ driver does the same).
    fn advance_width(&mut self, _: &BuiltinFontFuncs<'_>, g: GlyphId) -> i32 {
        self.advance.get(g.to_u32() as usize).copied().unwrap_or(0)
    }

    // skhb_glyph_extents: always succeeds.
    fn extents(&mut self, _: &BuiltinFontFuncs<'_>, g: GlyphId) -> Option<GlyphExtents> {
        let [x_bearing, y_bearing, width, height] = self
            .extents
            .get(g.to_u32() as usize)
            .copied()
            .unwrap_or_default();
        Some(GlyphExtents {
            x_bearing,
            y_bearing,
            width,
            height,
        })
    }
}
