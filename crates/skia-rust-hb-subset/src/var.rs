// Copyright © 2019  Adobe Inc.
// Copyright © 2019  Ebrahim Byagowi
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-var-gvar-table.hh (harfbuzz 9cb1fee5)

//! The variation tables: `gvar` without instancing (the glyph variation data is copied for the
//! retained glyphs).

use crate::ot::View;
use crate::plan::Plan;
use crate::{FLAG_NOTDEF_OUTLINE, Res, SubsetError};

/// `gvar_GVAR::get_glyph_var_data_bytes` (hb-ot-var-gvar-table.hh#L560-L574): the range of the
/// variation data of the glyph in the table.
fn glyph_var_data(gvar: View<'_>, glyph_count: u32, glyph: u32) -> &[u8] {
    let offset = |i: u32| -> u32 {
        if i > glyph_count {
            return 0;
        }
        if gvar.u16(14) & 1 != 0 {
            gvar.u32(20 + 4 * i as usize)
        } else {
            gvar.u16(20 + 2 * i as usize) * 2
        }
    };
    let start_offset = offset(glyph);
    let end_offset = offset(glyph.wrapping_add(1));
    if end_offset < start_offset {
        return &[];
    }
    let length = (end_offset - start_offset) as usize;
    // `hb_array_t::sub_array`
    let begin = gvar.u32(16).wrapping_add(start_offset) as usize;
    let data = gvar.d.get(begin..).unwrap_or(&[]);
    let data = &data[..length.min(data.len())];
    // `GlyphVariationData::min_size`
    if data.len() >= 4 { data } else { &[] }
}

/// `gvar::sanitize_shallow` (hb-ot-var-gvar-table.hh#L206-L217) for the checks that bear on a
/// well-formed font.
fn sanitize_shallow(gvar: View<'_>, num_glyphs: u32) -> bool {
    if gvar.d.len() < 20 || gvar.u16(0) != 1 {
        return false;
    }
    let tuples = u64::from(gvar.u16(4)) * u64::from(gvar.u16(6));
    let shared = u64::from(gvar.u32(8));
    if shared != 0 && shared + tuples * 2 > gvar.d.len() as u64 {
        return false;
    }
    let width = if gvar.u16(14) & 1 != 0 { 4 } else { 2 };
    20 + (u64::from(num_glyphs) + 1) * width <= gvar.d.len() as u64
}

/// `gvar_GVAR::subset` (hb-ot-var-gvar-table.hh#L384-L497) without instancing.
pub(crate) fn subset_gvar(plan: &mut Plan<'_>) -> Res<bool> {
    let t = crate::bytes::tag(b"gvar");
    let data = plan.source.table(t);
    let gvar = View::new(data);
    if data.is_empty() || !sanitize_shallow(gvar, plan.source.num_glyphs()) {
        return Err(SubsetError::Failed);
    }
    let glyph_count = if gvar.u32(0) != 0 {
        plan.source.num_glyphs()
    } else {
        0
    };
    let num_glyphs = plan.num_output_glyphs;
    let axis_count = gvar.u16(4);
    let shared_tuple_count = gvar.u16(6);

    let mut it: &[(u32, u32)] = &plan.new_to_old_gid_list;
    if it.first().is_some_and(|p| p.0 == 0) && plan.flags & FLAG_NOTDEF_OUTLINE == 0 {
        it = &it[1..];
    }
    let mut subset_data_size = 0u32;
    let mut padding_size = 0u32;
    for &(_, old_gid) in it {
        let mut glyph_data_size = glyph_var_data(gvar, glyph_count, old_gid).len() as u32;
        if !glyph_data_size.is_multiple_of(2) {
            glyph_data_size += 1;
            padding_size += 1;
        }
        subset_data_size += glyph_data_size;
    }
    let long_offset = subset_data_size > 0x1FFFE;

    let mut out: Vec<u8> = vec![0; 20];
    out[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
    out[4..6].copy_from_slice(&(axis_count as u16).to_be_bytes());
    out[6..8].copy_from_slice(&(shared_tuple_count as u16).to_be_bytes());
    out[12..14].copy_from_slice(&(num_glyphs.min(0xFFFF) as u16).to_be_bytes());
    out[14..16].copy_from_slice(&u16::from(long_offset).to_be_bytes());

    let offsets_pos = out.len();
    let offsets_size = (if long_offset { 4 } else { 2 }) * (num_glyphs as usize + 1);
    out.resize(offsets_pos + offsets_size, 0);

    let shared_offset = gvar.u32(8);
    if shared_tuple_count == 0 || shared_offset == 0 {
        // `out->sharedTuples = 0`
    } else {
        let size = 2 * axis_count as usize * shared_tuple_count as usize;
        let at = out.len() as u32;
        out[8..12].copy_from_slice(&at.to_be_bytes());
        let src = gvar.sub(shared_offset as usize).d;
        let mut tuples: Vec<u8> = src.iter().copied().take(size).collect();
        tuples.resize(size, 0);
        out.extend_from_slice(&tuples);
    }
    if long_offset {
        subset_data_size -= padding_size;
    }
    let at = out.len() as u32;
    out[16..20].copy_from_slice(&at.to_be_bytes());

    // The offsets array: entry 0 is 0; entry `g + 1` is the end of glyph `g`.
    let mut offsets: Vec<u32> = vec![0; num_glyphs as usize + 1];
    let mut glyph_offset = 0u32;
    let mut last = 0u32;
    let divisor = if long_offset { 1 } else { 2 };
    let mut body: Vec<u8> = Vec::with_capacity(subset_data_size as usize);
    for &(gid, old_gid) in it {
        while last < gid {
            offsets[last as usize + 1] = glyph_offset / divisor;
            last += 1;
        }
        let var_data = glyph_var_data(gvar, glyph_count, old_gid);
        body.extend_from_slice(var_data);
        glyph_offset += var_data.len() as u32;
        if !long_offset && !var_data.len().is_multiple_of(2) {
            body.push(0);
            glyph_offset += 1;
        }
        offsets[gid as usize + 1] = glyph_offset / divisor;
        last += 1;
    }
    while last < num_glyphs {
        offsets[last as usize + 1] = glyph_offset / divisor;
        last += 1;
    }
    for (i, &o) in offsets.iter().enumerate() {
        if long_offset {
            out[offsets_pos + 4 * i..offsets_pos + 4 * i + 4].copy_from_slice(&o.to_be_bytes());
        } else {
            out[offsets_pos + 2 * i..offsets_pos + 2 * i + 2]
                .copy_from_slice(&(o as u16).to_be_bytes());
        }
    }
    out.extend_from_slice(&body);
    plan.add_table(t, out);
    Ok(true)
}
