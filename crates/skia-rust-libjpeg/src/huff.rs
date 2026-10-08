// Port of: libjpeg-turbo src/jdhuff.c#L26-L836 and src/jdhuff.h#L24-L250 (libjpeg_turbo@e14cbfaa, 3.1.0)
//
// Copyright (C) 1991-1997, Thomas G. Lane. libjpeg-turbo Modifications: Copyright (C) 2009-2011,
// 2013-2016, 2019, 2021-2022, D. R. Commander. Rust port Copyright (C) 2025 The skia-rust Authors.
// Licence: IJG (see LICENSE).
//
//! Sequential (baseline and extended) Huffman entropy decoder.
//!
//! The bit buffer is 64 bits (`bit_buf_type` = `size_t` on the x64 oracle). The fast path
//! (`decode_mcu_fast`) and the slow path (`decode_mcu_slow`) are both ported, and `decode_mcu`
//! picks between them exactly as libjpeg does, so the same bytes are consumed in the same way.

// Clippy (pedantic) allows, for this module. Each one fires on the C arithmetic and naming this
// module mirrors, and the code is kept as the C writes it so it can be checked line by line:
// JLONG/int/JDIMENSION casts (sign, truncation and wrap), C operator precedence and identity
// terms that come out of macros (`x * 1`, `0 * n`), C loop shapes (`needless_range_loop`,
// `explicit_counter_loop`, `collapsible_if`, `match_same_arms`), the C variable names
// (`similar_names`, `struct_field_names`), libjpeg's constants written as in jdct.h
// (`approx_constant`, `unreadable_literal`), functions whose C form returns a status that
// this path never sets (`unnecessary_wraps`), and the long C routines (`too_many_lines`,
// `too_many_arguments`). Error docs point at the `Error` variants, which name the C codes.
#![allow(
    clippy::approx_constant,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::collapsible_if,
    clippy::doc_markdown,
    clippy::erasing_op,
    clippy::explicit_counter_loop,
    clippy::identity_op,
    clippy::manual_let_else,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::must_use_candidate,
    clippy::needless_range_loop,
    clippy::precedence,
    clippy::similar_names,
    clippy::single_match_else,
    clippy::struct_field_names,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::unnecessary_wraps,
    clippy::unreadable_literal,
    clippy::unused_self
)]

use std::rc::Rc;

use crate::Decompress;
use crate::error::{Error, Result};
use crate::srcio::Local;
use crate::tables::{
    D_MAX_BLOCKS_IN_MCU, DCTSIZE2, JHuffTbl, MAX_COMPS_IN_SCAN, NATURAL_ORDER, NUM_HUFF_TBLS,
};

/// `HUFF_LOOKAHEAD`: bits of lookahead in the fast table.
pub(crate) const HUFF_LOOKAHEAD: i32 = 8;
/// `BIT_BUF_SIZE` (64-bit `bit_buf_type`).
const BIT_BUF_SIZE: i32 = 64;
/// `MIN_GET_BITS` (`BIT_BUF_SIZE - 7`).
const MIN_GET_BITS: i32 = BIT_BUF_SIZE - 7;
/// `BUFSIZE` (`DCTSIZE2 * 8`): input bytes needed for the fast path, per block.
const BUFSIZE: usize = DCTSIZE2 * 8;

/// `d_derived_tbl`: the decoding tables derived from a `JHUFF_TBL`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DTbl {
    /// `maxcode[l]`: largest code of length `l` (`-1` if none), `maxcode[17] = 0xFFFFF`.
    pub(crate) maxcode: [i32; 18],
    /// `valoffset[l]`: offset from the code to the index in `huffval`.
    pub(crate) valoffset: [i32; 18],
    /// Lookahead table: `(length << HUFF_LOOKAHEAD) | value`, or `(HUFF_LOOKAHEAD + 1) << HUFF_LOOKAHEAD`.
    pub(crate) lookup: [i32; 1 << HUFF_LOOKAHEAD],
    /// The symbol values (`htbl->huffval`).
    pub(crate) huffval: [u8; 256],
}

/// `bitread_perm_state`: the bit buffer kept between MCUs.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct BitPerm {
    pub(crate) get_buffer: u64,
    pub(crate) bits_left: i32,
}

/// `savable_state`: the DC predictors, saved with the bit buffer. `eobrun` is the progressive
/// decoder's `EOBRUN` (`jdphuff.c`); the sequential decoder never uses it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Savable {
    pub(crate) last_dc_val: [i32; MAX_COMPS_IN_SCAN],
    pub(crate) eobrun: u32,
}

/// `huff_entropy_decoder`.
#[derive(Debug, Clone, Default)]
pub(crate) struct HuffDecoder {
    pub(crate) bitstate: BitPerm,
    pub(crate) saved: Savable,
    pub(crate) restarts_to_go: u32,
    pub(crate) dc_derived: [Option<Rc<DTbl>>; NUM_HUFF_TBLS],
    pub(crate) ac_derived: [Option<Rc<DTbl>>; NUM_HUFF_TBLS],
    pub(crate) dc_cur_tbls: [Option<Rc<DTbl>>; D_MAX_BLOCKS_IN_MCU],
    pub(crate) ac_cur_tbls: [Option<Rc<DTbl>>; D_MAX_BLOCKS_IN_MCU],
    pub(crate) dc_needed: [bool; D_MAX_BLOCKS_IN_MCU],
    pub(crate) ac_needed: [bool; D_MAX_BLOCKS_IN_MCU],
    /// Which progressive decoder the scan selected (`start_pass_phuff_decoder`).
    pub(crate) phuff_kind: crate::phuff::PhuffKind,
}

/// `jpeg_make_d_derived_tbl`: builds the decoding tables for one Huffman table.
pub(crate) fn make_d_derived_tbl(
    htbl: &JHuffTbl,
    is_dc: bool,
    dc_lossless_max: i32,
) -> Result<DTbl> {
    // Figure C.1: size table, then code table (Figure C.2).
    let mut huffsize = [0i32; 257];
    let mut p: usize = 0;
    for l in 1..=16 {
        let i = i32::from(htbl.bits[l]);
        if i < 0 || p as i32 + i > 256 {
            return Err(Error::BadHuffTable);
        }
        for _ in 0..i {
            huffsize[p] = l as i32;
            p += 1;
        }
    }
    huffsize[p] = 0;
    let numsymbols = p;

    let mut huffcode = [0u32; 257];
    let mut code: u32 = 0;
    let mut si = huffsize[0];
    p = 0;
    while huffsize[p] != 0 {
        while huffsize[p] == si {
            huffcode[p] = code;
            p += 1;
            code = code.wrapping_add(1);
        }
        // Code must fit in `si` bits.
        if i64::from(code) >= (1i64 << si) {
            return Err(Error::BadHuffTable);
        }
        code <<= 1;
        si += 1;
    }

    let mut dtbl = DTbl {
        maxcode: [0; 18],
        valoffset: [0; 18],
        lookup: [0; 1 << HUFF_LOOKAHEAD],
        huffval: htbl.huffval,
    };

    // Figure F.15: generate decoding tables for bit-sequential decoding.
    p = 0;
    for l in 1..=16 {
        if htbl.bits[l] != 0 {
            // valoffset[l] = huffval index minus minimum code of length l.
            dtbl.valoffset[l] = (p as i64 - i64::from(huffcode[p])) as i32;
            p += usize::from(htbl.bits[l]);
            dtbl.maxcode[l] = huffcode[p - 1] as i32;
        } else {
            dtbl.maxcode[l] = -1;
        }
    }
    // Sentinel: guarantees that the decode loop terminates.
    dtbl.valoffset[17] = 0;
    dtbl.maxcode[17] = 0xFFFFF;

    // Lookahead tables: indexed by the next HUFF_LOOKAHEAD bits.
    for i in 0..(1usize << HUFF_LOOKAHEAD) {
        dtbl.lookup[i] = (HUFF_LOOKAHEAD + 1) << HUFF_LOOKAHEAD;
    }
    p = 0;
    for l in 1..=HUFF_LOOKAHEAD {
        let mut i = 1;
        while i <= i32::from(htbl.bits[l as usize]) {
            // Generate left-justified code followed by all possible bit sequences.
            let mut lookbits = (huffcode[p] << (HUFF_LOOKAHEAD - l)) as usize;
            let mut ctr = 1i32 << (HUFF_LOOKAHEAD - l);
            while ctr > 0 {
                dtbl.lookup[lookbits] = (l << HUFF_LOOKAHEAD) | i32::from(htbl.huffval[p]);
                lookbits += 1;
                ctr -= 1;
            }
            i += 1;
            p += 1;
        }
    }

    // Validate symbols of DC tables (libjpeg also checks lossless range; `dc_lossless_max` is 15).
    if is_dc {
        for i in 0..numsymbols {
            let sym = i32::from(htbl.huffval[i]);
            if sym < 0 || sym > dc_lossless_max {
                return Err(Error::BadHuffTable);
            }
        }
    }
    Ok(dtbl)
}

/// `HUFF_EXTEND` (AVOID_TABLES variant): sign-extends an `s`-bit value.
#[inline]
pub(crate) fn huff_extend(x: i32, s: i32) -> i32 {
    // x + (((x - (1 << (s - 1))) >> 31) & (((NEG_1) << s) + 1)) with unsigned wrap-around.
    let t = (x as u32).wrapping_sub(1u32 << (s - 1));
    let mask = (((u32::MAX) << s).wrapping_add(1)) & ((t as i32) >> 31) as u32;
    (x as u32).wrapping_add(mask) as i32
}

/// `GET_BITS(nbits)` on the local bit buffer.
#[inline]
pub(crate) fn get_bits(get_buffer: u64, bits_left: &mut i32, nbits: i32) -> i32 {
    *bits_left -= nbits;
    ((get_buffer >> *bits_left) as i32) & ((1 << nbits) - 1)
}

/// `PEEK_BITS(nbits)`.
#[inline]
fn peek_bits(get_buffer: u64, bits_left: i32, nbits: i32) -> i32 {
    ((get_buffer >> (bits_left - nbits)) as i32) & ((1 << nbits) - 1)
}

impl Decompress {
    /// `jpeg_fill_bit_buffer`: refills the bit buffer until at least `MIN_GET_BITS` are
    /// present (or `nbits` if the stream has a marker). Returns `Ok(false)` on suspension.
    pub(crate) fn fill_bit_buffer(
        &mut self,
        br: &mut Local,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        nbits: i32,
    ) -> Result<bool> {
        // Note: the local copies are only written back on success, as in libjpeg.
        let mut gb = *get_buffer;
        let mut bl = *bits_left;
        let mut next = br.next;
        let mut bytes = br.bytes;
        if self.unread_marker == 0 {
            while bl < MIN_GET_BITS {
                if bytes == 0 {
                    // Sync the source, fill it, reload the locals.
                    if !self.fill_input()? {
                        return Ok(false);
                    }
                    next = self.srcbuf.next;
                    bytes = self.srcbuf.bytes_in_buffer;
                }
                bytes -= 1;
                let mut c = u32::from(self.srcbuf.data[next]);
                next += 1;
                if c == 0xFF {
                    // Need to check for the stuffed zero byte or a marker.
                    loop {
                        if bytes == 0 {
                            if !self.fill_input()? {
                                return Ok(false);
                            }
                            next = self.srcbuf.next;
                            bytes = self.srcbuf.bytes_in_buffer;
                        }
                        bytes -= 1;
                        c = u32::from(self.srcbuf.data[next]);
                        next += 1;
                        if c != 0xFF {
                            break;
                        }
                    }
                    if c == 0 {
                        // Stuffed zero: the byte is 0xFF.
                        c = 0xFF;
                    } else {
                        // A marker: remember it and pad with zeros.
                        self.unread_marker = c as i32;
                        // goto no_more_bytes
                        return self.fill_no_more_bytes(
                            br, get_buffer, bits_left, &mut gb, &mut bl, nbits, next, bytes,
                        );
                    }
                }
                gb = (gb << 8) | u64::from(c);
                bl += 8;
            }
            br.next = next;
            br.bytes = bytes;
            *get_buffer = gb;
            *bits_left = bl;
            Ok(true)
        } else {
            self.fill_no_more_bytes(
                br, get_buffer, bits_left, &mut gb, &mut bl, nbits, next, bytes,
            )
        }
    }

    /// The `no_more_bytes:` label of `jpeg_fill_bit_buffer`: pad with zero bits when the
    /// request cannot be satisfied.
    #[allow(clippy::too_many_arguments)]
    fn fill_no_more_bytes(
        &mut self,
        br: &mut Local,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        gb: &mut u64,
        bl: &mut i32,
        nbits: i32,
        next: usize,
        bytes: usize,
    ) -> Result<bool> {
        // If we run out of data, just leave the rest of the buffer zero: insufficient_data.
        if nbits > *bl {
            // Uh-oh.  Report corrupted data to user and stuff zeroes into the data stream, so
            // that we can produce some kind of image. One warning per data segment.
            if !self.insufficient_data {
                self.warn(JWRN_HIT_MARKER);
                self.insufficient_data = true;
            }
            // Fill the buffer with zero bits.
            *gb <<= MIN_GET_BITS - *bl;
            *bl = MIN_GET_BITS;
        }
        br.next = next;
        br.bytes = bytes;
        *get_buffer = *gb;
        *bits_left = *bl;
        Ok(true)
    }

    /// `jpeg_huff_decode`: slow decode of a code longer than the lookahead.
    pub(crate) fn huff_decode_slow(
        &mut self,
        br: &mut Local,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        htbl: &DTbl,
        min_bits: i32,
    ) -> Result<Option<i32>> {
        let mut l = min_bits;
        // HUFF_DECODE: CHECK_BIT_BUFFER(l) before taking the first `l` bits.
        if *bits_left < l {
            let mut gb = *get_buffer;
            let mut bl = *bits_left;
            if !self.fill_bit_buffer(br, &mut gb, &mut bl, l)? {
                return Ok(None);
            }
            *get_buffer = gb;
            *bits_left = bl;
        }
        let mut code: i64 = i64::from(get_bits(*get_buffer, bits_left, l));
        // Keep going while the code is larger than the maximum code of its length.
        while code > i64::from(htbl.maxcode[l as usize]) {
            code <<= 1;
            if *bits_left < 1 {
                let mut gb = *get_buffer;
                let mut bl = *bits_left;
                if !self.fill_bit_buffer(br, &mut gb, &mut bl, 1)? {
                    return Ok(None);
                }
                *get_buffer = gb;
                *bits_left = bl;
            }
            code |= i64::from(get_bits(*get_buffer, bits_left, 1));
            l += 1;
        }
        // Unlikely, but possible: a corrupt code.
        if l > 16 {
            self.warn(JWRN_HUFF_BAD_CODE);
            return Ok(Some(0));
        }
        // For l <= 16 the code is within the table by construction of valoffset/maxcode.
        let idx = usize::try_from(code + i64::from(htbl.valoffset[l as usize]))
            .map_err(|_| Error::HuffMissingCode)?;
        let sym = *htbl.huffval.get(idx).ok_or(Error::HuffMissingCode)?;
        Ok(Some(i32::from(sym)))
    }

    /// `process_restart`: at a restart boundary, drop the partial byte, read the RSTn marker
    /// and reset the DC predictors.
    pub(crate) fn process_restart(&mut self) -> Result<bool> {
        // Throw away any unused bits remaining in the bit buffer.
        let bits = self.huff.bitstate.bits_left / 8;
        self.marker.discarded_bytes += bits as u32;
        self.huff.bitstate.bits_left = 0;
        if !self.read_restart_marker()? {
            return Ok(false);
        }
        for ci in 0..self.comps_in_scan as usize {
            self.huff.saved.last_dc_val[ci] = 0;
        }
        self.huff.restarts_to_go = self.restart_interval;
        // Reset out-of-data flag, unless read_restart_marker left us smack up against a marker.
        if self.unread_marker == 0 {
            self.insufficient_data = false;
        }
        Ok(true)
    }

    /// `decode_mcu_slow`.
    fn decode_mcu_slow(&mut self, mcu: Option<&mut [[i16; DCTSIZE2]]>) -> Result<bool> {
        let mut br = Local {
            next: self.srcbuf.next,
            bytes: self.srcbuf.bytes_in_buffer,
        };
        let mut get_buffer = self.huff.bitstate.get_buffer;
        let mut bits_left = self.huff.bitstate.bits_left;
        let mut state = self.huff.saved;
        let mut mcu = mcu;
        for blkn in 0..self.blocks_in_mcu as usize {
            let dctbl = self.huff.dc_cur_tbls[blkn]
                .clone()
                .ok_or(Error::Internal("dc table"))?;
            let actbl = self.huff.ac_cur_tbls[blkn]
                .clone()
                .ok_or(Error::Internal("ac table"))?;
            let mut block = mcu.as_deref_mut().map(|m| &mut m[blkn]);
            // Section F.2.2.1: decode the DC coefficient difference.
            let mut s = match self.huff_decode(&mut br, &mut get_buffer, &mut bits_left, &dctbl)? {
                Some(v) => v,
                None => return self.decode_suspend(&br, get_buffer, bits_left),
            };
            if s != 0 {
                if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, s)? {
                    return self.decode_suspend(&br, get_buffer, bits_left);
                }
                let r = get_bits(get_buffer, &mut bits_left, s);
                s = huff_extend(r, s);
            }
            if self.huff.dc_needed[blkn] {
                let ci = self.mcu_membership[blkn] as usize;
                s += state.last_dc_val[ci];
                state.last_dc_val[ci] = s;
                if let Some(b) = block.as_deref_mut() {
                    b[0] = s as i16;
                }
            }
            if self.huff.ac_needed[blkn] && block.is_some() {
                // Section F.2.2.2: decode the AC coefficients.
                let mut k = 1usize;
                while k < DCTSIZE2 {
                    let mut s =
                        match self.huff_decode(&mut br, &mut get_buffer, &mut bits_left, &actbl)? {
                            Some(v) => v,
                            None => return self.decode_suspend(&br, get_buffer, bits_left),
                        };
                    let mut r = s >> 4;
                    s &= 15;
                    if s != 0 {
                        k += r as usize;
                        if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, s)? {
                            return self.decode_suspend(&br, get_buffer, bits_left);
                        }
                        r = get_bits(get_buffer, &mut bits_left, s);
                        s = huff_extend(r, s);
                        // Output coefficient in natural (not zigzag) order. Index may exceed 63
                        // for corrupt data; NATURAL_ORDER pads with 63 as libjpeg does.
                        if let Some(b) = block.as_deref_mut() {
                            b[NATURAL_ORDER[k]] = s as i16;
                        }
                    } else if r != 15 {
                        break;
                    } else {
                        k += 15;
                    }
                    k += 1;
                }
            } else {
                // Skipping the AC coefficients: same bit consumption, nothing stored.
                let mut k = 1usize;
                while k < DCTSIZE2 {
                    let mut s =
                        match self.huff_decode(&mut br, &mut get_buffer, &mut bits_left, &actbl)? {
                            Some(v) => v,
                            None => return self.decode_suspend(&br, get_buffer, bits_left),
                        };
                    let r = s >> 4;
                    s &= 15;
                    if s != 0 {
                        k += r as usize;
                        if !self.check_bit_buffer(&mut br, &mut get_buffer, &mut bits_left, s)? {
                            return self.decode_suspend(&br, get_buffer, bits_left);
                        }
                        bits_left -= s;
                    } else if r != 15 {
                        break;
                    } else {
                        k += 15;
                    }
                    k += 1;
                }
            }
        }
        // BITREAD_SAVE_STATE
        self.srcbuf.next = br.next;
        self.srcbuf.bytes_in_buffer = br.bytes;
        self.huff.bitstate = BitPerm {
            get_buffer,
            bits_left,
        };
        self.huff.saved = state;
        Ok(true)
    }

    /// The `return FALSE` exit of the slow path: the bit state is not saved (libjpeg returns
    /// before `BITREAD_SAVE_STATE`), so only the source keeps its last synced position.
    pub(crate) fn decode_suspend(
        &mut self,
        _br: &Local,
        _get_buffer: u64,
        _bits_left: i32,
    ) -> Result<bool> {
        Ok(false)
    }

    /// `CHECK_BIT_BUFFER(br_state, nbits, return FALSE)`.
    pub(crate) fn check_bit_buffer(
        &mut self,
        br: &mut Local,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        nbits: i32,
    ) -> Result<bool> {
        if *bits_left < nbits {
            let mut gb = *get_buffer;
            let mut bl = *bits_left;
            if !self.fill_bit_buffer(br, &mut gb, &mut bl, nbits)? {
                return Ok(false);
            }
            *get_buffer = gb;
            *bits_left = bl;
        }
        Ok(true)
    }

    /// `HUFF_DECODE` (slow path lookahead). `Ok(None)` is a suspension; `Ok(Some(v))` is the
    /// decoded symbol. A failing `jpeg_huff_decode` is a corrupt-code error in libjpeg (it
    /// returns -1 and the caller returns FALSE); here it is reported as a suspension-like
    /// failure to the same caller, which is what `failaction` does.
    pub(crate) fn huff_decode(
        &mut self,
        br: &mut Local,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        htbl: &DTbl,
    ) -> Result<Option<i32>> {
        let nb: i32;
        if *bits_left < HUFF_LOOKAHEAD {
            let mut gb = *get_buffer;
            let mut bl = *bits_left;
            if !self.fill_bit_buffer(br, &mut gb, &mut bl, 0)? {
                return Ok(None);
            }
            *get_buffer = gb;
            *bits_left = bl;
            if *bits_left < HUFF_LOOKAHEAD {
                nb = 1;
                return self.huff_decode_slow_from(br, get_buffer, bits_left, htbl, nb);
            }
        }
        let look = peek_bits(*get_buffer, *bits_left, HUFF_LOOKAHEAD) as usize;
        let entry = htbl.lookup[look];
        nb = entry >> HUFF_LOOKAHEAD;
        if nb <= HUFF_LOOKAHEAD {
            *bits_left -= nb;
            Ok(Some(entry & ((1 << HUFF_LOOKAHEAD) - 1)))
        } else {
            self.huff_decode_slow_from(br, get_buffer, bits_left, htbl, nb)
        }
    }

    /// The `slowlabel:` branch of `HUFF_DECODE`.
    fn huff_decode_slow_from(
        &mut self,
        br: &mut Local,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        htbl: &DTbl,
        nb: i32,
    ) -> Result<Option<i32>> {
        match self.huff_decode_slow(br, get_buffer, bits_left, htbl, nb)? {
            Some(v) if v >= 0 => Ok(Some(v)),
            Some(_) => Err(Error::HuffMissingCode),
            None => Ok(None),
        }
    }

    /// `decode_mcu_fast`: the same decode with the source known to hold enough bytes for the
    /// whole MCU, reading ahead without suspension checks.
    fn decode_mcu_fast(&mut self, mcu: Option<&mut [[i16; DCTSIZE2]]>) -> Result<bool> {
        let mut get_buffer = self.huff.bitstate.get_buffer;
        let mut bits_left = self.huff.bitstate.bits_left;
        let mut buffer = self.srcbuf.next;
        let mut state = self.huff.saved;
        let mut mcu = mcu;
        for blkn in 0..self.blocks_in_mcu as usize {
            let dctbl = self.huff.dc_cur_tbls[blkn]
                .clone()
                .ok_or(Error::Internal("dc table"))?;
            let actbl = self.huff.ac_cur_tbls[blkn]
                .clone()
                .ok_or(Error::Internal("ac table"))?;
            let mut block = mcu.as_deref_mut().map(|m| &mut m[blkn]);
            let (mut s, _) =
                self.huff_decode_fast(&mut get_buffer, &mut bits_left, &mut buffer, &dctbl);
            if s != 0 {
                self.fill_bit_buffer_fast(&mut get_buffer, &mut bits_left, &mut buffer);
                let r = get_bits(get_buffer, &mut bits_left, s);
                s = huff_extend(r, s);
            }
            if self.huff.dc_needed[blkn] {
                let ci = self.mcu_membership[blkn] as usize;
                s += state.last_dc_val[ci];
                state.last_dc_val[ci] = s;
                if let Some(b) = block.as_deref_mut() {
                    b[0] = s as i16;
                }
            }
            if self.huff.ac_needed[blkn] && block.is_some() {
                let mut k = 1usize;
                while k < DCTSIZE2 {
                    let (s0, _) =
                        self.huff_decode_fast(&mut get_buffer, &mut bits_left, &mut buffer, &actbl);
                    let mut s = s0;
                    let mut r = s >> 4;
                    s &= 15;
                    if s != 0 {
                        k += r as usize;
                        self.fill_bit_buffer_fast(&mut get_buffer, &mut bits_left, &mut buffer);
                        r = get_bits(get_buffer, &mut bits_left, s);
                        s = huff_extend(r, s);
                        if let Some(b) = block.as_deref_mut() {
                            b[NATURAL_ORDER[k]] = s as i16;
                        }
                    } else if r != 15 {
                        break;
                    } else {
                        k += 15;
                    }
                    k += 1;
                }
            } else {
                let mut k = 1usize;
                while k < DCTSIZE2 {
                    let (s0, _) =
                        self.huff_decode_fast(&mut get_buffer, &mut bits_left, &mut buffer, &actbl);
                    let mut s = s0;
                    let r = s >> 4;
                    s &= 15;
                    if s != 0 {
                        k += r as usize;
                        self.fill_bit_buffer_fast(&mut get_buffer, &mut bits_left, &mut buffer);
                        bits_left -= s;
                    } else if r != 15 {
                        break;
                    } else {
                        k += 15;
                    }
                    k += 1;
                }
            }
        }
        if self.unread_marker != 0 {
            self.unread_marker = 0;
            return Ok(false);
        }
        // br_state.bytes_in_buffer -= (buffer - br_state.next_input_byte);
        // br_state.next_input_byte = buffer; then BITREAD_SAVE_STATE.
        let consumed = buffer - self.srcbuf.next;
        self.srcbuf.bytes_in_buffer -= consumed;
        self.srcbuf.next = buffer;
        self.huff.bitstate = BitPerm {
            get_buffer,
            bits_left,
        };
        self.huff.saved = state;
        Ok(true)
    }

    /// `HUFF_DECODE_FAST` (no suspension). Returns `(symbol, nb)`.
    fn huff_decode_fast(
        &mut self,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        buffer: &mut usize,
        htbl: &DTbl,
    ) -> (i32, i32) {
        self.fill_bit_buffer_fast(get_buffer, bits_left, buffer);
        let mut s = peek_bits(*get_buffer, *bits_left, HUFF_LOOKAHEAD);
        s = htbl.lookup[s as usize];
        let mut nb = s >> HUFF_LOOKAHEAD;
        *bits_left -= nb;
        s &= (1 << HUFF_LOOKAHEAD) - 1;
        if nb > HUFF_LOOKAHEAD {
            s = ((*get_buffer >> *bits_left) as i32) & ((1 << nb) - 1);
            while s > htbl.maxcode[nb as usize] {
                s <<= 1;
                s |= get_bits(*get_buffer, bits_left, 1);
                nb += 1;
            }
            if nb > 16 {
                s = 0;
            } else {
                let idx = ((s + htbl.valoffset[nb as usize]) as u32 & 0xFF) as usize;
                s = i32::from(htbl.huffval[idx]);
            }
        }
        (s, nb)
    }

    /// `FILL_BIT_BUFFER_FAST`: when `bits_left <= 16`, shift in up to 6 bytes, handling 0xFF
    /// stuffing and stopping at a marker.
    fn fill_bit_buffer_fast(
        &mut self,
        get_buffer: &mut u64,
        bits_left: &mut i32,
        buffer: &mut usize,
    ) {
        if *bits_left <= 16 {
            for _ in 0..6 {
                self.get_byte_fast(get_buffer, bits_left, buffer);
            }
        }
    }

    /// `GET_BYTE`.
    fn get_byte_fast(&mut self, get_buffer: &mut u64, bits_left: &mut i32, buffer: &mut usize) {
        let c0 = u32::from(self.srcbuf.data[*buffer]);
        *buffer += 1;
        let c1 = u32::from(self.srcbuf.data[*buffer]);
        *get_buffer = (*get_buffer << 8) | u64::from(c0);
        *bits_left += 8;
        if c0 == 0xFF {
            *buffer += 1;
            if c1 != 0 {
                self.unread_marker = c1 as i32;
                *buffer -= 2;
                *get_buffer &= !0xFF;
            }
        }
    }

    /// `decode_mcu`: decodes one MCU into `mcu` (one block per entry of the component
    /// membership), or only consumes its bits when `mcu` is `None`.
    pub(crate) fn decode_mcu(&mut self, mcu: Option<&mut [[i16; DCTSIZE2]]>) -> Result<bool> {
        let mut usefast = true;
        if self.restart_interval != 0 {
            if self.huff.restarts_to_go == 0 && !self.process_restart()? {
                return Ok(false);
            }
            usefast = false;
        }
        if self.srcbuf.bytes_in_buffer < BUFSIZE * self.blocks_in_mcu as usize
            || self.unread_marker != 0
        {
            usefast = false;
        }
        if !self.insufficient_data {
            // The fast and slow decoders are both used exactly as libjpeg chooses them.
            let mut mcu = mcu;
            if usefast {
                if !self.decode_mcu_fast(mcu.as_deref_mut())? {
                    if !self.decode_mcu_slow(mcu.as_deref_mut())? {
                        return Ok(false);
                    }
                }
            } else if !self.decode_mcu_slow(mcu)? {
                return Ok(false);
            }
        }
        if self.restart_interval != 0 {
            self.huff.restarts_to_go = self.huff.restarts_to_go.wrapping_sub(1);
        }
        Ok(true)
    }

    /// `start_pass_huff_decoder`: builds the derived tables for the current scan.
    pub(crate) fn start_pass_huff_decoder(&mut self) -> Result<()> {
        // Check that the scan parameters Ss, Se, Ah/Al are OK for sequential JPEG.
        if self.ss != 0 || self.se != DCTSIZE2 as i32 - 1 || self.ah != 0 || self.al != 0 {
            return Err(Error::BadProgression);
        }
        for ci in 0..self.comps_in_scan as usize {
            let compptr = self.cur_comp_info[ci].ok_or(Error::Internal("cur_comp_info"))?;
            let dctbl = self.comp_info[compptr].dc_tbl_no as usize;
            let actbl = self.comp_info[compptr].ac_tbl_no as usize;
            self.huff.dc_derived[dctbl] = Some(Rc::new(self.make_derived(true, dctbl)?));
            self.huff.ac_derived[actbl] = Some(Rc::new(self.make_derived(false, actbl)?));
            self.huff.saved.last_dc_val[ci] = 0;
        }
        for blkn in 0..self.blocks_in_mcu as usize {
            let ci = self.mcu_membership[blkn] as usize;
            let compptr = self.cur_comp_info[ci].ok_or(Error::Internal("cur_comp_info"))?;
            let dc = self.comp_info[compptr].dc_tbl_no as usize;
            let ac = self.comp_info[compptr].ac_tbl_no as usize;
            self.huff.dc_cur_tbls[blkn].clone_from(&self.huff.dc_derived[dc]);
            self.huff.ac_cur_tbls[blkn].clone_from(&self.huff.ac_derived[ac]);
            if self.comp_info[compptr].component_needed {
                self.huff.dc_needed[blkn] = true;
                self.huff.ac_needed[blkn] = self.comp_info[compptr].dct_h_scaled_size > 1;
            } else {
                self.huff.dc_needed[blkn] = false;
                self.huff.ac_needed[blkn] = false;
            }
        }
        // Initialize bitread state variables.
        self.huff.bitstate = BitPerm::default();
        self.insufficient_data = false;
        // Initialize restart counter.
        self.huff.restarts_to_go = self.restart_interval;
        Ok(())
    }

    /// `jpeg_make_d_derived_tbl` for table `tblno`.
    pub(crate) fn make_derived(&self, is_dc: bool, tblno: usize) -> Result<DTbl> {
        if tblno >= NUM_HUFF_TBLS {
            return Err(Error::Internal("huffman table number"));
        }
        let htbl = if is_dc {
            self.dc_huff_tbl_ptrs[tblno].as_ref()
        } else {
            self.ac_huff_tbl_ptrs[tblno].as_ref()
        };
        let htbl = htbl.ok_or(Error::Internal("missing huffman table"))?;
        make_d_derived_tbl(htbl, is_dc, 15)
    }
}

/// `JWRN_HIT_MARKER`.
pub(crate) const JWRN_HIT_MARKER: i32 = 10;
/// `JWRN_HUFF_BAD_CODE`.
pub(crate) const JWRN_HUFF_BAD_CODE: i32 = 11;
