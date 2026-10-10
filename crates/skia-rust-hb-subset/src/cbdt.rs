// Copyright © 2016  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Color/CBDT/CBDT.hh (harfbuzz 9cb1fee5)

//! `CBLC` and `CBDT`: the bitmap location table is subset and the bitmap data table is rebuilt
//! beside it.

use crate::Res;
use crate::bytes::tag;
use crate::ot::View;
use crate::plan::Plan;
use crate::serialize::{ObjIdx, Serializer, Whence};

/// `cblc_bitmap_size_subset_context_t`.
struct SizeCtx {
    size: u32,
    num_tables: u32,
    start_glyph: u32,
    end_glyph: u32,
}

/// `IndexSubtableArray::find_table`: the index of the record that holds the glyph.
fn find_table(array: View<'_>, glyph: u32, num_tables: u32) -> Option<usize> {
    (0..num_tables as usize).find(|&i| array.u16(8 * i) <= glyph && glyph <= array.u16(8 * i + 2))
}

/// `IndexSubtable::get_image_data` for the offsets of glyph `idx` of the subtable: the glyph's
/// range in the data (`offset`, `length`), `None` for a format that is not 1 or 3 or an empty
/// glyph.
fn get_image_data(subtable: View<'_>, idx: usize) -> Option<(u32, u32)> {
    let (cur, next) = match subtable.u16(0) {
        1 => (subtable.u32(8 + 4 * idx), subtable.u32(8 + 4 * (idx + 1))),
        3 => (subtable.u16(8 + 2 * idx), subtable.u16(8 + 2 * (idx + 1))),
        _ => return None,
    };
    if next <= cur {
        return None;
    }
    Some((subtable.u32(4).wrapping_add(cur), next - cur))
}

/// `IndexSubtableFormat1Or3::add_offset`.
fn add_offset(s: &mut Serializer, index_format: u32, offset: u32, size: &mut u32) -> bool {
    match index_format {
        1 => {
            *size += 4;
            s.embed_u32(offset);
            true
        }
        3 => {
            *size += 2;
            s.embed_u16(offset as u16);
            true
        }
        _ => false,
    }
}

/// `CBLC::subset` and `CBDT::sink`: returns the new `CBDT` through `cbdt_out`.
#[allow(clippy::unnecessary_wraps)] // the callback shape of `run_table`
pub(crate) fn subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    cblc: View<'_>,
    cbdt_out: &mut Option<Vec<u8>>,
) -> Res<bool> {
    let out = s.allocate(8);
    s.set_u32(out, cblc.u32(0));
    let cbdt_data = plan.source.table(tag(b"CBDT"));
    if cbdt_data.len() < 4 {
        return Ok(false);
    }
    let cbdt = View::new(cbdt_data);
    let mut cbdt_prime: Vec<u8> = cbdt_data[..4].to_vec();
    let count = cblc.u32(4) as usize;
    let mut len = 0u32;
    for i in 0..count.min(cblc.d.len() / 48 + 1) {
        let table = cblc.sub(8 + 48 * i);
        // `subset_size_table`
        len += 1;
        s.set_u32(out + 4, len);
        let snap = s.snapshot();
        let cbdt_prime_len = cbdt_prime.len();
        if !size_table_subset(plan, s, cblc, table, cbdt, &mut cbdt_prime) {
            len -= 1;
            s.set_u32(out + 4, len);
            s.revert(snap);
            cbdt_prime.truncate(cbdt_prime_len);
        }
    }
    *cbdt_out = Some(cbdt_prime);
    Ok(true)
}

/// `BitmapSizeTable::subset`.
fn size_table_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    cblc: View<'_>,
    table: View<'_>,
    cbdt: View<'_>,
    cbdt_prime: &mut Vec<u8>,
) -> bool {
    let bytes: Vec<u8> = (0..48)
        .map(|k| table.d.get(k).copied().unwrap_or(0))
        .collect();
    let out = s.embed(&bytes);
    let mut ctx = SizeCtx {
        size: table.u32(4),
        num_tables: table.u32(8),
        start_glyph: 1,
        end_glyph: 0,
    };
    // `NNOffset32To<IndexSubtableArray>::serialize_subset`: an offset of 0 is the CBLC itself.
    let array = cblc.sub(table.u32(0) as usize);
    if !s.serialize_subset(out, 4, false, |s| {
        index_subtable_array_subset(plan, s, array, &mut ctx, cbdt, cbdt_prime)
    }) {
        return false;
    }
    if ctx.size == 0 || ctx.num_tables == 0 || ctx.start_glyph > ctx.end_glyph {
        return false;
    }
    s.set_u32(out + 4, ctx.size);
    s.set_u32(out + 8, ctx.num_tables);
    s.set_u16(out + 40, ctx.start_glyph as u16);
    s.set_u16(out + 42, ctx.end_glyph as u16);
    true
}

/// `IndexSubtableArray::subset` with `build_lookup`.
fn index_subtable_array_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    array: View<'_>,
    ctx: &mut SizeCtx,
    cbdt: View<'_>,
    cbdt_prime: &mut Vec<u8>,
) -> bool {
    // `build_lookup`: (new gid, record index)
    let mut lookup: Vec<(u32, usize)> = Vec::new();
    let mut start_glyph_is_set = false;
    for new_gid in 0..plan.num_output_glyphs {
        let Some(old_gid) = plan.old_gid_for_new_gid(new_gid) else {
            continue;
        };
        let Some(rec) = find_table(array, old_gid, ctx.num_tables) else {
            continue;
        };
        // `IndexSubtableRecord::get_image_data`
        let first = array.u16(8 * rec);
        let last = array.u16(8 * rec + 2);
        if old_gid < first || old_gid > last {
            continue;
        }
        let subtable = array.sub(array.u32(8 * rec + 4) as usize);
        if get_image_data(subtable, (old_gid - first) as usize).is_none() {
            continue;
        }
        lookup.push((new_gid, rec));
        if !start_glyph_is_set {
            ctx.start_glyph = new_gid;
            start_glyph_is_set = true;
        }
        ctx.end_glyph = new_gid;
    }
    ctx.size = 0;
    ctx.num_tables = 0;
    let mut records: Vec<(u32, u32)> = Vec::new();
    let mut start = 0;
    while start < lookup.len() {
        let rec = lookup[start].1;
        if !add_new_record(
            plan,
            s,
            array,
            rec,
            &lookup,
            &mut start,
            &mut records,
            ctx,
            cbdt,
            cbdt_prime,
        ) {
            for _ in 0..records.len() {
                s.pop_discard();
            }
            return false;
        }
    }
    let mut objidxs: Vec<ObjIdx> = Vec::new();
    for _ in 0..records.len() {
        objidxs.push(s.pop_pack(true));
    }
    let n = records.len();
    for (i, &(first, last)) in records.iter().enumerate() {
        let pos = s.embed(&[
            (first >> 8) as u8,
            first as u8,
            (last >> 8) as u8,
            last as u8,
            0,
            0,
            0,
            0,
        ]);
        s.add_link(pos + 4, 4, objidxs[n - 1 - i], Whence::Head, 0);
    }
    true
}

/// `IndexSubtableRecord::add_new_record`.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn add_new_record(
    plan: &Plan<'_>,
    s: &mut Serializer,
    array: View<'_>,
    rec: usize,
    lookup: &[(u32, usize)],
    start: &mut usize,
    records: &mut Vec<(u32, u32)>,
    ctx: &mut SizeCtx,
    cbdt: View<'_>,
    cbdt_prime: &mut Vec<u8>,
) -> bool {
    let snap = s.snapshot();
    let old_size = ctx.size;
    let old_cbdt_prime_length = cbdt_prime.len();
    records.push((1, 0));
    ctx.size += 8;
    s.push();
    if !add_new_subtable(
        plan, s, array, rec, lookup, start, records, ctx, cbdt, cbdt_prime,
    ) {
        s.pop_discard();
        s.revert(snap);
        cbdt_prime.truncate(old_cbdt_prime_length);
        ctx.size = old_size;
        records.pop();
        return false;
    }
    ctx.num_tables += 1;
    true
}

/// `IndexSubtableRecord::add_glyph_for_subset`.
fn add_glyph_for_subset(record: &mut (u32, u32), gid: u32) -> u32 {
    if record.0 > record.1 {
        *record = (gid, gid);
        return 0;
    }
    if record.1 > gid {
        return 0;
    }
    let num_missing = gid.wrapping_sub(record.1).wrapping_sub(1);
    record.1 = gid;
    num_missing
}

/// `IndexSubtableRecord::add_new_subtable`.
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn add_new_subtable(
    plan: &Plan<'_>,
    s: &mut Serializer,
    array: View<'_>,
    rec: usize,
    lookup: &[(u32, usize)],
    start: &mut usize,
    records: &mut [(u32, u32)],
    ctx: &mut SizeCtx,
    cbdt: View<'_>,
    cbdt_prime: &mut Vec<u8>,
) -> bool {
    let subtable = s.allocate(8);
    let old_off = array.u32(8 * rec + 4);
    let old = array.sub(old_off as usize);
    let index_format = old.u16(0);
    // `populate_header`
    let image_data_offset = cbdt_prime.len() as u32;
    s.set_u16(subtable, index_format as u16);
    s.set_u16(subtable + 2, old.u16(2) as u16);
    s.set_u32(subtable + 4, image_data_offset);
    if matches!(index_format, 1 | 3) {
        ctx.size += 8;
    }
    let mut num_glyphs = 0u32;
    let mut early_exit = false;
    let mut i = *start;
    while i < lookup.len() {
        let (new_gid, next_rec) = lookup[i];
        let next_off = array.u32(8 * next_rec + 4);
        if next_off != old_off {
            *start = i;
            early_exit = true;
            break;
        }
        let Some(record) = records.last_mut() else {
            return false;
        };
        let num_missing = add_glyph_for_subset(record, new_gid);
        // `fill_missing_glyphs`
        let local_offset = (cbdt_prime.len() as u32).wrapping_sub(image_data_offset);
        match index_format {
            1 | 3 => {
                for _ in 0..num_missing {
                    if !add_offset(s, index_format, local_offset, &mut ctx.size) {
                        return false;
                    }
                    num_glyphs += 1;
                }
            }
            _ => return false,
        }
        let old_gid = plan.old_gid_for_new_gid(new_gid).unwrap_or(0);
        let next_first = array.u16(8 * next_rec);
        if old_gid < next_first {
            return false;
        }
        let old_idx = (old_gid - next_first) as usize;
        // `copy_glyph_at_idx`
        let next_subtable = array.sub(next_off as usize);
        let Some((offset, length)) = get_image_data(next_subtable, old_idx) else {
            return false;
        };
        let cbdt_length = cbdt.d.len() as u32;
        if offset > cbdt_length || cbdt_length - offset < length {
            return false;
        }
        let new_local_offset = (cbdt_prime.len() as u32).wrapping_sub(image_data_offset);
        cbdt_prime.extend_from_slice(&cbdt.d[offset as usize..(offset + length) as usize]);
        if !add_offset(s, index_format, new_local_offset, &mut ctx.size) {
            return false;
        }
        num_glyphs += 1;
        i += 1;
    }
    if !early_exit {
        *start = lookup.len();
    }
    // `finish_subtable`
    let local_offset = (cbdt_prime.len() as u32).wrapping_sub(image_data_offset);
    match index_format {
        1 => add_offset(s, index_format, local_offset, &mut ctx.size),
        3 => {
            if !add_offset(s, index_format, local_offset, &mut ctx.size) {
                return false;
            }
            if num_glyphs & 1 == 0 {
                // Pad to 32-bit alignment if needed.
                return add_offset(s, index_format, 0, &mut ctx.size);
            }
            true
        }
        _ => false,
    }
}
