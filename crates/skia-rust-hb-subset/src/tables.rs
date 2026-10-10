// Copyright © 2018  Google, Inc.
// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/hb-ot-{maxp,head,post,hhea,hmtx,hdmx,os2}-table.hh, src/OT/name/name.hh,
// src/hb-subset-table-other.cc (harfbuzz 9cb1fee5)

//! The small tables: `maxp`, `head`, `post`, `OS/2`, `hmtx`/`hhea` and `vmtx`/`vhea`, `hdmx` and
//! `name`.

use crate::bytes::{ceil_to_4, i16_at, tag, u8_at, u16_at, u32_at};
use crate::plan::Plan;
use crate::serialize::{ERROR_INT_OVERFLOW, Serializer, Whence};
use crate::sfnt::{head_ok, maxp_ok};
use crate::{FLAG_NO_PRUNE_UNICODE_RANGES, Res, SubsetError};

const TAG_HEAD: u32 = tag(b"head");
const TAG_MAXP: u32 = tag(b"maxp");
const TAG_POST: u32 = tag(b"post");
const TAG_OS2: u32 = tag(b"OS/2");
const TAG_HDMX: u32 = tag(b"hdmx");
const TAG_NAME: u32 = tag(b"name");

const FAILED: SubsetError = SubsetError::Failed;

/// `_hb_subset_table<T>`: a source table that is empty (missing, or not sanitizing) fails.
fn source_table<'a>(plan: &Plan<'a>, t: u32, ok: impl Fn(&[u8]) -> bool) -> Res<&'a [u8]> {
    let data = plan.source.table(t);
    if data.is_empty() || !ok(data) {
        Err(FAILED)
    } else {
        Ok(data)
    }
}

/// Port of `OT::head::subset` (hb-ot-head-table.hh#L58-L80) without instancing: the first 54
/// bytes of the source table.
pub(crate) fn subset_head(plan: &mut Plan<'_>) -> Res<bool> {
    let head = source_table(plan, TAG_HEAD, head_ok)?;
    plan.add_table(TAG_HEAD, head[..54].to_vec());
    Ok(true)
}

/// Port of `OT::maxp::subset` (hb-ot-maxp-table.hh#L87-L113) without instancing and hinting
/// flags.
pub(crate) fn subset_maxp(plan: &mut Plan<'_>) -> Res<bool> {
    let maxp = source_table(plan, TAG_MAXP, maxp_ok)?;
    let size = if u16_at(maxp, 0) == 1 { 32 } else { 6 };
    let mut out = maxp[..size].to_vec();
    out[4..6].copy_from_slice(&(plan.num_output_glyphs.min(0xFFFF) as u16).to_be_bytes());
    plan.add_table(TAG_MAXP, out);
    Ok(true)
}

/// Port of `OT::post::subset` (hb-ot-post-table.hh#L173-L215) with the glyph names flag off: a
/// 32 byte version 3 table.
pub(crate) fn subset_post(plan: &mut Plan<'_>) -> Res<bool> {
    let post = source_table(plan, TAG_POST, |p| {
        if p.len() < 32 {
            return false;
        }
        match u32_at(p, 0) {
            0x0001_0000 | 0x0003_0000 => true,
            0x0002_0000 => {
                let n = usize::from(u16_at(p, 32));
                p.len() >= 34 + 2 * n
            }
            _ => false,
        }
    })?;
    let mut out = post[..32].to_vec();
    // `post_prime->version.major = 3`
    out[0..2].copy_from_slice(&3u16.to_be_bytes());
    plan.add_table(TAG_POST, out);
    Ok(true)
}

/// Port of `OT::OS2::subset` (hb-ot-os2-table.hh#L202-L285) without instancing and axis
/// locations.
pub(crate) fn subset_os2(plan: &mut Plan<'_>) -> Res<bool> {
    let os2 = source_table(plan, TAG_OS2, |t| {
        if t.len() < 78 {
            return false;
        }
        let version = u16_at(t, 0);
        let mut size = 78;
        if version >= 1 {
            size += 8;
        }
        if version >= 2 {
            size += 10;
        }
        if version >= 5 {
            size += 4;
        }
        t.len() >= size
    })?;
    let version = u16_at(os2, 0);
    let mut size = 78;
    if version >= 1 {
        size += 8;
    }
    if version >= 2 {
        size += 10;
    }
    if version >= 5 {
        size += 4;
    }
    let mut out = os2[..size].to_vec();
    out[64..66].copy_from_slice(&(plan.os2_min_cmap_codepoint.min(0xFFFF) as u16).to_be_bytes());
    out[66..68].copy_from_slice(&(plan.os2_max_cmap_codepoint.min(0xFFFF) as u16).to_be_bytes());

    if plan.flags & FLAG_NO_PRUNE_UNICODE_RANGES == 0 {
        update_unicode_ranges(&plan.unicodes, &mut out);
    }
    plan.add_table(TAG_OS2, out);
    Ok(true)
}

/// Port of `OS2::_update_unicode_ranges` (hb-ot-os2-table.hh#L287-L318). `os2` is the output
/// table, whose `ulUnicodeRange` array is at byte 42.
fn update_unicode_ranges(codepoints: &std::collections::BTreeSet<u32>, os2: &mut [u8]) {
    let mut new_bits = [0u32; 4];
    for &cp in codepoints {
        let bit = crate::os2_ranges::unicode_range_bit(cp);
        if bit < 128 {
            let block = (bit / 32) as usize;
            let bit_in_block = bit % 32;
            new_bits[block] |= 1 << bit_in_block;
        }
        if (0x0001_0000..=0x0011_0000).contains(&cp) {
            // the spec says that bit 57 ("Non Plane 0") implies that there's at least one
            // codepoint beyond the BMP; so I also include all the non-BMP codepoints here
            new_bits[1] |= 1 << 25;
        }
    }
    for (i, bits) in new_bits.iter().enumerate() {
        let p = 42 + 4 * i;
        let v = u32_at(os2, p) & bits; // set bits only if set in the original
        os2[p..p + 4].copy_from_slice(&v.to_be_bytes());
    }
}

/// The `hmtx`/`vmtx` accelerator (hb-ot-hmtx-table.hh#L225-L395).
struct MtxAccelerator<'a> {
    table: &'a [u8],
    num_long_metrics: usize,
    num_bearings: usize,
    num_advances: usize,
    num_glyphs: usize,
    default_advance: u32,
}

impl<'a> MtxAccelerator<'a> {
    fn new(plan: &Plan<'a>, horizontal: bool) -> Self {
        let face = plan.source;
        let table = face.table(if horizontal {
            tag(b"hmtx")
        } else {
            tag(b"vmtx")
        });
        let upem = face.upem();
        let default_advance = if horizontal { upem / 2 } else { upem };

        let mut len = table.len();
        if len & 1 != 0 {
            len -= 1;
        }
        // `face->table.hhea` / `vhea`: the `Null` header when it does not sanitize.
        let hea = face.table(if horizontal {
            tag(b"hhea")
        } else {
            tag(b"vhea")
        });
        let mut num_long_metrics = if hea.len() >= 36 && u16_at(hea, 0) == 1 {
            usize::from(u16_at(hea, 34))
        } else {
            0
        };
        if num_long_metrics * 4 > len {
            num_long_metrics = len / 4;
        }
        len -= num_long_metrics * 4;

        let maxp = face.table(TAG_MAXP);
        let mut num_bearings = if maxp_ok(maxp) {
            usize::from(u16_at(maxp, 4))
        } else {
            0
        };
        if num_bearings < num_long_metrics {
            num_bearings = num_long_metrics;
        }
        if (num_bearings - num_long_metrics) * 2 > len {
            num_bearings = num_long_metrics + len / 2;
        }
        len -= (num_bearings - num_long_metrics) * 2;

        // We MUST set num_bearings to zero if num_long_metrics is zero.
        if num_long_metrics == 0 {
            num_bearings = 0;
        }
        let num_advances = num_bearings + len / 2;
        let mut num_glyphs = face.num_glyphs() as usize;
        if num_glyphs < num_advances {
            num_glyphs = num_advances;
        }
        MtxAccelerator {
            table,
            num_long_metrics,
            num_bearings,
            num_advances,
            num_glyphs,
            default_advance,
        }
    }

    fn get_leading_bearing_without_var_unscaled(&self, glyph: u32) -> i32 {
        let glyph = glyph as usize;
        if glyph < self.num_long_metrics {
            return i32::from(i16_at(self.table, 4 * glyph + 2));
        }
        if glyph >= self.num_bearings {
            return 0;
        }
        i32::from(i16_at(
            self.table,
            4 * self.num_long_metrics + 2 * (glyph - self.num_long_metrics),
        ))
    }

    fn get_advance_without_var_unscaled(&self, glyph: u32) -> u32 {
        let glyph = glyph as usize;
        // OpenType case.
        if glyph < self.num_bearings {
            return u32::from(u16_at(self.table, 4 * glyph.min(self.num_long_metrics - 1)));
        }
        // If num_advances is zero, it means we don't have the metrics table for this direction:
        // return default advance.
        if self.num_advances == 0 {
            return self.default_advance;
        }
        if glyph >= self.num_glyphs {
            return 0;
        }
        // num_bearings <= glyph < num_glyphs; num_bearings <= num_advances
        if self.num_bearings == self.num_advances {
            return self.get_advance_without_var_unscaled((self.num_bearings - 1) as u32);
        }
        let advances = 4 * self.num_long_metrics + 2 * (self.num_bearings - self.num_long_metrics);
        let i = (glyph - self.num_bearings).min(self.num_advances - self.num_bearings - 1);
        u32::from(u16_at(self.table, advances + 2 * i))
    }
}

/// Port of `hmtxvmtx::subset` (hb-ot-hmtx-table.hh#L135-L223) with
/// `hmtxvmtx::subset_update_header` (L62-L133) without instancing.
pub(crate) fn subset_mtx(plan: &mut Plan<'_>, horizontal: bool) -> Res<bool> {
    let mtx_tag = if horizontal {
        tag(b"hmtx")
    } else {
        tag(b"vmtx")
    };
    let hea_tag = if horizontal {
        tag(b"hhea")
    } else {
        tag(b"vhea")
    };
    // `_hb_subset_table<hmtx>`: an empty source table fails.
    if plan.source.table(mtx_tag).is_empty() {
        return Err(FAILED);
    }
    let mtx = MtxAccelerator::new(plan, horizontal);

    // Determine num_long_metrics to encode.
    let new_gid_advance = |new_gid: u32| -> u32 {
        // `mtx_map` is empty without instancing.
        plan.old_gid_for_new_gid(new_gid)
            .map_or(0, |old| mtx.get_advance_without_var_unscaled(old))
    };
    let mut num_long_metrics = plan.num_output_glyphs.min(0xFFFF) as usize;
    let last_advance = new_gid_advance((num_long_metrics as u32).wrapping_sub(1));
    while num_long_metrics > 1 && last_advance == new_gid_advance((num_long_metrics - 2) as u32) {
        num_long_metrics -= 1;
    }

    // `serialize`
    let total_num_metrics = plan.num_output_glyphs as usize;
    let mut out = vec![0u8; num_long_metrics * 4 + (total_num_metrics - num_long_metrics) * 2];
    for &(new_gid, old_gid) in &plan.new_to_old_gid_list {
        let advance = mtx.get_advance_without_var_unscaled(old_gid);
        let lsb = mtx.get_leading_bearing_without_var_unscaled(old_gid);
        let gid = new_gid as usize;
        if gid < num_long_metrics {
            out[4 * gid..4 * gid + 2].copy_from_slice(&(advance as u16).to_be_bytes());
            out[4 * gid + 2..4 * gid + 4].copy_from_slice(&(lsb as i16).to_be_bytes());
        } else if gid < 0x10000 {
            let p = 4 * num_long_metrics + 2 * (gid - num_long_metrics);
            out[p..p + 2].copy_from_slice(&(lsb as i16).to_be_bytes());
        }
    }

    // Amend header num hmetrics
    let hea = plan.source.table(hea_tag);
    if hea.len() < 36 || u16_at(hea, 0) != 1 {
        return Err(FAILED);
    }
    let mut hea_out = hea.to_vec();
    hea_out[34..36].copy_from_slice(&(num_long_metrics as u16).to_be_bytes());

    plan.add_table(hea_tag, hea_out);
    plan.add_table(mtx_tag, out);
    Ok(true)
}

/// Port of `OT::hdmx::subset` (hb-ot-hdmx-table.hh#L106-L136).
pub(crate) fn subset_hdmx(plan: &mut Plan<'_>) -> Res<bool> {
    let hdmx = source_table(plan, TAG_HDMX, |t| {
        if t.len() < 8 {
            return false;
        }
        let num_records = u64::from(u16_at(t, 2));
        let size_device_record = u64::from(u32_at(t, 4));
        // `!hb_unsigned_mul_overflows`, `min_size + ... > ...`, `sizeDeviceRecord >= min_size`
        let Some(total) = (num_records * size_device_record).checked_add(8) else {
            return false;
        };
        total > num_records * size_device_record
            && size_device_record >= 2
            && t.len() as u64 >= total
    })?;
    let num_records = usize::from(u16_at(hdmx, 2));
    let size_device_record = u32_at(hdmx, 4) as usize;
    // `get_num_glyphs`: `sizeDeviceRecord - DeviceRecord::min_size`
    let num_input_glyphs = size_device_record - 2;
    let num_glyphs = plan.num_output_glyphs as usize;
    let record_size = ceil_to_4(2 + num_glyphs);

    let mut out = Vec::with_capacity(8 + num_records * record_size);
    out.extend_from_slice(&u16_at(hdmx, 0).to_be_bytes());
    out.extend_from_slice(&(num_records as u16).to_be_bytes());
    out.extend_from_slice(&(record_size as u32).to_be_bytes());
    for r in 0..num_records {
        let rec = 8 + r * size_device_record;
        // The row of widths for the retained glyphs; reads outside the input glyphs are zero.
        let row: Vec<u8> = plan
            .new_to_old_gid_list
            .iter()
            .map(|&(_, old)| {
                if (old as usize) < num_input_glyphs {
                    u8_at(hdmx, rec + 2 + old as usize)
                } else {
                    0
                }
            })
            .collect();
        let start = out.len();
        out.resize(start + record_size, 0);
        out[start] = u8_at(hdmx, rec);
        out[start + 1] = row.iter().copied().max().unwrap_or(0);
        for (i, &(new, _)) in plan.new_to_old_gid_list.iter().enumerate() {
            out[start + 2 + new as usize] = row[i];
        }
    }
    plan.add_table(TAG_HDMX, out);
    Ok(true)
}

/// A `NameRecord` (OT/name/name.hh#L90-L128).
struct Rec {
    platform: u16,
    encoding: u16,
    language: u16,
    name_id: u16,
    length: u16,
    offset: u16,
}
/// Port of `OT::name::subset` and `name::serialize` (OT/name/name.hh#L335-L450, L270-L333) with
/// `NameRecord::copy` (L130-L222), without name table overrides.
pub(crate) fn subset_name(plan: &mut Plan<'_>) -> Res<bool> {
    let name = source_table(plan, TAG_NAME, |t| {
        if t.len() < 6 {
            return false;
        }
        let format = u16_at(t, 0);
        let count = usize::from(u16_at(t, 2));
        let string_offset = usize::from(u16_at(t, 4));
        if format > 1 || 6 + 12 * count > t.len() || string_offset > t.len() {
            return false;
        }
        (0..count).all(|i| {
            let r = 6 + 12 * i;
            string_offset + usize::from(u16_at(t, r + 10)) + usize::from(u16_at(t, r + 8))
                <= t.len()
        })
    })?;
    let count = usize::from(u16_at(name, 2));
    let string_offset = usize::from(u16_at(name, 4));

    let mut records: Vec<Rec> = (0..count)
        .map(|i| {
            let r = 6 + 12 * i;
            Rec {
                platform: u16_at(name, r),
                encoding: u16_at(name, r + 2),
                language: u16_at(name, r + 4),
                name_id: u16_at(name, r + 6),
                length: u16_at(name, r + 8),
                offset: u16_at(name, r + 10),
            }
        })
        .filter(|r| plan.name_ids.contains(&u32::from(r.name_id)))
        .filter(|r| plan.name_languages.contains(&u32::from(r.language)))
        // `(flags & HB_SUBSET_FLAGS_NAME_LEGACY) || namerecord.isUnicode ()`
        .filter(|r| r.platform == 0 || (r.platform == 3 && matches!(r.encoding, 0 | 1 | 10)))
        .collect();

    // `records.qsort ()` with `NameRecord::cmp`.
    records.sort_by(|a, b| {
        a.platform
            .cmp(&b.platform)
            .then(a.encoding.cmp(&b.encoding))
            .then(a.language.cmp(&b.language))
            .then(a.name_id.cmp(&b.name_id))
            .then(a.length.cmp(&b.length))
    });

    let mut s = Serializer::new();
    s.start_serialize();
    let this = s.allocate(6);
    s.check_fits(records.len() as u64, 16, ERROR_INT_OVERFLOW);
    s.set_u16(this + 2, records.len() as u16);
    for r in &records {
        let rec = s.allocate(12);
        s.set_u16(rec, r.platform);
        s.set_u16(rec + 2, r.encoding);
        s.set_u16(rec + 4, r.language);
        s.set_u16(rec + 6, r.name_id);
        s.set_u16(rec + 8, r.length);
        // `out->offset.serialize_copy (c, offset, base, 0, Tail, length)`
        s.push();
        let start = string_offset + usize::from(r.offset);
        let bytes: Vec<u8> = (0..usize::from(r.length))
            .map(|i| u8_at(name, start + i))
            .collect();
        s.embed(&bytes);
        let objidx = s.pop_pack(true);
        s.add_link(rec + 10, 2, objidx, Whence::Tail, 0);
    }
    let length = s.length();
    s.set_u16(this + 4, length as u16);
    s.check_fits(length as u64, 16, ERROR_INT_OVERFLOW);
    let out = s.end_serialize().ok_or(FAILED)?;
    plan.add_table(TAG_NAME, out);
    Ok(true)
}
