// Use of this source code is governed by the "Old MIT" licence in the LICENSE file.
// Port of: the `HBUINT16`/`HBUINT32` big-endian accessors of src/hb-open-type.hh (harfbuzz 9cb1fee5)

//! Big-endian reads with `HarfBuzz`'s `Null` semantics: a read outside the data is zero.

use std::cmp::Ordering;

#[allow(clippy::trivially_copy_pass_by_ref)] // a reference to a byte-string literal, as in `tag (b"glyf")`
pub(crate) const fn tag(t: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*t)
}

/// Port of `hb_bsearch_impl` (hb-algs.hh#L1245-L1277): `cmp(i)` compares the key with item `i`.
/// Returns the index of a match.
pub(crate) fn bsearch(len: usize, cmp: impl Fn(usize) -> Ordering) -> Option<usize> {
    let (mut min, mut max) = (0isize, len as isize - 1);
    while min <= max {
        let mid = isize::midpoint(min, max);
        match cmp(mid as usize) {
            Ordering::Less => max = mid - 1,
            Ordering::Greater => min = mid + 1,
            Ordering::Equal => return Some(mid as usize),
        }
    }
    None
}

pub(crate) fn u8_at(d: &[u8], o: usize) -> u8 {
    d.get(o).copied().unwrap_or(0)
}

pub(crate) fn u16_at(d: &[u8], o: usize) -> u16 {
    match d.get(o..o.wrapping_add(2)) {
        Some(b) => u16::from_be_bytes([b[0], b[1]]),
        None => 0,
    }
}

pub(crate) fn i16_at(d: &[u8], o: usize) -> i16 {
    u16_at(d, o) as i16
}

pub(crate) fn u24_at(d: &[u8], o: usize) -> u32 {
    match d.get(o..o.wrapping_add(3)) {
        Some(b) => u32::from_be_bytes([0, b[0], b[1], b[2]]),
        None => 0,
    }
}

pub(crate) fn u32_at(d: &[u8], o: usize) -> u32 {
    match d.get(o..o.wrapping_add(4)) {
        Some(b) => u32::from_be_bytes([b[0], b[1], b[2], b[3]]),
        None => 0,
    }
}

/// `hb_bit_storage`: the number of bits needed to store `v`.
pub(crate) fn bit_storage(v: u32) -> u32 {
    32 - v.leading_zeros()
}

/// `hb_ceil_to_4`.
pub(crate) fn ceil_to_4(v: usize) -> usize {
    (v + 3) & !3
}
