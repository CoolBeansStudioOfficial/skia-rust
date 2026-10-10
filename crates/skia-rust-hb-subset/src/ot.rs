// Copyright © 2007,2008,2009  Red Hat, Inc.
// Copyright © 2010,2011,2012  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-open-type.hh, src/OT/Layout/Common/{Coverage,CoverageFormat1,CoverageFormat2,RangeRecord}.hh,
// the ClassDef part of src/hb-ot-layout-common.hh (harfbuzz 9cb1fee5)

//! Views over OpenType structures, `Coverage` and `ClassDef`.
//!
//! A [`View`] is a byte slice that starts at the structure it describes (`this`), so `this+offset`
//! is [`View::off16`]. A read outside the bytes gives zero, as reading `Null (T)` does.

use std::collections::{BTreeSet, HashMap};

use crate::bytes::{bit_storage, u8_at, u16_at, u24_at, u32_at};
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer, Whence};

/// `NOT_COVERED` (OT/Layout/Common/CoverageFormat1.hh#L45).
pub(crate) const NOT_COVERED: u32 = u32::MAX;
/// `HB_MAP_VALUE_INVALID` and `HB_SET_VALUE_INVALID`.
pub(crate) const INVALID: u32 = u32::MAX;

/// A structure of the source font.
#[derive(Clone, Copy, Debug)]
pub(crate) struct View<'a> {
    pub d: &'a [u8],
}

impl<'a> View<'a> {
    pub(crate) fn new(d: &'a [u8]) -> Self {
        View { d }
    }

    pub(crate) fn u8(&self, o: usize) -> u32 {
        u32::from(u8_at(self.d, o))
    }

    pub(crate) fn u16(&self, o: usize) -> u32 {
        u32::from(u16_at(self.d, o))
    }

    pub(crate) fn u24(&self, o: usize) -> u32 {
        u24_at(self.d, o)
    }

    pub(crate) fn u32(&self, o: usize) -> u32 {
        u32_at(self.d, o)
    }

    /// `this + o`: the bytes from offset `o` on.
    pub(crate) fn sub(&self, o: usize) -> View<'a> {
        View {
            d: self.d.get(o..).unwrap_or(&[]),
        }
    }

    /// `this + offset16`: the empty (`Null`) view when the offset field is zero.
    pub(crate) fn off16(&self, field: usize) -> View<'a> {
        let o = self.u16(field) as usize;
        if o == 0 { View { d: &[] } } else { self.sub(o) }
    }

    /// `this + offset32`.
    pub(crate) fn off32(&self, field: usize) -> View<'a> {
        let o = self.u32(field) as usize;
        if o == 0 { View { d: &[] } } else { self.sub(o) }
    }

    /// Whether the offset field at `field` is zero (`offset == 0`).
    pub(crate) fn is_null16(&self, field: usize) -> bool {
        self.u16(field) == 0
    }

    pub(crate) fn is_null32(&self, field: usize) -> bool {
        self.u32(field) == 0
    }
}

/// `hb_set_t::next`: the smallest element greater than `v`.
pub(crate) fn set_next(set: &BTreeSet<u32>, v: u32) -> Option<u32> {
    if v == INVALID {
        return set.first().copied();
    }
    set.range(v.checked_add(1)?..).next().copied()
}

/// `hb_set_t::intersects (first, last)`.
pub(crate) fn set_intersects_range(set: &BTreeSet<u32>, first: u32, last: u32) -> bool {
    first <= last && set.range(first..=last).next().is_some()
}

/// `hb_set_t::add_range`.
pub(crate) fn set_add_range(set: &mut BTreeSet<u32>, first: u32, last: u32) {
    if first > last {
        return;
    }
    for v in first..=last {
        set.insert(v);
    }
}

/// `Coverage` (OT/Layout/Common/Coverage.hh), formats 1 and 2 (formats 3 and 4 need glyph ids over
/// 0xFFFF, which a font with 16 bit glyph ids cannot have).
#[derive(Clone, Copy, Debug)]
pub(crate) struct Coverage<'a>(pub View<'a>);

impl Coverage<'_> {
    pub(crate) fn format(&self) -> u32 {
        self.0.u16(0)
    }

    /// `Coverage::get_coverage`.
    pub(crate) fn get_coverage(&self, g: u32) -> u32 {
        let v = self.0;
        match self.format() {
            1 => {
                // `glyphArray.bfind`
                let len = v.u16(2) as usize;
                let found = crate::bytes::bsearch(len, |i| g.cmp(&v.u16(4 + 2 * i)));
                found.map_or(NOT_COVERED, |i| i as u32)
            }
            2 => {
                // `rangeRecord.bsearch`
                let len = v.u16(2) as usize;
                let found = crate::bytes::bsearch(len, |i| {
                    let r = 4 + 6 * i;
                    if g < v.u16(r) {
                        std::cmp::Ordering::Less
                    } else if g <= v.u16(r + 2) {
                        std::cmp::Ordering::Equal
                    } else {
                        std::cmp::Ordering::Greater
                    }
                });
                match found {
                    Some(i) => {
                        let r = 4 + 6 * i;
                        let (first, last, value) = (v.u16(r), v.u16(r + 2), v.u16(r + 4));
                        if first <= last {
                            value + (g - first)
                        } else {
                            NOT_COVERED
                        }
                    }
                    // The `Null` range has first = last = value = 0.
                    None => NOT_COVERED,
                }
            }
            _ => NOT_COVERED,
        }
    }

    /// `Coverage::get_population`.
    pub(crate) fn get_population(&self) -> u32 {
        let v = self.0;
        match self.format() {
            1 => v.u16(2),
            2 => {
                let mut ret: u64 = 0;
                for i in 0..v.u16(2) as usize {
                    let r = 4 + 6 * i;
                    let (first, last) = (v.u16(r), v.u16(r + 2));
                    if last >= first {
                        ret += u64::from(last - first + 1);
                    }
                }
                ret.min(u64::from(u32::MAX)) as u32
            }
            _ => NOT_COVERED,
        }
    }

    pub(crate) fn has(&self, g: u32) -> bool {
        self.get_coverage(g) != NOT_COVERED
    }

    /// `Coverage::intersects`.
    pub(crate) fn intersects(&self, glyphs: &BTreeSet<u32>) -> bool {
        let v = self.0;
        match self.format() {
            1 => (0..v.u16(2) as usize).any(|i| glyphs.contains(&v.u16(4 + 2 * i))),
            2 => (0..v.u16(2) as usize).any(|i| {
                let r = 4 + 6 * i;
                set_intersects_range(glyphs, v.u16(r), v.u16(r + 2))
            }),
            _ => false,
        }
    }

    /// `Coverage::iter ()`: the glyphs in coverage order. Format 2 stops at a range whose start
    /// coverage index does not continue the previous one (`CoverageFormat2_4::iter_t`).
    #[allow(clippy::iter_not_returning_iterator)] // `Coverage::iter ()` collected into a vector
    pub(crate) fn iter(&self) -> Vec<u32> {
        let v = self.0;
        let mut out = Vec::new();
        match self.format() {
            1 => {
                for i in 0..v.u16(2) as usize {
                    out.push(v.u16(4 + 2 * i));
                }
            }
            2 => {
                let len = v.u16(2) as usize;
                let first = |i: usize| v.u16(4 + 6 * i);
                let last = |i: usize| v.u16(4 + 6 * i + 2);
                let value = |i: usize| v.u16(4 + 6 * i + 4);
                let mut coverage = 0u32;
                let mut i = 0usize;
                let mut j = if len != 0 { first(0) } else { 0 };
                if first(0) > last(0) {
                    i = len;
                    j = 0;
                }
                while i < len {
                    out.push(j);
                    // `__next__`
                    if j >= last(i) {
                        i += 1;
                        if i < len {
                            let old = coverage;
                            j = first(i);
                            coverage = value(i);
                            if coverage != old.wrapping_add(1) {
                                break;
                            }
                        } else {
                            j = 0;
                        }
                    } else {
                        coverage += 1;
                        j += 1;
                    }
                }
            }
            _ => {}
        }
        out
    }

    /// `Coverage::intersect_set`: the glyphs of `glyphs` this coverage has, ascending for format 1
    /// as the array is sorted and in range order for format 2.
    pub(crate) fn intersect_set(&self, glyphs: &BTreeSet<u32>) -> BTreeSet<u32> {
        let v = self.0;
        let mut out = BTreeSet::new();
        match self.format() {
            1 => {
                for i in 0..v.u16(2) as usize {
                    let g = v.u16(4 + 2 * i);
                    if glyphs.contains(&g) {
                        out.insert(g);
                    }
                }
            }
            2 => {
                let mut last = 0u32;
                for i in 0..v.u16(2) as usize {
                    let r = 4 + 6 * i;
                    let (f, l) = (v.u16(r), v.u16(r + 2));
                    if f < last {
                        break;
                    }
                    last = l;
                    let mut g = f.wrapping_sub(1);
                    while let Some(n) = set_next(glyphs, g) {
                        if n > last {
                            break;
                        }
                        out.insert(n);
                        g = n;
                    }
                }
            }
            _ => {}
        }
        out
    }
}

/// `Coverage::serialize` (Coverage.hh#L130-L172) with `CoverageFormat1_3::serialize` and
/// `CoverageFormat2_4::serialize`. Writes at the current position of the current object.
pub(crate) fn coverage_serialize(s: &mut Serializer, glyphs: &[u32]) -> bool {
    let start = s.allocate(2);
    let count = glyphs.len() as u32;
    let mut num_ranges = 0u32;
    let mut last = u32::MAX - 1; // (hb_codepoint_t) -2
    let mut max = 0u32;
    let mut unsorted = false;
    for &g in glyphs {
        if last != u32::MAX - 1 && g < last {
            unsorted = true;
        }
        if last.wrapping_add(1) != g {
            num_ranges += 1;
        }
        last = g;
        if g > max {
            max = g;
        }
    }
    let format = if !unsorted && count <= num_ranges * 3 {
        1
    } else {
        2
    };
    if max > 0xFFFF {
        // Formats 3 and 4 (24 bit glyph ids) are not ported: no 16 bit font has such glyphs.
        s.err(ERROR_INT_OVERFLOW);
        return false;
    }
    s.set_u16(start, format);
    if format == 1 {
        // `glyphArray.serialize (c, glyphs)`
        s.embed_u16(glyphs.len() as u16);
        for &g in glyphs {
            s.embed_u16(g as u16);
        }
        true
    } else {
        // `CoverageFormat2_4::serialize`
        let mut num_ranges = 0u32;
        let mut last = u32::MAX - 1;
        for &g in glyphs {
            if last.wrapping_add(1) != g {
                num_ranges += 1;
            }
            last = g;
        }
        s.embed_u16(num_ranges as u16);
        let base = s.allocate(6 * num_ranges as usize);
        if num_ranges == 0 {
            return true;
        }
        let mut range = usize::MAX;
        let mut last = u32::MAX - 1;
        let mut unsorted = false;
        for (count, &g) in glyphs.iter().enumerate() {
            if last.wrapping_add(1) != g {
                if last != u32::MAX - 1 && last.wrapping_add(1) > g {
                    unsorted = true;
                }
                range = range.wrapping_add(1);
                s.set_u16(base + 6 * range, g as u16);
                s.set_u16(base + 6 * range + 4, count as u16);
            }
            s.set_u16(base + 6 * range + 2, g as u16);
            last = g;
        }
        if unsorted {
            // `rangeRecord.as_array ().qsort (RangeRecord::cmp_range)`
            let mut recs: Vec<(u16, u16, u16)> = (0..num_ranges as usize)
                .map(|i| {
                    let b = s.bytes();
                    (
                        u16_at(b, base + 6 * i),
                        u16_at(b, base + 6 * i + 2),
                        u16_at(b, base + 6 * i + 4),
                    )
                })
                .collect();
            recs.sort_unstable();
            for (i, r) in recs.iter().enumerate() {
                s.set_u16(base + 6 * i, r.0);
                s.set_u16(base + 6 * i + 2, r.1);
                s.set_u16(base + 6 * i + 4, r.2);
            }
        }
        true
    }
}

/// `Coverage::subset` (Coverage.hh#L192-L211): the retained glyphs of the coverage under
/// `glyph_map_gsub`, serialized in place. Returns whether any glyph is left.
pub(crate) fn coverage_subset(
    s: &mut Serializer,
    cov: Coverage<'_>,
    num_source_glyphs: u32,
    glyph_map_gsub: &HashMap<u32, u32>,
) -> bool {
    // `iter () | hb_take (num_glyphs) | hb_map_retains_sorting (glyph_map_gsub) | hb_filter (!= INVALID)`
    let glyphs: Vec<u32> = cov
        .iter()
        .into_iter()
        .take(num_source_glyphs as usize)
        .map(|g| glyph_map_gsub.get(&g).copied().unwrap_or(INVALID))
        .filter(|&g| g != INVALID)
        .collect();
    coverage_serialize(s, &glyphs);
    !glyphs.is_empty()
}

impl Serializer {
    /// `OffsetTo::serialize_subset`: `*this = 0; push (); ret = f (); if (ret || !has_null)
    /// add_link (pop_pack ()) else pop_discard ()`. `pos` is the offset field in the current object.
    pub(crate) fn serialize_subset(
        &mut self,
        pos: usize,
        width: u8,
        has_null: bool,
        f: impl FnOnce(&mut Serializer) -> bool,
    ) -> bool {
        self.zero_field(pos, width);
        self.push();
        let ret = f(self);
        if ret || !has_null {
            let idx = self.pop_pack(true);
            self.add_link(pos, width, idx, Whence::Head, 0);
        } else {
            self.pop_discard();
        }
        ret
    }

    /// `OffsetTo::serialize_serialize`: the new object is packed only when `f` succeeds.
    pub(crate) fn serialize_serialize(
        &mut self,
        pos: usize,
        width: u8,
        f: impl FnOnce(&mut Serializer) -> bool,
    ) -> bool {
        self.zero_field(pos, width);
        self.push();
        let ret = f(self);
        if ret {
            let idx = self.pop_pack(true);
            self.add_link(pos, width, idx, Whence::Head, 0);
        } else {
            self.pop_discard();
        }
        ret
    }

    /// `*this = 0` for an offset field.
    pub(crate) fn zero_field(&mut self, pos: usize, width: u8) {
        let w = usize::from(width);
        if let Some(b) = self.bytes_mut().get_mut(pos..pos + w) {
            b.fill(0);
        }
    }

    /// `ArrayOf::serialize_append` for an array whose length is at `len_pos` and which ends the
    /// current object: increments the length and allocates the item. Returns the item position.
    pub(crate) fn array_append(&mut self, len_pos: usize, item_size: usize) -> usize {
        let len = u16_at(self.bytes(), len_pos);
        let pos = self.allocate(item_size);
        debug_assert_eq!(pos, len_pos + 2 + usize::from(len) * item_size);
        self.set_u16(len_pos, len.wrapping_add(1));
        pos
    }

    /// `ArrayOf::pop ()`.
    pub(crate) fn array_pop(&mut self, len_pos: usize) {
        let len = u16_at(self.bytes(), len_pos);
        self.set_u16(len_pos, len.wrapping_sub(1));
    }
}

/// Port of `ClassDef` reading (hb-ot-layout-common.hh#L1500-L2280), formats 1 and 2.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ClassDef<'a>(pub View<'a>);

impl ClassDef<'_> {
    pub(crate) fn format(&self) -> u32 {
        self.0.u16(0)
    }

    /// `ClassDef::get_class`.
    pub(crate) fn get_class(&self, g: u32) -> u32 {
        let v = self.0;
        match self.format() {
            1 => {
                // classValue[(unsigned) (glyph_id - startGlyph)]
                let idx = g.wrapping_sub(v.u16(2));
                if idx < v.u16(4) {
                    v.u16(6 + 2 * idx as usize)
                } else {
                    0
                }
            }
            2 => {
                let len = v.u16(2) as usize;
                let found = crate::bytes::bsearch(len, |i| {
                    let r = 4 + 6 * i;
                    if g < v.u16(r) {
                        std::cmp::Ordering::Less
                    } else if g <= v.u16(r + 2) {
                        std::cmp::Ordering::Equal
                    } else {
                        std::cmp::Ordering::Greater
                    }
                });
                found.map_or(0, |i| v.u16(4 + 6 * i + 4))
            }
            _ => 0,
        }
    }

    /// `ClassDef::get_population`.
    pub(crate) fn get_population(&self) -> u32 {
        let v = self.0;
        match self.format() {
            1 => v.u16(4),
            2 => {
                let mut ret: u64 = 0;
                for i in 0..v.u16(2) as usize {
                    let r = 4 + 6 * i;
                    let (first, last) = (v.u16(r), v.u16(r + 2));
                    if last >= first {
                        ret += u64::from(last - first + 1);
                    }
                }
                ret.min(u64::from(u32::MAX)) as u32
            }
            _ => NOT_COVERED,
        }
    }

    /// `ClassDef::intersects`.
    pub(crate) fn intersects(&self, glyphs: &BTreeSet<u32>) -> bool {
        let v = self.0;
        match self.format() {
            1 => {
                let start = v.u16(2);
                let end = start + v.u16(4);
                let mut iter = start.wrapping_sub(1);
                while let Some(n) = set_next(glyphs, iter) {
                    if n >= end {
                        break;
                    }
                    if v.u16(6 + 2 * (n - start) as usize) != 0 {
                        return true;
                    }
                    iter = n;
                }
                false
            }
            2 => {
                let len = v.u16(2);
                if len > glyphs.len() as u32 * bit_storage(len) {
                    return glyphs.iter().any(|&g| self.get_class(g) != 0);
                }
                (0..len as usize).any(|i| {
                    let r = 4 + 6 * i;
                    set_intersects_range(glyphs, v.u16(r), v.u16(r + 2)) && v.u16(r + 4) != 0
                })
            }
            _ => false,
        }
    }

    /// `ClassDef::intersects_class`.
    pub(crate) fn intersects_class(&self, glyphs: &BTreeSet<u32>, klass: u32) -> bool {
        let v = self.0;
        match self.format() {
            1 => {
                let count = v.u16(4);
                let start = v.u16(2);
                if klass == 0 {
                    let Some(g) = set_next(glyphs, INVALID) else {
                        return false;
                    };
                    if g < start {
                        return true;
                    }
                    let g = start.wrapping_add(count).wrapping_sub(1);
                    if set_next(glyphs, g).is_some() {
                        return true;
                    }
                }
                (0..count as usize)
                    .any(|i| v.u16(6 + 2 * i) == klass && glyphs.contains(&(start + i as u32)))
            }
            2 => {
                let len = v.u16(2) as usize;
                if klass == 0 {
                    let mut g = INVALID;
                    let mut last = INVALID;
                    // The C++ loop advances `it` only in the skip branch.
                    let mut it_idx = 0usize;
                    for i in 0..len {
                        let r = 4 + 6 * i;
                        if v.u16(4 + 6 * it_idx) == last.wrapping_add(1) {
                            it_idx += 1;
                            continue;
                        }
                        match set_next(glyphs, g) {
                            None => {
                                g = INVALID;
                                break;
                            }
                            Some(n) => g = n,
                        }
                        if g < v.u16(r) {
                            return true;
                        }
                        g = v.u16(r + 2);
                        last = g;
                    }
                    if g != INVALID && set_next(glyphs, g).is_some() {
                        return true;
                    }
                }
                (0..len).any(|i| {
                    let r = 4 + 6 * i;
                    v.u16(r + 4) == klass && set_intersects_range(glyphs, v.u16(r), v.u16(r + 2))
                })
            }
            _ => false,
        }
    }

    /// `ClassDef::intersected_class_glyphs`.
    pub(crate) fn intersected_class_glyphs(
        &self,
        glyphs: &BTreeSet<u32>,
        klass: u32,
        out: &mut BTreeSet<u32>,
    ) {
        let v = self.0;
        match self.format() {
            1 => {
                let count = v.u16(4);
                let start = v.u16(2);
                if klass == 0 {
                    let mut g = INVALID;
                    while let Some(n) = set_next(glyphs, g) {
                        if n >= start {
                            break;
                        }
                        out.insert(n);
                        g = n;
                    }
                    let mut g = start.wrapping_add(count).wrapping_sub(1);
                    while let Some(n) = set_next(glyphs, g) {
                        out.insert(n);
                        g = n;
                    }
                    return;
                }
                for i in 0..count as usize {
                    if v.u16(6 + 2 * i) == klass && glyphs.contains(&(start + i as u32)) {
                        out.insert(start + i as u32);
                    }
                }
            }
            2 => {
                let len = v.u16(2) as usize;
                if klass == 0 {
                    let mut g = INVALID;
                    'outer: {
                        for i in 0..len {
                            let r = 4 + 6 * i;
                            match set_next(glyphs, g) {
                                None => break 'outer,
                                Some(n) => g = n,
                            }
                            while g < v.u16(r) {
                                out.insert(g);
                                match set_next(glyphs, g) {
                                    None => break 'outer,
                                    Some(n) => g = n,
                                }
                            }
                            g = v.u16(r + 2);
                        }
                        while let Some(n) = set_next(glyphs, g) {
                            out.insert(n);
                            g = n;
                        }
                    }
                    return;
                }
                if len as u32 > glyphs.len() as u32 * bit_storage(len as u32) {
                    for &g in glyphs {
                        let found = crate::bytes::bsearch(len, |i| {
                            let r = 4 + 6 * i;
                            if g < v.u16(r) {
                                std::cmp::Ordering::Less
                            } else if g <= v.u16(r + 2) {
                                std::cmp::Ordering::Equal
                            } else {
                                std::cmp::Ordering::Greater
                            }
                        });
                        if let Some(i) = found
                            && v.u16(4 + 6 * i + 4) == klass
                        {
                            out.insert(g);
                        }
                    }
                    return;
                }
                for i in 0..len {
                    let r = 4 + 6 * i;
                    if v.u16(r + 4) != klass {
                        continue;
                    }
                    let end = v.u16(r + 2) + 1;
                    let mut g = v.u16(r).wrapping_sub(1);
                    while let Some(n) = set_next(glyphs, g) {
                        if n >= end {
                            break;
                        }
                        out.insert(n);
                        g = n;
                    }
                }
            }
            _ => {}
        }
    }

    /// `ClassDef::intersected_classes`.
    pub(crate) fn intersected_classes(&self, glyphs: &BTreeSet<u32>, out: &mut BTreeSet<u32>) {
        let v = self.0;
        match self.format() {
            1 => {
                if glyphs.is_empty() {
                    return;
                }
                let start = v.u16(2);
                let count = v.u16(4);
                let end_glyph = start.wrapping_add(count).wrapping_sub(1);
                if glyphs.first().copied().unwrap_or(0) < start
                    || glyphs.last().copied().unwrap_or(0) > end_glyph
                {
                    out.insert(0);
                }
                for i in 0..count as usize {
                    if glyphs.contains(&(start + i as u32)) {
                        out.insert(v.u16(6 + 2 * i));
                    }
                }
            }
            2 => {
                if glyphs.is_empty() {
                    return;
                }
                let len = v.u16(2) as usize;
                let mut g = INVALID;
                for i in 0..len {
                    let r = 4 + 6 * i;
                    match set_next(glyphs, g) {
                        None => {
                            g = INVALID;
                            break;
                        }
                        Some(n) => g = n,
                    }
                    if g < v.u16(r) {
                        out.insert(0);
                        break;
                    }
                    g = v.u16(r + 2);
                }
                if g != INVALID && set_next(glyphs, g).is_some() {
                    out.insert(0);
                }
                for i in 0..len {
                    let r = 4 + 6 * i;
                    if set_intersects_range(glyphs, v.u16(r), v.u16(r + 2)) {
                        out.insert(v.u16(r + 4));
                    }
                }
            }
            _ => {}
        }
    }
}

/// The arguments of `ClassDef::subset`.
pub(crate) struct ClassDefSubsetArgs<'x, 'a> {
    pub klass_map: Option<&'x mut HashMap<u32, u32>>,
    pub keep_empty_table: bool,
    pub use_class_zero: bool,
    pub glyph_filter: Option<Coverage<'a>>,
}

impl Default for ClassDefSubsetArgs<'_, '_> {
    fn default() -> Self {
        ClassDefSubsetArgs {
            klass_map: None,
            keep_empty_table: true,
            use_class_zero: true,
            glyph_filter: None,
        }
    }
}

/// The plan data `ClassDef::subset` reads.
pub(crate) struct ClassDefPlan<'p> {
    pub glyph_map_gsub: &'p HashMap<u32, u32>,
    pub glyphset_gsub: &'p BTreeSet<u32>,
    pub num_source_glyphs: u32,
}

/// `ClassDef_serialize` (hb-ot-layout-common.hh#L2279-L2284) with `ClassDef::serialize`
/// (L2057-L2116): `it` holds `(glyph, class)` sorted by glyph.
pub(crate) fn classdef_serialize(s: &mut Serializer, it_with_class_zero: &[(u32, u32)]) -> bool {
    let start = s.allocate(2);
    let it: Vec<(u32, u32)> = it_with_class_zero
        .iter()
        .copied()
        .filter(|p| p.1 != 0)
        .collect();
    let mut format = 2u32;
    let mut glyph_max = 0u32;
    if !it.is_empty() {
        let glyph_min = it[0].0;
        glyph_max = glyph_min;
        let mut num_glyphs = 0u32;
        let mut num_ranges = 1u32;
        let mut prev_gid = glyph_min;
        let mut prev_klass = it[0].1;
        for &(cur_gid, cur_klass) in &it {
            num_glyphs += 1;
            if cur_gid == glyph_min {
                continue;
            }
            if cur_gid > glyph_max {
                glyph_max = cur_gid;
            }
            if cur_gid != prev_gid + 1 || cur_klass != prev_klass {
                num_ranges += 1;
            }
            prev_gid = cur_gid;
            prev_klass = cur_klass;
        }
        if num_glyphs != 0 && (glyph_max - glyph_min + 1) < num_ranges * 3 {
            format = 1;
        }
    }
    if glyph_max > 0xFFFF {
        s.err(ERROR_INT_OVERFLOW);
        return false;
    }
    s.set_u16(start, format as u16);
    if format == 1 {
        // `ClassDefFormat1_3::serialize`
        if it.is_empty() {
            s.embed_u16(0); // startGlyph
            s.embed_u16(0); // classValue.len
            return true;
        }
        let glyph_min = it[0].0;
        let glyph_max = it.iter().map(|p| p.0).fold(0u32, u32::max);
        let glyph_count = glyph_max - glyph_min + 1;
        s.embed_u16(glyph_min as u16);
        s.embed_u16(glyph_count as u16);
        let base = s.allocate(2 * glyph_count as usize);
        for &(gid, klass) in &it {
            let idx = (gid - glyph_min) as usize;
            s.set_u16(base + 2 * idx, klass as u16);
        }
        true
    } else {
        // `ClassDefFormat2_4::serialize`
        if it.is_empty() {
            s.embed_u16(0);
            return true;
        }
        let len_pos = s.embed_u16(0);
        let mut unsorted = false;
        let mut num_ranges = 1u32;
        let mut prev_gid = it[0].0;
        let mut prev_klass = it[0].1;
        let mut rec = s.allocate(6);
        s.set_u16(rec, prev_gid as u16);
        s.set_u16(rec + 2, prev_gid as u16);
        s.set_u16(rec + 4, prev_klass as u16);
        for &(cur_gid, cur_klass) in &it[1..] {
            if cur_gid != prev_gid.wrapping_add(1) || cur_klass != prev_klass {
                if cur_gid < prev_gid {
                    unsorted = true;
                }
                s.set_u16(rec + 2, prev_gid as u16);
                num_ranges += 1;
                rec = s.allocate(6);
                s.set_u16(rec, cur_gid as u16);
                s.set_u16(rec + 2, cur_gid as u16);
                s.set_u16(rec + 4, cur_klass as u16);
            }
            prev_klass = cur_klass;
            prev_gid = cur_gid;
        }
        s.set_u16(rec + 2, prev_gid as u16);
        s.set_u16(len_pos, num_ranges as u16);
        if unsorted {
            let first = len_pos + 2;
            let mut recs: Vec<(u16, u16, u16)> = (0..num_ranges as usize)
                .map(|i| {
                    let b = s.bytes();
                    (
                        u16_at(b, first + 6 * i),
                        u16_at(b, first + 6 * i + 2),
                        u16_at(b, first + 6 * i + 4),
                    )
                })
                .collect();
            recs.sort_unstable();
            for (i, r) in recs.iter().enumerate() {
                s.set_u16(first + 6 * i, r.0);
                s.set_u16(first + 6 * i + 2, r.1);
                s.set_u16(first + 6 * i + 4, r.2);
            }
        }
        true
    }
}

/// `ClassDef_remap_and_serialize` (hb-ot-layout-common.hh#L1462-L1494).
fn classdef_remap_and_serialize(
    s: &mut Serializer,
    klasses: &BTreeSet<u32>,
    use_class_zero: bool,
    glyph_and_klass: &mut [(u32, u32)],
    klass_map: Option<&mut HashMap<u32, u32>>,
) -> bool {
    let Some(klass_map) = klass_map else {
        return classdef_serialize(s, glyph_and_klass);
    };
    if !use_class_zero {
        klass_map.insert(0, 0);
    }
    let mut idx = u32::from(klass_map.contains_key(&0));
    for &k in klasses {
        if klass_map.contains_key(&k) {
            continue;
        }
        klass_map.insert(k, idx);
        idx += 1;
    }
    for p in glyph_and_klass.iter_mut() {
        p.1 = klass_map.get(&p.1).copied().unwrap_or(INVALID);
    }
    classdef_serialize(s, glyph_and_klass)
}

/// `ClassDef::subset` for formats 1 and 2 (hb-ot-layout-common.hh#L1546-L1595, L1798-L1871).
pub(crate) fn classdef_subset(
    s: &mut Serializer,
    cd: ClassDef<'_>,
    plan: &ClassDefPlan<'_>,
    args: ClassDefSubsetArgs<'_, '_>,
) -> bool {
    let v = cd.0;
    let ClassDefSubsetArgs {
        klass_map,
        keep_empty_table,
        mut use_class_zero,
        glyph_filter,
    } = args;
    let mut glyph_and_klass: Vec<(u32, u32)> = Vec::new();
    let mut orig_klasses: BTreeSet<u32> = BTreeSet::new();
    match cd.format() {
        1 => {
            let start = v.u16(2);
            let end = start + v.u16(4);
            for gid in start..end {
                let new_gid = plan.glyph_map_gsub.get(&gid).copied().unwrap_or(INVALID);
                if new_gid == INVALID {
                    continue;
                }
                if let Some(f) = &glyph_filter
                    && !f.has(gid)
                {
                    continue;
                }
                let klass = v.u16(6 + 2 * (gid - start) as usize);
                if klass == 0 {
                    continue;
                }
                glyph_and_klass.push((new_gid, klass));
                orig_klasses.insert(klass);
            }
            if use_class_zero {
                let glyph_count = match &glyph_filter {
                    Some(f) => plan.glyph_map_gsub.keys().filter(|&&g| f.has(g)).count(),
                    None => plan.glyph_map_gsub.len(),
                };
                use_class_zero = glyph_count <= glyph_and_klass.len();
            }
        }
        2 => {
            let len = v.u16(2);
            if plan.glyphset_gsub.len() as u32 * bit_storage(len) < cd.get_population() {
                for &g in plan.glyphset_gsub {
                    let klass = cd.get_class(g);
                    if klass == 0 {
                        continue;
                    }
                    let new_gid = plan.glyph_map_gsub.get(&g).copied().unwrap_or(INVALID);
                    if new_gid == INVALID {
                        continue;
                    }
                    if let Some(f) = &glyph_filter
                        && !f.has(g)
                    {
                        continue;
                    }
                    glyph_and_klass.push((new_gid, klass));
                    orig_klasses.insert(klass);
                }
            } else {
                for i in 0..len as usize {
                    let r = 4 + 6 * i;
                    let klass = v.u16(r + 4);
                    if klass == 0 {
                        continue;
                    }
                    let start = v.u16(r);
                    let end = (v.u16(r + 2) + 1).min(plan.num_source_glyphs);
                    for g in start..end {
                        let new_gid = plan.glyph_map_gsub.get(&g).copied().unwrap_or(INVALID);
                        if new_gid == INVALID {
                            continue;
                        }
                        if let Some(f) = &glyph_filter
                            && !f.has(g)
                        {
                            continue;
                        }
                        glyph_and_klass.push((new_gid, klass));
                        orig_klasses.insert(klass);
                    }
                }
            }
            let glyph_count = match &glyph_filter {
                Some(f) => plan.glyphset_gsub.iter().filter(|&&g| f.has(g)).count(),
                None => plan.glyph_map_gsub.len(),
            };
            use_class_zero = use_class_zero && glyph_count <= glyph_and_klass.len();
        }
        _ => return false,
    }
    let non_empty = !glyph_and_klass.is_empty();
    if !classdef_remap_and_serialize(
        s,
        &orig_klasses,
        use_class_zero,
        &mut glyph_and_klass,
        klass_map,
    ) {
        return false;
    }
    keep_empty_table || non_empty
}

/// `HintingDevice::get_size` (hb-ot-layout-common.hh#L4799-L4803).
pub(crate) fn hinting_device_size(dev: View<'_>) -> usize {
    let (start_size, end_size, f) = (dev.u16(0), dev.u16(2), dev.u16(4));
    if !(1..=3).contains(&f) || start_size > end_size {
        return 6;
    }
    2 * (4 + ((end_size - start_size) >> (4 - f)) as usize)
}

/// `Device::copy (c, layout_variation_idx_delta_map)` (hb-ot-layout-common.hh#L4986-L5006) with
/// an empty map: hinting devices are copied, variation devices are dropped. Returns whether the
/// copy was made.
pub(crate) fn device_copy(s: &mut Serializer, dev: View<'_>) -> bool {
    match dev.u16(4) {
        1..=3 => {
            let size = hinting_device_size(dev);
            let mut bytes = dev.d[..size.min(dev.d.len())].to_vec();
            bytes.resize(size, 0);
            s.embed(&bytes);
            true
        }
        _ => false,
    }
}

/// `OffsetTo::serialize_subset` for the offset field `field` of `parent`, written at `pos` of the
/// current object: `*this = 0; if (src.is_null ()) return false; ...`.
pub(crate) fn offset_subset(
    s: &mut Serializer,
    pos: usize,
    parent: View<'_>,
    field: usize,
    f: impl FnOnce(&mut Serializer, View<'_>) -> bool,
) -> bool {
    if parent.is_null16(field) {
        s.zero_field(pos, 2);
        return false;
    }
    let child = parent.off16(field);
    s.serialize_subset(pos, 2, true, |s| f(s, child))
}

/// `OffsetTo<Device>::serialize_copy (c, src, base, 0, Head)` with the offset field `field` of
/// `parent`, written at `pos` of the current object: the new object is linked even when the copy
/// produced nothing (an empty object has no index and no link).
pub(crate) fn serialize_copy_device(
    s: &mut Serializer,
    pos: usize,
    parent: View<'_>,
    field: usize,
) -> bool {
    s.zero_field(pos, 2);
    if parent.is_null16(field) {
        return false;
    }
    s.push();
    let ret = device_copy(s, parent.off16(field));
    let idx = s.pop_pack(true);
    s.add_link(pos, 2, idx, Whence::Head, 0);
    ret
}

/// `OffsetTo<...>::serialize_subset` for an offset field of `width` bytes (2, 3 or 4) of `parent`.
pub(crate) fn offset_subset_w(
    s: &mut Serializer,
    pos: usize,
    width: u8,
    parent: View<'_>,
    field: usize,
    f: impl FnOnce(&mut Serializer, View<'_>) -> bool,
) -> bool {
    let off = match width {
        2 => parent.u16(field),
        3 => parent.u24(field),
        _ => parent.u32(field),
    } as usize;
    if off == 0 {
        s.zero_field(pos, width);
        return false;
    }
    let child = parent.sub(off);
    s.serialize_subset(pos, width, true, |s| f(s, child))
}
