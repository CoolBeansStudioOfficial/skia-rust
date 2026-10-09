// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkColorTable.h, src/core/SkColorTable.cpp

//! `SkColorTable`: four 256-entry byte tables (alpha, red, green, blue), used by the table color
//! filters.
//!
//! skia-rust: Skia stores the tables in an A8 bitmap (`fTable`) and serializes them. Here they
//! are four arrays shared behind an `Arc`; flattening is not ported.

use std::sync::Arc;

use crate::read_buffer::ReadBuffer;
use crate::write_buffer::BinaryWriteBuffer;

/// The four 256-entry tables of an `SkColorTable`, in Skia's row order: alpha, red, green, blue.
#[derive(Clone, PartialEq, Eq, Debug)]
struct Tables([[u8; 256]; 4]);

/// A shared table of four 256-entry byte maps, one per channel (`SkColorTable`).
// Port of: include/core/SkColorTable.h#L19-L55 (chrome/m156)
#[doc(alias = "SkColorTable")]
#[derive(Clone, Debug)]
pub struct ColorTable {
    tables: Arc<Tables>,
}

/// The identity map, `table[i] = i`, used for missing channels in [`ColorTable::make_argb`].
fn identity_table() -> [u8; 256] {
    let mut table = [0_u8; 256];
    for (i, entry) in table.iter_mut().enumerate() {
        *entry = u8::try_from(i).expect("i < 256");
    }
    table
}

impl ColorTable {
    /// The table that maps each channel of a color through `table` (`SkColorTable::Make(table)`).
    // Port of: include/core/SkColorTable.h#L29-L33 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(table: &[u8; 256]) -> ColorTable {
        ColorTable {
            tables: Arc::new(Tables([*table; 4])),
        }
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
        Some(ColorTable {
            tables: Arc::new(Tables([
                pick(table_a),
                pick(table_r),
                pick(table_g),
                pick(table_b),
            ])),
        })
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
}
