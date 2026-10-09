// Copyright 2014 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp `src/utils/huffman_utils.{c,h}` (the decoder half).
//!
//! libwebp stores every table of a decoder in chained segments and keeps pointers into them.
//! Here all tables of one decoder live in one `Vec<HuffmanCode>` and a table is identified by its
//! base index. The offsets stored inside a table (`value` of a second-level link) are relative to
//! the link's own position, as in the C code, so the lookup arithmetic is unchanged.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::too_many_lines: the function is one C function; splitting it would change the port structure.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

/// Port of `MAX_ALLOWED_CODE_LENGTH` (`format_constants.h`).
pub const MAX_ALLOWED_CODE_LENGTH: usize = 15;
/// Port of `HUFFMAN_TABLE_BITS` (`huffman_utils.h`).
pub const HUFFMAN_TABLE_BITS: i32 = 8;
/// Port of `HUFFMAN_TABLE_MASK`.
pub const HUFFMAN_TABLE_MASK: u32 = (1 << HUFFMAN_TABLE_BITS) - 1;
/// Port of `HUFFMAN_PACKED_BITS`.
pub const HUFFMAN_PACKED_BITS: i32 = 6;
/// Port of `HUFFMAN_PACKED_TABLE_SIZE`.
pub const HUFFMAN_PACKED_TABLE_SIZE: u32 = 1 << HUFFMAN_PACKED_BITS;

/// Port of `HuffmanCode`: `bits` is the code length, `value` the symbol or the offset of a
/// second-level table.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HuffmanCode {
    pub bits: u8,
    pub value: u16,
}

/// Port of `HuffmanCode32`: a packed-table entry. `bits` may carry the special-marker offset.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HuffmanCode32 {
    pub bits: i32,
    pub value: u32,
}

/// Port of `GetNextKey`.
#[inline]
fn get_next_key(key: u32, len: i32) -> u32 {
    let mut step = 1u32 << (len - 1);
    while key & step != 0 {
        step >>= 1;
    }
    if step != 0 {
        (key & (step - 1)) + step
    } else {
        key
    }
}

/// Port of `ReplicateValue`. `table` starts at `base`; `end` and `step` are as in C.
#[inline]
fn replicate_value(
    table: &mut [HuffmanCode],
    base: usize,
    step: usize,
    mut end: usize,
    code: HuffmanCode,
) {
    loop {
        end -= step;
        table[base + end] = code;
        if end == 0 {
            break;
        }
    }
}

/// Port of `NextTableBitSize`.
#[inline]
fn next_table_bit_size(count: &[i32], mut len: i32, root_bits: i32) -> i32 {
    let mut left = 1i32 << (len - root_bits);
    while (len as usize) < MAX_ALLOWED_CODE_LENGTH {
        left -= count[len as usize];
        if left <= 0 {
            break;
        }
        len += 1;
        left <<= 1;
    }
    len - root_bits
}

/// Port of `BuildHuffmanTable`. When `out` is `None` only the total size is computed (the
/// `root_table == NULL` case of the C code). When it is `Some((table, sorted))`, `table` must
/// start at the table base and have room for the returned size.
///
/// Returns the total size of the root and second-level tables, or 0 for an invalid code.
fn build_huffman_table(
    mut out: Option<(&mut [HuffmanCode], usize, &mut [u16])>,
    root_bits: i32,
    code_lengths: &[i32],
) -> usize {
    let code_lengths_size = code_lengths.len();
    let mut total_size = 1usize << root_bits;
    let mut count = [0i32; MAX_ALLOWED_CODE_LENGTH + 1];
    let mut offset = [0i32; MAX_ALLOWED_CODE_LENGTH + 1];

    for &len in code_lengths {
        if len as usize > MAX_ALLOWED_CODE_LENGTH {
            return 0;
        }
        count[len as usize] += 1;
    }
    if count[0] as usize == code_lengths_size {
        return 0;
    }

    offset[1] = 0;
    for len in 1..MAX_ALLOWED_CODE_LENGTH {
        if count[len] > (1 << len) {
            return 0;
        }
        offset[len + 1] = offset[len] + count[len];
    }

    for (symbol, &symbol_code_length) in code_lengths.iter().enumerate() {
        if symbol_code_length > 0 {
            let sl = symbol_code_length as usize;
            match out.as_mut() {
                Some((_, _, sorted)) => {
                    if offset[sl] as usize >= code_lengths_size {
                        return 0;
                    }
                    sorted[offset[sl] as usize] = symbol as u16;
                    offset[sl] += 1;
                }
                None => offset[sl] += 1,
            }
        }
    }

    if offset[MAX_ALLOWED_CODE_LENGTH] == 1 {
        if let Some((table, base, sorted)) = out.as_mut() {
            let code = HuffmanCode {
                bits: 0,
                value: sorted[0],
            };
            replicate_value(table, *base, 1, total_size, code);
        }
        return total_size;
    }

    // `write` mirrors `root_table != NULL`.
    let write = out.is_some();
    let (table_buf, root_base, sorted_buf): (
        Option<&mut [HuffmanCode]>,
        usize,
        Option<&mut [u16]>,
    ) = match out {
        Some((t, b, s)) => (Some(t), b, Some(s)),
        None => (None, 0, None),
    };
    let mut table_buf = table_buf;
    let mut sorted_buf = sorted_buf;

    let mut low: u32 = 0xffff_ffff; // low bits for current root entry
    let mask: u32 = (total_size - 1) as u32; // mask for low bits
    let mut key: u32 = 0; // reversed prefix code
    let mut num_nodes: i64 = 1; // number of Huffman tree nodes
    let mut num_open: i64 = 1; // number of open branches in current tree level
    let mut table_bits = root_bits; // key length of current table
    let mut table_size = 1usize << table_bits; // size of current table
    // Offset of the current table inside `table_buf` (the C `table` pointer, relative to base).
    let mut table_off: usize = 0;
    let mut symbol: usize = 0;

    let mut step = 2usize;
    for len in 1..=root_bits {
        num_open <<= 1;
        num_nodes += num_open;
        num_open -= i64::from(count[len as usize]);
        if num_open < 0 {
            return 0;
        }
        if !write {
            step <<= 1;
            continue;
        }
        let table = table_buf.as_deref_mut().expect("write mode has a table");
        let sorted = sorted_buf.as_deref_mut().expect("write mode has sorted");
        while count[len as usize] > 0 {
            let code = HuffmanCode {
                bits: len as u8,
                value: sorted[symbol],
            };
            symbol += 1;
            replicate_value(
                table,
                root_base + table_off + key as usize,
                step,
                table_size,
                code,
            );
            key = get_next_key(key, len);
            count[len as usize] -= 1;
        }
        step <<= 1;
    }

    step = 2;
    for len in (root_bits + 1)..=(MAX_ALLOWED_CODE_LENGTH as i32) {
        num_open <<= 1;
        num_nodes += num_open;
        num_open -= i64::from(count[len as usize]);
        if num_open < 0 {
            return 0;
        }
        while count[len as usize] > 0 {
            if (key & mask) != low {
                if write {
                    table_off += table_size;
                }
                table_bits = next_table_bit_size(&count, len, root_bits);
                table_size = 1 << table_bits;
                total_size += table_size;
                low = key & mask;
                if write {
                    let table = table_buf.as_deref_mut().expect("write mode has a table");
                    let t = table_bits + root_bits;
                    let rel = table_off as i64 - i64::from(low);
                    table[root_base + low as usize] = HuffmanCode {
                        bits: t as u8,
                        value: rel as u16,
                    };
                }
            }
            if write {
                let table = table_buf.as_deref_mut().expect("write mode has a table");
                let sorted = sorted_buf.as_deref_mut().expect("write mode has sorted");
                let code = HuffmanCode {
                    bits: (len - root_bits) as u8,
                    value: sorted[symbol],
                };
                symbol += 1;
                replicate_value(
                    table,
                    root_base + table_off + (key >> root_bits) as usize,
                    step,
                    table_size,
                    code,
                );
            }
            key = get_next_key(key, len);
            count[len as usize] -= 1;
        }
        step <<= 1;
    }

    if num_nodes != 2 * i64::from(offset[MAX_ALLOWED_CODE_LENGTH]) - 1 {
        return 0;
    }
    total_size
}

/// Port of `VP8LBuildHuffmanTable` with `root_table == NULL`: validates `code_lengths` and returns
/// the table size it would need, without building anything. Returns 0 for an invalid code.
#[doc(alias = "VP8LBuildHuffmanTable")]
#[must_use]
pub fn validate(root_bits: i32, code_lengths: &[i32]) -> usize {
    build_huffman_table(None, root_bits, code_lengths)
}

/// Port of `VP8LBuildHuffmanTable`: builds a table (appended to `arena`) from `code_lengths`.
///
/// Returns `Some((base, size))` on success and `None` for an invalid code (the C function returns
/// 0 in that case).
#[doc(alias = "VP8LBuildHuffmanTable")]
#[must_use]
pub fn build_table(
    arena: &mut Vec<HuffmanCode>,
    root_bits: i32,
    code_lengths: &[i32],
) -> Option<(usize, usize)> {
    let total_size = build_huffman_table(None, root_bits, code_lengths);
    if total_size == 0 {
        return None;
    }
    let base = arena.len();
    arena.resize(base + total_size, HuffmanCode::default());
    let mut sorted = vec![0u16; code_lengths.len()];
    let size = build_huffman_table(
        Some((&mut arena[..], base, &mut sorted[..])),
        root_bits,
        code_lengths,
    );
    debug_assert_eq!(size, total_size);
    Some((base, total_size))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_symbol_code_has_one_bit_entries() {
        let mut arena = Vec::new();
        let (base, size) = build_table(&mut arena, 8, &[1, 1]).expect("valid code");
        assert_eq!(size, 256);
        assert_eq!(arena[base].bits, 1);
        assert_eq!(arena[base].value, 0);
        assert_eq!(arena[base + 1].value, 1);
    }
}
