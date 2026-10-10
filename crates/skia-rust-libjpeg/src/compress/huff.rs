// Port of: libjpeg-turbo src/jchuff.c#L1-L700 (libjpeg_turbo@e14cbfaa, 3.1.0 source):
// `jpeg_make_c_derived_tbl`, `encode_one_block` (the non-SIMD path), `encode_mcu_huff`,
// `finish_pass_huff`, `flush_bits`, `htest_one_block`, `encode_mcu_gather`,
// `jpeg_gen_optimal_table` and `finish_pass_gather`. The bit buffer keeps libjpeg's output rules:
// MSB first, a 0x00 stuffed after every 0xFF, and the last byte padded with 1-bits.
//
// Copyright (C) 1991-1997, Thomas G. Lane. Modified 2009-2011 by D. R. Commander.
// Copyright (C) 2015, 2018-2019, 2025 D. R. Commander.
// Copyright (C) 2025 The skia-rust Authors.
//
// The restart interval is always zero here (Skia sets none), so `emit_restart` is not ported.

// The C loop shapes over several parallel arrays are kept (`for i in 0..n`), as in libjpeg.
#![allow(clippy::needless_range_loop)]

use super::coef::CoefBuffer;
use super::{Block, Compress};
use crate::error::{Error, Result};
use crate::tables::{JHuffTbl, NATURAL_ORDER, NUM_HUFF_TBLS};

/// `MAX_CLEN`: the longest code the optimal-table generator may produce.
const MAX_CLEN: usize = 32;

/// `c_derived_tbl`: the code and its length for each symbol (`ehufco`, `ehufsi`).
#[derive(Debug, Clone)]
pub(super) struct CDerivedTbl {
    ehufco: [u32; 256],
    ehufsi: [u8; 256],
}

/// `jpeg_make_c_derived_tbl`: the canonical Huffman codes of a table, by symbol.
pub(super) fn make_c_derived_tbl(htbl: &JHuffTbl, is_dc: bool) -> Result<CDerivedTbl> {
    // Code lengths, in code order.
    let mut huffsize = [0u8; 257];
    let mut p = 0usize;
    for l in 1..=16usize {
        let i = usize::from(htbl.bits[l]);
        if p + i > 256 {
            return Err(Error::BadHuffTable);
        }
        for _ in 0..i {
            huffsize[p] = l as u8;
            p += 1;
        }
    }
    huffsize[p] = 0;
    let lastp = p;
    // Canonical codes.
    let mut huffcode = [0u32; 257];
    let mut code: u32 = 0;
    let mut si = u32::from(huffsize[0]);
    p = 0;
    while huffsize[p] != 0 {
        while u32::from(huffsize[p]) == si {
            huffcode[p] = code;
            p += 1;
            code += 1;
        }
        if u64::from(code) >= (1u64 << si) {
            return Err(Error::BadHuffTable);
        }
        code <<= 1;
        si += 1;
    }
    let mut dtbl = CDerivedTbl {
        ehufco: [0; 256],
        ehufsi: [0; 256],
    };
    let maxsymbol = if is_dc { 15 } else { 255 };
    for p in 0..lastp {
        let i = usize::from(htbl.huffval[p]);
        if i > maxsymbol || dtbl.ehufsi[i] != 0 {
            return Err(Error::BadHuffTable);
        }
        dtbl.ehufco[i] = huffcode[p];
        dtbl.ehufsi[i] = huffsize[p];
    }
    Ok(dtbl)
}

/// `JPEG_NBITS`: the number of bits needed for `|value|` (0 for 0).
pub(super) fn jpeg_nbits(value: u32) -> u32 {
    32 - value.leading_zeros()
}

/// The output of the entropy coder: the bit buffer of `jchuff.c`, with its byte stuffing.
#[derive(Debug, Default)]
struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    nbits: u32,
}

impl BitWriter {
    /// `PUT_BITS(code, size)`: appends the low `size` bits of `code`, most significant first.
    fn put(&mut self, code: u32, size: u32) {
        if size == 0 {
            return;
        }
        let mask = if size >= 32 {
            u64::from(u32::MAX)
        } else {
            (1u64 << size) - 1
        };
        self.acc = (self.acc << size) | (u64::from(code) & mask);
        self.nbits += size;
        while self.nbits >= 8 {
            self.nbits -= 8;
            let byte = (self.acc >> self.nbits) as u8;
            self.emit_byte(byte);
        }
        self.acc &= (1u64 << self.nbits) - 1;
    }

    /// `EMIT_BYTE`: a byte, with a zero stuffed after 0xFF.
    fn emit_byte(&mut self, byte: u8) {
        self.out.push(byte);
        if byte == 0xFF {
            self.out.push(0);
        }
    }

    /// `flush_bits`: pads the last partial byte with 1-bits and writes it.
    fn flush(&mut self) {
        if self.nbits > 0 {
            let pad = 8 - self.nbits;
            let byte = ((self.acc << pad) | ((1u64 << pad) - 1)) as u8;
            self.nbits = 0;
            self.acc = 0;
            self.emit_byte(byte);
        }
    }
}

/// `encode_one_block` (the non-SIMD path): the DC difference, then the AC run-length codes.
fn encode_one_block(
    w: &mut BitWriter,
    block: &Block,
    last_dc_val: i32,
    dctbl: &CDerivedTbl,
    actbl: &CDerivedTbl,
    max_coef_bits: u32,
) -> Result<()> {
    // DC: the category of the difference, then its low bits (negative values as v - 1).
    let mut temp = i32::from(block[0]) - last_dc_val;
    let sign = temp >> 31;
    temp += sign;
    let nbits = jpeg_nbits((sign ^ temp) as u32);
    if nbits > max_coef_bits + 1 {
        return Err(Error::BadDctCoef);
    }
    w.put(
        dctbl.ehufco[nbits as usize],
        u32::from(dctbl.ehufsi[nbits as usize]),
    );
    w.put(temp as u32, nbits);
    // AC: r counts zeros in units of 16 (the C code's `r += 16`).
    let mut r: u32 = 0;
    for k in 1..64 {
        let temp = i32::from(block[NATURAL_ORDER[k]]);
        if temp == 0 {
            r += 16;
        } else {
            let sign = temp >> 31;
            let temp = temp + sign;
            let nbits = jpeg_nbits((sign ^ temp) as u32);
            if nbits > max_coef_bits {
                return Err(Error::BadDctCoef);
            }
            while r >= 16 * 16 {
                r -= 16 * 16;
                w.put(actbl.ehufco[0xF0], u32::from(actbl.ehufsi[0xF0]));
            }
            r += nbits;
            w.put(
                actbl.ehufco[r as usize],
                u32::from(actbl.ehufsi[r as usize]),
            );
            // The value bits follow the symbol, in the category's width.
            w.put(temp as u32, nbits);
            r = 0;
        }
    }
    if r > 0 {
        w.put(actbl.ehufco[0], u32::from(actbl.ehufsi[0]));
    }
    Ok(())
}

/// The symbol counts of one table pair (`dc_count_ptrs`, `ac_count_ptrs`), per table number.
#[derive(Debug, Clone)]
pub(super) struct Counts {
    dc: [[i64; 257]; NUM_HUFF_TBLS],
    ac: [[i64; 257]; NUM_HUFF_TBLS],
}

impl Default for Counts {
    fn default() -> Self {
        Self {
            dc: [[0; 257]; NUM_HUFF_TBLS],
            ac: [[0; 257]; NUM_HUFF_TBLS],
        }
    }
}

/// `htest_one_block`: counts the symbols that `encode_one_block` would emit.
fn htest_one_block(
    block: &Block,
    last_dc_val: i32,
    max_coef_bits: u32,
    dc_counts: &mut [i64; 257],
    ac_counts: &mut [i64; 257],
) -> Result<()> {
    let mut temp = i32::from(block[0]) - last_dc_val;
    if temp < 0 {
        temp = -temp;
    }
    let nbits = jpeg_nbits(temp as u32);
    if nbits > max_coef_bits + 1 {
        return Err(Error::BadDctCoef);
    }
    dc_counts[nbits as usize] += 1;
    let mut r: u32 = 0;
    for k in 1..64 {
        let temp = i32::from(block[NATURAL_ORDER[k]]);
        if temp == 0 {
            r += 1;
        } else {
            while r > 15 {
                ac_counts[0xF0] += 1;
                r -= 16;
            }
            let temp = temp.unsigned_abs();
            let nbits = jpeg_nbits(temp);
            if nbits > max_coef_bits {
                return Err(Error::BadDctCoef);
            }
            ac_counts[((r << 4) + nbits) as usize] += 1;
            r = 0;
        }
    }
    if r > 0 {
        ac_counts[0] += 1;
    }
    Ok(())
}

/// `jpeg_gen_optimal_table`: a Huffman table, with codes no longer than 16 bits, for the symbol
/// frequencies in `freq`. The entry 256 is reserved (the all-ones code is never used), so
/// `freq[256]` is set to 1.
pub(super) fn gen_optimal_table(freq: &mut [i64; 257]) -> Result<JHuffTbl> {
    // `bits` is UINT8 in C: its increments and decrements wrap.
    let mut bits = [0u8; MAX_CLEN + 1];
    let mut codesize = [0i32; 257];
    let mut others = [-1i32; 257];
    freq[256] = 1;
    // Compact the non-zero frequencies to the front, remembering their symbols.
    let mut nz_index = [0usize; 257];
    let mut num_nz_symbols = 0usize;
    for i in 0..257 {
        if freq[i] != 0 {
            nz_index[num_nz_symbols] = i;
            freq[num_nz_symbols] = freq[i];
            num_nz_symbols += 1;
        }
    }
    // Huffman's algorithm: merge the two least frequent trees until one is left.
    loop {
        let mut c1: i32 = -1;
        let mut c2: i32 = -1;
        let mut v: i64 = 1_000_000_000;
        let mut v2: i64 = 1_000_000_000;
        for i in 0..num_nz_symbols {
            if freq[i] <= v2 {
                if freq[i] <= v {
                    c2 = c1;
                    v2 = v;
                    v = freq[i];
                    c1 = i as i32;
                } else {
                    v2 = freq[i];
                    c2 = i as i32;
                }
            }
        }
        if c2 < 0 {
            break;
        }
        let c1u = c1 as usize;
        let c2u = c2 as usize;
        freq[c1u] += freq[c2u];
        freq[c2u] = 1_000_000_001;
        codesize[c1u] += 1;
        let mut c1 = c1u;
        while others[c1] >= 0 {
            c1 = others[c1] as usize;
            codesize[c1] += 1;
        }
        others[c1] = c2;
        codesize[c2u] += 1;
        let mut c2 = c2u;
        while others[c2] >= 0 {
            c2 = others[c2] as usize;
            codesize[c2] += 1;
        }
    }
    for i in 0..num_nz_symbols {
        if codesize[i] > MAX_CLEN as i32 {
            return Err(Error::HuffCodeLengthOverflow);
        }
        bits[codesize[i] as usize] = bits[codesize[i] as usize].wrapping_add(1);
    }
    // bit_pos: where each length's symbols start in huffval.
    let mut bit_pos = [0usize; MAX_CLEN + 1];
    let mut p = 0usize;
    for i in 1..=MAX_CLEN {
        bit_pos[i] = p;
        p += usize::from(bits[i]);
    }
    // Limit the code lengths to 16 bits (Annex K.3).
    let mut i = MAX_CLEN;
    while i > 16 {
        while bits[i] > 0 {
            let mut j = i - 2;
            while bits[j] == 0 {
                j -= 1;
            }
            bits[i] = bits[i].wrapping_sub(2);
            bits[i - 1] = bits[i - 1].wrapping_add(1);
            bits[j + 1] = bits[j + 1].wrapping_add(2);
            bits[j] = bits[j].wrapping_sub(1);
        }
        i -= 1;
    }
    while bits[i] == 0 {
        i -= 1;
    }
    bits[i] = bits[i].wrapping_sub(1);

    let mut htbl = JHuffTbl {
        bits: [0; 17],
        huffval: [0; 256],
        sent_table: false,
    };
    htbl.bits.copy_from_slice(&bits[..17]);
    for i in 0..num_nz_symbols - 1 {
        htbl.huffval[bit_pos[codesize[i] as usize]] = nz_index[i] as u8;
        bit_pos[codesize[i] as usize] += 1;
    }
    Ok(htbl)
}

impl Compress {
    /// `finish_pass_gather`: the optimal DC and AC tables for each table number the scan uses
    /// (`jpeg_gen_optimal_table` into `dc_huff_tbl_ptrs` and `ac_huff_tbl_ptrs`).
    pub(super) fn finish_pass_gather(&mut self, counts: &Counts) -> Result<()> {
        let mut did_dc = [false; NUM_HUFF_TBLS];
        let mut did_ac = [false; NUM_HUFF_TBLS];
        for ci in 0..self.num_components {
            let dctbl = self.comp_info[ci].dc_tbl_no as usize;
            let actbl = self.comp_info[ci].ac_tbl_no as usize;
            if dctbl >= NUM_HUFF_TBLS {
                return Err(Error::NoHuffTable(dctbl as i32));
            }
            if actbl >= NUM_HUFF_TBLS {
                return Err(Error::NoHuffTable(actbl as i32));
            }
            if !did_dc[dctbl] {
                let mut freq = counts.dc[dctbl];
                self.dc_huff_tbl_ptrs[dctbl] = Some(gen_optimal_table(&mut freq)?);
                did_dc[dctbl] = true;
            }
            if !did_ac[actbl] {
                let mut freq = counts.ac[actbl];
                self.ac_huff_tbl_ptrs[actbl] = Some(gen_optimal_table(&mut freq)?);
                did_ac[actbl] = true;
            }
        }
        Ok(())
    }

    /// `encode_mcu_gather` over the whole scan: the symbol counts for the optimal tables.
    pub(super) fn gather_statistics(&self, bufs: &[CoefBuffer]) -> Result<Counts> {
        let max_coef_bits = self.data_precision as u32 + 2;
        let mut counts = Counts::default();
        let mut last_dc = [0i32; crate::tables::MAX_COMPONENTS];
        self.for_each_mcu(bufs, |mcu| {
            for &(ci, ref block) in mcu {
                let compptr = self.comp_info[ci];
                htest_one_block(
                    block,
                    last_dc[ci],
                    max_coef_bits,
                    &mut counts.dc[compptr.dc_tbl_no as usize],
                    &mut counts.ac[compptr.ac_tbl_no as usize],
                )?;
                last_dc[ci] = i32::from(block[0]);
            }
            Ok(())
        })?;
        Ok(counts)
    }

    /// `encode_mcu_huff` over the whole scan, followed by `finish_pass_huff`'s bit flush.
    pub(super) fn encode_scan(&self, bufs: &[CoefBuffer], out: &mut Vec<u8>) -> Result<()> {
        let max_coef_bits = self.data_precision as u32 + 2;
        let mut dc_derived: Vec<CDerivedTbl> = Vec::with_capacity(NUM_HUFF_TBLS);
        let mut ac_derived: Vec<CDerivedTbl> = Vec::with_capacity(NUM_HUFF_TBLS);
        for t in 0..NUM_HUFF_TBLS {
            // Built for every table; a missing one is an error only if a component uses it.
            let dc = match &self.dc_huff_tbl_ptrs[t] {
                Some(h) => make_c_derived_tbl(h, true)?,
                None => CDerivedTbl {
                    ehufco: [0; 256],
                    ehufsi: [0; 256],
                },
            };
            let ac = match &self.ac_huff_tbl_ptrs[t] {
                Some(h) => make_c_derived_tbl(h, false)?,
                None => CDerivedTbl {
                    ehufco: [0; 256],
                    ehufsi: [0; 256],
                },
            };
            dc_derived.push(dc);
            ac_derived.push(ac);
        }
        for ci in 0..self.num_components {
            let compptr = self.comp_info[ci];
            if self.dc_huff_tbl_ptrs[compptr.dc_tbl_no as usize].is_none() {
                return Err(Error::NoHuffTable(compptr.dc_tbl_no));
            }
            if self.ac_huff_tbl_ptrs[compptr.ac_tbl_no as usize].is_none() {
                return Err(Error::NoHuffTable(compptr.ac_tbl_no));
            }
        }
        let mut w = BitWriter {
            out: std::mem::take(out),
            acc: 0,
            nbits: 0,
        };
        let mut last_dc = [0i32; crate::tables::MAX_COMPONENTS];
        let result = self.for_each_mcu(bufs, |mcu| {
            for &(ci, ref block) in mcu {
                let compptr = self.comp_info[ci];
                let dctbl = &dc_derived[compptr.dc_tbl_no as usize];
                let actbl = &ac_derived[compptr.ac_tbl_no as usize];
                encode_one_block(&mut w, block, last_dc[ci], dctbl, actbl, max_coef_bits)?;
                last_dc[ci] = i32::from(block[0]);
            }
            Ok(())
        });
        w.flush();
        *out = w.out;
        result
    }
}
