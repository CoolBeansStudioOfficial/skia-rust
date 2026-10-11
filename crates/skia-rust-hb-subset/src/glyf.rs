// Copyright © 2015  Google, Inc.
// Copyright © 2019  Adobe, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/glyf/{glyf,Glyph,SubsetGlyph,glyf-helpers,loca,GlyphHeader,SimpleGlyph,CompositeGlyph,composite-iter}.hh (harfbuzz 9cb1fee5)

//! `glyf` and `loca` subsetting, without instancing: Skia never passes axis locations, so the
//! `normalized_coords` branches (`compile_bytes_with_deltas`, `update_mtx`, the `head`/`maxp`
//! recomputation) are unreachable and are not ported.

use crate::bytes::{i16_at, tag, u16_at, u24_at, u32_at};
use crate::plan::Plan;
use crate::sfnt::{Face, head_ok};
use crate::{Res, SubsetError};

const TAG_HEAD: u32 = tag(b"head");
const TAG_LOCA: u32 = tag(b"loca");
const TAG_GLYF: u32 = tag(b"glyf");

const FLAG_X_SHORT: u8 = 0x02;
const FLAG_Y_SHORT: u8 = 0x04;
const FLAG_REPEAT: u8 = 0x08;
const FLAG_X_SAME: u8 = 0x10;
const FLAG_Y_SAME: u8 = 0x20;

const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_AND_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
const GID_IS_24BIT: u16 = 0x2000;

/// `GlyphHeader::static_size`.
const GLYPH_HEADER_SIZE: usize = 10;
/// `CompositeGlyphRecord::min_size`: flags and a 16-bit glyph index.
const COMPOSITE_MIN_SIZE: usize = 4;

/// Port of `glyf_impl::Glyph::glyph_type_t` (Glyph.hh#L31-L35).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum GlyphType {
    Empty,
    Simple,
    Composite,
}

/// Port of `glyf_impl::Glyph` (Glyph.hh#L37-L580), the parts reachable without instancing.
#[derive(Clone, Copy, Debug)]
struct Glyph<'a> {
    bytes: &'a [u8],
    glyph_type: GlyphType,
}

impl<'a> Glyph<'a> {
    /// Port of `Glyph (hb_bytes_t, gid)` (Glyph.hh#L556-L568). Too few bytes for a header read the
    /// `Null` header, whose `numberOfContours` is zero.
    fn new(bytes: &'a [u8]) -> Self {
        let num_contours = if bytes.len() >= GLYPH_HEADER_SIZE {
            i16_at(bytes, 0)
        } else {
            0
        };
        let glyph_type = match num_contours {
            0 => GlyphType::Empty,
            1.. => GlyphType::Simple,
            _ => GlyphType::Composite,
        };
        Glyph { bytes, glyph_type }
    }

    /// Port of `Glyph::trim_padding` (Glyph.hh#L49-L58) with `SimpleGlyph::trim_padding`
    /// (SimpleGlyph.hh#L53-L97). Composite glyphs are not trimmed.
    fn trim_padding(&self) -> &'a [u8] {
        match self.glyph_type {
            GlyphType::Simple => simple_trim_padding(self.bytes),
            GlyphType::Composite | GlyphType::Empty => self.bytes,
        }
    }
}

/// Port of `SimpleGlyph::trim_padding` (SimpleGlyph.hh#L53-L97), based on fontTools'
/// `_g_l_y_f.py::trim`.
fn simple_trim_padding(bytes: &[u8]) -> &[u8] {
    let number_of_contours = i16_at(bytes, 0);
    let end = bytes.len();
    // `glyph += instruction_len_offset ()`
    let mut glyph = (GLYPH_HEADER_SIZE as isize + 2 * isize::from(number_of_contours)) as usize;
    if glyph + 2 >= end {
        return &[];
    }
    let num_coordinates = usize::from(u16_at(bytes, glyph - 2)) + 1;
    let num_instructions = usize::from(u16_at(bytes, glyph));
    glyph += 2 + num_instructions;

    let mut coord_bytes = 0usize;
    let mut coords_with_flags = 0usize;
    while glyph < end {
        let flag = bytes[glyph];
        glyph += 1;
        let mut repeat = 1usize;
        if flag & FLAG_REPEAT != 0 {
            if glyph >= end {
                return &[];
            }
            repeat = usize::from(bytes[glyph]) + 1;
            glyph += 1;
        }
        let mut x_bytes = 0;
        let mut y_bytes = 0;
        if flag & FLAG_X_SHORT != 0 {
            x_bytes = 1;
        } else if flag & FLAG_X_SAME == 0 {
            x_bytes = 2;
        }
        if flag & FLAG_Y_SHORT != 0 {
            y_bytes = 1;
        } else if flag & FLAG_Y_SAME == 0 {
            y_bytes = 2;
        }
        coord_bytes += (x_bytes + y_bytes) * repeat;
        coords_with_flags += repeat;
        if coords_with_flags >= num_coordinates {
            break;
        }
    }
    if coords_with_flags != num_coordinates {
        return &[];
    }
    // `bytes.sub_array (0, bytes.length + coord_bytes - (glyph_end - glyph))`
    let len = bytes.len() + coord_bytes - (end - glyph);
    &bytes[..len.min(bytes.len())]
}

/// Port of `CompositeGlyphRecord::get_size` (CompositeGlyph.hh#L36-L56). `rec` starts at the
/// record's flags.
fn composite_record_size(rec: &[u8]) -> usize {
    let flags = u16_at(rec, 0);
    let mut size = COMPOSITE_MIN_SIZE;
    // glyphIndex is 24 bit instead of 16 bit
    if flags & GID_IS_24BIT != 0 {
        size += 1;
    }
    if flags & ARG_1_AND_2_ARE_WORDS != 0 {
        size += 4;
    } else {
        size += 2;
    }
    if flags & WE_HAVE_A_SCALE != 0 {
        size += 2;
    } else if flags & WE_HAVE_AN_X_AND_Y_SCALE != 0 {
        size += 4;
    } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
        size += 8;
    }
    size
}

/// Port of `composite_iter_tmpl` (composite-iter.hh#L16-L66): the offsets of the records of a
/// composite glyph, each fully inside `glyph`.
fn composite_records(glyph: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut cur = GLYPH_HEADER_SIZE;
    loop {
        // `set_current`
        if cur
            .checked_add(COMPOSITE_MIN_SIZE)
            .is_none_or(|e| e > glyph.len())
        {
            break;
        }
        let size = composite_record_size(&glyph[cur..]);
        if cur + size > glyph.len() {
            break;
        }
        out.push((cur, size));
        // `__next__`
        if u16_at(glyph, cur) & MORE_COMPONENTS == 0 {
            break;
        }
        cur += size;
    }
    out
}

/// Port of `CompositeGlyphRecord::get_gid` (CompositeGlyph.hh#L269-L277).
fn composite_get_gid(glyph: &[u8], rec: usize) -> u32 {
    if u16_at(glyph, rec) & GID_IS_24BIT != 0 {
        u24_at(glyph, rec + 2)
    } else {
        u32::from(u16_at(glyph, rec + 2))
    }
}

/// Port of `CompositeGlyphRecord::set_gid` (CompositeGlyph.hh#L278-L287).
fn composite_set_gid(glyph: &mut [u8], rec: usize, gid: u32) {
    if u16_at(glyph, rec) & GID_IS_24BIT != 0 {
        glyph[rec + 2..rec + 5].copy_from_slice(&gid.to_be_bytes()[1..]);
    } else {
        glyph[rec + 2..rec + 4].copy_from_slice(&(gid as u16).to_be_bytes());
    }
}

/// Port of `glyf_accelerator_t` (glyf.hh#L116-L190, L338-L365).
pub(crate) struct GlyfAccelerator<'a> {
    short_offset: bool,
    num_glyphs: u32,
    loca: &'a [u8],
    glyf: &'a [u8],
}

impl<'a> GlyfAccelerator<'a> {
    /// Port of `glyf_accelerator_t::glyf_accelerator_t` (glyf.hh#L118-L152) and
    /// `glyf::has_valid_glyf_format` (glyf.hh#L47-L51).
    pub(crate) fn new(face: &Face<'a>) -> Self {
        let head = face.table(TAG_HEAD);
        // An unusable `head` reads as the `Null` head, whose formats are zero.
        let (loca_format, data_format) = if head_ok(head) {
            (u16_at(head, 50), u16_at(head, 52))
        } else {
            (0, 0)
        };
        let mut acc = GlyfAccelerator {
            short_offset: false,
            num_glyphs: 0,
            loca: &[],
            glyf: &[],
        };
        if loca_format > 1 || data_format > 1 {
            // Unknown format. Leave num_glyphs=0, that takes care of disabling us.
            return acc;
        }
        acc.short_offset = loca_format == 0;
        acc.loca = face.table(TAG_LOCA);
        acc.glyf = face.table(TAG_GLYF);
        let entry = if acc.short_offset { 2 } else { 4 };
        let n = ((acc.loca.len() / entry).max(1) - 1) as u32;
        acc.num_glyphs = n.min(face.num_glyphs());
        acc
    }

    pub(crate) fn has_data(&self) -> bool {
        self.num_glyphs != 0
    }

    /// Port of `glyph_for_gid` (glyf.hh#L303-L330): the glyph's bytes, or none when the `loca`
    /// range is invalid.
    fn glyph_bytes(&self, gid: u32) -> &'a [u8] {
        if gid >= self.num_glyphs {
            return &[];
        }
        let g = gid as usize;
        let (start, end) = if self.short_offset {
            (
                2 * usize::from(u16_at(self.loca, 2 * g)),
                2 * usize::from(u16_at(self.loca, 2 * g + 2)),
            )
        } else {
            (
                u32_at(self.loca, 4 * g) as usize,
                u32_at(self.loca, 4 * g + 4) as usize,
            )
        };
        if start > end || end > self.glyf.len() {
            return &[];
        }
        &self.glyf[start..end]
    }

    /// The gids a glyph references as components (`Glyph::get_composite_iterator`).
    pub(crate) fn component_gids(&self, gid: u32) -> Vec<u32> {
        let bytes = self.glyph_bytes(gid);
        let glyph = Glyph::new(bytes);
        if glyph.glyph_type != GlyphType::Composite {
            return Vec::new();
        }
        composite_records(bytes)
            .into_iter()
            .map(|(rec, _)| composite_get_gid(bytes, rec))
            .collect()
    }
}

/// Port of `glyf::subset` (glyf.hh#L83-L135) with `_populate_subset_glyphs` (glyf.hh#L452-L492),
/// `glyf::serialize` (glyf.hh#L62-L82) and `SubsetGlyph::serialize` (SubsetGlyph.hh#L24-L116).
/// Returns an error on failure and `Ok(false)` when the table is dropped.
pub(crate) fn subset(plan: &mut Plan<'_>) -> Res<bool> {
    let face = plan.source;
    // `_hb_subset_table`: a missing `glyf` table fails the subset.
    if face.table(TAG_GLYF).is_empty() {
        return Err(SubsetError::Failed);
    }
    let head = face.table(TAG_HEAD);
    let head_valid = head_ok(head);
    let (loca_format, data_format) = if head_valid {
        (u16_at(head, 50), u16_at(head, 52))
    } else {
        (0, 0)
    };
    if loca_format > 1 || data_format > 1 {
        // glyf format is unknown, don't attempt to subset it.
        return Ok(false);
    }
    let acc = GlyfAccelerator::new(face);

    // `_populate_subset_glyphs`
    let notdef_outline = plan.flags & crate::FLAG_NOTDEF_OUTLINE != 0;
    let mut glyphs: Vec<&[u8]> = Vec::with_capacity(plan.new_to_old_gid_list.len());
    for &(new_gid, old_gid) in &plan.new_to_old_gid_list {
        let bytes: &[u8] = if old_gid == 0 && new_gid == 0 && !notdef_outline {
            &[]
        } else {
            // `glyf.glyph_for_gid (old_gid, !plan->accelerator)`: the padding is removed.
            let g = Glyph::new(acc.glyph_bytes(old_gid));
            Glyph::new(g.trim_padding()).bytes
        };
        glyphs.push(bytes);
    }

    let mut padded_offsets: Vec<usize> = Vec::with_capacity(glyphs.len());
    let mut max_offset = 0usize;
    for g in &glyphs {
        let size = g.len() + g.len() % 2;
        padded_offsets.push(size);
        max_offset += size;
    }
    // `plan->force_long_loca` is not set by Skia.
    let use_short_loca = max_offset < 0x1FFFF;
    if !use_short_loca {
        padded_offsets = glyphs.iter().map(|g| g.len()).collect();
    }

    // `glyf::serialize`
    let mut table: Vec<u8> = Vec::new();
    for g in &glyphs {
        serialize_glyph(&mut table, g, use_short_loca, plan);
    }
    // As a special case when all glyph in the font are empty, add a zero byte to the table.
    if table.is_empty() {
        table.push(0);
    }

    // `_add_loca_and_head`
    let num_offsets = plan.num_output_glyphs as usize + 1;
    let loca = write_loca(
        &padded_offsets,
        &plan.new_to_old_gid_list,
        use_short_loca,
        num_offsets,
    );
    if !head_valid {
        return Err(SubsetError::Failed);
    }
    let mut head_prime = head.to_vec();
    head_prime[50..52].copy_from_slice(&(u16::from(!use_short_loca)).to_be_bytes());
    plan.add_table(TAG_GLYF, table);
    plan.add_table(TAG_LOCA, loca);
    plan.add_table(TAG_HEAD, head_prime);
    Ok(true)
}

/// Port of `SubsetGlyph::serialize` (SubsetGlyph.hh#L24-L116).
fn serialize_glyph(out: &mut Vec<u8>, src: &[u8], use_short_loca: bool, plan: &Plan<'_>) {
    let start = out.len();
    out.extend_from_slice(src);
    let pad_length = if use_short_loca { src.len() % 2 } else { 0 };
    out.resize(out.len() + pad_length, 0);
    if src.is_empty() {
        return;
    }
    let glyph = &mut out[start..start + src.len()];

    // update components gids.
    let records = if Glyph::new(glyph).glyph_type == GlyphType::Composite {
        composite_records(glyph)
    } else {
        Vec::new()
    };
    for &(rec, _) in &records {
        if let Some(&new_gid) = plan.glyph_map.get(&composite_get_gid(glyph, rec)) {
            composite_set_gid(glyph, rec, new_gid);
        }
    }

    // lower GID24 to GID16 in components if possible.
    if !records.is_empty() {
        let end = glyph.len();
        let mut p = records[0].0;
        let mut q = p;
        for &(rec, orig_size) in &records {
            q += orig_size;
            // `rec.lower_gid_24_to_16 ()`
            let flags = u16_at(glyph, rec);
            let gid = composite_get_gid(glyph, rec);
            if flags & GID_IS_24BIT != 0 && gid <= 0xFFFF {
                // Lower the flag and move the rest of the struct down.
                let rec_end = rec + orig_size;
                glyph[rec..rec + 2].copy_from_slice(&(flags & !GID_IS_24BIT).to_be_bytes());
                glyph[rec + 2..rec + 4].copy_from_slice(&(gid as u16).to_be_bytes());
                glyph.copy_within(rec + 5..rec_end, rec + 4);
            }
            let size = composite_record_size(&glyph[rec..]);
            glyph.copy_within(rec..rec + size, p);
            p += size;
        }
        glyph.copy_within(q..end, p);
        p += end - q;
        // fill the rest of the glyph with harmless instructions (ROFF)
        glyph[p..end].fill(0x7A);
    }
}

/// Port of `_write_loca` (glyf-helpers.hh#L20-L56).
fn write_loca(
    padded_offsets: &[usize],
    new_to_old_gid_list: &[(u32, u32)],
    short_offsets: bool,
    num_offsets: usize,
) -> Vec<u8> {
    let right_shift = u32::from(short_offsets);
    let mut values: Vec<u32> = Vec::with_capacity(num_offsets);
    let mut offset = 0usize;
    let mut value = 0u32;
    values.push(value);
    let mut last = 0u32;
    for (i, &(gid, _)) in new_to_old_gid_list.iter().enumerate() {
        while last < gid {
            values.push(value);
            last += 1;
        }
        offset += padded_offsets[i];
        value = (offset >> right_shift) as u32;
        values.push(value);
        last += 1; // Skip over gid
    }
    let num_glyphs = (num_offsets - 1) as u32;
    while last < num_glyphs {
        values.push(value);
        last += 1;
    }
    let mut out = Vec::with_capacity(values.len() * if short_offsets { 2 } else { 4 });
    for v in values {
        if short_offsets {
            out.extend_from_slice(&(v as u16).to_be_bytes());
        } else {
            out.extend_from_slice(&v.to_be_bytes());
        }
    }
    out
}
