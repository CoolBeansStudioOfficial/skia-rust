// Copyright © 2014  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-cmap-table.hh (harfbuzz 9cb1fee5)

//! `cmap` reading (`collect_mapping`, `collect_unicodes`) and subsetting.
//!
//! Skia's input has glyphs but no unicodes, so the plan's `unicodes` hold only code points that
//! the best subtable maps to a requested glyph, and format 14 (variation selector) subtables
//! never produce output: `CmapSubtableFormat14::serialize` keeps a record only when
//! `plan->unicodes` has its selector. The code that serializes format 14 is therefore not
//! ported, and a plan whose `unicodes` hold a selector of a format 14 record makes the subset
//! fail rather than produce different bytes.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::bytes::{bit_storage, bsearch, u8_at, u16_at, u24_at, u32_at};
use crate::plan::Plan;
use crate::serialize::{ERROR_INT_OVERFLOW, ObjIdx, Serializer, Whence};
use crate::{Res, SubsetError};

const HB_UNICODE_MAX: u32 = 0x10_FFFF;
const HB_MAP_VALUE_INVALID: u32 = 0xFFFF_FFFF;

/// Port of `CmapSubtableLongGroup` (hb-ot-cmap-table.hh#L664-L690), read-only.
fn group(d: &[u8], sub: usize, i: usize) -> (u32, u32, u32) {
    let g = sub + 16 + 12 * i;
    (u32_at(d, g), u32_at(d, g + 4), u32_at(d, g + 8))
}

/// The `cmap` table of the source face.
#[derive(Debug)]
pub(crate) struct Cmap<'a> {
    pub data: &'a [u8],
}

/// One `EncodingRecord` (hb-ot-cmap-table.hh#L1423-L1480).
#[derive(Clone, Copy, Debug)]
pub(crate) struct EncodingRecord {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub subtable: u32,
}

impl<'a> Cmap<'a> {
    /// Port of `cmap::sanitize` (hb-ot-cmap-table.hh#L2213-L2221) with the subtable sanitizers.
    /// `None` when the table does not sanitize.
    pub(crate) fn new(data: &'a [u8]) -> Option<Self> {
        let cmap = Cmap { data };
        if data.len() < 4 || u16_at(data, 0) != 0 {
            return None;
        }
        let n = usize::from(u16_at(data, 2));
        if 4 + 8 * n > data.len() {
            return None;
        }
        for r in cmap.records() {
            if !cmap.sanitize_subtable(r.subtable as usize) {
                return None;
            }
        }
        Some(cmap)
    }

    fn sanitize_subtable(&self, off: usize) -> bool {
        let d = self.data;
        if off.checked_add(2).is_none_or(|e| e > d.len()) {
            return false;
        }
        let fits = |end: usize| off.checked_add(end).is_some_and(|e| e <= d.len());
        match u16_at(d, off) {
            0 => fits(6 + 256),
            4 => fits(14) && fits(14 + 2 + 4 * usize::from(u16_at(d, off + 6))),
            6 => fits(10) && fits(10 + 2 * usize::from(u16_at(d, off + 8))),
            10 => fits(20) && fits(20 + 2 * (u32_at(d, off + 16) as usize)),
            12 | 13 => fits(16) && fits(16 + 12 * (u32_at(d, off + 12) as usize)),
            14 => {
                if !fits(10) {
                    return false;
                }
                let count = u32_at(d, off + 6) as usize;
                if !fits(10 + 11 * count) {
                    return false;
                }
                for i in 0..count {
                    let r = off + 10 + 11 * i;
                    let default_uvs = u32_at(d, r + 3) as usize;
                    let non_default_uvs = u32_at(d, r + 7) as usize;
                    if default_uvs != 0
                        && !(off + default_uvs + 4 <= d.len()
                            && off + default_uvs + 4 + 4 * (u32_at(d, off + default_uvs) as usize)
                                <= d.len())
                    {
                        return false;
                    }
                    if non_default_uvs != 0
                        && !(off + non_default_uvs + 4 <= d.len()
                            && off
                                + non_default_uvs
                                + 4
                                + 5 * (u32_at(d, off + non_default_uvs) as usize)
                                <= d.len())
                    {
                        return false;
                    }
                }
                true
            }
            _ => true,
        }
    }

    pub(crate) fn records(&self) -> Vec<EncodingRecord> {
        let n = usize::from(u16_at(self.data, 2));
        (0..n)
            .map(|i| {
                let r = 4 + 8 * i;
                EncodingRecord {
                    platform_id: u16_at(self.data, r),
                    encoding_id: u16_at(self.data, r + 2),
                    subtable: u32_at(self.data, r + 4),
                }
            })
            .collect()
    }

    fn format_of(&self, r: EncodingRecord) -> u16 {
        u16_at(self.data, r.subtable as usize)
    }

    /// Port of `cmap::find_subtable` (hb-ot-cmap-table.hh#L2166-L2178): a binary search of the
    /// (sorted) encoding records. `0xFFFF` as the encoding matches any encoding.
    fn find_subtable(&self, platform_id: u16, encoding_id: u16) -> Option<usize> {
        let records = self.records();
        let found = bsearch(records.len(), |i| {
            // `EncodingRecord::cmp (key)`: the key compared with the element.
            let r = &records[i];
            let c = platform_id.cmp(&r.platform_id);
            if c == Ordering::Equal && encoding_id != 0xFFFF {
                encoding_id.cmp(&r.encoding_id)
            } else {
                c
            }
        })?;
        let subtable = records[found].subtable;
        if subtable == 0 {
            None
        } else {
            Some(subtable as usize)
        }
    }

    /// Port of `cmap::find_best_subtable` (hb-ot-cmap-table.hh#L2084-L2124), returning the
    /// subtable offset; `None` is the `Null` subtable.
    fn find_best_subtable(&self) -> Option<usize> {
        // Symbol, then 32-bit, 16-bit, MacRoman and any other Mac subtable.
        const ORDER: [(u16, u16); 11] = [
            (3, 0),
            (3, 10),
            (0, 6),
            (0, 4),
            (3, 1),
            (0, 3),
            (0, 2),
            (0, 1),
            (0, 0),
            (1, 0),
            (1, 0xFFFF),
        ];
        for &(p, e) in &ORDER {
            if let Some(off) = self.find_subtable(p, e) {
                return Some(off);
            }
        }
        None
    }

    /// Port of `cmap::accelerator_t::collect_mapping` (hb-ot-cmap-table.hh#L2051-L2054): the
    /// unicodes and the code point to glyph map of the best subtable.
    pub(crate) fn collect_mapping(&self) -> (BTreeSet<u32>, BTreeMap<u32, u32>) {
        let mut unicodes = BTreeSet::new();
        let mut mapping = BTreeMap::new();
        if let Some(off) = self.find_best_subtable() {
            self.subtable_collect_mapping(off, &mut unicodes, &mut mapping);
        }
        (unicodes, mapping)
    }

    /// The `CmapSubtable::collect_mapping` dispatch (hb-ot-cmap-table.hh#L1566-L1590) with
    /// `num_glyphs = UINT_MAX`.
    fn subtable_collect_mapping(
        &self,
        off: usize,
        unicodes: &mut BTreeSet<u32>,
        mapping: &mut BTreeMap<u32, u32>,
    ) {
        let d = self.data;
        let subtable_data_size = d.len() - off;
        match u16_at(d, off) {
            0 => {
                for i in 0..256usize {
                    let g = u32::from(u8_at(d, off + 6 + i));
                    if g != 0 {
                        unicodes.insert(i as u32);
                        mapping.insert(i as u32, g);
                    }
                }
            }
            4 => {
                let f = Format4::new(d, off, subtable_data_size);
                f.collect_mapping(unicodes, mapping);
            }
            6 | 10 => {
                // `CmapSubtableTrimmed::collect_mapping`
                let (start, count, array) = if u16_at(d, off) == 6 {
                    (
                        u32::from(u16_at(d, off + 6)),
                        usize::from(u16_at(d, off + 8)),
                        off + 10,
                    )
                } else {
                    (u32_at(d, off + 12), u32_at(d, off + 16) as usize, off + 20)
                };
                for i in 0..count {
                    let g = u32::from(u16_at(d, array + 2 * i));
                    if g != 0 {
                        let u = start.wrapping_add(i as u32);
                        unicodes.insert(u);
                        mapping.insert(u, g);
                    }
                }
            }
            f @ (12 | 13) => {
                // `CmapSubtableLongSegmented::collect_mapping`
                let num_glyphs = u32::MAX;
                let mut last_end = 0u32;
                let count = u32_at(d, off + 12) as usize;
                for i in 0..count {
                    let (s, e, g) = group(d, off, i);
                    let mut start = s;
                    let mut end = e.min(HB_UNICODE_MAX);
                    if start > end || start < last_end {
                        // Range is not in order and is invalid, skip it.
                        continue;
                    }
                    last_end = end;
                    let mut gid = g;
                    if gid == 0 {
                        if f == 13 {
                            continue;
                        }
                        start = start.wrapping_add(1);
                        gid = gid.wrapping_add(1);
                    }
                    if gid >= num_glyphs {
                        continue;
                    }
                    if gid.wrapping_add(end).wrapping_sub(start) >= num_glyphs {
                        end = start.wrapping_add(num_glyphs).wrapping_sub(gid);
                    }
                    add_range(unicodes, start, end);
                    let increment = u32::from(f == 12);
                    let mut cp = start;
                    while cp <= end {
                        mapping.insert(cp, gid);
                        gid = gid.wrapping_add(increment);
                        if cp == u32::MAX {
                            break;
                        }
                        cp += 1;
                    }
                }
            }
            _ => {}
        }
    }

    /// Port of `CmapSubtable::collect_unicodes` (hb-ot-cmap-table.hh#L1540-L1564) with
    /// `num_glyphs = UINT_MAX` and `subtable_data_size` as `SubtableUnicodesCache::set_for`
    /// passes it.
    fn subtable_collect_unicodes(&self, off: usize) -> BTreeSet<u32> {
        let d = self.data;
        let mut out = BTreeSet::new();
        let subtable_data_size = d.len() - off;
        match u16_at(d, off) {
            0 => {
                for i in 0..256usize {
                    if u8_at(d, off + 6 + i) != 0 {
                        out.insert(i as u32);
                    }
                }
            }
            4 => Format4::new(d, off, subtable_data_size).collect_unicodes(&mut out),
            6 | 10 => {
                let (start, count, array) = if u16_at(d, off) == 6 {
                    (
                        u32::from(u16_at(d, off + 6)),
                        usize::from(u16_at(d, off + 8)),
                        off + 10,
                    )
                } else {
                    (u32_at(d, off + 12), u32_at(d, off + 16) as usize, off + 20)
                };
                for i in 0..count {
                    if u16_at(d, array + 2 * i) != 0 {
                        out.insert(start.wrapping_add(i as u32));
                    }
                }
            }
            f @ (12 | 13) => {
                let num_glyphs = u32::MAX;
                let count = u32_at(d, off + 12) as usize;
                for i in 0..count {
                    let (s, e, g) = group(d, off, i);
                    let mut start = s;
                    let mut end = e.min(HB_UNICODE_MAX);
                    let mut gid = g;
                    if gid == 0 {
                        // `T::group_get_glyph (this->groups[i], end)`
                        let got = if f == 12 {
                            if s <= e {
                                g.wrapping_add(end.wrapping_sub(s))
                            } else {
                                0
                            }
                        } else {
                            g
                        };
                        if got == 0 {
                            continue;
                        }
                        start = start.wrapping_add(1);
                        gid = gid.wrapping_add(1);
                    }
                    if gid >= num_glyphs {
                        continue;
                    }
                    if gid.wrapping_add(end).wrapping_sub(start) >= num_glyphs {
                        end = start.wrapping_add(num_glyphs).wrapping_sub(gid);
                    }
                    add_range(&mut out, start, end.min(0x10_FFFF));
                }
            }
            _ => {}
        }
        out
    }

    /// Port of `CmapSubtable::get_language` (hb-ot-cmap-table.hh#L1592-L1606).
    fn language(&self, off: usize) -> u32 {
        let d = self.data;
        match u16_at(d, off) {
            0 | 4 | 6 => u32::from(u16_at(d, off + 4)),
            10 | 12 | 13 => u32_at(d, off + 8),
            _ => 0,
        }
    }

    /// Port of `cmap::filter_encoding_records_for_subset` (hb-ot-cmap-table.hh#L2231-L2243).
    fn filter_encoding_records_for_subset(&self, r: EncodingRecord) -> bool {
        matches!((r.platform_id, r.encoding_id), (0, 3 | 4) | (3, 1 | 10))
            || self.format_of(r) == 14
    }

    /// Port of `cmap::closure_glyphs` (hb-ot-cmap-table.hh#L1985-L1995): format 14 subtables add
    /// the glyphs of the non-default mappings of the retained selectors. `plan.unicodes` holds
    /// no selector (see the module comment) unless the cmap maps one, in which case this adds the
    /// glyphs as `HarfBuzz` does.
    pub(crate) fn closure_glyphs(&self, unicodes: &BTreeSet<u32>, glyphset: &mut BTreeSet<u32>) {
        let d = self.data;
        for r in self.records() {
            let off = r.subtable as usize;
            if u16_at(d, off) != 14 {
                continue;
            }
            let count = u32_at(d, off + 6) as usize;
            for i in 0..count {
                let rec = off + 10 + 11 * i;
                let non_default = u32_at(d, rec + 7) as usize;
                if non_default == 0 || !unicodes.contains(&u24_at(d, rec)) {
                    continue;
                }
                let base = off + non_default;
                let n = u32_at(d, base) as usize;
                for k in 0..n {
                    let m = base + 4 + 5 * k;
                    if unicodes.contains(&u24_at(d, m)) {
                        glyphset.insert(u32::from(u16_at(d, m + 3)));
                    }
                }
            }
        }
    }
}

/// `hb_set_t::add_range`.
fn add_range(set: &mut BTreeSet<u32>, a: u32, b: u32) {
    if a > b {
        return;
    }
    for v in a..=b {
        set.insert(v);
    }
}

/// `hb_set_t::del_range`.
fn del_range(set: &mut BTreeSet<u32>, a: u32, b: u32) {
    if a > b {
        return;
    }
    let doomed: Vec<u32> = set.range(a..=b).copied().collect();
    for v in doomed {
        set.remove(&v);
    }
}

/// Port of `CmapSubtableFormat4::accelerator_t` (hb-ot-cmap-table.hh#L322-L437), as used for
/// `collect_unicodes` and `collect_mapping`.
struct Format4<'a> {
    d: &'a [u8],
    seg_count: usize,
    end_count: usize,
    start_count: usize,
    id_delta: usize,
    id_range_offset: usize,
    glyph_id_array: usize,
    glyph_id_array_length: usize,
}

impl<'a> Format4<'a> {
    fn new(d: &'a [u8], off: usize, subtable_data_size: usize) -> Self {
        let seg_count = usize::from(u16_at(d, off + 6)) / 2;
        let end_count = off + 14;
        let start_count = end_count + 2 * (seg_count + 1);
        let id_delta = start_count + 2 * seg_count;
        let id_range_offset = id_delta + 2 * seg_count;
        let glyph_id_array = id_range_offset + 2 * seg_count;
        let values_offset = 16 + 8 * seg_count;
        let glyph_id_array_length = if subtable_data_size > values_offset {
            (subtable_data_size - values_offset) / 2
        } else {
            0
        };
        Format4 {
            d,
            seg_count,
            end_count,
            start_count,
            id_delta,
            id_range_offset,
            glyph_id_array,
            glyph_id_array_length,
        }
    }

    fn end(&self, i: usize) -> u32 {
        u32::from(u16_at(self.d, self.end_count + 2 * i))
    }

    fn start(&self, i: usize) -> u32 {
        u32::from(u16_at(self.d, self.start_count + 2 * i))
    }

    fn delta(&self, i: usize) -> u32 {
        u32::from(u16_at(self.d, self.id_delta + 2 * i))
    }

    fn range_offset(&self, i: usize) -> usize {
        usize::from(u16_at(self.d, self.id_range_offset + 2 * i))
    }

    fn glyph_array(&self, index: usize) -> u32 {
        u32::from(u16_at(self.d, self.glyph_id_array + 2 * index))
    }

    /// The number of segments to visit: the sentinel segment is skipped.
    fn count(&self) -> usize {
        let mut count = self.seg_count;
        if count != 0 && self.start(count - 1) == 0xFFFF {
            count -= 1;
        }
        count
    }

    /// Port of `accelerator_t::collect_unicodes` (hb-ot-cmap-table.hh#L359-L398).
    fn collect_unicodes(&self, out: &mut BTreeSet<u32>) {
        for i in 0..self.count() {
            let start = self.start(i);
            let end = self.end(i);
            let range_offset = self.range_offset(i);
            add_range(out, start, end);
            if range_offset == 0 {
                for cp in start..=end {
                    let gid = (cp.wrapping_add(self.delta(i))) & 0xFFFF;
                    if gid == 0 {
                        out.remove(&cp);
                    }
                }
            } else {
                for cp in start..=end {
                    let index = (range_offset / 2 + (cp - self.start(i)) as usize + i)
                        .wrapping_sub(self.seg_count);
                    if index >= self.glyph_id_array_length {
                        del_range(out, cp, end);
                        break;
                    }
                    if self.glyph_array(index) == 0 {
                        out.remove(&cp);
                    }
                }
            }
        }
    }

    /// Port of `accelerator_t::collect_mapping` (hb-ot-cmap-table.hh#L400-L437).
    fn collect_mapping(&self, unicodes: &mut BTreeSet<u32>, mapping: &mut BTreeMap<u32, u32>) {
        for i in 0..self.count() {
            let start = self.start(i);
            let end = self.end(i);
            let range_offset = self.range_offset(i);
            if range_offset == 0 {
                for cp in start..=end {
                    let gid = (cp.wrapping_add(self.delta(i))) & 0xFFFF;
                    if gid == 0 {
                        continue;
                    }
                    unicodes.insert(cp);
                    mapping.insert(cp, gid);
                }
            } else {
                for cp in start..=end {
                    let index = (range_offset / 2 + (cp - self.start(i)) as usize + i)
                        .wrapping_sub(self.seg_count);
                    if index >= self.glyph_id_array_length {
                        break;
                    }
                    let gid = self.glyph_array(index);
                    if gid == 0 {
                        continue;
                    }
                    unicodes.insert(cp);
                    mapping.insert(cp, gid);
                }
            }
        }
    }
}

/// Port of `CmapSubtableFormat4::to_ranges` (hb-ot-cmap-table.hh#L88-L184).
fn to_ranges(it: &[(u32, u32)], writer: &mut impl FnMut(u32, u32, i32)) {
    #[derive(PartialEq)]
    enum Mode {
        FirstSubRange,
        FollowingSubRange,
    }
    let mut pos = 0usize;
    let mut end_cp = 0u32;
    while pos < it.len() {
        // Start a new range
        let mut start_cp = it[pos].0;
        let mut prev_run_start_cp = start_cp;
        let mut run_start_cp = start_cp;
        end_cp = start_cp;
        let mut last_gid = it[pos].1;
        let mut run_length: i32 = 1;
        let mut prev_delta: i32 = 0;

        let mut delta: i32 = last_gid.wrapping_sub(start_cp) as i32;
        let mut mode = Mode::FirstSubRange;
        pos += 1;

        while pos < it.len() {
            // Process range
            let next_cp = it[pos].0;
            let next_gid = it[pos].1;
            if next_cp != end_cp.wrapping_add(1) {
                // Current range is over, stop processing.
                break;
            }
            if next_gid == last_gid.wrapping_add(1) {
                // The current run continues.
                end_cp = next_cp;
                run_length += 1;
                last_gid = next_gid;
                pos += 1;
                continue;
            }
            // A new run is starting, decide if we want to commit the current run.
            let split_cost: i32 = if mode == Mode::FirstSubRange { 8 } else { 16 };
            let run_cost = run_length * 2;
            if run_cost >= split_cost {
                commit_current_range(
                    start_cp,
                    prev_run_start_cp,
                    run_start_cp,
                    end_cp,
                    delta,
                    prev_delta,
                    split_cost,
                    writer,
                );
                start_cp = next_cp;
            }
            // Start the new run
            mode = Mode::FollowingSubRange;
            prev_run_start_cp = run_start_cp;
            run_start_cp = next_cp;
            end_cp = next_cp;
            prev_delta = delta;
            delta = next_gid.wrapping_sub(run_start_cp) as i32;
            run_length = 1;
            last_gid = next_gid;
            pos += 1;
        }
        // Finalize range
        commit_current_range(
            start_cp,
            prev_run_start_cp,
            run_start_cp,
            end_cp,
            delta,
            prev_delta,
            8,
            writer,
        );
    }
    if end_cp != 0xFFFF {
        writer(0xFFFF, 0xFFFF, 1);
    }
}

/// Port of `CmapSubtableFormat4::commit_current_range` (hb-ot-cmap-table.hh#L186-L225).
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn commit_current_range(
    start: u32,
    prev_run_start: u32,
    run_start: u32,
    end: u32,
    run_delta: i32,
    previous_run_delta: i32,
    split_cost: i32,
    writer: &mut impl FnMut(u32, u32, i32),
) {
    let mut should_split = false;
    if start < run_start && run_start < end {
        let run_cost = (end - run_start + 1).wrapping_mul(2) as i32;
        if run_cost >= split_cost {
            should_split = true;
        }
    }
    if should_split {
        if start == prev_run_start {
            writer(start, run_start - 1, previous_run_delta);
        } else {
            writer(start, run_start - 1, 0);
        }
        writer(run_start, end, run_delta);
        return;
    }
    if start == run_start {
        // Range is only a run
        writer(start, end, run_delta);
        return;
    }
    // Write only a single non-run range.
    writer(start, end, 0);
}

/// Port of `CmapSubtableFormat4::serialize` (hb-ot-cmap-table.hh#L261-L318).
fn serialize_format4(s: &mut Serializer, it: &[(u32, u32)]) {
    let cp_to_gid: Vec<(u32, u32)> = it.iter().copied().filter(|p| p.0 <= 0xFFFF).collect();
    if cp_to_gid.is_empty() {
        return;
    }
    let table_initpos = s.length();
    let this = s.allocate(14);
    s.set_u16(this, 4);

    // serialize endCode[], startCode[], idDelta[]
    let mut segcount = 0usize;
    to_ranges(&cp_to_gid, &mut |_, _, _| segcount += 1);

    let end_code = s.allocate(2 * segcount);
    s.allocate(2); // padding
    let start_code = s.allocate(2 * segcount);
    let id_delta = s.allocate(2 * segcount);
    let mut ranges: Vec<(u32, u32, i32)> = Vec::with_capacity(segcount);
    to_ranges(&cp_to_gid, &mut |start, end, delta| {
        ranges.push((start, end, delta));
    });
    for (index, &(start, end, delta)) in ranges.iter().enumerate() {
        s.set_u16(start_code + 2 * index, start as u16);
        s.set_u16(end_code + 2 * index, end as u16);
        s.set_u16(id_delta + 2 * index, delta as u16);
    }

    // serialize_rangeoffset_glyid
    let map: HashMap<u32, u32> = cp_to_gid.iter().copied().collect();
    let id_range_offset = s.allocate(2 * segcount);
    for i in 0..segcount {
        let delta = u16_at(s.bytes(), id_delta + 2 * i);
        if delta != 0 {
            continue;
        }
        let here = s.length();
        let v = 2 * ((here - id_range_offset) / 2 - i);
        s.set_u16(id_range_offset + 2 * i, v as u16);
        let start = u32::from(u16_at(s.bytes(), start_code + 2 * i));
        let end = u32::from(u16_at(s.bytes(), end_code + 2 * i));
        for cp in start..=end {
            let gid = map.get(&cp).copied().unwrap_or(HB_MAP_VALUE_INVALID);
            s.embed_u16(gid as u16);
        }
    }

    let length = s.length() - table_initpos;
    s.set_u16(this + 2, length as u16);
    if length > 0xFFFF {
        s.err(ERROR_INT_OVERFLOW);
        return;
    }
    s.set_u16(this + 6, (segcount * 2) as u16);
    let entry_selector = bit_storage(segcount as u32).max(1) - 1;
    let search_range = 2 * (1u32 << entry_selector);
    s.set_u16(
        this + 12,
        (segcount as u32 * 2).saturating_sub(search_range) as u16,
    );
    s.set_u16(this + 8, search_range as u16);
    s.set_u16(this + 10, entry_selector as u16);
}

/// Port of `CmapSubtableFormat12::serialize` (hb-ot-cmap-table.hh#L957-L1011).
fn serialize_format12(s: &mut Serializer, it: &[(u32, u32)]) {
    if it.is_empty() {
        return;
    }
    let table_initpos = s.length();
    let this = s.allocate(16);

    let mut start_char_code = u32::MAX;
    let mut end_char_code = u32::MAX;
    let mut glyph_id = 0u32;
    for &(cp, gid) in it {
        if start_char_code == u32::MAX {
            start_char_code = cp;
            end_char_code = cp;
            glyph_id = gid;
        } else if !(cp.wrapping_sub(1) == end_char_code
            && gid == glyph_id.wrapping_add(cp.wrapping_sub(start_char_code)))
        {
            s.embed_u32(start_char_code);
            s.embed_u32(end_char_code);
            s.embed_u32(glyph_id);
            start_char_code = cp;
            end_char_code = cp;
            glyph_id = gid;
        } else {
            end_char_code = cp;
        }
    }
    s.embed_u32(start_char_code);
    s.embed_u32(end_char_code);
    s.embed_u32(glyph_id);

    s.set_u16(this, 12);
    s.set_u16(this + 2, 0);
    let length = s.length() - table_initpos;
    s.set_u32(this + 4, length as u32);
    s.set_u32(this + 12, ((length - 16) / 12) as u32);
}

/// Port of `SubtableUnicodesCache` (hb-ot-cmap-table.hh#L1650-L1745), keyed by record index.
struct UnicodesCache {
    sets: HashMap<usize, BTreeSet<u32>>,
}

impl UnicodesCache {
    fn set_for(&mut self, cmap: &Cmap<'_>, index: usize, r: EncodingRecord) -> BTreeSet<u32> {
        self.sets
            .entry(index)
            .or_insert_with(|| cmap.subtable_collect_unicodes(r.subtable as usize))
            .clone()
    }
}

/// Port of `cmap::subset` (hb-ot-cmap-table.hh#L2018-L2080) and `cmap::serialize`
/// (hb-ot-cmap-table.hh#L1787-L1867). Returns an error on failure and `Ok(false)` when the table
/// is dropped.
pub(crate) fn subset(plan: &mut Plan<'_>, cmap: &Cmap<'_>) -> Res<bool> {
    let all_records = cmap.records();
    let records: Vec<(usize, EncodingRecord)> = all_records
        .iter()
        .copied()
        .enumerate()
        .filter(|(_, r)| cmap.filter_encoding_records_for_subset(*r))
        .collect();
    if records.is_empty() {
        return Ok(false);
    }

    let mut unicode_bmp = false;
    let mut unicode_ucs4 = false;
    let mut ms_bmp = false;
    let mut ms_ucs4 = false;
    let mut has_format12 = false;
    for (_, r) in &records {
        if cmap.format_of(*r) == 12 {
            has_format12 = true;
        }
        match (r.platform_id, r.encoding_id) {
            (0, 3) => unicode_bmp = true,
            (0, 4) => unicode_ucs4 = true,
            (3, 1) => ms_bmp = true,
            (3, 10) => ms_ucs4 = true,
            _ => {}
        }
    }
    if !has_format12 && !unicode_bmp && !ms_bmp {
        return Ok(false);
    }
    if has_format12 && !unicode_ucs4 && !ms_ucs4 {
        return Ok(false);
    }

    let it: Vec<(u32, u32)> = plan
        .unicode_to_new_gid_list
        .iter()
        .copied()
        .filter(|p| p.1 != HB_MAP_VALUE_INVALID)
        .collect();

    let mut cache = UnicodesCache {
        sets: HashMap::new(),
    };
    let out = serialize(plan, cmap, &it, &records, &mut cache, false).ok_or(SubsetError::Failed)?;
    plan.add_table(crate::bytes::tag(b"cmap"), out);
    Ok(true)
}

/// Port of `cmap::serialize`. Retries without format 4 subtables when serializing one
/// overflowed. `None` is a failure of the whole subset.
fn serialize(
    plan: &Plan<'_>,
    cmap: &Cmap<'_>,
    it: &[(u32, u32)],
    records: &[(usize, EncodingRecord)],
    cache: &mut UnicodesCache,
    drop_format_4: bool,
) -> Option<Vec<u8>> {
    let mut s = Serializer::new();
    s.start_serialize();
    let this = s.allocate(4); // version = 0
    let mut format4objidx: ObjIdx = 0;
    let mut format12objidx: ObjIdx = 0;
    let mut format14objidx: ObjIdx = 0;
    let snap = s.snapshot();

    for (index, r) in records {
        if s.in_error() {
            return None;
        }
        let format = cmap.format_of(*r);
        if format != 4 && format != 12 && format != 14 {
            continue;
        }
        let unicodes_set = cache.set_for(cmap, *index, *r);

        if !drop_format_4 && format == 4 {
            let filtered: Vec<(u32, u32)> = it
                .iter()
                .copied()
                .filter(|p| unicodes_set.contains(&p.0))
                .collect();
            copy_encoding_record(&mut s, *r, &filtered, 4, plan, &mut format4objidx);
            if s.in_error() && s.only_overflow() {
                // cmap4 overflowed, reset and retry serialization without format 4 subtables.
                s.revert(snap);
                return serialize(plan, cmap, it, records, cache, true);
            }
        } else if format == 12 {
            if can_drop(cmap, *r, &unicodes_set, it, records, cache) {
                continue;
            }
            let filtered: Vec<(u32, u32)> = it
                .iter()
                .copied()
                .filter(|p| unicodes_set.contains(&p.0))
                .collect();
            copy_encoding_record(&mut s, *r, &filtered, 12, plan, &mut format12objidx);
        } else if format == 14 {
            copy_encoding_record(&mut s, *r, it, 14, plan, &mut format14objidx);
        }
    }
    let length = s.length();
    let available = length.saturating_sub(4);
    s.check_fits((available / 8) as u64, 16, ERROR_INT_OVERFLOW);
    s.set_u16(this + 2, (available / 8) as u16);

    // Fail if format 4 was dropped and there is no cmap12.
    if drop_format_4 && format12objidx == 0 {
        return None;
    }
    s.end_serialize()
}

/// Port of `EncodingRecord::copy` (hb-ot-cmap-table.hh#L1440-L1475).
fn copy_encoding_record(
    s: &mut Serializer,
    r: EncodingRecord,
    it: &[(u32, u32)],
    format: u16,
    plan: &Plan<'_>,
    objidx: &mut ObjIdx,
) {
    let snap = s.snapshot();
    // `c->embed (this)`: the record's 8 bytes with `subtable = 0`.
    let rec_pos = s.embed(&[
        (r.platform_id >> 8) as u8,
        r.platform_id as u8,
        (r.encoding_id >> 8) as u8,
        r.encoding_id as u8,
        0,
        0,
        0,
        0,
    ]);

    if *objidx == 0 {
        s.push();
        let origin_length = s.length();
        match format {
            4 => serialize_format4(s, it),
            12 => serialize_format12(s, it),
            14 => {
                // Only reachable when `plan.unicodes` has a variation selector (module comment).
                let _ = plan;
            }
            _ => {}
        }
        if s.length() > origin_length && !s.in_error() {
            *objidx = s.pop_pack(true);
        } else {
            s.pop_discard();
        }
    }
    if *objidx == 0 {
        s.revert(snap);
        return;
    }
    s.add_link(rec_pos + 4, 4, *objidx, Whence::Head, 0);
}

/// Port of `cmap::_can_drop` (hb-ot-cmap-table.hh#L1869-L1923).
fn can_drop(
    cmap: &Cmap<'_>,
    cmap12: EncodingRecord,
    cmap12_unicodes: &BTreeSet<u32>,
    subset_pairs: &[(u32, u32)],
    records: &[(usize, EncodingRecord)],
    cache: &mut UnicodesCache,
) -> bool {
    let subset_unicodes: Vec<u32> = subset_pairs.iter().map(|p| p.0).collect();
    if subset_unicodes
        .iter()
        .any(|cp| cmap12_unicodes.contains(cp) && *cp >= 0x10000)
    {
        return false;
    }

    let target_language = cmap.language(cmap12.subtable as usize);
    let (target_platform, target_encoding) = if cmap12.platform_id == 0 && cmap12.encoding_id == 4 {
        (0, 3)
    } else if cmap12.platform_id == 3 && cmap12.encoding_id == 10 {
        (3, 1)
    } else {
        return false;
    };

    for (index, r) in records {
        if r.platform_id != target_platform
            || r.encoding_id != target_encoding
            || cmap.language(r.subtable as usize) != target_language
        {
            continue;
        }
        let sibling_unicodes = cache.set_for(cmap, *index, *r);
        let a: Vec<u32> = subset_unicodes
            .iter()
            .copied()
            .filter(|cp| cmap12_unicodes.contains(cp))
            .collect();
        let b: Vec<u32> = subset_unicodes
            .iter()
            .copied()
            .filter(|cp| sibling_unicodes.contains(cp))
            .collect();
        return a == b;
    }
    false
}
