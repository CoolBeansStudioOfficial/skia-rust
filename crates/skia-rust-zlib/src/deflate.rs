// Copyright 1995-2023 Jean-loup Gailly and Mark Adler (zlib); port by The skia-rust Authors.
// Use of this source code is governed by the zlib licence in the LICENSE file.
// Port of: deflate.c#L1-L1381 (chromium zlib@646b7f56, zlib 1.3.0.1-motley), the zlib-format
// compressor. Ported from: deflate.c, deflate.h, contrib/optimizations/insert_string.h.

//! The deflate compressor (RFC 1951) inside a zlib (RFC 1950) wrapper, as Chromium's zlib builds
//! it. Skia's PNG encoder compresses its IDAT stream with this at the level it is given, so the
//! output bytes depend on every choice here: Chromium's ANZAC hash (`insert_string`), the
//! `LIT_MEM` symbol layout, the `configuration_table`, and the block-type decision in `trees`.
//!
//! The call interface is [`Deflate::deflate`], which reads from the front of the input and writes
//! at the front of the output, as the inflate port does. zlib's `z_stream` pointers become slices
//! and offsets, and `strm->total_in`, `adler` and `msg` are kept by the stream.
//!
//! Not ported: gzip framing (`windowBits > 15`), preset dictionaries (`deflateSetDictionary`),
//! `deflateParams` and `deflateTune`, `deflateCopy`, and the Rabin-Karp hash
//! (`CHROMIUM_ZLIB_NO_CASTAGNOLI`), which Chromium only builds without its default hash. None of
//! them is reached by the PNG encoder.

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
use crate::adler32::{ADLER32_INIT, adler32};
use crate::trees::{
    CtData, D_CODES, HEAP_SIZE, MAX_BITS, MAX_MATCH, MIN_MATCH, STATIC_BL_DESC, STATIC_D_DESC,
    STATIC_L_DESC, TreeDesc, Z_FIXED, Z_UNKNOWN, tally_dist, tally_lit, tr_align, tr_flush_bits,
    tr_flush_block, tr_init, tr_stored_block,
};
use crate::{Flush, ReturnCode};

/// Port of `NIL`.
const NIL: u32 = 0;
/// Port of `TOO_FAR`.
const TOO_FAR: u32 = 4096;
/// Port of `MIN_LOOKAHEAD`.
const MIN_LOOKAHEAD: u32 = MAX_MATCH + MIN_MATCH + 1;
/// Port of `WIN_INIT`: how much of the window beyond the data is zeroed.
const WIN_INIT: u64 = MAX_MATCH as u64;
/// Port of `MAX_STORED`.
const MAX_STORED: u64 = 65535;
/// Port of `DEF_MEM_LEVEL`.
pub const DEF_MEM_LEVEL: i32 = 8;
/// Port of `MAX_MEM_LEVEL`.
const MAX_MEM_LEVEL: i32 = 9;
/// Port of `Z_DEFLATED`.
const Z_DEFLATED: u32 = 8;
/// Port of `PRESET_DICT`.
const PRESET_DICT: u32 = 0x20;
/// Port of `Z_DEFAULT_COMPRESSION`.
pub const Z_DEFAULT_COMPRESSION: i32 = -1;

/// Port of `INIT_STATE`.
const INIT_STATE: i32 = 42;
/// Port of `BUSY_STATE`.
const BUSY_STATE: i32 = 113;
/// Port of `FINISH_STATE`.
const FINISH_STATE: i32 = 666;

/// Port of the `Z_*` flush values as integers, which deflate's ordering (`RANK`) needs.
fn flush_value(flush: Flush) -> i32 {
    match flush {
        Flush::NoFlush => 0,
        Flush::PartialFlush => 1,
        Flush::SyncFlush => 2,
        Flush::FullFlush => 3,
        Flush::Finish => 4,
        Flush::Block => 5,
        Flush::Trees => 6,
    }
}

/// Port of `Z_NO_FLUSH`, `Z_PARTIAL_FLUSH`, `Z_FULL_FLUSH`, `Z_FINISH`, `Z_BLOCK` as integers.
const Z_NO_FLUSH: i32 = 0;
const Z_PARTIAL_FLUSH: i32 = 1;
const Z_FULL_FLUSH: i32 = 3;
/// Port of `Z_FILTERED`, the strategy value.
const Z_FILTERED: i32 = 1;
const Z_FINISH: i32 = 4;
const Z_BLOCK: i32 = 5;

/// Port of `RANK(f)`: orders the flush values so that a repeated flush is detected.
fn rank(f: i32) -> i32 {
    (f * 2) - if f > 4 { 9 } else { 0 }
}

/// Port of `block_state`: what a compression function returned.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum BlockState {
    /// Port of `need_more`.
    NeedMore,
    /// Port of `block_done`.
    BlockDone,
    /// Port of `finish_started`.
    FinishStarted,
    /// Port of `finish_done`.
    FinishDone,
}

/// The compression function of a level (`configuration_table[level].func`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompressFunc {
    Stored,
    Fast,
    Slow,
}

/// Port of `config`: the per-level tuning.
#[derive(Debug, Clone, Copy)]
struct Config {
    good_length: u32,
    max_lazy: u32,
    nice_length: u32,
    max_chain: u32,
    func: CompressFunc,
}

/// Port of `configuration_table`.
static CONFIGURATION_TABLE: [Config; 10] = [
    Config {
        good_length: 0,
        max_lazy: 0,
        nice_length: 0,
        max_chain: 0,
        func: CompressFunc::Stored,
    },
    Config {
        good_length: 4,
        max_lazy: 4,
        nice_length: 8,
        max_chain: 4,
        func: CompressFunc::Fast,
    },
    Config {
        good_length: 4,
        max_lazy: 5,
        nice_length: 16,
        max_chain: 8,
        func: CompressFunc::Fast,
    },
    Config {
        good_length: 4,
        max_lazy: 6,
        nice_length: 32,
        max_chain: 32,
        func: CompressFunc::Fast,
    },
    Config {
        good_length: 4,
        max_lazy: 4,
        nice_length: 16,
        max_chain: 16,
        func: CompressFunc::Slow,
    },
    Config {
        good_length: 8,
        max_lazy: 16,
        nice_length: 32,
        max_chain: 32,
        func: CompressFunc::Slow,
    },
    Config {
        good_length: 8,
        max_lazy: 16,
        nice_length: 128,
        max_chain: 128,
        func: CompressFunc::Slow,
    },
    Config {
        good_length: 8,
        max_lazy: 32,
        nice_length: 128,
        max_chain: 256,
        func: CompressFunc::Slow,
    },
    Config {
        good_length: 32,
        max_lazy: 128,
        nice_length: 258,
        max_chain: 1024,
        func: CompressFunc::Slow,
    },
    Config {
        good_length: 32,
        max_lazy: 258,
        nice_length: 258,
        max_chain: 4096,
        func: CompressFunc::Slow,
    },
];

/// The `z_stream` fields that deflate reads and writes while it runs, with `next_in` and
/// `next_out` as slices and offsets into them.
pub(crate) struct Strm<'a> {
    /// The whole input slice; `in_pos` is zlib's `next_in` as an offset.
    pub(crate) input: &'a [u8],
    /// Offset of `next_in` in `input`.
    pub(crate) in_pos: usize,
    /// The whole output slice; `out_pos` is zlib's `next_out` as an offset.
    pub(crate) output: &'a mut [u8],
    /// Offset of `next_out` in `output`.
    pub(crate) out_pos: usize,
    /// Port of `total_in`.
    pub(crate) total_in: u64,
    /// Port of `total_out`.
    pub(crate) total_out: u64,
    /// Port of `adler`: the running Adler-32 (or the zlib header's checksum for a preset dictionary).
    pub(crate) adler: u32,
    /// Port of `data_type`.
    pub(crate) data_type: i32,
    /// Port of `msg`.
    pub(crate) msg: Option<&'static str>,
}

impl Strm<'_> {
    /// Port of `avail_in`.
    fn avail_in(&self) -> usize {
        self.input.len() - self.in_pos
    }

    /// Port of `avail_out`.
    fn avail_out(&self) -> usize {
        self.output.len() - self.out_pos
    }

    /// Port of `read_buf` (deflate.c#L212-L237): copies up to `dst.len()` input bytes to `dst`,
    /// and updates the checksum when the zlib wrapper is on (`wrap == 1`).
    fn read_buf(&mut self, wrap: i32, dst: &mut [u8]) -> usize {
        let len = self.avail_in().min(dst.len());
        if len == 0 {
            return 0;
        }
        dst[..len].copy_from_slice(&self.input[self.in_pos..self.in_pos + len]);
        self.in_pos += len;
        if wrap == 1 {
            self.adler = adler32(self.adler, &dst[..len]);
        }
        self.total_in += len as u64;
        len
    }

    /// `read_buf(strm, strm->next_out, len)` with the output's offset advanced by `len`.
    fn read_into_output(&mut self, wrap: i32, len: usize) {
        let out = std::mem::take(&mut self.output);
        let start = self.out_pos;
        self.read_buf(wrap, &mut out[start..start + len]);
        self.output = out;
        self.out_pos += len;
        self.total_out += len as u64;
    }
}

/// Port of `struct internal_state` (deflate.h#L104-L285), with the `LIT_MEM` symbol buffers.
pub(crate) struct DeflateState {
    /// Port of `status`.
    status: i32,
    /// Port of `pending_buf`. Its first `4 * lit_bufsize` bytes hold the pending output.
    pub(crate) pending_buf: Vec<u8>,
    /// Port of `pending_buf_size`.
    pending_buf_size: u64,
    /// Port of `pending_out`, as an offset into `pending_buf`.
    pending_out: usize,
    /// Port of `pending`.
    pub(crate) pending: u64,
    /// Port of `wrap`: 0 raw, 1 zlib, negative after the trailer (finished).
    wrap: i32,
    /// Port of `last_flush`.
    last_flush: i32,
    /// Port of `w_size`.
    w_size: u32,
    /// Port of `w_bits`.
    w_bits: u32,
    /// Port of `w_mask`.
    w_mask: u32,
    /// Port of `window`, with the `WIN_INIT` padding.
    pub(crate) window: Vec<u8>,
    /// Port of `window_size`.
    window_size: u64,
    /// Port of `prev`.
    prev: Vec<u16>,
    /// Port of `head`.
    head: Vec<u16>,
    /// Port of `ins_h`.
    ins_h: u32,
    /// Port of `hash_size`.
    hash_size: u32,
    /// Port of `hash_mask`.
    hash_mask: u32,
    /// Port of `block_start`: a window offset, which can become negative when the window slides.
    block_start: i64,
    /// Port of `match_length`.
    match_length: u32,
    /// Port of `prev_match`.
    prev_match: u32,
    /// Port of `match_available`.
    match_available: i32,
    /// Port of `strstart`.
    strstart: u32,
    /// Port of `match_start`.
    match_start: u32,
    /// Port of `lookahead`.
    lookahead: u32,
    /// Port of `prev_length`.
    prev_length: u32,
    /// Port of `max_chain_length`.
    max_chain_length: u32,
    /// Port of `max_lazy_match` (`max_insert_length`).
    max_lazy_match: u32,
    /// Port of `level`.
    pub(crate) level: i32,
    /// Port of `strategy`.
    pub(crate) strategy: i32,
    /// Port of `good_match`.
    good_match: u32,
    /// Port of `nice_match`.
    nice_match: i32,
    /// Port of `dyn_ltree`.
    pub(crate) dyn_ltree: Vec<CtData>,
    /// Port of `dyn_dtree`.
    pub(crate) dyn_dtree: Vec<CtData>,
    /// Port of `bl_tree`.
    pub(crate) bl_tree: Vec<CtData>,
    /// Port of `l_desc`.
    pub(crate) l_desc: TreeDesc,
    /// Port of `d_desc`.
    pub(crate) d_desc: TreeDesc,
    /// Port of `bl_desc`.
    pub(crate) bl_desc: TreeDesc,
    /// Port of `bl_count`.
    pub(crate) bl_count: [u16; MAX_BITS + 1],
    /// Port of `heap`.
    pub(crate) heap: [i32; HEAP_SIZE],
    /// Port of `heap_len`.
    pub(crate) heap_len: i32,
    /// Port of `heap_max`.
    pub(crate) heap_max: i32,
    /// Port of `depth`.
    pub(crate) depth: [u8; HEAP_SIZE],
    /// Port of `d_buf`: the match distances of the block (the `LIT_MEM` layout).
    pub(crate) d_buf: Vec<u16>,
    /// Port of `l_buf`: the literals and match lengths of the block (the `LIT_MEM` layout).
    pub(crate) l_buf: Vec<u8>,
    /// Port of `sym_next`.
    pub(crate) sym_next: u32,
    /// Port of `sym_end`.
    pub(crate) sym_end: u32,
    /// Port of `opt_len`.
    pub(crate) opt_len: u64,
    /// Port of `static_len`.
    pub(crate) static_len: u64,
    /// Port of `matches`.
    pub(crate) matches: u32,
    /// Port of `insert`.
    insert: u32,
    /// Port of `bi_buf`.
    pub(crate) bi_buf: u16,
    /// Port of `bi_valid`.
    pub(crate) bi_valid: i32,
    /// Port of `high_water`.
    high_water: u64,
    /// Port of `chromium_zlib_hash`: always set here (see the module docs).
    chromium_zlib_hash: bool,
}

/// Port of `MAX_DIST(s)`.
#[inline]
fn max_dist(s: &DeflateState) -> u32 {
    s.w_size - MIN_LOOKAHEAD
}

/// Port of `put_byte` (deflate.h#L290).
#[inline]
fn put_byte(s: &mut DeflateState, c: u8) {
    s.pending_buf[s.pending as usize] = c;
    s.pending += 1;
}

/// Port of `putShortMSB`.
#[inline]
fn put_short_msb(s: &mut DeflateState, b: u32) {
    put_byte(s, (b >> 8) as u8);
    put_byte(s, (b & 0xff) as u8);
}

/// Port of `CLEAR_HASH`.
fn clear_hash(s: &mut DeflateState) {
    let last = s.hash_size as usize - 1;
    s.head[last] = 0;
    s.head[..last].fill(0);
}

/// Port of `slide_hash`: after the window slides by `w_size`, rebases the hash chains.
fn slide_hash(s: &mut DeflateState) {
    let wsize = s.w_size as u16;
    for p in &mut s.head {
        *p = p.saturating_sub(wsize);
    }
    for p in s.prev.iter_mut().take(s.w_size as usize) {
        *p = p.saturating_sub(wsize);
    }
}

/// Port of `insert_string` (`contrib/optimizations/insert_string.h`): inserts `str` in the
/// dictionary and returns the previous head of its hash chain.
// Port of: contrib/optimizations/insert_string.h#L35-L58 (insert_string, ANZAC hash)
fn insert_string(s: &mut DeflateState, str_: u32) -> u32 {
    let i = str_ as usize;
    // Little-endian read of four bytes, as zmemcpy into a uint32_t.
    let value = u32::from_le_bytes([
        s.window[i],
        s.window[i + 1],
        s.window[i + 2],
        s.window[i + 3],
    ]);
    s.ins_h = (value.wrapping_mul(66521).wrapping_add(66521) >> 16) & s.hash_mask;
    let h = s.ins_h as usize;
    let ret = u32::from(s.head[h]);
    s.prev[(str_ & s.w_mask) as usize] = s.head[h];
    s.head[h] = str_ as u16;
    ret
}

/// Port of `read_buf`'s caller in `fill_window`, and `fill_window` (deflate.c#L249-L380): reads
/// input into the window, sliding it when it is full, and inserts the pending strings in the hash.
// Port of: deflate.c#L249-L380 (fill_window)
fn fill_window(s: &mut DeflateState, strm: &mut Strm<'_>) {
    let wsize = s.w_size;

    loop {
        let mut more = (s
            .window_size
            .wrapping_sub(u64::from(s.lookahead))
            .wrapping_sub(u64::from(s.strstart))) as u32;

        // If the window is almost full and there is insufficient lookahead, move the upper half
        // to the lower one to make room in the upper half.
        if s.strstart >= wsize + max_dist(s) {
            let keep = (wsize - more) as usize;
            let src = wsize as usize;
            s.window.copy_within(src..src + keep, 0);
            s.match_start = s.match_start.wrapping_sub(wsize);
            s.strstart -= wsize;
            s.block_start -= i64::from(wsize);
            if s.insert > s.strstart {
                s.insert = s.strstart;
            }
            slide_hash(s);
            more += wsize;
        }
        if strm.avail_in() == 0 {
            break;
        }

        // Read as much as possible to fill the window.
        let start = (s.strstart + s.lookahead) as usize;
        let n = strm.read_buf(s.wrap, &mut s.window[start..start + more as usize]);
        s.lookahead += n as u32;

        // Initialize the hash value now that we have some input: this is the chromium hash path
        // (insert_string on each pending string).
        if s.chromium_zlib_hash && s.lookahead + s.insert > MIN_MATCH {
            let mut str_ = s.strstart - s.insert;
            while s.insert != 0 {
                insert_string(s, str_);
                str_ += 1;
                s.insert -= 1;
                if s.lookahead + s.insert <= MIN_MATCH {
                    break;
                }
            }
        }

        if !(s.lookahead < MIN_LOOKAHEAD && strm.avail_in() != 0) {
            break;
        }
    }

    // Zero the part of the window that has not been written, so that the comparisons in
    // `longest_match` past the data read deterministic bytes (zlib's high_water).
    if s.high_water < s.window_size {
        let curr = u64::from(s.strstart) + u64::from(s.lookahead);
        if s.high_water < curr {
            let mut init = s.window_size - curr;
            if init > WIN_INIT {
                init = WIN_INIT;
            }
            let a = curr as usize;
            s.window[a..a + init as usize].fill(0);
            s.high_water = curr + init;
        } else if s.high_water < curr + WIN_INIT {
            let mut init = curr + WIN_INIT - s.high_water;
            if init > s.window_size - s.high_water {
                init = s.window_size - s.high_water;
            }
            let a = s.high_water as usize;
            s.window[a..a + init as usize].fill(0);
            s.high_water += init;
        }
    }
}

/// Port of `lm_init`: resets the matching state at the start of a stream.
fn lm_init(s: &mut DeflateState) {
    s.window_size = 2 * u64::from(s.w_size);
    clear_hash(s);

    let c = CONFIGURATION_TABLE[s.level as usize];
    s.max_lazy_match = c.max_lazy;
    s.good_match = c.good_length;
    s.nice_match = c.nice_length as i32;
    s.max_chain_length = c.max_chain;

    s.strstart = 0;
    s.block_start = 0;
    s.lookahead = 0;
    s.insert = 0;
    s.match_length = MIN_MATCH - 1;
    s.prev_length = MIN_MATCH - 1;
    s.match_available = 0;
    s.ins_h = 0;
}

/// Port of `longest_match` (the byte-compare path of deflate.c, which Chromium also builds with
/// `UNALIGNED_OK`; the two compare the same bytes, skipping byte 2 as the hash already fixes it).
// Port of: deflate.c#L1440-L1553 (longest_match)
fn longest_match(s: &mut DeflateState, mut cur_match: u32) -> u32 {
    let mut chain_length = s.max_chain_length;
    let scan = s.strstart as usize;
    let mut best_len = s.prev_length as i32;
    let mut nice_match = s.nice_match;
    let limit: u32 = if s.strstart > max_dist(s) {
        s.strstart - max_dist(s)
    } else {
        NIL
    };
    let wmask = s.w_mask;
    let strend = scan + MAX_MATCH as usize;
    let mut scan_end1 = s.window[scan + best_len as usize - 1];
    let mut scan_end = s.window[scan + best_len as usize];

    // Do not waste too much time if we already have a good match.
    if s.prev_length >= s.good_match {
        chain_length >>= 2;
    }
    // Do not look for matches beyond the end of the input.
    if nice_match as u32 > s.lookahead {
        nice_match = s.lookahead as i32;
    }

    loop {
        let m = cur_match as usize;
        // Skip to the next match if the match length cannot increase or if it is less than 2.
        let candidate = !(s.window[m + best_len as usize] != scan_end
            || s.window[m + best_len as usize - 1] != scan_end1
            || s.window[m] != s.window[scan]
            || s.window[m + 1] != s.window[scan + 1]);
        if candidate {
            // Byte 2 is not compared: the hash keys are equal, which fixes it.
            let mut sc = scan + 2;
            let mut mt = m + 2;
            loop {
                let mut ok = true;
                for _ in 0..8 {
                    sc += 1;
                    mt += 1;
                    if s.window[sc] != s.window[mt] {
                        ok = false;
                        break;
                    }
                }
                if !ok || sc >= strend {
                    break;
                }
            }
            let len = MAX_MATCH as i32 - (strend - sc) as i32;

            if len > best_len {
                s.match_start = cur_match;
                best_len = len;
                if len >= nice_match {
                    break;
                }
                scan_end1 = s.window[scan + best_len as usize - 1];
                scan_end = s.window[scan + best_len as usize];
            }
        }
        cur_match = u32::from(s.prev[(cur_match & wmask) as usize]);
        if cur_match <= limit {
            break;
        }
        chain_length = chain_length.wrapping_sub(1);
        if chain_length == 0 {
            break;
        }
    }

    if best_len as u32 <= s.lookahead {
        best_len as u32
    } else {
        s.lookahead
    }
}

/// Port of `flush_pending`: copies as much pending output as the stream has room for.
fn flush_pending(s: &mut DeflateState, strm: &mut Strm<'_>) {
    tr_flush_bits(s);
    let len = (s.pending as usize).min(strm.avail_out());
    if len == 0 {
        return;
    }
    let p = s.pending_out;
    let o = strm.out_pos;
    strm.output[o..o + len].copy_from_slice(&s.pending_buf[p..p + len]);
    strm.out_pos += len;
    s.pending_out += len;
    strm.total_out += len as u64;
    s.pending -= len as u64;
    if s.pending == 0 {
        s.pending_out = 0;
    }
}

/// Port of `FLUSH_BLOCK_ONLY`: writes the block that starts at `block_start`.
fn flush_block_only(s: &mut DeflateState, strm: &mut Strm<'_>, last: i32) {
    let buf = if s.block_start >= 0 {
        Some(s.block_start as usize)
    } else {
        None
    };
    let stored_len = (i64::from(s.strstart) - s.block_start) as u64;
    tr_flush_block(s, &mut strm.data_type, buf, stored_len, last);
    s.block_start = i64::from(s.strstart);
    flush_pending(s, strm);
}

/// Port of `FLUSH_BLOCK`: flushes a block and returns from the caller when the output is full.
macro_rules! flush_block {
    ($s:ident, $strm:ident, $last:expr) => {{
        flush_block_only($s, $strm, $last);
        if $strm.avail_out() == 0 {
            return if $last != 0 {
                BlockState::FinishStarted
            } else {
                BlockState::NeedMore
            };
        }
    }};
}

/// Port of `deflate_stored`: level 0, copies the input into stored blocks.
// Port of: deflate.c#L1684-L1859 (deflate_stored)
fn deflate_stored(s: &mut DeflateState, strm: &mut Strm<'_>, flush: i32) -> BlockState {
    // Smallest worthy block size when not flushing or finishing.
    let mut min_block = (s.pending_buf_size - 5).min(u64::from(s.w_size));

    let mut last: i32 = 0;
    let mut used = strm.avail_in() as u64;

    loop {
        // Set len to the maximum size block that we can copy directly with the available input
        // data and output space.
        let mut len = MAX_STORED;
        let mut have = (u64::from(s.bi_valid as u32) + 42) >> 3;
        if (strm.avail_out() as u64) < have {
            break;
        }
        have = strm.avail_out() as u64 - have;
        let mut left = (i64::from(s.strstart) - s.block_start) as u64;
        if len > left + strm.avail_in() as u64 {
            len = left + strm.avail_in() as u64;
        }
        if len > have {
            len = have;
        }
        if len < min_block
            && ((len == 0 && flush != Z_FINISH)
                || flush == Z_NO_FLUSH
                || len != left + strm.avail_in() as u64)
        {
            break;
        }

        last = i32::from(flush == Z_FINISH && len == left + strm.avail_in() as u64);
        tr_stored_block(s, None, 0, last);

        // Replace the lengths in the dummy stored block with len.
        let p = s.pending as usize;
        s.pending_buf[p - 4] = len as u8;
        s.pending_buf[p - 3] = (len >> 8) as u8;
        s.pending_buf[p - 2] = !(len as u8);
        s.pending_buf[p - 1] = (!len >> 8) as u8;

        flush_pending(s, strm);

        // Copy uncompressed bytes from the window to next_out.
        if left != 0 {
            if left > len {
                left = len;
            }
            let o = strm.out_pos;
            let b = s.block_start as usize;
            let n = left as usize;
            strm.output[o..o + n].copy_from_slice(&s.window[b..b + n]);
            strm.out_pos += n;
            strm.total_out += left;
            s.block_start += left as i64;
            len -= left;
        }

        // Copy uncompressed bytes directly from next_in to next_out, updating the check value.
        if len != 0 {
            strm.read_into_output(s.wrap, len as usize);
        }
        if last != 0 {
            break;
        }
    }

    // Update the sliding window with the last s->w_size bytes of the copied data, or append all
    // of the copied data to the existing window if less than s->w_size bytes were copied.
    used -= strm.avail_in() as u64;
    if used != 0 {
        if used >= u64::from(s.w_size) {
            s.matches = 2;
            let w = s.w_size as usize;
            let src = strm.in_pos - w;
            s.window[..w].copy_from_slice(&strm.input[src..src + w]);
            s.strstart = s.w_size;
            s.insert = s.strstart;
        } else {
            if s.window_size - u64::from(s.strstart) <= used {
                // Slide the window down.
                s.strstart -= s.w_size;
                let keep = s.strstart as usize;
                let src = s.w_size as usize;
                s.window.copy_within(src..src + keep, 0);
                if s.matches < 2 {
                    s.matches += 1;
                }
                if s.insert > s.strstart {
                    s.insert = s.strstart;
                }
            }
            let n = used as usize;
            let dst = s.strstart as usize;
            let src = strm.in_pos - n;
            s.window[dst..dst + n].copy_from_slice(&strm.input[src..src + n]);
            s.strstart += used as u32;
            s.insert += (used as u32).min(s.w_size - s.insert);
        }
        s.block_start = i64::from(s.strstart);
    }
    if s.high_water < u64::from(s.strstart) {
        s.high_water = u64::from(s.strstart);
    }

    // If the last block was written to next_out, then done.
    if last != 0 {
        return BlockState::FinishDone;
    }

    // If flushing and all input has been consumed, then done.
    if flush != Z_NO_FLUSH
        && flush != Z_FINISH
        && strm.avail_in() == 0
        && i64::from(s.strstart) == s.block_start
    {
        return BlockState::BlockDone;
    }

    // Fill the window with any remaining input.
    let mut have = (s.window_size - u64::from(s.strstart)) as usize;
    if strm.avail_in() > have && s.block_start >= i64::from(s.w_size) {
        // Slide the window down.
        s.block_start -= i64::from(s.w_size);
        s.strstart -= s.w_size;
        let keep = s.strstart as usize;
        let src = s.w_size as usize;
        s.window.copy_within(src..src + keep, 0);
        if s.matches < 2 {
            s.matches += 1;
        }
        have += s.w_size as usize;
        if s.insert > s.strstart {
            s.insert = s.strstart;
        }
    }
    if have > strm.avail_in() {
        have = strm.avail_in();
    }
    if have != 0 {
        let dst = s.strstart as usize;
        strm.read_buf(s.wrap, &mut s.window[dst..dst + have]);
        s.strstart += have as u32;
        s.insert += (have as u32).min(s.w_size - s.insert);
    }
    if s.high_water < u64::from(s.strstart) {
        s.high_water = u64::from(s.strstart);
    }

    // There was not enough avail_out to write a complete worthy or flushed stored block to
    // next_out. Write a stored block to pending instead, if we have enough input for a worthy
    // block, or if flushing and there is enough room for the remaining input as a stored block
    // in the pending buffer.
    have = ((u64::from(s.bi_valid as u32) + 42) >> 3) as usize;
    have = ((s.pending_buf_size - have as u64).min(MAX_STORED)) as usize;
    min_block = (have as u64).min(u64::from(s.w_size));
    let left = (i64::from(s.strstart) - s.block_start) as u64;
    if left >= min_block
        || ((left != 0 || flush == Z_FINISH)
            && flush != Z_NO_FLUSH
            && strm.avail_in() == 0
            && left <= have as u64)
    {
        let len = left.min(have as u64);
        last = i32::from(flush == Z_FINISH && strm.avail_in() == 0 && len == left);
        let b = s.block_start as usize;
        tr_stored_block(s, Some(b), len, last);
        s.block_start += len as i64;
        flush_pending(s, strm);
    }

    if last != 0 {
        BlockState::FinishStarted
    } else {
        BlockState::NeedMore
    }
}

/// Port of `deflate_fast`: levels 1 to 3, greedy matching.
// Port of: deflate.c#L1868-L1962 (deflate_fast)
fn deflate_fast(s: &mut DeflateState, strm: &mut Strm<'_>, flush: i32) -> BlockState {
    loop {
        // Make sure that we always have enough lookahead, except at the end of the input file.
        if s.lookahead < MIN_LOOKAHEAD {
            fill_window(s, strm);
            if s.lookahead < MIN_LOOKAHEAD && flush == Z_NO_FLUSH {
                return BlockState::NeedMore;
            }
            if s.lookahead == 0 {
                break;
            }
        }

        // Insert the string window[strstart .. strstart+2] in the dictionary, and set hash_head to
        // the head of the hash chain.
        let mut hash_head = NIL;
        if s.lookahead >= MIN_MATCH {
            hash_head = insert_string(s, s.strstart);
        }

        // Find the longest match, discarding those <= prev_length.
        if hash_head != NIL && s.strstart - hash_head <= max_dist(s) {
            s.match_length = longest_match(s, hash_head);
        }

        let bflush;
        if s.match_length >= MIN_MATCH {
            bflush = tally_dist(s, s.strstart - s.match_start, s.match_length - MIN_MATCH);

            s.lookahead -= s.match_length;

            // Insert new strings in the hash table only if the match length is not too large.
            if s.match_length <= s.max_lazy_match && s.lookahead >= MIN_MATCH {
                s.match_length -= 1;
                loop {
                    s.strstart += 1;
                    insert_string(s, s.strstart);
                    s.match_length -= 1;
                    if s.match_length == 0 {
                        break;
                    }
                }
                s.strstart += 1;
            } else {
                s.strstart += s.match_length;
                s.match_length = 0;
            }
        } else {
            // No match, output a literal byte.
            bflush = tally_lit(s, s.window[s.strstart as usize]);
            s.lookahead -= 1;
            s.strstart += 1;
        }
        if bflush {
            flush_block!(s, strm, 0);
        }
    }
    s.insert = if s.strstart < MIN_MATCH - 1 {
        s.strstart
    } else {
        MIN_MATCH - 1
    };
    if flush == Z_FINISH {
        flush_block!(s, strm, 1);
        return BlockState::FinishDone;
    }
    if s.sym_next != 0 {
        flush_block!(s, strm, 0);
    }
    BlockState::BlockDone
}

/// Port of `deflate_slow`: levels 4 to 9, lazy matching.
// Port of: deflate.c#L1970-L2096 (deflate_slow)
fn deflate_slow(s: &mut DeflateState, strm: &mut Strm<'_>, flush: i32) -> BlockState {
    loop {
        if s.lookahead < MIN_LOOKAHEAD {
            fill_window(s, strm);
            if s.lookahead < MIN_LOOKAHEAD && flush == Z_NO_FLUSH {
                return BlockState::NeedMore;
            }
            if s.lookahead == 0 {
                break;
            }
        }

        let mut hash_head = NIL;
        if s.lookahead >= MIN_MATCH {
            hash_head = insert_string(s, s.strstart);
        }

        // Find the longest match, discarding those <= prev_length.
        s.prev_length = s.match_length;
        s.prev_match = s.match_start;
        s.match_length = MIN_MATCH - 1;

        if hash_head != NIL
            && s.prev_length < s.max_lazy_match
            && s.strstart - hash_head <= max_dist(s)
        {
            s.match_length = longest_match(s, hash_head);

            // If prev_match is also MIN_MATCH, match_start is garbage but we will ignore the
            // current match anyway.
            if s.match_length <= 5
                && (s.strategy == Z_FILTERED
                    || (s.match_length == MIN_MATCH && s.strstart - s.match_start > TOO_FAR))
            {
                s.match_length = MIN_MATCH - 1;
            }
        }

        // If there was a match at the previous step and the current match is not better, output
        // the previous match.
        if s.prev_length >= MIN_MATCH && s.match_length <= s.prev_length {
            let max_insert = s.strstart + s.lookahead - MIN_MATCH;

            let bflush = tally_dist(s, s.strstart - 1 - s.prev_match, s.prev_length - MIN_MATCH);

            // Insert in hash table all strings up to the end of the match. strstart - 1 and
            // strstart are already inserted. If there is not enough lookahead, the last two
            // strings are not inserted in the hash table.
            s.lookahead -= s.prev_length - 1;
            s.prev_length -= 2;
            loop {
                s.strstart += 1;
                if s.strstart <= max_insert {
                    insert_string(s, s.strstart);
                }
                s.prev_length -= 1;
                if s.prev_length == 0 {
                    break;
                }
            }
            s.match_available = 0;
            s.match_length = MIN_MATCH - 1;
            s.strstart += 1;

            if bflush {
                flush_block!(s, strm, 0);
            }
        } else if s.match_available != 0 {
            // If there was no match at the previous position, output a single literal.
            let bflush = tally_lit(s, s.window[s.strstart as usize - 1]);
            if bflush {
                flush_block_only(s, strm, 0);
            }
            s.strstart += 1;
            s.lookahead -= 1;
            if strm.avail_out() == 0 {
                return BlockState::NeedMore;
            }
        } else {
            // There is no previous match to compare with, wait for the next step to decide.
            s.match_available = 1;
            s.strstart += 1;
            s.lookahead -= 1;
        }
    }

    if s.match_available != 0 {
        tally_lit(s, s.window[s.strstart as usize - 1]);
        s.match_available = 0;
    }
    s.insert = if s.strstart < MIN_MATCH - 1 {
        s.strstart
    } else {
        MIN_MATCH - 1
    };
    if flush == Z_FINISH {
        flush_block!(s, strm, 1);
        return BlockState::FinishDone;
    }
    if s.sym_next != 0 {
        flush_block!(s, strm, 0);
    }
    BlockState::BlockDone
}

/// Port of `deflate_rle`: the `Z_RLE` strategy, matches of distance 1 only.
// Port of: deflate.c#L2104-L2169 (deflate_rle)
fn deflate_rle(s: &mut DeflateState, strm: &mut Strm<'_>, flush: i32) -> BlockState {
    loop {
        // Make sure that we always have enough lookahead, except at the end of the input file.
        if s.lookahead <= MAX_MATCH {
            fill_window(s, strm);
            if s.lookahead <= MAX_MATCH && flush == Z_NO_FLUSH {
                return BlockState::NeedMore;
            }
            if s.lookahead == 0 {
                break;
            }
        }

        s.match_length = 0;
        if s.lookahead >= MIN_MATCH && s.strstart > 0 {
            let mut scan = s.strstart as usize - 1;
            let prev = s.window[scan];
            scan += 1;
            if prev == s.window[scan] {
                scan += 1;
                if prev == s.window[scan] {
                    scan += 1;
                    if prev == s.window[scan] {
                        let strend = s.strstart as usize + MAX_MATCH as usize;
                        loop {
                            let mut ok = true;
                            for _ in 0..8 {
                                scan += 1;
                                if prev != s.window[scan] {
                                    ok = false;
                                    break;
                                }
                            }
                            if !ok || scan >= strend {
                                break;
                            }
                        }
                        s.match_length = MAX_MATCH - (strend - scan) as u32;
                        if s.match_length > s.lookahead {
                            s.match_length = s.lookahead;
                        }
                    }
                }
            }
        }

        let bflush;
        if s.match_length >= MIN_MATCH {
            bflush = tally_dist(s, 1, s.match_length - MIN_MATCH);
            s.lookahead -= s.match_length;
            s.strstart += s.match_length;
            s.match_length = 0;
        } else {
            bflush = tally_lit(s, s.window[s.strstart as usize]);
            s.lookahead -= 1;
            s.strstart += 1;
        }
        if bflush {
            flush_block!(s, strm, 0);
        }
    }
    s.insert = 0;
    if flush == Z_FINISH {
        flush_block!(s, strm, 1);
        return BlockState::FinishDone;
    }
    if s.sym_next != 0 {
        flush_block!(s, strm, 0);
    }
    BlockState::BlockDone
}

/// Port of `deflate_huff`: the `Z_HUFFMAN_ONLY` strategy, literals only.
// Port of: deflate.c#L2175-L2205 (deflate_huff)
fn deflate_huff(s: &mut DeflateState, strm: &mut Strm<'_>, flush: i32) -> BlockState {
    loop {
        // Make sure that we have a literal to write.
        if s.lookahead == 0 {
            fill_window(s, strm);
            if s.lookahead == 0 {
                if flush == Z_NO_FLUSH {
                    return BlockState::NeedMore;
                }
                break;
            }
        }

        s.match_length = 0;
        let bflush = tally_lit(s, s.window[s.strstart as usize]);
        s.lookahead -= 1;
        s.strstart += 1;
        if bflush {
            flush_block!(s, strm, 0);
        }
    }
    s.insert = 0;
    if flush == Z_FINISH {
        flush_block!(s, strm, 1);
        return BlockState::FinishDone;
    }
    if s.sym_next != 0 {
        flush_block!(s, strm, 0);
    }
    BlockState::BlockDone
}

/// The output of one [`Deflate::deflate`] call: zlib's return code, and how many input bytes were
/// consumed and output bytes produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deflated {
    /// The return code (`Z_OK`, `Z_STREAM_END`, `Z_BUF_ERROR`, `Z_STREAM_ERROR`).
    pub ret: ReturnCode,
    /// Bytes taken from the front of the input.
    pub consumed: usize,
    /// Bytes written to the front of the output.
    pub produced: usize,
}

/// A deflate stream: the state behind `z_streamp->state` and the `z_stream` fields that zlib keeps
/// next to it.
pub struct Deflate {
    s: Box<DeflateState>,
    total_in: u64,
    total_out: u64,
    adler: u32,
    data_type: i32,
    msg: Option<&'static str>,
    /// The `windowBits` the stream was created with, after the 8 to 9 adjustment.
    window_bits: i32,
}

impl std::fmt::Debug for Deflate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Deflate")
            .field("level", &self.s.level)
            .field("strategy", &self.s.strategy)
            .field("window_bits", &self.window_bits)
            .field("total_in", &self.total_in)
            .field("total_out", &self.total_out)
            .finish_non_exhaustive()
    }
}

impl Deflate {
    /// Port of `deflateInit2_` (deflate.c#L391-L566) for the zlib and raw formats.
    ///
    /// `level` is 0 to 9 (or -1 for the default 6). `window_bits` is 15 for zlib streams and -15
    /// for raw deflate. `mem_level` is 1 to 9 (Skia's zlib default is 8). `strategy` is the zlib
    /// strategy value (0 default, 1 filtered, 2 Huffman only, 3 RLE, 4 fixed).
    ///
    /// # Errors
    /// `StreamError` for an invalid argument, including gzip window sizes (`windowBits > 15`).
    #[doc(alias = "deflateInit2")]
    pub fn new(
        level: i32,
        window_bits: i32,
        mem_level: i32,
        strategy: i32,
    ) -> Result<Self, ReturnCode> {
        let mut level = level;
        if level == Z_DEFAULT_COMPRESSION {
            level = 6;
        }
        let mut wrap = 1;
        let mut window_bits = window_bits;
        if window_bits < 0 {
            wrap = 0;
            if window_bits < -15 {
                return Err(ReturnCode::StreamError);
            }
            window_bits = -window_bits;
        } else if window_bits > 15 {
            // gzip framing is not ported.
            return Err(ReturnCode::StreamError);
        }
        if !(1..=MAX_MEM_LEVEL).contains(&mem_level)
            || !(8..=15).contains(&window_bits)
            || !(0..=9).contains(&level)
            || !(0..=Z_FIXED).contains(&strategy)
            || (window_bits == 8 && wrap != 1)
        {
            return Err(ReturnCode::StreamError);
        }
        if window_bits == 8 {
            window_bits = 9;
        }

        let w_bits = window_bits as u32;
        let w_size = 1u32 << w_bits;
        let mut hash_bits = (mem_level + 7) as u32;
        // Chromium's hash needs at least 15 bits.
        if hash_bits < 15 {
            hash_bits = 15;
        }
        let hash_size = 1u32 << hash_bits;
        let lit_bufsize = 1u32 << (mem_level + 6);

        let s = Box::new(DeflateState {
            status: INIT_STATE,
            pending_buf: vec![0; lit_bufsize as usize * 4],
            pending_buf_size: u64::from(lit_bufsize) * 4,
            pending_out: 0,
            pending: 0,
            wrap,
            last_flush: -2,
            w_size,
            w_bits,
            w_mask: w_size - 1,
            // Zeroed, as zlib's `zmemzero` of the window, with the 8-entry padding.
            window: vec![0; (w_size as usize + 8) * 2],
            window_size: 2 * u64::from(w_size),
            prev: vec![0; w_size as usize],
            head: vec![0; hash_size as usize],
            ins_h: 0,
            hash_size,
            hash_mask: hash_size - 1,
            block_start: 0,
            match_length: 0,
            prev_match: 0,
            match_available: 0,
            strstart: 0,
            match_start: 0,
            lookahead: 0,
            prev_length: 0,
            max_chain_length: 0,
            max_lazy_match: 0,
            level,
            strategy,
            good_match: 0,
            nice_match: 0,
            dyn_ltree: vec![CtData::default(); HEAP_SIZE],
            dyn_dtree: vec![CtData::default(); 2 * D_CODES + 1],
            bl_tree: vec![CtData::default(); 2 * crate::trees::BL_CODES + 1],
            l_desc: TreeDesc {
                max_code: 0,
                stat: &STATIC_L_DESC,
            },
            d_desc: TreeDesc {
                max_code: 0,
                stat: &STATIC_D_DESC,
            },
            bl_desc: TreeDesc {
                max_code: 0,
                stat: &STATIC_BL_DESC,
            },
            bl_count: [0; MAX_BITS + 1],
            heap: [0; HEAP_SIZE],
            heap_len: 0,
            heap_max: 0,
            depth: [0; HEAP_SIZE],
            d_buf: vec![0; lit_bufsize as usize],
            l_buf: vec![0; lit_bufsize as usize],
            sym_next: 0,
            sym_end: lit_bufsize - 1,
            opt_len: 0,
            static_len: 0,
            matches: 0,
            insert: 0,
            bi_buf: 0,
            bi_valid: 0,
            high_water: 0,
            chromium_zlib_hash: true,
        });
        // Port of the `deflateReset` call at the end of deflateInit2_.
        let mut d = Self {
            s,
            total_in: 0,
            total_out: 0,
            adler: ADLER32_INIT,
            data_type: Z_UNKNOWN,
            msg: None,
            window_bits,
        };
        let ret = d.reset();
        debug_assert_eq!(ret, ReturnCode::Ok);
        Ok(d)
    }

    /// Port of `deflateReset`: restarts the stream with the same parameters.
    #[doc(alias = "deflateReset")]
    pub fn reset(&mut self) -> ReturnCode {
        // Port of deflateResetKeep.
        self.total_in = 0;
        self.total_out = 0;
        self.msg = None;
        self.data_type = Z_UNKNOWN;
        let s = &mut self.s;
        s.pending = 0;
        s.pending_out = 0;
        if s.wrap < 0 {
            s.wrap = -s.wrap;
        }
        s.status = INIT_STATE;
        self.adler = ADLER32_INIT;
        s.last_flush = -2;
        tr_init(s);
        lm_init(s);
        ReturnCode::Ok
    }

    /// The Adler-32 of the data so far (zlib's `strm->adler`).
    #[must_use]
    pub fn adler(&self) -> u32 {
        self.adler
    }

    /// Port of `total_in`: input bytes consumed so far.
    #[must_use]
    pub fn total_in(&self) -> u64 {
        self.total_in
    }

    /// Port of `total_out`: output bytes produced so far.
    #[must_use]
    pub fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Port of `msg`: the error message of the last failed call, if any.
    #[must_use]
    pub fn msg(&self) -> Option<&'static str> {
        self.msg
    }

    /// Port of `deflate` (deflate.c#L984-L1294): compresses from the front of `input` into the
    /// front of `output`, as far as the output space allows, and reports how far it got.
    // Port of: deflate.c#L984-L1294 (deflate)
    pub fn deflate(&mut self, input: &[u8], output: &mut [u8], flush: Flush) -> Deflated {
        let mut strm = Strm {
            input,
            in_pos: 0,
            output,
            out_pos: 0,
            total_in: self.total_in,
            total_out: self.total_out,
            adler: self.adler,
            data_type: self.data_type,
            msg: None,
        };
        let ret = deflate_impl(&mut self.s, &mut strm, flush_value(flush));
        self.total_in = strm.total_in;
        self.total_out = strm.total_out;
        self.adler = strm.adler;
        self.data_type = strm.data_type;
        if strm.msg.is_some() {
            self.msg = strm.msg;
        }
        Deflated {
            ret,
            consumed: strm.in_pos,
            produced: strm.out_pos,
        }
    }
}

/// Port of the body of `deflate` after the argument checks: runs the header, the compression
/// function and the trailer.
// Port of: deflate.c#L984-L1294 (deflate, the non-gzip path)
fn deflate_impl(s: &mut DeflateState, strm: &mut Strm<'_>, flush: i32) -> ReturnCode {
    let err = |strm: &mut Strm<'_>, ret: ReturnCode| -> ReturnCode {
        strm.msg = Some(match ret {
            ReturnCode::StreamError => "stream error",
            _ => "buffer error",
        });
        ret
    };

    if !(0..=Z_BLOCK).contains(&flush) {
        return err(strm, ReturnCode::StreamError);
    }
    if s.status == FINISH_STATE && flush != Z_FINISH {
        return err(strm, ReturnCode::StreamError);
    }
    if strm.avail_out() == 0 {
        return err(strm, ReturnCode::BufError);
    }

    // Make sure there is something to do and avoid duplicate consecutive flushes.
    let old_flush = s.last_flush;
    s.last_flush = flush;

    // Flush as much pending output as possible.
    if s.pending != 0 {
        flush_pending(s, strm);
        if strm.avail_out() == 0 {
            // Since avail_out is 0, deflate will be called again with more output space.
            s.last_flush = -1;
            return ReturnCode::Ok;
        }
    } else if strm.avail_in() == 0 && rank(flush) <= rank(old_flush) && flush != Z_FINISH {
        return err(strm, ReturnCode::BufError);
    }

    // User must not provide more input after the first FINISH.
    if s.status == FINISH_STATE && strm.avail_in() != 0 {
        return err(strm, ReturnCode::BufError);
    }

    // Write the zlib header.
    if s.status == INIT_STATE && s.wrap == 0 {
        s.status = BUSY_STATE;
    }
    if s.status == INIT_STATE {
        let mut header: u32 = (Z_DEFLATED + ((s.w_bits - 8) << 4)) << 8;
        let level_flags: u32 = if s.strategy >= 2 || s.level < 2 {
            0
        } else if s.level < 6 {
            1
        } else if s.level == 6 {
            2
        } else {
            3
        };
        header |= level_flags << 6;
        if s.strstart != 0 {
            header |= PRESET_DICT;
        }
        header += 31 - (header % 31);

        put_short_msb(s, header);

        // Save the adler32 of the preset dictionary.
        if s.strstart != 0 {
            put_short_msb(s, strm.adler >> 16);
            put_short_msb(s, strm.adler & 0xffff);
        }
        strm.adler = ADLER32_INIT;
        s.status = BUSY_STATE;

        flush_pending(s, strm);
        if s.pending != 0 {
            s.last_flush = -1;
            return ReturnCode::Ok;
        }
    }

    // Start a new block or continue the current one.
    if strm.avail_in() != 0 || s.lookahead != 0 || (flush != Z_NO_FLUSH && s.status != FINISH_STATE)
    {
        let bstate = if s.level == 0 {
            deflate_stored(s, strm, flush)
        } else if s.strategy == 2 {
            deflate_huff(s, strm, flush)
        } else if s.strategy == 3 {
            deflate_rle(s, strm, flush)
        } else {
            match CONFIGURATION_TABLE[s.level as usize].func {
                CompressFunc::Stored => deflate_stored(s, strm, flush),
                CompressFunc::Fast => deflate_fast(s, strm, flush),
                CompressFunc::Slow => deflate_slow(s, strm, flush),
            }
        };

        if bstate == BlockState::FinishStarted || bstate == BlockState::FinishDone {
            s.status = FINISH_STATE;
        }
        if bstate == BlockState::NeedMore || bstate == BlockState::FinishStarted {
            if strm.avail_out() == 0 {
                s.last_flush = -1;
            }
            return ReturnCode::Ok;
        }
        if bstate == BlockState::BlockDone {
            if flush == Z_PARTIAL_FLUSH {
                tr_align(s);
            } else if flush != Z_BLOCK {
                // Empty stored block, so the output is byte aligned.
                tr_stored_block(s, None, 0, 0);
                if flush == Z_FULL_FLUSH {
                    clear_hash(s);
                    if s.lookahead == 0 {
                        s.strstart = 0;
                        s.block_start = 0;
                        s.insert = 0;
                    }
                }
            }
            flush_pending(s, strm);
            if strm.avail_out() == 0 {
                s.last_flush = -1;
                return ReturnCode::Ok;
            }
        }
    }

    if flush != Z_FINISH {
        return ReturnCode::Ok;
    }
    if s.wrap <= 0 {
        return ReturnCode::StreamEnd;
    }

    // Write the zlib trailer (adler32).
    put_short_msb(s, strm.adler >> 16);
    put_short_msb(s, strm.adler & 0xffff);
    flush_pending(s, strm);
    // If avail_out is zero, the application will call deflate again to flush the rest.
    if s.wrap > 0 {
        s.wrap = -s.wrap;
    }
    if s.pending != 0 {
        ReturnCode::Ok
    } else {
        ReturnCode::StreamEnd
    }
}
