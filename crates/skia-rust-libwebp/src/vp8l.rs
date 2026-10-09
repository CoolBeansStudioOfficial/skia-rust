// Copyright 2014 Google Inc. All Rights Reserved.
//
// Use of this source code is governed by a BSD-style license that can be
// found in the COPYING file. Port by The skia-rust Authors.

//! Port of libwebp `src/dec/vp8l_dec.c` and `src/dec/vp8li_dec.h`: the lossless (VP8L) decoder,
//! including the lossless-coded alpha plane of `ALPH` chunks.
//!
//! Differences from the C structure, none of which change results:
//! - The input bytes are not owned. Every call that reads bits takes the current byte slice, so
//!   the incremental decoder can grow its buffer between updates (`VP8LBitReaderSetBuffer`).
//! - All Huffman tables of one metadata set live in one arena (`Vec<HuffmanCode>`), and an
//!   `HTreeGroup` stores base indices into it.
//! - The pixel buffer is one `Vec<u32>` holding the decoded image, the top-row scratch and the
//!   ARGB cache, in the order libwebp allocates them. The 8-bit alpha path uses a `Vec<u8>`.
//! - `CopyBlock8b`/`CopyBlock32b` are done as the byte-by-byte forward copy they are defined to
//!   be. libwebp's pattern-fill shortcut gives the same result.
//! - The YUV output modes are not ported; Skia asks only for RGB-family modes.

// Module-level clippy allows. Each one mirrors the C source of this module.
// clippy::cast_possible_truncation: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_possible_wrap: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::cast_sign_loss: C integer conversions (int, uint8_t, uint16_t, uint32_t, size_t) are written as `as` casts of the same width and sign as in the C source.
// clippy::chunks_exact_to_as_chunks: mirrors the C word loop over the transform data.
// clippy::enum_variant_names: variant names mirror the C state names.
// clippy::needless_range_loop: the loop index mirrors the C loop, which indexes several arrays.
// clippy::similar_names: local names mirror the C identifiers, which differ by one letter or a suffix.
// clippy::too_many_lines: the function is one C function; splitting it would change the port structure.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::chunks_exact_to_as_chunks,
    clippy::enum_variant_names,
    clippy::needless_range_loop,
    clippy::similar_names,
    clippy::too_many_lines
)]

use crate::alpha::AlphaOutput;
use crate::bit_reader::VP8LBitReader;
use crate::huffman::{
    self, HUFFMAN_PACKED_TABLE_SIZE, HUFFMAN_TABLE_BITS, HUFFMAN_TABLE_MASK, HuffmanCode,
    HuffmanCode32,
};
use crate::io::{self, Io, Status};
use crate::lossless::{self, ColorCache, Transform, TransformType};

/// Port of `NUM_ARGB_CACHE_ROWS`.
pub(crate) const NUM_ARGB_CACHE_ROWS: i32 = 16;
/// Port of `VP8L_MAGIC_BYTE`.
pub const VP8L_MAGIC_BYTE: u8 = 0x2f;
/// Port of `VP8L_FRAME_HEADER_SIZE`.
pub const VP8L_FRAME_HEADER_SIZE: usize = 5;
/// Port of `NUM_LITERAL_CODES`.
const NUM_LITERAL_CODES: i32 = 256;
/// Port of `NUM_LENGTH_CODES`.
const NUM_LENGTH_CODES: i32 = 24;
/// Port of `NUM_DISTANCE_CODES`.
const NUM_DISTANCE_CODES: i32 = 40;
/// Port of `MAX_CACHE_BITS`.
const MAX_CACHE_BITS: i32 = 11;
/// Port of `HUFFMAN_CODES_PER_META_CODE`.
const HUFFMAN_CODES_PER_META_CODE: usize = 5;
/// Port of `VP8L_IMAGE_SIZE_BITS`.
const VP8L_IMAGE_SIZE_BITS: i32 = 14;
/// Port of `VP8L_VERSION_BITS`.
const VP8L_VERSION_BITS: i32 = 3;
/// Port of `DEFAULT_CODE_LENGTH`.
const DEFAULT_CODE_LENGTH: i32 = 8;
/// Port of `LENGTHS_TABLE_BITS`.
const LENGTHS_TABLE_BITS: i32 = 7;
/// Port of `LENGTHS_TABLE_MASK`.
const LENGTHS_TABLE_MASK: u32 = (1 << LENGTHS_TABLE_BITS) - 1;
/// Port of `NUM_CODE_LENGTH_CODES`.
const NUM_CODE_LENGTH_CODES: usize = 19;
/// Port of `SYNC_EVERY_N_ROWS`.
const SYNC_EVERY_N_ROWS: i32 = 8;
/// Port of `BITS_SPECIAL_MARKER`.
const BITS_SPECIAL_MARKER: i32 = 0x100;
/// Port of `PACKED_NON_LITERAL_CODE`.
const PACKED_NON_LITERAL_CODE: i32 = 0;
/// Port of `CODE_TO_PLANE_CODES`.
const CODE_TO_PLANE_CODES: i32 = 120;
/// Size of libwebp's code-length buffer: `(1 << MAX_CACHE_BITS) + NUM_LITERAL_CODES + NUM_LENGTH_CODES`.
const MAX_CODE_LENGTHS_SIZE: usize = (1 << MAX_CACHE_BITS) + 256 + 24;
/// Port of `HUFFMAN_PACKED_BITS`.
const HUFFMAN_PACKED_BITS: i32 = 6;

/// Port of `kCodeLengthLiterals`.
const CODE_LENGTH_LITERALS: i32 = 16;
/// Port of `kCodeLengthRepeatCode`.
const CODE_LENGTH_REPEAT_CODE: i32 = 16;
/// Port of `kCodeLengthExtraBits`.
const CODE_LENGTH_EXTRA_BITS: [i32; 3] = [2, 3, 7];
/// Port of `kCodeLengthRepeatOffsets`.
const CODE_LENGTH_REPEAT_OFFSETS: [i32; 3] = [3, 3, 11];
/// Port of `kCodeLengthCodeOrder`.
const CODE_LENGTH_CODE_ORDER: [usize; NUM_CODE_LENGTH_CODES] = [
    17, 18, 0, 1, 2, 3, 4, 5, 16, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15,
];
/// Port of `kAlphabetSize`.
const ALPHABET_SIZE: [i32; HUFFMAN_CODES_PER_META_CODE] = [
    NUM_LITERAL_CODES + NUM_LENGTH_CODES,
    NUM_LITERAL_CODES,
    NUM_LITERAL_CODES,
    NUM_LITERAL_CODES,
    NUM_DISTANCE_CODES,
];
/// Port of `kLiteralMap`.
const LITERAL_MAP: [i32; HUFFMAN_CODES_PER_META_CODE] = [0, 1, 1, 1, 0];
/// Port of `kCodeToPlane`.
const CODE_TO_PLANE: [u8; 120] = [
    0x18, 0x07, 0x17, 0x19, 0x28, 0x06, 0x27, 0x29, 0x16, 0x1a, 0x26, 0x2a, 0x38, 0x05, 0x37,
    0x39, //
    0x15, 0x1b, 0x36, 0x3a, 0x25, 0x2b, 0x48, 0x04, 0x47, 0x49, 0x14, 0x1c, 0x35, 0x3b, 0x46,
    0x4a, //
    0x24, 0x2c, 0x58, 0x45, 0x4b, 0x34, 0x3c, 0x03, 0x57, 0x59, 0x13, 0x1d, 0x56, 0x5a, 0x23,
    0x2d, //
    0x44, 0x4c, 0x55, 0x5b, 0x33, 0x3d, 0x68, 0x02, 0x67, 0x69, 0x12, 0x1e, 0x66, 0x6a, 0x22,
    0x2e, //
    0x54, 0x5c, 0x43, 0x4d, 0x65, 0x6b, 0x32, 0x3e, 0x78, 0x01, 0x77, 0x79, 0x53, 0x5d, 0x11,
    0x1f, //
    0x64, 0x6c, 0x42, 0x4e, 0x76, 0x7a, 0x21, 0x2f, 0x75, 0x7b, 0x31, 0x3f, 0x63, 0x6d, 0x52,
    0x5e, //
    0x00, 0x74, 0x7c, 0x41, 0x4f, 0x10, 0x20, 0x62, 0x6e, 0x30, 0x73, 0x7d, 0x51, 0x5f, 0x40,
    0x72, //
    0x7e, 0x61, 0x6f, 0x50, 0x71, 0x7f, 0x60, 0x70, //
];
/// Port of `FIXED_TABLE_SIZE`.
const FIXED_TABLE_SIZE: usize = 630 * 3 + 410;
/// Port of `kTableSize`: the arena space reserved per Huffman group, by colour-cache bits. Only
/// used as a capacity hint, since the arena grows.
#[allow(dead_code)] // Capacity hint only; the arena grows as needed, so the values are not read.
const TABLE_SIZE: [usize; 12] = [
    FIXED_TABLE_SIZE + 654,
    FIXED_TABLE_SIZE + 656,
    FIXED_TABLE_SIZE + 658,
    FIXED_TABLE_SIZE + 662,
    FIXED_TABLE_SIZE + 670,
    FIXED_TABLE_SIZE + 686,
    FIXED_TABLE_SIZE + 718,
    FIXED_TABLE_SIZE + 782,
    FIXED_TABLE_SIZE + 912,
    FIXED_TABLE_SIZE + 1168,
    FIXED_TABLE_SIZE + 1680,
    FIXED_TABLE_SIZE + 2704,
];

/// Indices into `HTreeGroup::htrees` (`HuffIndex` in C).
const GREEN: usize = 0;
const RED: usize = 1;
const BLUE: usize = 2;
const ALPHA: usize = 3;
const DIST: usize = 4;

/// Port of `HTreeGroup`. `htrees` are base indices into the metadata's table arena.
#[derive(Debug, Clone)]
pub(crate) struct HTreeGroup {
    pub htrees: [usize; HUFFMAN_CODES_PER_META_CODE],
    pub is_trivial_literal: bool,
    pub literal_arb: u32,
    pub is_trivial_code: bool,
    pub use_packed_table: bool,
    pub packed_table: Vec<HuffmanCode32>,
}

impl Default for HTreeGroup {
    fn default() -> Self {
        Self {
            htrees: [0; HUFFMAN_CODES_PER_META_CODE],
            is_trivial_literal: false,
            literal_arb: 0,
            is_trivial_code: false,
            use_packed_table: false,
            packed_table: vec![HuffmanCode32::default(); HUFFMAN_PACKED_TABLE_SIZE as usize],
        }
    }
}

/// Port of `VP8LMetadata`.
#[derive(Debug, Clone, Default)]
#[doc(alias = "VP8LMetadata")]
pub(crate) struct Metadata {
    pub color_cache_size: i32,
    pub color_cache: ColorCache,
    pub saved_color_cache: ColorCache,
    pub huffman_mask: i32,
    pub huffman_subsample_bits: i32,
    pub huffman_xsize: i32,
    pub huffman_image: Vec<u32>,
    pub htree_groups: Vec<HTreeGroup>,
    /// Arena holding every Huffman table of this metadata (`HuffmanTables` in C).
    pub huffman_tables: Vec<HuffmanCode>,
}

/// Port of `VP8LDecodeState`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum DecodeState {
    #[default]
    ReadDim,
    ReadHdr,
    ReadData,
}

/// Where decoded rows go. `Sink::None` is `process_func == NULL` in C.
pub(crate) enum Sink<'s, 'o> {
    None,
    /// `ProcessRows`: colour-converts rows into the caller's output.
    Argb(&'s mut Io<'o>),
    /// `ExtractAlphaRows`/`ExtractPalettedAlphaRows`: stores the green plane as alpha.
    Alpha(&'s mut AlphaOutput),
}

/// Port of `VP8LDecoder`. The input bytes are passed to each method; see the module notes.
#[derive(Debug, Default)]
#[doc(alias = "VP8LDecoder")]
pub(crate) struct Vp8lDecoder {
    pub status: Status,
    state: DecodeState,
    pub width: i32,
    pub height: i32,
    /// `pixels_` (ARGB): the decoded image, then the top-row scratch and the ARGB cache.
    pub pixels: Vec<u32>,
    /// `pixels_` in the 8-bit alpha path.
    pub pixels8: Vec<u8>,
    pub argb_cache: usize,
    pub br: VP8LBitReader,
    pub incremental: bool,
    saved_br: VP8LBitReader,
    saved_last_pixel: i32,
    pub last_row: i32,
    pub last_pixel: i32,
    pub last_out_row: i32,
    pub hdr: Metadata,
    pub next_transform: usize,
    pub transforms: [Transform; 4],
    pub transforms_seen: u32,
}

/// Port of `VP8LSetError`: the oldest error takes precedence over a new one.
fn set_error(dec: &mut Vp8lDecoder, error: Status) -> bool {
    if dec.status == Status::Ok || dec.status == Status::Suspended {
        dec.status = error;
    }
    false
}

/// Port of `VP8LCheckSignature`.
#[doc(alias = "VP8LCheckSignature")]
#[must_use]
pub fn check_signature(data: &[u8]) -> bool {
    data.len() >= VP8L_FRAME_HEADER_SIZE && data[0] == VP8L_MAGIC_BYTE && (data[4] >> 5) == 0
}

/// Port of `ReadImageInfo`. Returns `(width, height, has_alpha)`.
fn read_image_info(br: &mut VP8LBitReader, data: &[u8]) -> Option<(i32, i32, bool)> {
    if br.read_bits(data, 8) != u32::from(VP8L_MAGIC_BYTE) {
        return None;
    }
    let width = br.read_bits(data, VP8L_IMAGE_SIZE_BITS) as i32 + 1;
    let height = br.read_bits(data, VP8L_IMAGE_SIZE_BITS) as i32 + 1;
    let has_alpha = br.read_bits(data, 1) != 0;
    if br.read_bits(data, VP8L_VERSION_BITS) != 0 {
        return None;
    }
    if br.eos() {
        None
    } else {
        Some((width, height, has_alpha))
    }
}

/// Port of `VP8LGetInfo`: `(width, height, has_alpha)` of a VP8L payload, or `None`.
#[doc(alias = "VP8LGetInfo")]
#[must_use]
pub fn get_info(data: &[u8]) -> Option<(i32, i32, bool)> {
    if data.len() < VP8L_FRAME_HEADER_SIZE || !check_signature(data) {
        return None;
    }
    let mut br = VP8LBitReader::new(data);
    read_image_info(&mut br, data)
}

/// Port of `GetCopyDistance`.
#[inline]
fn get_copy_distance(distance_symbol: i32, br: &mut VP8LBitReader, data: &[u8]) -> i32 {
    if distance_symbol < 4 {
        return distance_symbol + 1;
    }
    let extra_bits = (distance_symbol - 2) >> 1;
    let offset = (2 + (distance_symbol & 1)) << extra_bits;
    offset + br.read_bits(data, extra_bits) as i32 + 1
}

/// Port of `PlaneCodeToDistance`.
#[inline]
fn plane_code_to_distance(xsize: i32, plane_code: i32) -> i32 {
    if plane_code > CODE_TO_PLANE_CODES {
        plane_code - CODE_TO_PLANE_CODES
    } else {
        let dist_code = i32::from(CODE_TO_PLANE[(plane_code - 1) as usize]);
        let yoffset = dist_code >> 4;
        let xoffset = 8 - (dist_code & 0xf);
        let dist = yoffset * xsize + xoffset;
        if dist >= 1 { dist } else { 1 } // dist<1 can happen if xsize is very small
    }
}

/// Port of `ReadSymbol`. `table` is the arena index of the root table. It only uses the
/// prefetched bits; `_data` keeps the call sites uniform with the other readers.
#[inline]
fn read_symbol(arena: &[HuffmanCode], table: usize, br: &mut VP8LBitReader, _data: &[u8]) -> i32 {
    let val = br.prefetch_bits();
    let mut t = table + (val & HUFFMAN_TABLE_MASK) as usize;
    let mut code = arena[t];
    let nbits = i32::from(code.bits) - HUFFMAN_TABLE_BITS;
    if nbits > 0 {
        br.set_bit_pos(br.bit_pos() + HUFFMAN_TABLE_BITS);
        let val = br.prefetch_bits();
        t += usize::from(code.value);
        t += (val & ((1 << nbits) - 1)) as usize;
        code = arena[t];
    }
    br.set_bit_pos(br.bit_pos() + i32::from(code.bits));
    i32::from(code.value)
}

/// Port of `ReadPackedSymbols`. Returns the code and writes `dst` for a packed literal.
#[inline]
fn read_packed_symbols(group: &HTreeGroup, br: &mut VP8LBitReader, dst: &mut u32) -> i32 {
    let val = br.prefetch_bits() & (HUFFMAN_PACKED_TABLE_SIZE - 1);
    let code = group.packed_table[val as usize];
    if code.bits < BITS_SPECIAL_MARKER {
        br.set_bit_pos(br.bit_pos() + code.bits);
        *dst = code.value;
        PACKED_NON_LITERAL_CODE
    } else {
        br.set_bit_pos(br.bit_pos() + code.bits - BITS_SPECIAL_MARKER);
        code.value as i32
    }
}

/// Port of `AccumulateHCode`.
fn accumulate_hcode(hcode: HuffmanCode, shift: i32, huff: &mut HuffmanCode32) -> i32 {
    huff.bits += i32::from(hcode.bits);
    huff.value |= u32::from(hcode.value) << shift;
    i32::from(hcode.bits)
}

/// Port of `BuildPackedTable`.
fn build_packed_table(group: &mut HTreeGroup, arena: &[HuffmanCode]) {
    for code in 0..HUFFMAN_PACKED_TABLE_SIZE {
        let mut bits = code;
        let hcode = arena[group.htrees[GREEN] + bits as usize];
        if i32::from(hcode.value) >= NUM_LITERAL_CODES {
            group.packed_table[bits as usize] = HuffmanCode32 {
                bits: i32::from(hcode.bits) + BITS_SPECIAL_MARKER,
                value: u32::from(hcode.value),
            };
        } else {
            let mut huff = HuffmanCode32::default();
            bits >>= accumulate_hcode(hcode, 8, &mut huff);
            let h = arena[group.htrees[RED] + bits as usize];
            bits >>= accumulate_hcode(h, 16, &mut huff);
            let h = arena[group.htrees[BLUE] + bits as usize];
            bits >>= accumulate_hcode(h, 0, &mut huff);
            let h = arena[group.htrees[ALPHA] + bits as usize];
            let _ = accumulate_hcode(h, 24, &mut huff);
            group.packed_table[code as usize] = huff;
        }
    }
}

/// Port of `ReadHuffmanCodeLengths`. Fills `code_lengths[..num_symbols]`.
fn read_huffman_code_lengths(
    dec: &mut Vp8lDecoder,
    data: &[u8],
    code_length_code_lengths: &[i32; NUM_CODE_LENGTH_CODES],
    num_symbols: i32,
    code_lengths: &mut [i32],
) -> bool {
    let mut prev_code_len = DEFAULT_CODE_LENGTH;
    let mut tables: Vec<HuffmanCode> = Vec::new();
    let Some((table_base, _)) =
        huffman::build_table(&mut tables, LENGTHS_TABLE_BITS, code_length_code_lengths)
    else {
        return set_error(dec, Status::BitstreamError);
    };
    let mut max_symbol = if dec.br.read_bits(data, 1) != 0 {
        // use length
        let length_nbits = 2 + 2 * dec.br.read_bits(data, 3) as i32;
        let ms = 2 + dec.br.read_bits(data, length_nbits) as i32;
        if ms > num_symbols {
            return set_error(dec, Status::BitstreamError);
        }
        ms
    } else {
        num_symbols
    };
    let mut symbol: i32 = 0;
    while symbol < num_symbols {
        if max_symbol == 0 {
            break;
        }
        max_symbol -= 1;
        dec.br.fill_bit_window(data);
        let p = tables[table_base + (dec.br.prefetch_bits() & LENGTHS_TABLE_MASK) as usize];
        dec.br.set_bit_pos(dec.br.bit_pos() + i32::from(p.bits));
        let code_len = i32::from(p.value);
        if code_len < CODE_LENGTH_LITERALS {
            code_lengths[symbol as usize] = code_len;
            symbol += 1;
            if code_len != 0 {
                prev_code_len = code_len;
            }
        } else {
            let use_prev = code_len == CODE_LENGTH_REPEAT_CODE;
            let slot = (code_len - CODE_LENGTH_LITERALS) as usize;
            let extra_bits = CODE_LENGTH_EXTRA_BITS[slot];
            let repeat_offset = CODE_LENGTH_REPEAT_OFFSETS[slot];
            let mut repeat = dec.br.read_bits(data, extra_bits) as i32 + repeat_offset;
            if symbol + repeat > num_symbols {
                return set_error(dec, Status::BitstreamError);
            }
            let length = if use_prev { prev_code_len } else { 0 };
            while repeat > 0 {
                repeat -= 1;
                code_lengths[symbol as usize] = length;
                symbol += 1;
            }
        }
    }
    true
}

/// Port of `ReadHuffmanCode`. When `arena` is `None` only the validity is checked (the
/// `table == NULL` case). Returns `(size, base)`, where `size == 0` is an error.
fn read_huffman_code(
    dec: &mut Vp8lDecoder,
    data: &[u8],
    alphabet_size: i32,
    code_lengths: &mut [i32],
    arena: Option<&mut Vec<HuffmanCode>>,
) -> (usize, usize) {
    let mut ok;
    let simple_code = dec.br.read_bits(data, 1);
    for v in &mut code_lengths[..alphabet_size as usize] {
        *v = 0;
    }
    if simple_code != 0 {
        // Read symbols, codes & code lengths directly.
        let num_symbols = dec.br.read_bits(data, 1) as i32 + 1;
        let first_symbol_len_code = dec.br.read_bits(data, 1);
        // The first code is either 1 bit or 8 bit code.
        let symbol = dec
            .br
            .read_bits(data, if first_symbol_len_code == 0 { 1 } else { 8 })
            as usize;
        code_lengths[symbol] = 1;
        // The second code (if present), is always 8 bits long.
        if num_symbols == 2 {
            let symbol = dec.br.read_bits(data, 8) as usize;
            code_lengths[symbol] = 1;
        }
        ok = true;
    } else {
        // Decode Huffman-coded code lengths.
        let mut code_length_code_lengths = [0i32; NUM_CODE_LENGTH_CODES];
        let num_codes = dec.br.read_bits(data, 4) as usize + 4;
        for &order in CODE_LENGTH_CODE_ORDER.iter().take(num_codes) {
            code_length_code_lengths[order] = dec.br.read_bits(data, 3) as i32;
        }
        ok = read_huffman_code_lengths(
            dec,
            data,
            &code_length_code_lengths,
            alphabet_size,
            code_lengths,
        );
    }
    ok = ok && !dec.br.eos();
    let mut size = 0usize;
    let mut base = 0usize;
    if ok {
        match arena {
            None => {
                size =
                    huffman::validate(HUFFMAN_TABLE_BITS, &code_lengths[..alphabet_size as usize]);
            }
            Some(arena) => {
                if let Some((b, s)) = huffman::build_table(
                    arena,
                    HUFFMAN_TABLE_BITS,
                    &code_lengths[..alphabet_size as usize],
                ) {
                    base = b;
                    size = s;
                }
            }
        }
    }
    if !ok || size == 0 {
        set_error(dec, Status::BitstreamError);
        return (0, 0);
    }
    (size, base)
}

/// Port of `ReadHuffmanCodesHelper`. Builds `hdr.htree_groups` and the arena.
fn read_huffman_codes_helper(
    dec: &mut Vp8lDecoder,
    data: &[u8],
    color_cache_bits: i32,
    num_htree_groups: usize,
    num_htree_groups_max: usize,
    mapping: Option<&[i32]>,
) -> bool {
    let mut code_lengths = vec![0i32; MAX_CODE_LENGTHS_SIZE];
    if (mapping.is_none() && num_htree_groups != num_htree_groups_max)
        || num_htree_groups > num_htree_groups_max
    {
        return false;
    }
    let mut groups: Vec<HTreeGroup> = vec![HTreeGroup::default(); num_htree_groups];
    let mut arena: Vec<HuffmanCode> = Vec::new();
    for i in 0..num_htree_groups_max {
        // If the index "i" is unused in the Huffman image, just make sure the coefficients are
        // valid but do not store them.
        if mapping.is_some_and(|m| m[i] == -1) {
            for (j, &size) in ALPHABET_SIZE.iter().enumerate() {
                let mut alphabet_size = size;
                if j == 0 && color_cache_bits > 0 {
                    alphabet_size += 1 << color_cache_bits;
                }
                // Passing in NULL so that nothing gets filled.
                if read_huffman_code(dec, data, alphabet_size, &mut code_lengths, None).0 == 0 {
                    return false;
                }
            }
        } else {
            let group_index = mapping.map_or(i, |m| m[i] as usize);
            let mut htrees = [0usize; HUFFMAN_CODES_PER_META_CODE];
            let mut total_size: i32 = 0;
            let mut is_trivial_literal = true;
            let mut max_bits: i32 = 0;
            for j in 0..HUFFMAN_CODES_PER_META_CODE {
                let mut alphabet_size = ALPHABET_SIZE[j];
                if j == 0 && color_cache_bits > 0 {
                    alphabet_size += 1 << color_cache_bits;
                }
                let (size, base) = read_huffman_code(
                    dec,
                    data,
                    alphabet_size,
                    &mut code_lengths,
                    Some(&mut arena),
                );
                htrees[j] = base;
                if size == 0 {
                    return false;
                }
                if is_trivial_literal && LITERAL_MAP[j] == 1 {
                    is_trivial_literal = arena[base].bits == 0;
                }
                total_size += i32::from(arena[base].bits);
                if j <= ALPHA {
                    let mut local_max_bits = code_lengths[0];
                    for k in 1..alphabet_size as usize {
                        if code_lengths[k] > local_max_bits {
                            local_max_bits = code_lengths[k];
                        }
                    }
                    max_bits += local_max_bits;
                }
            }
            let group = &mut groups[group_index];
            group.htrees = htrees;
            group.is_trivial_literal = is_trivial_literal;
            group.is_trivial_code = false;
            if is_trivial_literal {
                let red = u32::from(arena[htrees[RED]].value);
                let blue = u32::from(arena[htrees[BLUE]].value);
                let alpha = u32::from(arena[htrees[ALPHA]].value);
                group.literal_arb = (alpha << 24) | (red << 16) | blue;
                if total_size == 0 && i32::from(arena[htrees[GREEN]].value) < NUM_LITERAL_CODES {
                    group.is_trivial_code = true;
                    group.literal_arb |= u32::from(arena[htrees[GREEN]].value) << 8;
                }
            }
            group.use_packed_table = !group.is_trivial_code && max_bits < HUFFMAN_PACKED_BITS;
            if group.use_packed_table {
                build_packed_table(group, &arena);
            }
        }
    }
    dec.hdr.huffman_tables = arena;
    dec.hdr.htree_groups = groups;
    true
}

/// Port of `ReadHuffmanCodes`. Reads the optional meta-Huffman image and the groups of codes.
fn read_huffman_codes(
    dec: &mut Vp8lDecoder,
    data: &[u8],
    xsize: i32,
    ysize: i32,
    color_cache_bits: i32,
    allow_recursion: bool,
) -> bool {
    let mut num_htree_groups = 1usize;
    let mut num_htree_groups_max = 1usize;
    let mut mapping: Option<Vec<i32>> = None;
    let mut huffman_image: Vec<u32> = Vec::new();
    if allow_recursion && dec.br.read_bits(data, 1) != 0 {
        // use meta Huffman codes.
        let huffman_precision = dec.br.read_bits(data, 3) as i32 + 2;
        let huffman_xsize = lossless::sub_sample_size(xsize, huffman_precision);
        let huffman_ysize = lossless::sub_sample_size(ysize, huffman_precision);
        let huffman_pixs = (huffman_xsize * huffman_ysize) as usize;
        let Some(img) = decode_image_stream(dec, data, huffman_xsize, huffman_ysize, false) else {
            return false;
        };
        huffman_image = img;
        dec.hdr.huffman_subsample_bits = huffman_precision;
        for pix in huffman_image.iter_mut().take(huffman_pixs) {
            // The huffman data is stored in red and green bytes.
            let group = (*pix >> 8) & 0xffff;
            *pix = group;
            if group as usize >= num_htree_groups_max {
                num_htree_groups_max = group as usize + 1;
            }
        }
        // Check the validity of num_htree_groups_max. If it seems too big, use a smaller value for
        // later, so that a bad bitstream does not cause big allocations.
        if num_htree_groups_max > 1000
            || num_htree_groups_max as i64 > i64::from(xsize) * i64::from(ysize)
        {
            // Create a mapping from the used indices to the minimal set of used values.
            let mut map = vec![-1i32; num_htree_groups_max];
            num_htree_groups = 0;
            for pix in huffman_image.iter_mut().take(huffman_pixs) {
                // Get the current mapping for the group and remap the Huffman image.
                let mapped = &mut map[*pix as usize];
                if *mapped == -1 {
                    *mapped = num_htree_groups as i32;
                    num_htree_groups += 1;
                }
                *pix = *mapped as u32;
            }
            mapping = Some(map);
        } else {
            num_htree_groups = num_htree_groups_max;
        }
    }
    if dec.br.eos() {
        return false;
    }
    if !read_huffman_codes_helper(
        dec,
        data,
        color_cache_bits,
        num_htree_groups,
        num_htree_groups_max,
        mapping.as_deref(),
    ) {
        dec.hdr.huffman_image.clear();
        return false;
    }
    dec.hdr.huffman_image = huffman_image;
    true
}

/// Port of `ExpandColorMap`.
fn expand_color_map(num_colors: i32, transform: &mut Transform) {
    let final_num_colors = 1usize << (8 >> transform.bits);
    let mut new_data = vec![0u8; 4 * final_num_colors];
    let data: Vec<u8> = transform
        .data
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect();
    new_data[0..4].copy_from_slice(&data[0..4]);
    for i in 4..(4 * num_colors as usize) {
        // Equivalent to VP8LAddPixels(), on a byte-basis.
        new_data[i] = data[i].wrapping_add(new_data[i - 4]);
    }
    // Remaining entries are black (zero) as in C.
    transform.data = new_data
        .chunks_exact(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
}

/// Port of `ReadTransform`.
fn read_transform(dec: &mut Vp8lDecoder, data: &[u8], xsize: &mut i32, ysize: i32) -> bool {
    let transform_type = dec.br.read_bits(data, 2);
    // Each transform type can only be present once in the stream.
    if dec.transforms_seen & (1 << transform_type) != 0 {
        return false; // Already there, let's not accept the second same transform.
    }
    dec.transforms_seen |= 1 << transform_type;
    let idx = dec.next_transform;
    let ttype = match transform_type {
        0 => TransformType::PredictorTransform,
        1 => TransformType::CrossColorTransform,
        2 => TransformType::SubtractGreenTransform,
        _ => TransformType::ColorIndexingTransform,
    };
    {
        let t = &mut dec.transforms[idx];
        t.ttype = Some(ttype);
        t.xsize = *xsize;
        t.ysize = ysize;
        t.data = Vec::new();
    }
    dec.next_transform += 1;
    match ttype {
        TransformType::PredictorTransform | TransformType::CrossColorTransform => {
            let bits = dec.br.read_bits(data, 3) as i32 + 2;
            dec.transforms[idx].bits = bits;
            let (tx, ty) = (dec.transforms[idx].xsize, dec.transforms[idx].ysize);
            let Some(img) = decode_image_stream(
                dec,
                data,
                lossless::sub_sample_size(tx, bits),
                lossless::sub_sample_size(ty, bits),
                false,
            ) else {
                return false;
            };
            dec.transforms[idx].data = img;
            true
        }
        TransformType::ColorIndexingTransform => {
            let num_colors = dec.br.read_bits(data, 8) as i32 + 1;
            let bits = if num_colors > 16 {
                0
            } else if num_colors > 4 {
                1
            } else if num_colors > 2 {
                2
            } else {
                3
            };
            *xsize = lossless::sub_sample_size(dec.transforms[idx].xsize, bits);
            dec.transforms[idx].bits = bits;
            let Some(img) = decode_image_stream(dec, data, num_colors, 1, false) else {
                return false;
            };
            dec.transforms[idx].data = img;
            expand_color_map(num_colors, &mut dec.transforms[idx]);
            true
        }
        TransformType::SubtractGreenTransform => true,
    }
}

/// Port of `ClearMetadata`.
fn clear_metadata(hdr: &mut Metadata) {
    *hdr = Metadata::default();
}

/// Port of `UpdateDecoder`.
fn update_decoder(dec: &mut Vp8lDecoder, width: i32, height: i32) {
    let num_bits = dec.hdr.huffman_subsample_bits;
    dec.width = width;
    dec.height = height;
    dec.hdr.huffman_xsize = lossless::sub_sample_size(width, num_bits);
    dec.hdr.huffman_mask = if num_bits == 0 {
        !0
    } else {
        (1 << num_bits) - 1
    };
}

/// Port of `GetMetaIndex` and `GetHtreeGroupForPos`: the group index for pixel `(x, y)`.
#[inline]
fn group_index_for_pos(hdr: &Metadata, x: i32, y: i32) -> usize {
    let bits = hdr.huffman_subsample_bits;
    if bits == 0 {
        return 0;
    }
    hdr.huffman_image[(hdr.huffman_xsize * (y >> bits) + (x >> bits)) as usize] as usize
}

/// Port of `DecodeImageStream`. Returns the decoded data for sub-images (`is_level0 == false`),
/// an empty vector for a successful level-0 header read, and `None` on error.
fn decode_image_stream(
    dec: &mut Vp8lDecoder,
    data: &[u8],
    xsize: i32,
    ysize: i32,
    is_level0: bool,
) -> Option<Vec<u32>> {
    let mut ok = true;
    let mut transform_xsize = xsize;
    let transform_ysize = ysize;
    let mut color_cache_bits = 0;
    // Read the transforms (may recurse).
    if is_level0 {
        while ok && dec.br.read_bits(data, 1) != 0 {
            ok = read_transform(dec, data, &mut transform_xsize, transform_ysize);
        }
    }
    // Color cache
    if ok && dec.br.read_bits(data, 1) != 0 {
        color_cache_bits = dec.br.read_bits(data, 4) as i32;
        ok = (1..=MAX_CACHE_BITS).contains(&color_cache_bits);
        if !ok {
            set_error(dec, Status::BitstreamError);
            clear_metadata(&mut dec.hdr);
            return None;
        }
    }
    // Read the Huffman codes (may recurse).
    ok = ok
        && read_huffman_codes(
            dec,
            data,
            transform_xsize,
            transform_ysize,
            color_cache_bits,
            is_level0,
        );
    if !ok {
        set_error(dec, Status::BitstreamError);
        clear_metadata(&mut dec.hdr);
        return None;
    }
    // Finish setting up the colour cache.
    if color_cache_bits > 0 {
        dec.hdr.color_cache_size = 1 << color_cache_bits;
        dec.hdr.color_cache = ColorCache::new(color_cache_bits);
    } else {
        dec.hdr.color_cache_size = 0;
    }
    update_decoder(dec, transform_xsize, transform_ysize);
    if is_level0 {
        // level 0 complete
        dec.state = DecodeState::ReadHdr;
        dec.last_pixel = 0; // Reset for future DECODE_DATA_FUNC() calls.
        return Some(Vec::new());
    }
    // Use the Huffman trees to decode the LZ77 encoded data.
    let mut pix = vec![0u32; (transform_xsize as usize) * (transform_ysize as usize)];
    ok = dec.decode_image_data(
        data,
        &mut pix,
        transform_xsize,
        transform_ysize,
        transform_ysize,
        &mut Sink::None,
    );
    ok = ok && !dec.br.eos();
    if !ok {
        clear_metadata(&mut dec.hdr);
        return None;
    }
    dec.last_pixel = 0; // Reset for future DECODE_DATA_FUNC() calls.
    clear_metadata(&mut dec.hdr); // Clean up temporary data behind.
    Some(pix)
}

/// Port of `SaveState`.
fn save_state(dec: &mut Vp8lDecoder, last_pixel: i32) {
    dec.saved_br = dec.br.clone();
    dec.saved_last_pixel = last_pixel;
    if dec.hdr.color_cache_size > 0 {
        dec.hdr.saved_color_cache = dec.hdr.color_cache.clone();
    }
}

/// Port of `RestoreState`.
fn restore_state(dec: &mut Vp8lDecoder) {
    dec.status = Status::Suspended;
    dec.br = dec.saved_br.clone();
    dec.last_pixel = dec.saved_last_pixel;
    if dec.hdr.color_cache_size > 0 {
        dec.hdr.color_cache = dec.hdr.saved_color_cache.clone();
    }
}

/// Port of `ApplyInverseTransforms`. The input rows start at `pix[rows]` with stride `dec.width`;
/// the output is written to `pix[dec.argb_cache..]`. The row above the cache (C's
/// `argb_cache_ - width`) is the top-row scratch for the predictor.
///
/// The C code runs the transforms in place in the cache. Here each stage copies its input (and the
/// window it writes, including the top row) out of `pix`, so no two borrows alias.
fn apply_inverse_transforms(
    dec: &Vp8lDecoder,
    pix: &mut [u32],
    start_row: i32,
    num_rows: i32,
    rows: usize,
) {
    let width = dec.width as usize;
    let cache_pixs = width * num_rows as usize;
    let end_row = start_row + num_rows;
    let n_rows = num_rows as usize;
    let cache = dec.argb_cache;
    if dec.next_transform == 0 {
        // No transform called, hence just copy.
        let src = pix[rows..rows + cache_pixs].to_vec();
        pix[cache..cache + cache_pixs].copy_from_slice(&src);
        return;
    }
    // Stage k (transforms applied from the last one read to the first) reads the previous stage's
    // output, which has the stride of that stage's xsize; the first stage reads `dec.width`.
    let mut in_stride = width;
    let mut from_pixels = true;
    let mut n = dec.next_transform;
    while n > 0 {
        n -= 1;
        let t = &dec.transforms[n];
        let tw = t.xsize as usize;
        let input: Vec<u32> = if from_pixels {
            pix[rows..rows + in_stride * n_rows].to_vec()
        } else {
            pix[cache..cache + in_stride * n_rows].to_vec()
        };
        // The window the stage writes: its top row plus `num_rows` rows of width `tw`.
        let win_start = cache - tw;
        let win_len = tw * (n_rows + 1);
        let mut window = pix[win_start..win_start + win_len].to_vec();
        lossless::inverse_transform(t, start_row, end_row, &input, &mut window);
        pix[win_start..win_start + win_len].copy_from_slice(&window);
        in_stride = tw;
        from_pixels = false;
    }
}

/// Port of `SetCropWindow`, in pixel units: `in_offset` is the index of the first input pixel, and
/// the input rows are `io.width` pixels apart. Returns `false` if the crop window is empty.
fn set_crop_window(
    io: &mut Io<'_>,
    mut y_start: i32,
    mut y_end: i32,
    in_offset: &mut usize,
) -> bool {
    if y_end > io.crop_bottom {
        y_end = io.crop_bottom; // make sure we don't overflow on last row.
    }
    if y_start < io.crop_top {
        let delta = io.crop_top - y_start;
        y_start = io.crop_top;
        *in_offset += delta as usize * io.width as usize;
    }
    if y_start >= y_end {
        return false; // Crop window is empty.
    }
    *in_offset += io.crop_left as usize;
    io.mb_y = y_start - io.crop_top;
    io.mb_w = io.crop_right - io.crop_left;
    io.mb_h = y_end - y_start;
    true
}

impl Vp8lDecoder {
    /// Port of `VP8LNew`.
    #[doc(alias = "VP8LNew")]
    #[must_use]
    pub fn new() -> Self {
        Self {
            status: Status::Ok,
            state: DecodeState::ReadDim,
            ..Self::default()
        }
    }

    /// Port of `DecodeImageData`.
    fn decode_image_data(
        &mut self,
        data: &[u8],
        pix: &mut [u32],
        width: i32,
        height: i32,
        last_row: i32,
        sink: &mut Sink<'_, '_>,
    ) -> bool {
        let mut row = self.last_pixel / width;
        let mut col = self.last_pixel % width;
        let mut src = self.last_pixel as usize;
        let mut last_cached = src;
        let src_end = (width * height) as usize;
        let src_last = (width * last_row) as usize;
        let len_code_limit = NUM_LITERAL_CODES + NUM_LENGTH_CODES;
        let color_cache_limit = len_code_limit + self.hdr.color_cache_size;
        let mut next_sync_row = if self.incremental { row } else { 1 << 24 };
        let has_cache = self.hdr.color_cache_size > 0;
        let mask = self.hdr.huffman_mask;
        let mut group_idx = if src < src_last {
            group_index_for_pos(&self.hdr, col, row)
        } else {
            0
        };
        let mut error = false;
        'outer: while src < src_last {
            if row >= next_sync_row {
                save_state(self, src as i32);
                next_sync_row = row + SYNC_EVERY_N_ROWS;
            }
            // Only update when changing tile.
            if (col & mask) == 0 {
                group_idx = group_index_for_pos(&self.hdr, col, row);
            }
            let group = &self.hdr.htree_groups[group_idx];
            let mut literal_done = false;
            if group.is_trivial_code {
                pix[src] = group.literal_arb;
                literal_done = true;
            } else {
                self.br.fill_bit_window(data);
                let code: i32;
                if group.use_packed_table {
                    let mut v = 0u32;
                    let c = read_packed_symbols(group, &mut self.br, &mut v);
                    if c == PACKED_NON_LITERAL_CODE {
                        // ReadPackedSymbols writes the literal before the end-of-stream check.
                        pix[src] = v;
                    }
                    if self.br.is_end_of_stream(data) {
                        break 'outer;
                    }
                    if c == PACKED_NON_LITERAL_CODE {
                        literal_done = true;
                        code = 0;
                    } else {
                        code = c;
                    }
                } else {
                    code = read_symbol(
                        &self.hdr.huffman_tables,
                        group.htrees[GREEN],
                        &mut self.br,
                        data,
                    );
                }
                if !literal_done {
                    if self.br.is_end_of_stream(data) {
                        break 'outer;
                    }
                    if code < NUM_LITERAL_CODES {
                        // Literal
                        if group.is_trivial_literal {
                            pix[src] = group.literal_arb | ((code as u32) << 8);
                        } else {
                            let arena = &self.hdr.huffman_tables;
                            let red = read_symbol(arena, group.htrees[RED], &mut self.br, data);
                            self.br.fill_bit_window(data);
                            let blue = read_symbol(arena, group.htrees[BLUE], &mut self.br, data);
                            let alpha = read_symbol(arena, group.htrees[ALPHA], &mut self.br, data);
                            if self.br.is_end_of_stream(data) {
                                break 'outer;
                            }
                            pix[src] = ((alpha as u32) << 24)
                                | ((red as u32) << 16)
                                | ((code as u32) << 8)
                                | (blue as u32);
                        }
                        literal_done = true;
                    } else if code < len_code_limit {
                        // Backward reference
                        let length_sym = code - NUM_LITERAL_CODES;
                        let length = get_copy_distance(length_sym, &mut self.br, data);
                        let arena = &self.hdr.huffman_tables;
                        let dist_symbol =
                            read_symbol(arena, group.htrees[DIST], &mut self.br, data);
                        self.br.fill_bit_window(data);
                        let dist_code = get_copy_distance(dist_symbol, &mut self.br, data);
                        let dist = plane_code_to_distance(width, dist_code);
                        if self.br.is_end_of_stream(data) {
                            break 'outer;
                        }
                        if (src as i64) < i64::from(dist)
                            || ((src_end - src) as i64) < i64::from(length)
                        {
                            error = true;
                            break 'outer;
                        }
                        copy_block(pix, src, dist as usize, length as usize);
                        src += length as usize;
                        col += length;
                        while col >= width {
                            col -= width;
                            row += 1;
                            if row <= last_row && (row % NUM_ARGB_CACHE_ROWS == 0) {
                                self.process_rows_hook(pix, sink, row);
                            }
                        }
                        // Because of the check done above, `src <= src_end` holds.
                        if (col & mask) != 0 {
                            group_idx = group_index_for_pos(&self.hdr, col, row);
                        }
                        if has_cache {
                            while last_cached < src {
                                self.hdr.color_cache.insert(pix[last_cached]);
                                last_cached += 1;
                            }
                        }
                        continue 'outer;
                    } else if code < color_cache_limit {
                        // Color cache
                        let key = code - len_code_limit;
                        while last_cached < src {
                            self.hdr.color_cache.insert(pix[last_cached]);
                            last_cached += 1;
                        }
                        pix[src] = self.hdr.color_cache.lookup(key as u32);
                        literal_done = true;
                    } else {
                        // Not reached
                        error = true;
                        break 'outer;
                    }
                }
            }
            // AdvanceByOne
            if literal_done {
                src += 1;
                col += 1;
                if col >= width {
                    col = 0;
                    row += 1;
                    if row <= last_row && (row % NUM_ARGB_CACHE_ROWS == 0) {
                        self.process_rows_hook(pix, sink, row);
                    }
                    if has_cache {
                        while last_cached < src {
                            self.hdr.color_cache.insert(pix[last_cached]);
                            last_cached += 1;
                        }
                    }
                }
            }
        }
        if error {
            return set_error(self, Status::BitstreamError);
        }
        let eos = self.br.is_end_of_stream(data);
        self.br.set_eos(eos);
        // In incremental decoding: if the input ended before `src_last`, the decoder state is
        // restored and more data is awaited. Otherwise the rows are finished.
        if self.incremental && eos && src < src_last {
            restore_state(self);
        } else if (self.incremental && src >= src_last) || !eos {
            // Process the remaining rows corresponding to last row-block.
            let r = if row > last_row { last_row } else { row };
            self.process_rows_hook(pix, sink, r);
            self.status = Status::Ok;
            self.last_pixel = src as i32; // end-of-scan marker
        } else {
            // if not incremental, and we are past the end of buffer (eos_=1), then this is a real
            // bitstream error.
            return set_error(self, Status::BitstreamError);
        }
        true
    }

    /// Port of `ProcessRows`: transforms the rows decoded since the last call, crops them and
    /// converts them into the caller's output.
    fn process_rows(&mut self, pix: &mut [u32], io: &mut Io<'_>, row: i32) {
        let rows_idx = (self.width * self.last_row) as usize;
        let num_rows = row - self.last_row;
        if num_rows > 0 {
            apply_inverse_transforms(self, pix, self.last_row, num_rows, rows_idx);
            let mut in_off = self.argb_cache;
            if set_crop_window(io, self.last_row, row, &mut in_off) {
                if io.use_scaling {
                    // Rescaling is not ported yet (see the crate documentation).
                    set_error(self, Status::UnsupportedFeature);
                } else if io::is_rgb_mode(io.colorspace) {
                    // EmitRows: one converted row per input row.
                    let in_stride = io.width as usize;
                    let mb_w = io.mb_w as usize;
                    let mb_h = io.mb_h as usize;
                    let out_stride = io.out_stride;
                    let colorspace = io.colorspace;
                    for i in 0..mb_h {
                        let src = &pix[in_off + i * in_stride..in_off + i * in_stride + mb_w];
                        let out_row = (self.last_out_row as usize + i) * out_stride;
                        lossless::convert_from_bgra(src, colorspace, &mut io.out[out_row..]);
                    }
                    // Update 'last_out_row'.
                    self.last_out_row += mb_h as i32;
                } else {
                    // YUV output is not ported; Skia never asks for it.
                    set_error(self, Status::UnsupportedFeature);
                }
            }
        }
        // Update 'last_row'.
        self.last_row = row;
    }

    /// Port of `ExtractAlphaRows`: stores the green plane of the decoded rows as alpha.
    fn extract_alpha_rows(&mut self, pix: &mut [u32], alph: &mut AlphaOutput, last_row: i32) {
        let mut cur_row = self.last_row;
        let mut num_rows = last_row - cur_row;
        let mut in_idx = (self.width * cur_row) as usize;
        while num_rows > 0 {
            let num_rows_to_process = num_rows.min(NUM_ARGB_CACHE_ROWS);
            let width = alph.width as usize;
            let cache_pixs = width * num_rows_to_process as usize;
            let dst = width * cur_row as usize;
            apply_inverse_transforms(self, pix, cur_row, num_rows_to_process, in_idx);
            for i in 0..cache_pixs {
                // WebPExtractGreen: the alpha value is stored in the green plane.
                alph.output[dst + i] = ((pix[self.argb_cache + i] >> 8) & 0xff) as u8;
            }
            crate::alpha::apply_filter(alph, cur_row, cur_row + num_rows_to_process, dst);
            num_rows -= num_rows_to_process;
            in_idx += (num_rows_to_process * self.width) as usize;
            cur_row += num_rows_to_process;
        }
        self.last_row = last_row;
        self.last_out_row = last_row;
    }

    /// Port of `ExtractPalettedAlphaRows`: the 8-bit alpha path, where the alpha plane is colour
    /// indexed and only the cropped rows are expanded.
    fn extract_paletted_alpha_rows(&mut self, pix8: &[u8], alph: &mut AlphaOutput, last_row: i32) {
        let top_row = if alph.filter == crate::alpha::Filter::None
            || alph.filter == crate::alpha::Filter::Horizontal
        {
            alph.crop_top
        } else {
            self.last_row
        };
        let first_row = if self.last_row < top_row {
            top_row
        } else {
            self.last_row
        };
        if last_row > first_row {
            // Special method for paletted alpha data. We only process the cropped area.
            let width = alph.width as usize;
            let dst = width * first_row as usize;
            let src = self.width as usize * first_row as usize;
            let transform = &self.transforms[0];
            let out_rows = (last_row - first_row) as usize;
            let mut out_tmp = vec![0u8; width * out_rows];
            lossless::color_index_inverse_transform_alpha(
                transform,
                first_row,
                last_row,
                &pix8[src..],
                &mut out_tmp,
            );
            alph.output[dst..dst + width * out_rows].copy_from_slice(&out_tmp);
            crate::alpha::apply_filter(alph, first_row, last_row, dst);
        }
        self.last_row = last_row;
        self.last_out_row = last_row;
    }

    /// Port of `DecodeAlphaData`: the 8-bit alpha decoder (colour-indexed alpha, no colour cache).
    fn decode_alpha_data(
        &mut self,
        data: &[u8],
        pix8: &mut [u8],
        width: i32,
        height: i32,
        last_row: i32,
        alph: &mut AlphaOutput,
    ) -> bool {
        let mut ok = true;
        let mut row = self.last_pixel / width;
        let mut col = self.last_pixel % width;
        let mut pos = self.last_pixel;
        let end = width * height;
        let last = width * last_row;
        let len_code_limit = NUM_LITERAL_CODES + NUM_LENGTH_CODES;
        let mask = self.hdr.huffman_mask;
        let mut group_idx = if pos < last {
            group_index_for_pos(&self.hdr, col, row)
        } else {
            0
        };
        while !self.br.eos() && pos < last {
            // Only update when changing tile.
            if (col & mask) == 0 {
                group_idx = group_index_for_pos(&self.hdr, col, row);
            }
            self.br.fill_bit_window(data);
            let group = &self.hdr.htree_groups[group_idx];
            let code = read_symbol(
                &self.hdr.huffman_tables,
                group.htrees[GREEN],
                &mut self.br,
                data,
            );
            if code < NUM_LITERAL_CODES {
                // Literal
                pix8[pos as usize] = code as u8;
                pos += 1;
                col += 1;
                if col >= width {
                    col = 0;
                    row += 1;
                    if row <= last_row && (row % NUM_ARGB_CACHE_ROWS == 0) {
                        self.extract_paletted_alpha_rows(pix8, alph, row);
                    }
                }
            } else if code < len_code_limit {
                // Backward reference
                let length_sym = code - NUM_LITERAL_CODES;
                let length = get_copy_distance(length_sym, &mut self.br, data);
                let group = &self.hdr.htree_groups[group_idx];
                let dist_symbol = read_symbol(
                    &self.hdr.huffman_tables,
                    group.htrees[DIST],
                    &mut self.br,
                    data,
                );
                self.br.fill_bit_window(data);
                let dist_code = get_copy_distance(dist_symbol, &mut self.br, data);
                let dist = plane_code_to_distance(width, dist_code);
                if pos >= dist && end - pos >= length {
                    copy_block(pix8, pos as usize, dist as usize, length as usize);
                } else {
                    ok = false;
                    break;
                }
                pos += length;
                col += length;
                while col >= width {
                    col -= width;
                    row += 1;
                    if row <= last_row && (row % NUM_ARGB_CACHE_ROWS == 0) {
                        self.extract_paletted_alpha_rows(pix8, alph, row);
                    }
                }
                if pos < last && (col & mask) != 0 {
                    group_idx = group_index_for_pos(&self.hdr, col, row);
                }
            } else {
                // Not reached
                ok = false;
                break;
            }
            let eos = self.br.is_end_of_stream(data);
            self.br.set_eos(eos);
        }
        if ok {
            // Process the remaining rows corresponding to last row-block.
            let r = if row > last_row { last_row } else { row };
            self.extract_paletted_alpha_rows(pix8, alph, r);
        }
        let eos = self.br.is_end_of_stream(data);
        self.br.set_eos(eos);
        if !ok || (eos && pos < end) {
            let status = if eos {
                Status::Suspended
            } else {
                Status::BitstreamError
            };
            return set_error(self, status);
        }
        self.last_pixel = pos;
        ok
    }

    /// Calls the row processing selected by `sink` (`process_func(dec, row)` in C).
    fn process_rows_hook(&mut self, pix: &mut [u32], sink: &mut Sink<'_, '_>, row: i32) {
        match sink {
            Sink::None => {}
            Sink::Argb(io) => self.process_rows(pix, io, row),
            Sink::Alpha(out) => self.extract_alpha_rows(pix, out, row),
        }
    }
}

/// Port of `VP8LDecodeHeader`: reads the header of a VP8L payload and sets `io.width`/`io.height`.
pub(crate) fn decode_header(dec: &mut Vp8lDecoder, data: &[u8], io: &mut Io<'_>) -> bool {
    dec.status = Status::Ok;
    dec.br = VP8LBitReader::new(data);
    let Some((width, height, _has_alpha)) = read_image_info(&mut dec.br, data) else {
        set_error(dec, Status::BitstreamError);
        return false;
    };
    dec.state = DecodeState::ReadDim;
    io.width = width;
    io.height = height;
    decode_image_stream(dec, data, width, height, true).is_some()
}

/// Port of `VP8LDecodeImage`: decodes the image rows into `io` (incremental when `dec.incremental`).
pub(crate) fn decode_image(dec: &mut Vp8lDecoder, data: &[u8], io: &mut Io<'_>) -> bool {
    if dec.state != DecodeState::ReadData {
        // AllocateInternalBuffers32b(dec, io->width)
        let num_pixels = (dec.width * dec.height) as usize;
        let final_width = io.width as usize;
        let cache_top_pixels = final_width;
        let cache_pixels = final_width * NUM_ARGB_CACHE_ROWS as usize;
        dec.pixels = vec![0u32; num_pixels + cache_top_pixels + cache_pixels];
        dec.argb_cache = num_pixels + cache_top_pixels;
        if dec.incremental
            && dec.hdr.color_cache_size > 0
            && dec.hdr.saved_color_cache.colors.is_empty()
        {
            dec.hdr.saved_color_cache = ColorCache::new(dec.hdr.color_cache.hash_bits);
        }
        dec.state = DecodeState::ReadData;
    }
    let mut pix = std::mem::take(&mut dec.pixels);
    let (w, h, crop_bottom) = (dec.width, dec.height, io.crop_bottom);
    let ok = dec.decode_image_data(data, &mut pix, w, h, crop_bottom, &mut Sink::Argb(io));
    dec.pixels = pix;
    if !ok {
        return false;
    }
    io.last_y = dec.last_out_row;
    true
}

/// Port of `VP8LDecodeAlphaHeader`. Returns the decoder and whether the 8-bit path is used.
pub(crate) fn alpha_header(data: &[u8], width: i32, height: i32) -> Option<(Vp8lDecoder, bool)> {
    let mut dec = Vp8lDecoder::new();
    dec.width = width;
    dec.height = height;
    dec.status = Status::Ok;
    dec.br = VP8LBitReader::new(data);
    decode_image_stream(&mut dec, data, width, height, true)?;
    // Special case: if alpha data uses only the colour indexing transform and no colour cache,
    // the 8-bit path is used.
    let use_8b = dec.next_transform == 1
        && dec.transforms[0].ttype == Some(TransformType::ColorIndexingTransform)
        && is_8b_optimizable(&dec.hdr, &dec.hdr.huffman_tables);
    if use_8b {
        dec.pixels8 = vec![0u8; (dec.width * dec.height) as usize];
    } else {
        let num_pixels = (dec.width * dec.height) as usize;
        let final_width = width as usize;
        dec.pixels =
            vec![0u32; num_pixels + final_width + final_width * NUM_ARGB_CACHE_ROWS as usize];
        dec.argb_cache = num_pixels + final_width;
    }
    Some((dec, use_8b))
}

/// Port of `Is8bOptimizable`.
fn is_8b_optimizable(hdr: &Metadata, arena: &[HuffmanCode]) -> bool {
    if hdr.color_cache_size > 0 {
        return false;
    }
    // When the Huffman tree contains only one symbol, the red/blue/alpha reads are skipped.
    for group in &hdr.htree_groups {
        if arena[group.htrees[RED]].bits > 0
            || arena[group.htrees[BLUE]].bits > 0
            || arena[group.htrees[ALPHA]].bits > 0
        {
            return false;
        }
    }
    true
}

/// Port of `VP8LDecodeAlphaImageStream`: decodes alpha rows up to `last_row` into `alph`.
pub(crate) fn alpha_image_stream(
    dec: &mut Vp8lDecoder,
    data: &[u8],
    use_8b: bool,
    alph: &mut AlphaOutput,
    last_row: i32,
) -> bool {
    if dec.last_row >= last_row {
        return true; // done
    }
    let (w, h) = (dec.width, dec.height);
    if use_8b {
        let mut pix8 = std::mem::take(&mut dec.pixels8);
        let ok = dec.decode_alpha_data(data, &mut pix8, w, h, last_row, alph);
        dec.pixels8 = pix8;
        ok
    } else {
        let mut pix = std::mem::take(&mut dec.pixels);
        let ok = dec.decode_image_data(data, &mut pix, w, h, last_row, &mut Sink::Alpha(alph));
        dec.pixels = pix;
        ok
    }
}

/// Port of `CopyBlock32b`/`CopyBlock8b`: copies `length` elements from `dist` back, forward, so
/// overlapping copies repeat the pattern exactly as the C code does.
#[inline]
fn copy_block<T: Copy>(pix: &mut [T], src: usize, dist: usize, length: usize) {
    let dst = src;
    for i in 0..length {
        pix[dst + i] = pix[dst + i - dist];
    }
}
