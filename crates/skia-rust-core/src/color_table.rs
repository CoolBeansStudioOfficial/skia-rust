// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkColorTable.h, src/core/SkColorTable.cpp

//! `SkColorTable`: four 256-entry byte tables (alpha, red, green, blue), used by the table color
//! filters.
//!
//! The tables are also kept as Skia's 256x4 A8 bitmap (`fTable`), which is what the Graphite
//! table color filter uploads and caches by pixel identity. The bitmap is built once, when the
//! table is made, so every use of the same table shares its pixel ref.
//!
//! skia-rust: Skia's `flatten` writes the bitmap's pixels; here the four arrays are kept behind an
//! `Arc` and flattened from them.

use std::sync::Arc;

use crate::alpha_type::AlphaType;
use crate::bitmap::Bitmap;
use crate::color_type::ColorType;
use crate::image_info::ImageInfo;
use crate::read_buffer::ReadBuffer;
use crate::write_buffer::BinaryWriteBuffer;

/// The four 256-entry tables of an `SkColorTable`, in Skia's row order: alpha, red, green, blue.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Tables([[u8; 256]; 4]);

/// A shared table of four 256-entry byte maps, one per channel (`SkColorTable`).
// Port of: include/core/SkColorTable.h#L19-L59 (chrome/m156)
#[doc(alias = "SkColorTable")]
#[derive(Clone, Debug)]
pub struct ColorTable {
    tables: Arc<Tables>,
    /// `fTable`: the 256x4 A8 bitmap, row 0 alpha, row 1 red, row 2 green, row 3 blue. Immutable.
    bitmap: Arc<Bitmap>,
}

/// The identity map, `table[i] = i`, used for missing channels in [`ColorTable::make_argb`].
fn identity_table() -> [u8; 256] {
    let mut table = [0_u8; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = u8::try_from(i).expect("i < 256");
    }
    table
}

/// Builds the 256x4 A8 bitmap of `tables` (`SkColorTable::Make`'s `fTable`), then makes it
/// immutable.
// Port of: src/core/SkColorTable.cpp#L14-L39 (chrome/m156), the bitmap fill
fn make_table_bitmap(tables: &Tables) -> Bitmap {
    let mut table = Bitmap::new();
    table.alloc_pixels_info(
        &ImageInfo::new((256, 4), ColorType::Alpha8, AlphaType::Premul, None),
        None,
    );
    for (row, map) in tables.0.iter().enumerate() {
        for (x, &value) in map.iter().enumerate() {
            let x = i32::try_from(x).expect("x < 256");
            let y = i32::try_from(row).expect("row < 4");
            table.set_addr8(x, y, value);
        }
    }
    table.set_immutable();
    table
}

impl ColorTable {
    fn from_tables(tables: Tables) -> ColorTable {
        let bitmap = make_table_bitmap(&tables);
        ColorTable {
            tables: Arc::new(tables),
            bitmap: Arc::new(bitmap),
        }
    }

    /// The table that maps each channel of a color through `table` (`SkColorTable::Make(table)`).
    // Port of: include/core/SkColorTable.h#L29-L33 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(table: &[u8; 256]) -> ColorTable {
        Self::from_tables(Tables([*table; 4]))
    }

    /// The table with one map per channel; `None` channels are the identity. Returns `None` when
    /// every channel is the identity (`SkColorTable::Make(tableA, tableR, tableG, tableB)`).
    // Port of: src/core/SkColorTable.cpp#L10-L39 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make_argb(
        table_a: Option<&[u8; 256]>,
        table_r: Option<&[u8; 256]>,
        table_g: Option<&[u8; 256]>,
        table_b: Option<&[u8; 256]>,
    ) -> Option<ColorTable> {
        if table_a.is_none() && table_r.is_none() && table_g.is_none() && table_b.is_none() {
            return None; // The table is the identity
        }
        let identity = identity_table();
        let pick = |t: Option<&[u8; 256]>| *t.unwrap_or(&identity);
        Some(Self::from_tables(Tables([
            pick(table_a),
            pick(table_r),
            pick(table_g),
            pick(table_b),
        ])))
    }

    /// The 256x4 A8 bitmap of the four maps (`SkColorTable::bitmap`). Its pixel ref is shared by
    /// every copy of this table.
    // Port of: include/core/SkColorTable.h#L57 (chrome/m156)
    #[must_use]
    pub fn bitmap(&self) -> &Bitmap {
        &self.bitmap
    }

    /// `SkColorTable::flatten`: the four maps as one 1024-byte array, alpha first.
    // Port of: src/core/SkColorTable.cpp#L41-L43 (chrome/m156)
    pub(crate) fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        let mut bytes = [0_u8; 4 * 256];
        for (i, table) in self.tables.0.iter().enumerate() {
            bytes[i * 256..(i + 1) * 256].copy_from_slice(table);
        }
        buffer.write_byte_array(&bytes);
    }

    /// `SkColorTable::Deserialize`: reads the 1024 bytes that [`flatten`](Self::flatten) wrote.
    // Port of: src/core/SkColorTable.cpp#L45-L51 (chrome/m156)
    pub(crate) fn deserialize(buffer: &mut ReadBuffer<'_>) -> Option<ColorTable> {
        let mut argb = [0_u8; 4 * 256];
        if !buffer.read_byte_array(&mut argb) {
            return None;
        }
        let (a, rest) = argb.split_at(256);
        let (r, rest) = rest.split_at(256);
        let (g, b) = rest.split_at(256);
        ColorTable::make_argb(
            Some(a.try_into().ok()?),
            Some(r.try_into().ok()?),
            Some(g.try_into().ok()?),
            Some(b.try_into().ok()?),
        )
    }

    /// The alpha map (`alphaTable`).
    #[doc(alias = "alphaTable")]
    #[must_use]
    pub fn alpha_table(&self) -> &[u8; 256] {
        &self.tables.0[0]
    }

    /// The red map (`redTable`).
    #[doc(alias = "redTable")]
    #[must_use]
    pub fn red_table(&self) -> &[u8; 256] {
        &self.tables.0[1]
    }

    /// The green map (`greenTable`).
    #[doc(alias = "greenTable")]
    #[must_use]
    pub fn green_table(&self) -> &[u8; 256] {
        &self.tables.0[2]
    }

    /// The blue map (`blueTable`).
    #[doc(alias = "blueTable")]
    #[must_use]
    pub fn blue_table(&self) -> &[u8; 256] {
        &self.tables.0[3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::size::ISize;

    #[test]
    fn identity_channels_map_to_themselves() {
        let mut inverted = [0_u8; 256];
        for (i, e) in inverted.iter_mut().enumerate() {
            *e = u8::try_from(255 - i).expect("255 - i < 256");
        }
        assert!(ColorTable::make_argb(None, None, None, None).is_none());
        let t = ColorTable::make_argb(Some(&inverted), None, None, None).unwrap();
        assert_eq!(t.alpha_table(), &inverted);
        assert_eq!(t.red_table()[7], 7);
        assert_eq!(t.blue_table()[255], 255);
    }

    #[test]
    fn bitmap_holds_the_four_rows() {
        let mut inverted = [0_u8; 256];
        for (i, e) in inverted.iter_mut().enumerate() {
            *e = u8::try_from(255 - i).expect("255 - i < 256");
        }
        let t = ColorTable::make_argb(Some(&inverted), None, None, None).unwrap();
        let bm = t.bitmap();
        assert_eq!(bm.dimensions(), ISize::new(256, 4));
        assert!(bm.is_immutable());
        assert_eq!(bm.get_addr8(0, 0), 255);
        assert_eq!(bm.get_addr8(7, 1), 7);
        assert_eq!(bm.get_addr8(255, 3), 255);
        // Clones share the bitmap, and so its pixel ref.
        let copy = t.clone();
        assert!(std::ptr::eq(copy.bitmap(), t.bitmap()));
    }
}
