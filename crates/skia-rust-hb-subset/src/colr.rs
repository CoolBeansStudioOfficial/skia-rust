// Copyright © 2018  Ebrahim Byagowi
// Copyright © 2020  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Color/COLR/COLR.hh, src/OT/Color/COLR/colrv1-closure.hh,
// src/OT/Color/CPAL/CPAL.hh, src/hb-subset-plan.cc#_colr_closure (harfbuzz 9cb1fee5)

//! `COLR` (version 0 and 1) and `CPAL`: the glyph, layer and palette closure, and the subsetters.
//! Variation data (`varStore`, `varIdxMap`, variable paints that refer to them) is not ported: a
//! font with such data reports `Unsupported`. Skia does not subset variable fonts.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use crate::Res;
use crate::bytes::{bsearch, tag};
use crate::ot::{INVALID, View, offset_subset_w, set_add_range, set_intersects_range};
use crate::plan::{Plan, unsupported};
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer, Whence};

const NO_VARIATION: u32 = 0xFFFF_FFFF;
const HB_MAX_NESTING_LEVEL: u32 = 64;

/// The `COLR` table header.
#[derive(Clone, Copy)]
struct Colr<'a>(View<'a>);

impl<'a> Colr<'a> {
    fn version(&self) -> u32 {
        self.0.u16(0)
    }
    fn num_base_glyphs(&self) -> u32 {
        self.0.u16(2)
    }
    /// `NNOffset32To`: an offset of 0 is the table itself.
    fn base_glyphs(&self) -> View<'a> {
        self.0.sub(self.0.u32(4) as usize)
    }
    fn layers(&self) -> View<'a> {
        self.0.sub(self.0.u32(8) as usize)
    }
    fn num_layers(&self) -> u32 {
        self.0.u16(12)
    }
    fn v1_offset(&self, field: usize) -> u32 {
        if self.version() >= 1 {
            self.0.u32(field)
        } else {
            0
        }
    }
    fn base_glyph_list(&self) -> View<'a> {
        if self.v1_offset(14) == 0 {
            View::new(&[])
        } else {
            self.0.sub(self.0.u32(14) as usize)
        }
    }
    fn layer_list(&self) -> View<'a> {
        if self.v1_offset(18) == 0 {
            View::new(&[])
        } else {
            self.0.sub(self.0.u32(18) as usize)
        }
    }
    fn clip_list(&self) -> View<'a> {
        if self.v1_offset(22) == 0 {
            View::new(&[])
        } else {
            self.0.sub(self.0.u32(22) as usize)
        }
    }
    fn has_var_store(&self) -> bool {
        self.v1_offset(30) != 0
    }
    /// `get_base_glyph_record`: `(glyphId, firstLayerIdx, numLayers)` of the version 0 record.
    fn base_glyph_record(&self, gid: u32) -> Option<(u32, u32, u32)> {
        let recs = self.base_glyphs();
        let n = self.num_base_glyphs() as usize;
        let i = bsearch(n, |i| gid.cmp(&recs.u16(6 * i)))?;
        Some((recs.u16(6 * i), recs.u16(6 * i + 2), recs.u16(6 * i + 4)))
    }

    /// `get_base_glyph_paintrecord`: the index of the record in the `BaseGlyphList`.
    fn base_glyph_paint_record(&self, gid: u32) -> Option<usize> {
        let list = self.base_glyph_list();
        let n = list.u32(0) as usize;
        bsearch(n, |i| gid.cmp(&list.u16(4 + 6 * i)))
    }

    /// The layers of a record: `all_layers.sub_array (first, num)` as `(glyphId, colorIdx)`.
    fn layer_records(&self, first: u32, num: u32) -> Vec<(u32, u32)> {
        let total = self.num_layers();
        let layers = self.layers();
        if first > total {
            return Vec::new();
        }
        let count = num.min(total - first);
        (0..count)
            .map(|k| {
                (
                    layers.u16(4 * (first + k) as usize),
                    layers.u16(4 * (first + k) as usize + 2),
                )
            })
            .collect()
    }
}

/// `hb_set_t::add_range` over `u32` bounds.
fn add_range(set: &mut BTreeSet<u32>, first: u32, last: u32) {
    if first <= last {
        set_add_range(set, first, last);
    }
}

/// `remap_indexes`.
fn remap_indexes(indexes: &BTreeSet<u32>) -> HashMap<u32, u32> {
    indexes
        .iter()
        .enumerate()
        .map(|(i, &v)| (v, i as u32))
        .collect()
}

/// `_remap_palette_indexes`.
fn remap_palette_indexes(palette_indexes: &BTreeSet<u32>) -> HashMap<u32, u32> {
    let mut mapping = HashMap::new();
    let mut new_idx = 0;
    for &palette_index in palette_indexes {
        if palette_index == 0xFFFF {
            mapping.insert(palette_index, palette_index);
            continue;
        }
        mapping.insert(palette_index, new_idx);
        new_idx += 1;
    }
    mapping
}

// -------------------------------------------------------------------------------------------
// Paint formats
// -------------------------------------------------------------------------------------------

/// `Variable<T>` wrappers: the paint formats that carry a `VarIdx` after the structure.
fn is_wrapped_variable(format: u32) -> bool {
    matches!(
        format,
        3 | 5 | 7 | 9 | 15 | 17 | 19 | 21 | 23 | 25 | 27 | 29 | 31
    )
}

/// The size of the paint structure (without the `VarIdx` of a `Variable<T>`), and the position of
/// the `Offset24To<Paint>` children.
fn paint_size(format: u32) -> usize {
    match format {
        1 | 10 | 20 | 21 | 24 | 25 => 6,
        2 | 3 => 5,
        4..=7 => 16,
        8 | 9 | 18 | 19 | 30 | 31 => 12,
        11 => 3,
        12 | 13 => 7,
        14..=17 | 28 | 29 | 32 => 8,
        22 | 23 | 26 | 27 => 10,
        _ => 0,
    }
}

/// `hb_colrv1_closure_context_t`.
struct ClosureCtx<'a> {
    colr: Colr<'a>,
    visited_paint: BTreeSet<u32>,
    glyphs: BTreeSet<u32>,
    layer_indices: BTreeSet<u32>,
    palette_indices: BTreeSet<u32>,
    variation_indices: BTreeSet<u32>,
    num_var_idxes: u32,
    nesting_level_left: u32,
}

impl ClosureCtx<'_> {
    fn add_var_idxes(&mut self, first: u32, num: u32) {
        if num == 0 || first == NO_VARIATION {
            return;
        }
        add_range(
            &mut self.variation_indices,
            first,
            first.wrapping_add(num - 1),
        );
    }

    /// `Paint::dispatch (hb_colrv1_closure_context_t)`.
    fn dispatch(&mut self, paint: View<'_>) {
        let format = paint.u8(0);
        if !(1..=32).contains(&format) {
            return;
        }
        if self.nesting_level_left == 0 {
            return;
        }
        let delta =
            (paint.d.as_ptr() as usize).wrapping_sub(self.colr.0.d.as_ptr() as usize) as u32;
        if !self.visited_paint.insert(delta) {
            return;
        }
        self.nesting_level_left -= 1;
        self.closurev1(paint, format);
        self.nesting_level_left += 1;
    }

    /// `ColorLine::closurev1` of the colour line at the `Offset24To` field 1 of the gradient.
    fn color_line(&mut self, paint: View<'_>, variable_stops: bool) {
        let line = paint.sub(paint.u24(1) as usize);
        let stride = if variable_stops { 10 } else { 6 };
        for i in 0..line.u16(1) as usize {
            let stop = 3 + stride * i;
            if variable_stops {
                // `Variable<ColorStop>::closurev1`
                self.num_var_idxes = 0;
                self.palette_indices.insert(line.u16(stop + 2));
                self.num_var_idxes = 2;
                let base = line.u32(stop + 6);
                self.add_var_idxes(base, 2);
            } else {
                self.palette_indices.insert(line.u16(stop + 2));
                self.num_var_idxes = 2;
            }
        }
    }

    fn child(&mut self, paint: View<'_>, field: usize) {
        let off = paint.u24(field) as usize;
        if off != 0 {
            self.dispatch(paint.sub(off));
        }
    }

    /// The `closurev1` of the paint, through `Variable<T>` or `NoVariable<T>`.
    fn closurev1(&mut self, paint: View<'_>, format: u32) {
        let wrapped = is_wrapped_variable(format);
        if wrapped {
            self.num_var_idxes = 0;
        }
        match format {
            1 => {
                // `PaintColrLayers::closurev1`
                let num = paint.u8(1);
                let first = paint.u32(2);
                if num != 0 {
                    add_range(&mut self.layer_indices, first, first.wrapping_add(num - 1));
                }
                let list = self.colr.layer_list();
                for i in first..first.wrapping_add(num) {
                    let off = if i < list.u32(0) {
                        list.u32(4 + 4 * i as usize)
                    } else {
                        0
                    };
                    if off != 0 {
                        self.dispatch(list.sub(off as usize));
                    }
                }
            }
            2 | 3 => {
                self.palette_indices.insert(paint.u16(1));
                self.num_var_idxes = 1;
            }
            4..=9 => {
                self.color_line(paint, format % 2 == 1);
                self.num_var_idxes = if matches!(format, 8 | 9) { 4 } else { 6 };
            }
            10 => {
                self.glyphs.insert(paint.u16(4));
                self.child(paint, 1);
            }
            11 => {
                let gid = paint.u16(1);
                if let Some(i) = self.colr.base_glyph_paint_record(gid) {
                    self.glyphs.insert(gid);
                    let list = self.colr.base_glyph_list();
                    let off = list.u32(4 + 6 * i + 2);
                    if off != 0 {
                        self.dispatch(list.sub(off as usize));
                    }
                }
            }
            12 | 13 => {
                self.child(paint, 1);
                // `(this+transform).closurev1 (c)`: `Var<Affine2x3>`
                let off = paint.u24(4) as usize;
                if off != 0 {
                    if format == 13 {
                        self.num_var_idxes = 0;
                        // `Affine2x3::closurev1`
                        self.num_var_idxes = 6;
                        let base = paint.sub(off).u32(24);
                        self.add_var_idxes(base, 6);
                    } else {
                        self.num_var_idxes = 6;
                    }
                }
            }
            14..=31 => {
                self.child(paint, 1);
                self.num_var_idxes = match format {
                    14 | 15 | 16 | 17 | 28 | 29 => 2,
                    18 | 19 | 30 | 31 => 4,
                    20 | 21 | 24 | 25 => 1,
                    _ => 3,
                };
            }
            32 => {
                self.child(paint, 1);
                self.child(paint, 5);
            }
            _ => {}
        }
        if wrapped {
            let base = paint.u32(paint_size(format));
            let n = self.num_var_idxes;
            self.add_var_idxes(base, n);
        }
    }
}

/// `COLR::closure_forV1` (COLR.hh#L1796-L1830).
fn closure_for_v1(
    colr: Colr<'_>,
    glyphset: &mut BTreeSet<u32>,
) -> (BTreeSet<u32>, BTreeSet<u32>, BTreeSet<u32>) {
    let mut c = ClosureCtx {
        colr,
        visited_paint: BTreeSet::new(),
        glyphs: BTreeSet::new(),
        layer_indices: BTreeSet::new(),
        palette_indices: BTreeSet::new(),
        variation_indices: BTreeSet::new(),
        num_var_idxes: 1,
        nesting_level_left: HB_MAX_NESTING_LEVEL,
    };
    if colr.version() < 1 {
        return (c.layer_indices, c.palette_indices, c.variation_indices);
    }
    let list = colr.base_glyph_list();
    for i in 0..list.u32(0) as usize {
        let gid = list.u16(4 + 6 * i);
        if !glyphset.contains(&gid) {
            continue;
        }
        let off = list.u32(4 + 6 * i + 2) as usize;
        if off != 0 {
            c.dispatch(list.sub(off));
        }
    }
    glyphset.extend(c.glyphs.iter().copied());
    let clips = colr.clip_list();
    c.glyphs = glyphset.clone();
    for i in 0..clips.u32(1) as usize {
        let rec = 5 + 7 * i;
        if !set_intersects_range(&c.glyphs, clips.u16(rec), clips.u16(rec + 2)) {
            continue;
        }
        let off = clips.u24(rec + 4) as usize;
        if off == 0 {
            continue;
        }
        let clip_box = clips.sub(off);
        if clip_box.u8(0) == 2 {
            let base = clip_box.u32(9);
            add_range(&mut c.variation_indices, base, base.wrapping_add(3));
        }
    }
    (c.layer_indices, c.palette_indices, c.variation_indices)
}

/// Port of `_colr_closure` (hb-subset-plan.cc#L111-L166).
pub(crate) fn colr_closure(plan: &mut Plan<'_>, glyphs_colred: &mut BTreeSet<u32>) -> Res<()> {
    let data = plan.source.table(tag(b"COLR"));
    if data.is_empty() {
        return Ok(());
    }
    let colr = Colr(View::new(data));
    // Collect all glyphs referenced by COLRv0.
    let mut glyphset_colrv0: BTreeSet<u32> = BTreeSet::new();
    for &gid in glyphs_colred.iter() {
        if let Some((_, first, num)) = colr.base_glyph_record(gid) {
            for (glyph_id, _) in colr.layer_records(first, num) {
                glyphset_colrv0.insert(glyph_id);
            }
        }
    }
    glyphs_colred.extend(glyphset_colrv0);

    // Closure for COLRv1.
    let (layer_indices, mut palette_indices, variation_indices) =
        closure_for_v1(colr, glyphs_colred);

    // `closure_V0palette_indices`
    if colr.num_base_glyphs() != 0 && colr.num_layers() != 0 {
        for i in 0..colr.num_base_glyphs() as usize {
            let recs = colr.base_glyphs();
            if !glyphs_colred.contains(&recs.u16(6 * i)) {
                continue;
            }
            for (_, color_idx) in colr.layer_records(recs.u16(6 * i + 2), recs.u16(6 * i + 4)) {
                palette_indices.insert(color_idx);
            }
        }
    }
    plan.colrv1_layers = remap_indexes(&layer_indices);
    plan.colr_palettes = remap_palette_indexes(&palette_indices);

    if colr.has_var_store() && !variation_indices.is_empty() {
        return unsupported("COLR variation store");
    }
    Ok(())
}

// -------------------------------------------------------------------------------------------
// Subsetting
// -------------------------------------------------------------------------------------------

struct SubsetCtx<'p, 'a> {
    plan: &'p Plan<'a>,
}

fn embed_bytes(s: &mut Serializer, v: View<'_>, off: usize, len: usize) -> usize {
    let bytes: Vec<u8> = (0..len)
        .map(|k| v.d.get(off + k).copied().unwrap_or(0))
        .collect();
    s.embed(&bytes)
}

// The helpers keep `self` so that the calls read like the C++ member calls.
#[allow(clippy::unused_self)]
impl SubsetCtx<'_, '_> {
    fn check16(&self, s: &mut Serializer, pos: usize, value: u32) -> bool {
        if !s.check_fits(u64::from(value), 16, ERROR_INT_OVERFLOW) {
            return false;
        }
        s.set_u16(pos, value as u16);
        true
    }

    fn glyph(&self, gid: u32) -> u32 {
        self.plan.glyph_map.get(&gid).copied().unwrap_or(INVALID)
    }

    fn palette(&self, idx: u32) -> u32 {
        self.plan
            .colr_palettes
            .get(&idx)
            .copied()
            .unwrap_or(INVALID)
    }

    /// The offset of the child `Paint` at `field` of `paint` (24 bit), subset into the object.
    fn child(&self, s: &mut Serializer, out: usize, paint: View<'_>, field: usize) -> bool {
        offset_subset_w(s, out + field, 3, paint, field, |s, p| {
            self.paint_subset(s, p)
        })
    }

    /// `Variable<T>::subset`'s trailer: the `VarIdx`, which must be mapped unless it is
    /// `NO_VARIATION`.
    fn var_trailer(&self, s: &mut Serializer, var_idx_base: u32) -> bool {
        if var_idx_base != NO_VARIATION {
            // `colrv1_variation_idx_delta_map` has no entry.
            return false;
        }
        s.embed_u32(var_idx_base);
        true
    }

    /// `ColorLine<Var>::subset`.
    fn color_line_subset(&self, s: &mut Serializer, line: View<'_>, variable_stops: bool) -> bool {
        let out = s.allocate(3);
        // `extend` is a `HBUINT8`
        s.set_u8(out, line.u8(0) as u8);
        let n = line.u16(1);
        s.set_u16(out + 1, n as u16);
        let stride = if variable_stops { 10 } else { 6 };
        for i in 0..n as usize {
            let stop = 3 + stride * i;
            // `ColorStop::subset`
            let pos = embed_bytes(s, line, stop, 6);
            if !self.check16(s, pos + 2, self.palette(line.u16(stop + 2))) {
                return false;
            }
            if variable_stops && !self.var_trailer(s, line.u32(stop + 6)) {
                return false;
            }
        }
        true
    }

    /// `Paint::dispatch (hb_subset_context_t)`.
    fn paint_subset(&self, s: &mut Serializer, paint: View<'_>) -> bool {
        let format = paint.u8(0);
        if !(1..=32).contains(&format) {
            // `c->default_return_value ()`
            return true;
        }
        let size = paint_size(format);
        let wrapped = is_wrapped_variable(format);
        let out = embed_bytes(s, paint, 0, size);
        let ret = match format {
            1 => {
                // `PaintColrLayers::subset`
                let first = if paint.u8(1) != 0 {
                    self.plan
                        .colrv1_layers
                        .get(&paint.u32(2))
                        .copied()
                        .unwrap_or(INVALID)
                } else {
                    0
                };
                s.set_u32(out + 2, first);
                true
            }
            2 | 3 => self.check16(s, out + 1, self.palette(paint.u16(1))),
            4..=9 => offset_subset_w(s, out + 1, 3, paint, 1, |s, l| {
                self.color_line_subset(s, l, format % 2 == 1)
            }),
            10 => {
                if !self.check16(s, out + 4, self.glyph(paint.u16(4))) {
                    return false;
                }
                self.child(s, out, paint, 1)
            }
            11 => self.check16(s, out + 1, self.glyph(paint.u16(1))),
            12 | 13 => {
                // The transform first.
                let off = paint.u24(4) as usize;
                let ok = if off == 0 {
                    s.zero_field(out + 4, 3);
                    false
                } else {
                    let variable = format == 13;
                    s.serialize_subset(out + 4, 3, true, |s| {
                        embed_bytes(s, paint.sub(off), 0, 24);
                        if variable {
                            self.var_trailer(s, paint.sub(off).u32(24))
                        } else {
                            true
                        }
                    })
                };
                if !ok {
                    return false;
                }
                self.child(s, out, paint, 1)
            }
            14..=31 => self.child(s, out, paint, 1),
            32 => {
                let mut ret = false;
                ret |= self.child(s, out, paint, 1);
                ret |= self.child(s, out, paint, 5);
                ret
            }
            _ => true,
        };
        if !ret {
            return false;
        }
        if wrapped {
            return self.var_trailer(s, paint.u32(size));
        }
        true
    }

    /// `ClipBox::subset`.
    fn clip_box_subset(&self, s: &mut Serializer, b: View<'_>) -> bool {
        match b.u8(0) {
            1 => {
                embed_bytes(s, b, 0, 9);
                true
            }
            2 => {
                embed_bytes(s, b, 0, 9);
                self.var_trailer(s, b.u32(9))
            }
            _ => true,
        }
    }

    /// `ClipList::subset` (COLR.hh#L1370-L1410) with `serialize_clip_records`.
    fn clip_list_subset(&self, s: &mut Serializer, list: View<'_>) -> bool {
        let out = s.allocate(5);
        if !s.check_fits(u64::from(list.u8(0)), 8, ERROR_INT_OVERFLOW) {
            return false;
        }
        s.set_u8(out, list.u8(0) as u8);
        let glyphset = &self.plan.glyphset_colred;
        let mut new_gid_offset_map: BTreeMap<u32, u32> = BTreeMap::new();
        for i in 0..list.u32(1) as usize {
            let rec = 5 + 7 * i;
            let (start_gid, end_gid) = (list.u16(rec), list.u16(rec + 2));
            let box_offset = list.u24(rec + 4);
            for gid in start_gid..=end_gid {
                if !glyphset.contains(&gid) || !self.plan.glyph_map.contains_key(&gid) {
                    continue;
                }
                new_gid_offset_map.insert(self.glyph(gid), box_offset);
            }
        }
        let count = self.serialize_clip_records(s, list, &new_gid_offset_map);
        if count == 0 {
            return false;
        }
        if !s.check_fits(u64::from(count), 32, ERROR_INT_OVERFLOW) {
            return false;
        }
        s.set_u32(out + 1, count);
        true
    }

    fn serialize_clip_records(
        &self,
        s: &mut Serializer,
        list: View<'_>,
        map: &BTreeMap<u32, u32>,
    ) -> u32 {
        if map.is_empty() {
            return 0;
        }
        let mut gids = map.keys().copied();
        let mut start_gid = gids.next().unwrap_or(0);
        let mut prev_gid = start_gid;
        let mut prev_offset = map[&start_gid];
        let mut count = 0;
        let record = |s: &mut Serializer, start: u32, end: u32, offset: u32| -> bool {
            // `ClipRecord::subset`: the record is embedded with the source offset, which
            // `serialize_subset` then replaces.
            let pos = s.embed(&[
                (start >> 8) as u8,
                start as u8,
                (end >> 8) as u8,
                end as u8,
                (offset >> 16) as u8,
                (offset >> 8) as u8,
                offset as u8,
            ]);
            if offset == 0 {
                s.zero_field(pos + 4, 3);
                return false;
            }
            let b = list.sub(offset as usize);
            s.serialize_subset(pos + 4, 3, true, |s| self.clip_box_subset(s, b))
        };
        for g in gids {
            let offset = map[&g];
            if g == prev_gid + 1 && offset == prev_offset {
                prev_gid = g;
                continue;
            }
            if !record(s, start_gid, prev_gid, prev_offset) {
                return 0;
            }
            count += 1;
            start_gid = g;
            prev_gid = g;
            prev_offset = offset;
        }
        if !record(s, start_gid, prev_gid, prev_offset) {
            return 0;
        }
        count + 1
    }

    /// `BaseGlyphList::subset`.
    fn base_glyph_list_subset(&self, s: &mut Serializer, list: View<'_>) -> bool {
        let out = s.allocate(4);
        let mut len = 0u32;
        for i in 0..list.u32(0) as usize {
            let rec = 4 + 6 * i;
            let gid = list.u16(rec);
            if !self.plan.glyphset_colred.contains(&gid) {
                continue;
            }
            // `BaseGlyphPaintRecord::serialize`
            let pos = embed_bytes(s, list, rec, 6);
            if !self.check16(s, pos, self.glyph(gid)) {
                return false;
            }
            if !offset_subset_w(s, pos + 2, 4, list, rec + 2, |s, p| self.paint_subset(s, p)) {
                return false;
            }
            len += 1;
            s.set_u32(out, len);
        }
        len != 0
    }

    /// `LayerList::subset`.
    fn layer_list_subset(&self, s: &mut Serializer, list: View<'_>) -> bool {
        let out = s.allocate(4);
        let mut ret = false;
        let mut len = 0u32;
        for i in 0..list.u32(0) {
            if !self.plan.colrv1_layers.contains_key(&i) {
                continue;
            }
            len += 1;
            s.set_u32(out, len);
            let pos = s.allocate(4);
            ret |= offset_subset_w(s, pos, 4, list, 4 + 4 * i as usize, |s, p| {
                self.paint_subset(s, p)
            });
        }
        ret
    }
}

/// `COLR::subset` (COLR.hh#L2037-L2128).
#[allow(clippy::unnecessary_wraps)] // the callback shape of `run_table`
pub(crate) fn colr_subset(plan: &Plan<'_>, s: &mut Serializer, data: View<'_>) -> Res<bool> {
    let colr = Colr(data);
    let ctx = SubsetCtx { plan };
    let glyphset = &plan.glyphset_colred;
    let num_output = plan.num_output_glyphs;

    // `base_it`
    let mut base_records: Vec<(u32, u32)> = Vec::new(); // (new gid, numLayers)
    for new_gid in 0..num_output {
        let old_gid = plan
            .reverse_glyph_map
            .get(&new_gid)
            .copied()
            .unwrap_or(INVALID);
        if !glyphset.contains(&old_gid) {
            continue;
        }
        if let Some((_, _, num)) = colr.base_glyph_record(old_gid) {
            base_records.push((new_gid, num));
        }
    }
    // `layer_it`
    let mut layer_vectors: Vec<Vec<(u32, u32)>> = Vec::new();
    for new_gid in 0..num_output {
        let old_gid = plan
            .reverse_glyph_map
            .get(&new_gid)
            .copied()
            .unwrap_or(INVALID);
        if !glyphset.contains(&old_gid) {
            continue;
        }
        let Some((_, first, num)) = colr.base_glyph_record(old_gid) else {
            continue;
        };
        if first >= colr.num_layers() || first + num > colr.num_layers() {
            continue;
        }
        let mut out_layers = Vec::new();
        let mut ok = true;
        for (glyph_id, color_idx) in colr.layer_records(first, num) {
            let Some(&new_layer_gid) = plan.glyph_map.get(&glyph_id) else {
                ok = false;
                break;
            };
            let new_color = plan
                .colr_palettes
                .get(&color_idx)
                .copied()
                .unwrap_or(INVALID);
            out_layers.push((new_layer_gid, new_color));
        }
        if ok {
            layer_vectors.push(out_layers);
        }
    }
    if colr.version() == 0 && (base_records.is_empty() || layer_vectors.is_empty()) {
        return Ok(false);
    }
    let out = s.allocate(14);
    if colr.version() == 0 || downgrade_to_v0(colr, glyphset) {
        return Ok(serialize_v0(s, out, 0, &base_records, &layer_vectors));
    }
    s.allocate(20);
    if !serialize_v0(s, out, colr.version(), &base_records, &layer_vectors) {
        return Ok(false);
    }
    // `subset_varstore` and `subset_delta_set_index_map` do nothing without variation indices
    // (the closure reports those as unsupported).
    if !offset_subset_w(s, out + 14, 4, data, 14, |s, l| {
        ctx.base_glyph_list_subset(s, l)
    }) {
        return Ok(false);
    }
    offset_subset_w(s, out + 18, 4, data, 18, |s, l| ctx.layer_list_subset(s, l));
    offset_subset_w(s, out + 22, 4, data, 22, |s, l| ctx.clip_list_subset(s, l));
    Ok(true)
}

/// `COLR::downgrade_to_V0`.
fn downgrade_to_v0(colr: Colr<'_>, glyphset: &BTreeSet<u32>) -> bool {
    let list = colr.base_glyph_list();
    !(0..list.u32(0) as usize).any(|i| glyphset.contains(&list.u16(4 + 6 * i)))
}

/// `COLR::serialize_V0` (COLR.hh#L1960-L1998).
fn serialize_v0(
    s: &mut Serializer,
    out: usize,
    version: u32,
    base_records: &[(u32, u32)],
    layer_vectors: &[Vec<(u32, u32)>],
) -> bool {
    if base_records.len() != layer_vectors.len() {
        return false;
    }
    s.set_u16(out, version as u16);
    let mut num_layers = 0u16;
    s.set_u16(out + 12, 0);
    s.set_u16(out + 2, base_records.len() as u16);
    if base_records.is_empty() {
        s.zero_field(out + 4, 4);
        s.zero_field(out + 8, 4);
        return true;
    }
    s.push();
    for &(new_gid, n) in base_records {
        let pos = s.allocate(6);
        s.set_u16(pos, new_gid as u16);
        s.set_u16(pos + 2, num_layers);
        s.set_u16(pos + 4, n as u16);
        num_layers = num_layers.wrapping_add(n as u16);
    }
    let idx = s.pop_pack(true);
    s.set_u16(out + 12, num_layers);
    s.add_link(out + 4, 4, idx, Whence::Head, 0);
    s.push();
    for layers in layer_vectors {
        for &(g, c) in layers {
            s.embed_u16(g as u16);
            s.embed_u16(c as u16);
        }
    }
    let idx = s.pop_pack(true);
    s.add_link(out + 8, 4, idx, Whence::Head, 0);
    true
}

// -------------------------------------------------------------------------------------------
// CPAL
// -------------------------------------------------------------------------------------------

/// `CPAL::subset` (CPAL.hh#L267-L325) with `CPAL::serialize` and `CPALV1Tail::serialize`.
#[allow(clippy::unnecessary_wraps)] // the callback shape of `run_table`
pub(crate) fn cpal_subset(plan: &Plan<'_>, s: &mut Serializer, cpal: View<'_>) -> Res<bool> {
    let num_palettes = cpal.u16(4) as usize;
    if num_palettes == 0 {
        return Ok(false);
    }
    let color_index_map = &plan.colr_palettes;
    if color_index_map.is_empty() {
        return Ok(false);
    }
    let retained: BTreeSet<u32> = color_index_map
        .keys()
        .copied()
        .filter(|&k| k != 0xFFFF)
        .collect();
    if retained.is_empty() {
        return Ok(false);
    }
    let version = cpal.u16(0);
    let out = s.allocate(12);
    s.set_u16(out, version as u16);
    s.set_u16(out + 2, retained.len() as u16);
    s.set_u16(out + 4, num_palettes as u16);

    let mut first_color_index_for_layer: Vec<u32> = Vec::new();
    let mut first_color_to_layer_index: HashMap<u32, u32> = HashMap::new();
    let record_indices: Vec<u32> = (0..num_palettes).map(|i| cpal.u16(12 + 2 * i)).collect();
    for &first in &record_indices {
        if first_color_to_layer_index.contains_key(&first) {
            continue;
        }
        first_color_index_for_layer.push(first);
        first_color_to_layer_index.insert(first, first_color_index_for_layer.len() as u32 - 1);
    }
    s.set_u16(
        out + 6,
        (first_color_index_for_layer.len() * retained.len()) as u16,
    );

    // `CPAL::serialize`
    for &idx in &record_indices {
        let layer_index = first_color_to_layer_index
            .get(&idx)
            .copied()
            .unwrap_or(INVALID);
        s.embed_u16(layer_index.wrapping_mul(retained.len() as u32) as u16);
    }
    let num_color_records = cpal.u16(6) as usize;
    let color_records = cpal.sub(cpal.u32(8) as usize);
    s.push();
    for &first in &first_color_index_for_layer {
        for &color_index in &retained {
            let i = (first + color_index) as usize;
            let v = if i < num_color_records {
                color_records.u32(4 * i)
            } else {
                0
            };
            s.embed_u32(v);
        }
    }
    let idx = s.pop_pack(true);
    s.add_link(out + 8, 4, idx, Whence::Head, 0);

    if version == 1 {
        let num_colors = cpal.u16(2) as usize;
        let tail = s.allocate(12);
        let src_tail = 12 + 2 * num_palettes;
        let flags = cpal.u32(src_tail) as usize;
        if flags != 0 {
            s.push();
            let a = cpal.sub(flags);
            for i in 0..num_palettes {
                s.embed_u32(a.u32(4 * i));
            }
            let idx = s.pop_pack(true);
            s.add_link(tail, 4, idx, Whence::Head, 0);
        }
        let labels = cpal.u32(src_tail + 4) as usize;
        if labels != 0 {
            s.push();
            let a = cpal.sub(labels);
            for i in 0..num_palettes {
                s.embed_u16(a.u16(2 * i) as u16);
            }
            let idx = s.pop_pack(true);
            s.add_link(tail + 4, 4, idx, Whence::Head, 0);
        }
        let color_labels = cpal.u32(src_tail + 8) as usize;
        if color_labels != 0 {
            s.push();
            let a = cpal.sub(color_labels);
            for i in 0..num_colors {
                if !color_index_map.contains_key(&(i as u32)) {
                    continue;
                }
                s.embed_u16(a.u16(2 * i) as u16);
            }
            let idx = s.pop_pack(true);
            s.add_link(tail + 8, 4, idx, Whence::Head, 0);
        }
    }
    Ok(true)
}
