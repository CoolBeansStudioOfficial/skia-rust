// Copyright © 2007,2008,2009,2010  Red Hat, Inc.
// Copyright © 2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-open-file.hh, src/hb-face.cc, src/hb-face-builder.cc (harfbuzz 9cb1fee5)

//! The source face (`hb_face_create` over a blob) and the destination face builder
//! (`hb_face_builder_create`, `hb_face_reference_blob`).

use std::collections::HashMap;

use crate::bytes::{bit_storage, bsearch, ceil_to_4, tag, u16_at, u32_at};

const TAG_HEAD: u32 = tag(b"head");
const TAG_CFF: u32 = tag(b"CFF ");
const TAG_CFF2: u32 = tag(b"CFF2");

/// Port of `OT::OpenTypeFontFile::get_face_count()` plus the sanitizer's structural checks
/// (hb-open-file.hh#L396-L411, L441-L452). Returns the byte offset of each face's table
/// directory; empty for an unrecognized or truncated file.
pub(crate) fn face_offsets(data: &[u8]) -> Vec<usize> {
    if data.len() < 4 {
        return Vec::new();
    }
    let sfnt_tag = u32_at(data, 0);
    let directory_ok = |off: usize| -> bool {
        // `OpenTypeOffsetTable::sanitize`: the header and the table records are in the blob.
        if off + 12 > data.len() {
            return false;
        }
        let n = usize::from(u16_at(data, off + 4));
        off + 12 + 16 * n <= data.len()
    };
    match sfnt_tag {
        0x0001_0000 | 0x4F54_544F /* OTTO */ | 0x7472_7565 /* true */ | 0x7479_7031 /* typ1 */ => {
            if directory_ok(0) { vec![0] } else { Vec::new() }
        }
        0x7474_6366 /* ttcf */ => {
            // `TTCHeader`: version 1 and 2 share the layout of `TTCHeaderVersion1`.
            let major = u16_at(data, 4);
            if (major != 1 && major != 2) || data.len() < 12 {
                return Vec::new();
            }
            let count = u32_at(data, 8) as usize;
            if 12usize.saturating_add(count.saturating_mul(4)) > data.len() {
                return Vec::new();
            }
            let mut offs = Vec::with_capacity(count);
            for i in 0..count {
                let off = u32_at(data, 12 + 4 * i) as usize;
                if !directory_ok(off) {
                    return Vec::new();
                }
                offs.push(off);
            }
            offs
        }
        _ => Vec::new(),
    }
}

/// One face of a font file: `hb_face_t` made by `hb_face_create (blob, index)`.
#[derive(Debug)]
pub(crate) struct Face<'a> {
    data: &'a [u8],
    /// `(tag, offset, length)` in directory order.
    records: Vec<(u32, u32, u32)>,
}

impl<'a> Face<'a> {
    /// Port of `hb_face_create` for a face index that `hb_face_count` accepted.
    pub(crate) fn create(data: &'a [u8], index: u32) -> Option<Face<'a>> {
        let offs = face_offsets(data);
        // `hb_face_create` keeps the low 16 bits of the index.
        let off = *offs.get((index & 0xFFFF) as usize)?;
        let n = usize::from(u16_at(data, off + 4));
        let records = (0..n)
            .map(|i| {
                let r = off + 12 + 16 * i;
                (u32_at(data, r), u32_at(data, r + 8), u32_at(data, r + 12))
            })
            .collect();
        Some(Face { data, records })
    }

    /// `hb_face_get_table_tags`: the tags in directory order.
    pub(crate) fn table_tags(&self) -> impl Iterator<Item = u32> + '_ {
        self.records.iter().map(|r| r.0)
    }

    /// Port of `OpenTypeOffsetTable::find_table_index` (hb-open-file.hh#L100-L111): a linear
    /// search under 16 tables, a binary search otherwise.
    fn find_table_index(&self, t: u32) -> Option<usize> {
        if self.records.len() < 16 {
            return self.records.iter().position(|r| r.0 == t);
        }
        bsearch(self.records.len(), |i| t.cmp(&self.records[i].0))
    }

    /// Port of `_hb_face_for_data_reference_table` (hb-face.cc#L185-L200): the table as a
    /// sub-blob, empty when the table is missing or starts outside the data.
    pub(crate) fn table(&self, t: u32) -> &'a [u8] {
        let Some(i) = self.find_table_index(t) else {
            return &[];
        };
        let (_, offset, length) = self.records[i];
        let (offset, length) = (offset as usize, length as usize);
        if length == 0 || offset >= self.data.len() {
            return &[];
        }
        let length = length.min(self.data.len() - offset);
        &self.data[offset..offset + length]
    }

    /// `hb_face_get_upem`: `head.unitsPerEm` in 16..=16384, else 1000.
    pub(crate) fn upem(&self) -> u32 {
        let head = self.table(TAG_HEAD);
        if !head_ok(head) {
            return 1000;
        }
        let upem = u32::from(u16_at(head, 18));
        if (16..=16384).contains(&upem) {
            upem
        } else {
            1000
        }
    }

    /// `hb_face_t::get_num_glyphs`: `maxp.numGlyphs`, zero when `maxp` does not sanitize.
    pub(crate) fn num_glyphs(&self) -> u32 {
        let maxp = self.table(tag(b"maxp"));
        if !maxp_ok(maxp) {
            return 0;
        }
        u32::from(u16_at(maxp, 4))
    }
}

/// The checks of `OT::head::sanitize` (hb-ot-head-table.hh#L156-L164).
pub(crate) fn head_ok(head: &[u8]) -> bool {
    head.len() >= 54 && u16_at(head, 0) == 1 && u32_at(head, 12) == 0x5F0F_3CF5
}

/// The checks of `OT::maxp::sanitize` (hb-ot-maxp-table.hh#L72-L85).
pub(crate) fn maxp_ok(maxp: &[u8]) -> bool {
    if maxp.len() < 6 {
        return false;
    }
    let major = u16_at(maxp, 0);
    if major == 1 {
        return maxp.len() >= 32;
    }
    major == 0 && u16_at(maxp, 2) == 0x5000
}

/// The destination face: `hb_face_builder_create` and `hb_face_builder_add_table`.
#[derive(Default, Debug)]
pub(crate) struct FaceBuilder {
    tables: HashMap<u32, Vec<u8>>,
}

impl FaceBuilder {
    /// Port of `hb_face_builder_add_table` (hb-face-builder.cc#L219-L240): a second table with
    /// the same tag replaces the first.
    pub(crate) fn add_table(&mut self, t: u32, contents: Vec<u8>) {
        self.tables.insert(t, contents);
    }

    /// Port of `_hb_face_builder_data_reference_blob` and `OpenTypeOffsetTable::serialize`
    /// (hb-face-builder.cc#L103-L155, hb-open-file.hh#L121-L196).
    pub(crate) fn build(&self) -> Vec<u8> {
        let is_cff = self.tables.contains_key(&TAG_CFF) || self.tables.contains_key(&TAG_CFF2);
        let sfnt_tag: u32 = if is_cff { 0x4F54_544F } else { 0x0001_0000 };

        // Sort the tags so that the produced face is deterministic: by blob size first
        // (smallest to largest), then by table tag (`compare_entries`).
        let mut entries: Vec<(u32, &Vec<u8>)> = self.tables.iter().map(|(k, v)| (*k, v)).collect();
        entries.sort_by(|a, b| a.1.len().cmp(&b.1.len()).then(a.0.cmp(&b.0)));

        let n = entries.len();
        let dir_len = 12 + 16 * n;
        let mut out = vec![0u8; dir_len];
        out[0..4].copy_from_slice(&sfnt_tag.to_be_bytes());
        // `BinSearchHeader::operator=`
        let entry_selector = bit_storage(n as u32).max(1) - 1;
        let search_range = 16 * (1u32 << entry_selector);
        let range_shift = (n as u32 * 16).saturating_sub(search_range);
        out[4..6].copy_from_slice(&(n as u16).to_be_bytes());
        out[6..8].copy_from_slice(&(search_range as u16).to_be_bytes());
        out[8..10].copy_from_slice(&(entry_selector as u16).to_be_bytes());
        out[10..12].copy_from_slice(&(range_shift as u16).to_be_bytes());

        // (tag, checksum, offset, length) per table, in output order.
        let mut records: Vec<(u32, u32, u32, u32)> = Vec::with_capacity(n);
        let mut checksum_adjustment_pos = None;
        for (t, blob) in &entries {
            let start = out.len();
            out.extend_from_slice(blob);
            out.resize(start + ceil_to_4(blob.len()), 0);
            let end = out.len();
            if *t == TAG_HEAD && end - start >= 54 {
                // `head::checkSumAdjustment` is zero while the table checksum is computed.
                out[start + 8..start + 12].fill(0);
                checksum_adjustment_pos = Some(start + 8);
            }
            records.push((
                *t,
                table_checksum(&out[start..end]),
                start as u32,
                blob.len() as u32,
            ));
        }

        // `tables.qsort ()`
        records.sort_by_key(|r| r.0);
        for (i, r) in records.iter().enumerate() {
            let p = 12 + 16 * i;
            out[p..p + 4].copy_from_slice(&r.0.to_be_bytes());
            out[p + 4..p + 8].copy_from_slice(&r.1.to_be_bytes());
            out[p + 8..p + 12].copy_from_slice(&r.2.to_be_bytes());
            out[p + 12..p + 16].copy_from_slice(&r.3.to_be_bytes());
        }

        if let Some(pos) = checksum_adjustment_pos {
            let mut checksum = table_checksum(&out[..dir_len]);
            for r in &records {
                checksum = checksum.wrapping_add(r.1);
            }
            let adjustment = 0xB1B0_AFBAu32.wrapping_sub(checksum);
            out[pos..pos + 4].copy_from_slice(&adjustment.to_be_bytes());
        }
        out
    }
}

/// Port of `CheckSum::CalcTableChecksum` (hb-open-type.hh#L343-L353) over 4-byte aligned data.
fn table_checksum(data: &[u8]) -> u32 {
    debug_assert_eq!(data.len() % 4, 0);
    data.as_chunks::<4>()
        .0
        .iter()
        .fold(0u32, |sum, c| sum.wrapping_add(u32::from_be_bytes(*c)))
}
