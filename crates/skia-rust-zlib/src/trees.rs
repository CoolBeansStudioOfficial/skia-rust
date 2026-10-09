// Copyright 1995-2023 Jean-loup Gailly and Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: trees.c#L37-L1118 (chromium zlib@646b7f56, zlib 1.3.0.1-motley)
// Ported from: trees.c, deflate.h (the tree types and the `_tr_*` functions).

//! Huffman tree construction and block output for the deflate compressor (zlib's `trees.c`).
//!
//! The literal and distance symbols of a block are kept in the state's symbol buffers
//! (`LIT_MEM` layout). Each `_tr_flush_block` builds the two dynamic trees and the bit-length
//! tree from the symbol frequencies, picks stored, fixed or dynamic coding, and writes the block
//! into the pending output.
//!
//! zlib's `ct_data` is a union: `fc` is `Freq` before a tree is built and `Code` after, and `dl`
//! is `Dad` while the tree is built and `Len` after. The port keeps the union as two `u16`
//! fields with the same accessors' meaning, which is what keeps the code order and bit output
//! identical.

// zlib's arithmetic is in unsigned and narrow integers (`uInt`, `ush`, `uch`, `ulg`, `long`) mixed
// with `int`. The casts in this module mirror the C conversions of deflate.c and trees.c one for
// one, including the truncations and sign changes that zlib relies on, so the lints that flag them
// are allowed here. The arithmetic itself is not changed.
#![allow(clippy::cast_possible_truncation)] // zlib's narrowing casts (ush, uch, uInt)
#![allow(clippy::cast_sign_loss)] // zlib's unsigned views of int and long values
#![allow(clippy::cast_possible_wrap)] // zlib's int views of unsigned values
#![allow(clippy::cast_lossless)] // written as the C conversion, for a like-for-like reading
#![allow(clippy::too_many_lines)] // deflate_fast, deflate_slow and deflate_stored are one function each in zlib
#![allow(clippy::cognitive_complexity)] // as above
#![allow(clippy::similar_names)] // zlib's names: strstart, match_start, prev_match, ...
#![allow(clippy::many_single_char_names)] // zlib's names: s, p, n, m, h, b
#![allow(clippy::needless_range_loop)] // loops keep zlib's index form
use crate::deflate::DeflateState;
use crate::trees_tables::{
    BASE_DIST, BASE_LENGTH, DIST_CODE, LENGTH_CODE, STATIC_DTREE, STATIC_LTREE,
};

/// Port of `MAX_BL_BITS`.
const MAX_BL_BITS: i32 = 7;
/// Port of `END_BLOCK`.
pub(crate) const END_BLOCK: usize = 256;
/// Port of `REP_3_6`.
const REP_3_6: usize = 16;
/// Port of `REPZ_3_10`.
const REPZ_3_10: usize = 17;
/// Port of `REPZ_11_138`.
const REPZ_11_138: usize = 18;

/// Port of `LITERALS`.
pub(crate) const LITERALS: u32 = 256;
/// Port of `LENGTH_CODES`.
pub(crate) const LENGTH_CODES: usize = 29;
/// Port of `L_CODES`.
pub(crate) const L_CODES: usize = LITERALS as usize + 1 + LENGTH_CODES;
/// Port of `D_CODES`.
pub(crate) const D_CODES: usize = 30;
/// Port of `BL_CODES`.
pub(crate) const BL_CODES: usize = 19;
/// Port of `HEAP_SIZE`.
pub(crate) const HEAP_SIZE: usize = 2 * L_CODES + 1;
/// Port of `MAX_BITS`.
pub(crate) const MAX_BITS: usize = 15;
/// Port of `MIN_MATCH`.
pub(crate) const MIN_MATCH: u32 = 3;
/// Port of `MAX_MATCH`.
pub(crate) const MAX_MATCH: u32 = 258;
/// Port of `Buf_size`: the width of the bit buffer.
pub(crate) const BUF_SIZE: i32 = 16;

/// Port of `STORED_BLOCK`.
const STORED_BLOCK: u32 = 0;
/// Port of `STATIC_TREES`.
const STATIC_TREES: u32 = 1;
/// Port of `DYN_TREES`.
const DYN_TREES: u32 = 2;

/// Port of `Z_BINARY`.
pub(crate) const Z_BINARY: i32 = 0;
/// Port of `Z_TEXT` (`Z_ASCII`).
pub(crate) const Z_TEXT: i32 = 1;
/// Port of `Z_UNKNOWN`.
pub(crate) const Z_UNKNOWN: i32 = 2;
/// Port of `Z_FIXED` (strategy value).
pub(crate) const Z_FIXED: i32 = 4;

/// Port of `extra_lbits`.
static EXTRA_LBITS: [i32; LENGTH_CODES] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];
/// Port of `extra_dbits`.
static EXTRA_DBITS: [i32; D_CODES] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];
/// Port of `extra_blbits`.
static EXTRA_BLBITS: [i32; BL_CODES] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 3, 7];
/// Port of `bl_order`: the order in which bit-length codes are sent.
static BL_ORDER: [usize; BL_CODES] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

/// Port of `ct_data`: `fc` is `Freq` or `Code`, `dl` is `Dad` or `Len` (zlib's unions).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct CtData {
    /// `Freq` while counting, `Code` once the tree has codes.
    pub(crate) fc: u16,
    /// `Dad` while the tree is built, `Len` (the code length) afterwards.
    pub(crate) dl: u16,
}

impl CtData {
    /// Builds a table entry holding a code and its length (the generated static tables).
    pub(crate) const fn new(code: u16, len: u16) -> Self {
        Self { fc: code, dl: len }
    }
}

/// Port of `struct static_tree_desc_s`.
pub(crate) struct StaticTreeDesc {
    /// Port of `static_tree`: `None` for the bit-length tree (`(const ct_data *)0`).
    pub(crate) static_tree: Option<&'static [CtData]>,
    /// Port of `extra_bits`.
    pub(crate) extra_bits: &'static [i32],
    /// Port of `extra_base`.
    pub(crate) extra_base: usize,
    /// Port of `elems`.
    pub(crate) elems: usize,
    /// Port of `max_length`.
    pub(crate) max_length: i32,
}

/// Port of `static_l_desc`.
pub(crate) static STATIC_L_DESC: StaticTreeDesc = StaticTreeDesc {
    static_tree: Some(&STATIC_LTREE),
    extra_bits: &EXTRA_LBITS,
    extra_base: LITERALS as usize + 1,
    elems: L_CODES,
    max_length: MAX_BITS as i32,
};

/// Port of `static_d_desc`.
pub(crate) static STATIC_D_DESC: StaticTreeDesc = StaticTreeDesc {
    static_tree: Some(&STATIC_DTREE),
    extra_bits: &EXTRA_DBITS,
    extra_base: 0,
    elems: D_CODES,
    max_length: MAX_BITS as i32,
};

/// Port of `static_bl_desc`.
pub(crate) static STATIC_BL_DESC: StaticTreeDesc = StaticTreeDesc {
    static_tree: None,
    extra_bits: &EXTRA_BLBITS,
    extra_base: 0,
    elems: BL_CODES,
    max_length: MAX_BL_BITS,
};

/// Port of `tree_desc_s`: the dynamic tree's largest used code, and its static description.
/// The tree's storage is held by the state (`dyn_ltree`, `dyn_dtree`, `bl_tree`).
#[derive(Clone, Copy)]
pub(crate) struct TreeDesc {
    /// Port of `max_code`.
    pub(crate) max_code: i32,
    /// Port of `stat_desc`.
    pub(crate) stat: &'static StaticTreeDesc,
}

/// Which of the three dynamic trees a `tree_desc` refers to.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeKind {
    /// `dyn_ltree` (`l_desc`).
    Lit,
    /// `dyn_dtree` (`d_desc`).
    Dist,
}

/// Port of `d_code(dist)`.
#[must_use]
pub(crate) fn d_code(dist: u32) -> usize {
    if dist < 256 {
        DIST_CODE[dist as usize] as usize
    } else {
        DIST_CODE[256 + (dist >> 7) as usize] as usize
    }
}

/// Port of `bi_reverse`: reverses the low `len` bits of `code`.
#[must_use]
fn bi_reverse(code: u32, len: i32) -> u32 {
    let mut code = code;
    let mut res: u32 = 0;
    let mut len = len;
    loop {
        res |= code & 1;
        code >>= 1;
        res <<= 1;
        len -= 1;
        if len <= 0 {
            break;
        }
    }
    res >> 1
}

/// Port of `put_byte` (deflate.h).
#[inline]
fn put_byte(s: &mut DeflateState, c: u8) {
    s.pending_buf[s.pending as usize] = c;
    s.pending += 1;
}

/// Port of `put_short` (trees.c, the little-endian pair of bytes).
#[inline]
fn put_short(s: &mut DeflateState, w: u16) {
    put_byte(s, (w & 0xff) as u8);
    put_byte(s, (w >> 8) as u8);
}

/// Port of `bi_flush`: writes out whole bytes from the bit buffer.
fn bi_flush(s: &mut DeflateState) {
    if s.bi_valid == 16 {
        put_short(s, s.bi_buf);
        s.bi_buf = 0;
        s.bi_valid = 0;
    } else if s.bi_valid >= 8 {
        put_byte(s, s.bi_buf as u8);
        s.bi_buf >>= 8;
        s.bi_valid -= 8;
    }
}

/// Port of `bi_windup`: flushes the bit buffer, padding to a byte boundary.
fn bi_windup(s: &mut DeflateState) {
    if s.bi_valid > 8 {
        put_short(s, s.bi_buf);
    } else if s.bi_valid > 0 {
        put_byte(s, s.bi_buf as u8);
    }
    s.bi_buf = 0;
    s.bi_valid = 0;
}

/// Port of `gen_codes`: assigns the canonical Huffman codes for the lengths in `tree`.
fn gen_codes(tree: &mut [CtData], max_code: usize, bl_count: &[u16; MAX_BITS + 1]) {
    let mut next_code = [0u16; MAX_BITS + 1];
    let mut code: u32 = 0;
    for bits in 1..=MAX_BITS {
        code = (code + u32::from(bl_count[bits - 1])) << 1;
        next_code[bits] = code as u16;
    }
    for node in tree.iter_mut().take(max_code + 1) {
        let len = node.dl as usize;
        if len == 0 {
            continue;
        }
        node.fc = bi_reverse(u32::from(next_code[len]), len as i32) as u16;
        next_code[len] = next_code[len].wrapping_add(1);
    }
}

/// Port of `send_bits`: appends `length` bits of `value` to the bit buffer.
fn send_bits(s: &mut DeflateState, value: u32, length: i32) {
    let val16 = u32::from(value as u16);
    if s.bi_valid > BUF_SIZE - length {
        s.bi_buf |= (val16 << s.bi_valid) as u16;
        put_short(s, s.bi_buf);
        s.bi_buf = (val16 >> (BUF_SIZE - s.bi_valid)) as u16;
        s.bi_valid += length - BUF_SIZE;
    } else {
        s.bi_buf |= (val16 << s.bi_valid) as u16;
        s.bi_valid += length;
    }
}

/// Port of `send_code`: sends the code of symbol `c` from `tree`.
fn send_code(s: &mut DeflateState, c: usize, tree: &[CtData]) {
    send_bits(s, u32::from(tree[c].fc), i32::from(tree[c].dl));
}

/// Port of `tr_static_init` (the tables are generated constants here, see `trees_tables`).
/// Port of `_tr_init`: resets the bit buffer and the first block.
pub(crate) fn tr_init(s: &mut DeflateState) {
    s.bi_buf = 0;
    s.bi_valid = 0;
    init_block(s);
}

/// Port of `init_block`: clears the frequencies for the next block.
fn init_block(s: &mut DeflateState) {
    for n in 0..L_CODES {
        s.dyn_ltree[n].fc = 0;
    }
    for n in 0..D_CODES {
        s.dyn_dtree[n].fc = 0;
    }
    for n in 0..BL_CODES {
        s.bl_tree[n].fc = 0;
    }
    s.dyn_ltree[END_BLOCK].fc = 1;
    s.opt_len = 0;
    s.static_len = 0;
    s.sym_next = 0;
    s.matches = 0;
}

/// Port of `SMALLEST`: the heap slot of the smallest element.
const SMALLEST: usize = 1;

/// Port of `smaller(tree, n, m, depth)`.
#[inline]
fn smaller(tree: &[CtData], n: usize, m: usize, depth: &[u8]) -> bool {
    tree[n].fc < tree[m].fc || (tree[n].fc == tree[m].fc && depth[n] <= depth[m])
}

/// Port of `pqdownheap`: restores the heap property below slot `k`.
fn pqdownheap(s: &mut DeflateState, tree: &[CtData], k: usize) {
    let v = s.heap[k];
    let mut k = k;
    let mut j = k << 1;
    while j <= s.heap_len as usize {
        if j < s.heap_len as usize
            && smaller(tree, s.heap[j + 1] as usize, s.heap[j] as usize, &s.depth)
        {
            j += 1;
        }
        if smaller(tree, v as usize, s.heap[j] as usize, &s.depth) {
            break;
        }
        s.heap[k] = s.heap[j];
        k = j;
        j <<= 1;
    }
    s.heap[k] = v;
}

/// Port of `gen_bitlen`: computes the optimal code lengths, limited to `max_length`.
fn gen_bitlen(s: &mut DeflateState, tree: &mut [CtData], desc: &TreeDesc) {
    let max_code = desc.max_code;
    let stree = desc.stat.static_tree;
    let extra = desc.stat.extra_bits;
    let base = desc.stat.extra_base;
    let max_length = desc.stat.max_length;
    let mut overflow: i32 = 0;

    for bits in 0..=MAX_BITS {
        s.bl_count[bits] = 0;
    }

    // The root of the tree has length 0.
    let root = s.heap[s.heap_max as usize] as usize;
    tree[root].dl = 0;

    let mut h = s.heap_max as usize + 1;
    while h < HEAP_SIZE {
        let n = s.heap[h] as usize;
        let mut bits = i32::from(tree[tree[n].dl as usize].dl) + 1;
        if bits > max_length {
            bits = max_length;
            overflow += 1;
        }
        tree[n].dl = bits as u16;
        h += 1;
        // Not a leaf: the code lengths of internal nodes are only used for the children.
        if n > max_code as usize {
            continue;
        }
        s.bl_count[bits as usize] += 1;
        let mut xbits = 0;
        if n >= base {
            xbits = extra[n - base];
        }
        let f = u64::from(tree[n].fc);
        s.opt_len = s
            .opt_len
            .wrapping_add(f.wrapping_mul((bits + xbits) as u64));
        if let Some(stree) = stree {
            s.static_len = s
                .static_len
                .wrapping_add(f.wrapping_mul((i32::from(stree[n].dl) + xbits) as u64));
        }
    }
    if overflow == 0 {
        return;
    }

    // Find the first bit length which could increase.
    loop {
        let mut bits = max_length - 1;
        while s.bl_count[bits as usize] == 0 {
            bits -= 1;
        }
        s.bl_count[bits as usize] -= 1;
        s.bl_count[(bits + 1) as usize] += 2;
        s.bl_count[max_length as usize] -= 1;
        overflow -= 2;
        if overflow <= 0 {
            break;
        }
    }

    let mut h = HEAP_SIZE;
    let mut bits = max_length;
    while bits != 0 {
        let mut n = i32::from(s.bl_count[bits as usize]);
        while n != 0 {
            h -= 1;
            let m = s.heap[h] as usize;
            if m > max_code as usize {
                continue;
            }
            if u32::from(tree[m].dl) != bits as u32 {
                s.opt_len = s.opt_len.wrapping_add(
                    (bits as u64)
                        .wrapping_sub(u64::from(tree[m].dl))
                        .wrapping_mul(u64::from(tree[m].fc)),
                );
                tree[m].dl = bits as u16;
            }
            n -= 1;
        }
        bits -= 1;
    }
}

/// Port of `build_tree`: builds one Huffman tree from the frequencies in `tree`.
fn build_tree(s: &mut DeflateState, tree: &mut [CtData], desc: &mut TreeDesc) {
    let stree = desc.stat.static_tree;
    let elems = desc.stat.elems;
    let mut max_code: i32 = -1;

    s.heap_len = 0;
    s.heap_max = HEAP_SIZE as i32;

    for n in 0..elems {
        if tree[n].fc != 0 {
            s.heap_len += 1;
            max_code = n as i32;
            s.heap[s.heap_len as usize] = n as i32;
            s.depth[n] = 0;
        } else {
            tree[n].dl = 0;
        }
    }

    // The pkzip format requires at least one distance code and at least two bit-length codes,
    // so force the tree to have two leaves.
    while s.heap_len < 2 {
        let node = if max_code < 2 {
            max_code += 1;
            max_code
        } else {
            0
        };
        s.heap_len += 1;
        s.heap[s.heap_len as usize] = node;
        tree[node as usize].fc = 1;
        s.depth[node as usize] = 0;
        s.opt_len = s.opt_len.wrapping_sub(1);
        if let Some(stree) = stree {
            s.static_len = s
                .static_len
                .wrapping_sub(u64::from(stree[node as usize].dl));
        }
    }
    desc.max_code = max_code;

    let mut n = s.heap_len / 2;
    while n >= 1 {
        pqdownheap(s, tree, n as usize);
        n -= 1;
    }

    let mut node = elems;
    loop {
        // pqremove: the two smallest nodes.
        let n = s.heap[SMALLEST] as usize;
        s.heap[SMALLEST] = s.heap[s.heap_len as usize];
        s.heap_len -= 1;
        pqdownheap(s, tree, SMALLEST);
        let m = s.heap[SMALLEST] as usize;

        s.heap_max -= 1;
        s.heap[s.heap_max as usize] = n as i32;
        s.heap_max -= 1;
        s.heap[s.heap_max as usize] = m as i32;

        tree[node].fc = tree[n].fc.wrapping_add(tree[m].fc);
        s.depth[node] = (s.depth[n].max(s.depth[m])).wrapping_add(1);
        tree[n].dl = node as u16;
        tree[m].dl = node as u16;

        s.heap[SMALLEST] = node as i32;
        node += 1;
        pqdownheap(s, tree, SMALLEST);

        if s.heap_len < 2 {
            break;
        }
    }

    s.heap_max -= 1;
    s.heap[s.heap_max as usize] = s.heap[SMALLEST];

    gen_bitlen(s, tree, desc);

    let bl_count = s.bl_count;
    gen_codes(tree, max_code as usize, &bl_count);
}

/// Port of `scan_tree`: counts the bit-length codes that `send_tree` will send for `tree`.
fn scan_tree(bl_tree: &mut [CtData], tree: &mut [CtData], max_code: i32) {
    let mut prevlen: i32 = -1;
    let mut nextlen = i32::from(tree[0].dl);
    let mut count: i32 = 0;
    let mut max_count: i32 = 7;
    let mut min_count: i32 = 4;

    if nextlen == 0 {
        max_count = 138;
        min_count = 3;
    }
    // Guard, so that the last iteration sees a length that matches nothing.
    tree[(max_code + 1) as usize].dl = 0xffff;

    for n in 0..=max_code as usize {
        let curlen = nextlen;
        nextlen = i32::from(tree[n + 1].dl);
        count += 1;
        if count < max_count && curlen == nextlen {
            continue;
        }
        if count < min_count {
            let slot = &mut bl_tree[curlen as usize].fc;
            *slot = slot.wrapping_add(count as u16);
        } else if curlen != 0 {
            if curlen != prevlen {
                let slot = &mut bl_tree[curlen as usize].fc;
                *slot = slot.wrapping_add(1);
            }
            bl_tree[REP_3_6].fc = bl_tree[REP_3_6].fc.wrapping_add(1);
        } else if count <= 10 {
            bl_tree[REPZ_3_10].fc = bl_tree[REPZ_3_10].fc.wrapping_add(1);
        } else {
            bl_tree[REPZ_11_138].fc = bl_tree[REPZ_11_138].fc.wrapping_add(1);
        }
        count = 0;
        prevlen = curlen;
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        } else if curlen == nextlen {
            max_count = 6;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }
    }
}

/// Port of `send_tree`: sends the code lengths of `tree` with the bit-length codes `bl_tree`.
fn send_tree(s: &mut DeflateState, bl_tree: &[CtData], tree: &[CtData], max_code: i32) {
    let mut prevlen: i32 = -1;
    let mut nextlen = i32::from(tree[0].dl);
    let mut count: i32 = 0;
    let mut max_count: i32 = 7;
    let mut min_count: i32 = 4;

    if nextlen == 0 {
        max_count = 138;
        min_count = 3;
    }

    for n in 0..=max_code as usize {
        let curlen = nextlen;
        nextlen = i32::from(tree[n + 1].dl);
        count += 1;
        if count < max_count && curlen == nextlen {
            continue;
        }
        if count < min_count {
            loop {
                send_code(s, curlen as usize, bl_tree);
                count -= 1;
                if count == 0 {
                    break;
                }
            }
        } else if curlen != 0 {
            if curlen != prevlen {
                send_code(s, curlen as usize, bl_tree);
                count -= 1;
            }
            send_code(s, REP_3_6, bl_tree);
            send_bits(s, (count - 3) as u32, 2);
        } else if count <= 10 {
            send_code(s, REPZ_3_10, bl_tree);
            send_bits(s, (count - 3) as u32, 3);
        } else {
            send_code(s, REPZ_11_138, bl_tree);
            send_bits(s, (count - 11) as u32, 7);
        }
        count = 0;
        prevlen = curlen;
        if nextlen == 0 {
            max_count = 138;
            min_count = 3;
        } else if curlen == nextlen {
            max_count = 6;
            min_count = 3;
        } else {
            max_count = 7;
            min_count = 4;
        }
    }
}

/// Port of `build_bl_tree`: builds the bit-length tree and returns the index of the last code
/// that is sent.
fn build_bl_tree(s: &mut DeflateState) -> usize {
    let mut ltree = std::mem::take(&mut s.dyn_ltree);
    let mut dtree = std::mem::take(&mut s.dyn_dtree);
    let mut bl = std::mem::take(&mut s.bl_tree);

    scan_tree(&mut bl, &mut ltree, s.l_desc.max_code);
    scan_tree(&mut bl, &mut dtree, s.d_desc.max_code);

    let mut bl_desc = s.bl_desc;
    build_tree(s, &mut bl, &mut bl_desc);
    s.bl_desc = bl_desc;

    // The lengths of the bit-length codes are sent in `bl_order`. Index 2 is the floor: the
    // format sends at least four of them.
    let mut max_blindex = BL_CODES - 1;
    while max_blindex >= 3 {
        if bl[BL_ORDER[max_blindex]].dl != 0 {
            break;
        }
        max_blindex -= 1;
    }
    s.opt_len = s
        .opt_len
        .wrapping_add(3 * (max_blindex as u64 + 1) + 5 + 5 + 4);

    s.dyn_ltree = ltree;
    s.dyn_dtree = dtree;
    s.bl_tree = bl;
    max_blindex
}

/// Port of `send_all_trees`: writes the header of a dynamic block.
fn send_all_trees(s: &mut DeflateState, lcodes: i32, dcodes: i32, blcodes: i32) {
    send_bits(s, (lcodes - 257) as u32, 5);
    send_bits(s, (dcodes - 1) as u32, 5);
    send_bits(s, (blcodes - 4) as u32, 4);
    let bl = std::mem::take(&mut s.bl_tree);
    for rank in 0..blcodes as usize {
        send_bits(s, u32::from(bl[BL_ORDER[rank]].dl), 3);
    }
    s.bl_tree = bl;

    let ltree = std::mem::take(&mut s.dyn_ltree);
    let dtree = std::mem::take(&mut s.dyn_dtree);
    let bl = std::mem::take(&mut s.bl_tree);
    send_tree(s, &bl, &ltree, lcodes - 1);
    send_tree(s, &bl, &dtree, dcodes - 1);
    s.dyn_ltree = ltree;
    s.dyn_dtree = dtree;
    s.bl_tree = bl;
}

/// Port of `_tr_stored_block`: writes a stored block of `stored_len` bytes copied from the window
/// at `buf` (`None` when the length is 0 and no data is copied).
// Port of: trees.c#L858-L873 (_tr_stored_block)
pub(crate) fn tr_stored_block(
    s: &mut DeflateState,
    buf: Option<usize>,
    stored_len: u64,
    last: i32,
) {
    send_bits(s, (STORED_BLOCK << 1) + last as u32, 3);
    bi_windup(s);
    put_short(s, stored_len as u16);
    put_short(s, (!stored_len) as u16);
    if let Some(start) = buf.filter(|_| stored_len != 0) {
        let n = stored_len as usize;
        let p = s.pending as usize;
        s.pending_buf[p..p + n].copy_from_slice(&s.window[start..start + n]);
    }
    s.pending += stored_len;
}

/// Port of `_tr_flush_bits`.
pub(crate) fn tr_flush_bits(s: &mut DeflateState) {
    bi_flush(s);
}

/// Port of `_tr_align`: sends an empty static block so that the output can be flushed.
pub(crate) fn tr_align(s: &mut DeflateState) {
    send_bits(s, STATIC_TREES << 1, 3);
    send_code(s, END_BLOCK, &STATIC_LTREE);
    bi_flush(s);
}

/// Port of `compress_block`: writes the symbols of the block with the given trees.
fn compress_block(s: &mut DeflateState, ltree: &[CtData], dtree: &[CtData]) {
    let mut sx: usize = 0;
    if s.sym_next != 0 {
        loop {
            let mut dist = u32::from(s.d_buf[sx]);
            let mut lc = u32::from(s.l_buf[sx]);
            sx += 1;
            if dist == 0 {
                send_code(s, lc as usize, ltree);
            } else {
                // Here `lc` is the match length minus MIN_MATCH.
                let mut code = LENGTH_CODE[lc as usize] as usize;
                send_code(s, code + LITERALS as usize + 1, ltree);
                let mut extra = EXTRA_LBITS[code];
                if extra != 0 {
                    lc -= BASE_LENGTH[code] as u32;
                    send_bits(s, lc, extra);
                }
                dist -= 1;
                code = d_code(dist);
                send_code(s, code, dtree);
                extra = EXTRA_DBITS[code];
                if extra != 0 {
                    dist -= BASE_DIST[code] as u32;
                    send_bits(s, dist, extra);
                }
            }
            if sx >= s.sym_next as usize {
                break;
            }
        }
    }
    send_code(s, END_BLOCK, ltree);
}

/// Port of `detect_data_type`: guesses whether the input is binary or text.
fn detect_data_type(s: &DeflateState) -> i32 {
    // Bits set for the control codes that are never seen in text.
    let mut block_mask: u32 = 0xf3ff_c07f;
    for n in 0..=31usize {
        if block_mask & 1 != 0 && s.dyn_ltree[n].fc != 0 {
            return Z_BINARY;
        }
        block_mask >>= 1;
    }
    if s.dyn_ltree[9].fc != 0 || s.dyn_ltree[10].fc != 0 || s.dyn_ltree[13].fc != 0 {
        return Z_TEXT;
    }
    for n in 32..LITERALS as usize {
        if s.dyn_ltree[n].fc != 0 {
            return Z_TEXT;
        }
    }
    Z_BINARY
}

/// Port of `_tr_flush_block`: writes one block, choosing stored, fixed or dynamic coding.
/// `buf` is the window offset of the block's data (`None` when it is no longer in the window).
// Port of: trees.c#L996-L1088 (_tr_flush_block)
pub(crate) fn tr_flush_block(
    s: &mut DeflateState,
    data_type: &mut i32,
    buf: Option<usize>,
    stored_len: u64,
    last: i32,
) {
    let opt_lenb: u64;
    let static_lenb: u64;
    let mut max_blindex: usize = 0;

    if s.level > 0 {
        if *data_type == Z_UNKNOWN {
            *data_type = detect_data_type(s);
        }
        build_kind(s, TreeKind::Lit);
        build_kind(s, TreeKind::Dist);
        max_blindex = build_bl_tree(s);

        let mut o = s.opt_len.wrapping_add(3 + 7) >> 3;
        let st = s.static_len.wrapping_add(3 + 7) >> 3;
        if st <= o || s.strategy == Z_FIXED {
            o = st;
        }
        opt_lenb = o;
        static_lenb = st;
    } else {
        opt_lenb = stored_len + 5;
        static_lenb = opt_lenb;
    }

    if stored_len + 4 <= opt_lenb && buf.is_some() {
        tr_stored_block(s, buf, stored_len, last);
    } else if static_lenb == opt_lenb {
        send_bits(s, (STATIC_TREES << 1) + last as u32, 3);
        compress_block(s, &STATIC_LTREE, &STATIC_DTREE);
    } else {
        send_bits(s, (DYN_TREES << 1) + last as u32, 3);
        let lcodes = s.l_desc.max_code + 1;
        let dcodes = s.d_desc.max_code + 1;
        send_all_trees(s, lcodes, dcodes, max_blindex as i32 + 1);
        let ltree = std::mem::take(&mut s.dyn_ltree);
        let dtree = std::mem::take(&mut s.dyn_dtree);
        compress_block(s, &ltree, &dtree);
        s.dyn_ltree = ltree;
        s.dyn_dtree = dtree;
    }

    init_block(s);

    if last != 0 {
        bi_windup(s);
    }
}

/// Builds one of the literal and distance trees (`build_tree` on `l_desc`, `d_desc`).
fn build_kind(s: &mut DeflateState, kind: TreeKind) {
    match kind {
        TreeKind::Lit => {
            let mut t = std::mem::take(&mut s.dyn_ltree);
            let mut d = s.l_desc;
            build_tree(s, &mut t, &mut d);
            s.l_desc = d;
            s.dyn_ltree = t;
        }
        TreeKind::Dist => {
            let mut t = std::mem::take(&mut s.dyn_dtree);
            let mut d = s.d_desc;
            build_tree(s, &mut t, &mut d);
            s.d_desc = d;
            s.dyn_dtree = t;
        }
    }
}

/// Port of `_tr_tally_lit` (the `LIT_MEM` form in deflate.h): records a literal. Returns true when
/// the symbol buffer is full and the block must be flushed.
// Port of: deflate.h#L335-L342 (_tr_tally_lit, LIT_MEM)
pub(crate) fn tally_lit(s: &mut DeflateState, c: u8) -> bool {
    let i = s.sym_next as usize;
    s.d_buf[i] = 0;
    s.l_buf[i] = c;
    s.sym_next += 1;
    let f = &mut s.dyn_ltree[c as usize].fc;
    *f = f.wrapping_add(1);
    s.sym_next == s.sym_end
}

/// Port of `_tr_tally_dist` (the `LIT_MEM` form in deflate.h): records a match of `length` bytes at
/// `distance`. Returns true when the symbol buffer is full.
// Port of: deflate.h#L343-L352 (_tr_tally_dist, LIT_MEM)
pub(crate) fn tally_dist(s: &mut DeflateState, distance: u32, length: u32) -> bool {
    let len = length as u8;
    let dist = distance as u16;
    let i = s.sym_next as usize;
    s.d_buf[i] = dist;
    s.l_buf[i] = len;
    s.sym_next += 1;
    let dist = dist.wrapping_sub(1);
    let lt = LENGTH_CODE[len as usize] as usize + LITERALS as usize + 1;
    s.dyn_ltree[lt].fc = s.dyn_ltree[lt].fc.wrapping_add(1);
    let dc = d_code(u32::from(dist));
    s.dyn_dtree[dc].fc = s.dyn_dtree[dc].fc.wrapping_add(1);
    s.sym_next == s.sym_end
}
