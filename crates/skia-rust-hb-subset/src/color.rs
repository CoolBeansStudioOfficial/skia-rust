// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: src/OT/Color/*, src/hb-subset-table-color.cc (harfbuzz 9cb1fee5)

//! Colour tables (`COLR`, `CPAL`, `CBLC`/`CBDT`, `sbix`). `sbix` is ported; the others are not
//! yet.

use crate::ot::View;
use crate::plan::Plan;
use crate::serialize::{ObjIdx, Serializer, Whence};
use crate::{Res, SubsetError};

// -------------------------------------------------------------------------------------------
// sbix
// -------------------------------------------------------------------------------------------

/// `sbix::sanitize` (OT/Color/sbix/sbix.hh#L324-L331), as far as it decides whether the table is
/// usable: the version, the strike offsets and the offset arrays of the strikes.
fn sbix_sanitize(sbix: View<'_>, num_glyphs: u32) -> bool {
    if sbix.d.len() < 8 || sbix.u16(0) < 1 {
        return false;
    }
    let count = u64::from(sbix.u32(4));
    if 8 + 4 * count > sbix.d.len() as u64 {
        return false;
    }
    (0..count as usize).all(|i| {
        let off = sbix.u32(8 + 4 * i) as usize;
        off == 0
            || (off < sbix.d.len()
                && 4 + 4 * (u64::from(num_glyphs) + 1) <= (sbix.d.len() - off) as u64)
    })
}

/// `sbix::subset` (OT/Color/sbix/sbix.hh#L333-L340) with `serialize_strike_offsets`.
pub(crate) fn subset_sbix(plan: &Plan<'_>, s: &mut Serializer, sbix: View<'_>) -> Res<bool> {
    if !sbix_sanitize(sbix, plan.source.num_glyphs()) {
        return Err(SubsetError::Failed);
    }
    s.embed_u16(sbix.u16(0) as u16);
    s.embed_u16(sbix.u16(2) as u16);
    // `Array32OfOffset32To<SBIXStrike>`: the length, then the offsets.
    let arr = s.allocate(4);
    let mut len = 0u32;
    let mut new_strikes: Vec<usize> = Vec::new();
    let mut objidxs: Vec<ObjIdx> = Vec::new();
    for i in (0..sbix.u32(4) as usize).rev() {
        // `serialize_append`: the array grows to hold `len` offsets (a slot left by a dropped
        // strike is reused).
        len += 1;
        let pos = arr + 4 + 4 * (len as usize - 1);
        if s.length() < pos + 4 {
            let missing = pos + 4 - s.length();
            s.allocate(missing);
        }
        s.set_u32(arr, len);
        s.set_u32(pos, 0);
        let snap = s.snapshot();
        s.push();
        let strike_off = sbix.u32(8 + 4 * i) as usize;
        let ret = strike_off != 0
            && sbix.d.len() >= strike_off
            && strike_subset(
                plan,
                s,
                sbix.sub(strike_off),
                (sbix.d.len() - strike_off) as u32,
            );
        if ret {
            objidxs.push(s.pop_pack(true));
            new_strikes.push(pos);
        } else {
            s.pop_discard();
            len -= 1;
            s.set_u32(arr, len);
            s.revert(snap);
        }
    }
    for (i, &pos) in new_strikes.iter().enumerate() {
        s.add_link(pos, 4, objidxs[new_strikes.len() - 1 - i], Whence::Head, 0);
    }
    Ok(true)
}

/// `SBIXStrike::subset` (OT/Color/sbix/sbix.hh#L125-L172).
fn strike_subset(
    plan: &Plan<'_>,
    s: &mut Serializer,
    strike: View<'_>,
    available_len: u32,
) -> bool {
    let num_output_glyphs = plan.num_output_glyphs as usize;
    let snap = s.snapshot();
    let out = s.allocate(4 + 4 * (num_output_glyphs + 1));
    s.set_u16(out, strike.u16(0) as u16);
    s.set_u16(out + 2, strike.u16(2) as u16);
    let mut head = (4 + 4 * (num_output_glyphs + 1)) as u32;
    let mut has_glyphs = false;
    for new_gid in 0..num_output_glyphs {
        let old = plan.reverse_glyph_map.get(&(new_gid as u32)).copied();
        let offsets = old.map(|g| {
            (
                strike.u32(4 + 4 * g as usize),
                strike.u32(4 + 4 * (g as usize + 1)),
            )
        });
        let skip = match offsets {
            None => true,
            Some((a, b)) => a == 0 || b == 0 || b <= a || b - a <= 8 || b > available_len,
        };
        if skip {
            s.set_u32(out + 4 + 4 * new_gid, head);
            continue;
        }
        has_glyphs = true;
        let (a, b) = offsets.unwrap_or_default();
        let delta = b - a;
        // `SBIXGlyph::copy`: the 8 byte header and the data.
        let bytes: Vec<u8> = (0..delta as usize)
            .map(|k| strike.d.get(a as usize + k).copied().unwrap_or(0))
            .collect();
        s.embed(&bytes);
        s.set_u32(out + 4 + 4 * new_gid, head);
        head += delta;
    }
    if has_glyphs {
        s.set_u32(out + 4 + 4 * num_output_glyphs, head);
    } else {
        s.revert(snap);
    }
    has_glyphs
}
