// Copyright 2011 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of libwebp `src/enc/backward_references_enc.{c,h}`: the hash chain of match candidates,
//! the LZ77 parse of it, and the 2D-locality rewrite of distances.
//!
//! The colour cache is never used on the paths `SkWebpEncoder` reaches (lossless `method 0`
//! picks `cache_bits == 0`), so the parses here are the colour-cache-free ones. The box parse and
//! the colour-cache search belong to the `method > 0` searches and are not ported.
//!
//! The C references are stored in blocks; the blocks only affect allocation, not the sequence
//! that the cursor walks, so a flat vector holds the same content.

// Module-level clippy allows. The C arithmetic mixes int, uint32_t, size_t and float, and the
// casts below are the width and sign conversions of the C source. The index loops, `if`/`else`
// chains and exact float comparisons keep the C control flow and evaluation order, so that the
// code can be read against the C source; they are not simplified.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::cast_precision_loss,
    clippy::too_many_lines,
    clippy::similar_names,
    clippy::many_single_char_names,
    clippy::unreadable_literal,
    clippy::needless_range_loop,
    clippy::float_cmp,
    clippy::manual_midpoint,
    clippy::redundant_else,
    clippy::single_match,
    clippy::items_after_statements,
    clippy::let_and_return,
    clippy::needless_for_each,
    clippy::while_let_loop,
    clippy::approx_constant,
    clippy::too_many_arguments,
    clippy::match_same_arms,
    clippy::if_not_else,
    clippy::needless_pass_by_value,
    clippy::explicit_iter_loop,
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::manual_range_contains
)]

/// Port of `MAX_LENGTH`.
pub const MAX_LENGTH: u32 = (1 << MAX_LENGTH_BITS) - 1;
/// Port of `MAX_LENGTH_BITS`.
pub const MAX_LENGTH_BITS: u32 = 12;
/// Port of `HASH_BITS`.
const HASH_BITS: u32 = 18;
/// Port of `HASH_SIZE`.
const HASH_SIZE: usize = 1 << HASH_BITS;
/// Port of `WINDOW_SIZE_BITS`.
const WINDOW_SIZE_BITS: u32 = 20;
/// Port of `WINDOW_SIZE` (`(1 << WINDOW_SIZE_BITS) - 120`).
const WINDOW_SIZE: i32 = (1 << WINDOW_SIZE_BITS) - 120;
/// Port of `MIN_LENGTH`.
pub const MIN_LENGTH: i32 = 4;
/// Port of `kHashMultiplierHi`.
const HASH_MULTIPLIER_HI: u32 = 0xc6a4_a793;
/// Port of `kHashMultiplierLo`.
const HASH_MULTIPLIER_LO: u32 = 0x5bd1_e996;

/// Port of `plane_to_code_lut`.
const PLANE_TO_CODE_LUT: [u8; 128] = [
    96, 73, 55, 39, 23, 13, 5, 1, 255, 255, 255, 255, 255, 255, 255, 255, //
    101, 78, 58, 42, 26, 16, 8, 2, 0, 3, 9, 17, 27, 43, 59, 79, //
    102, 86, 62, 46, 32, 20, 10, 6, 4, 7, 11, 21, 33, 47, 63, 87, //
    105, 90, 70, 52, 37, 28, 18, 14, 12, 15, 19, 29, 38, 53, 71, 91, //
    110, 99, 82, 66, 48, 35, 30, 24, 22, 25, 31, 36, 49, 67, 83, 100, //
    115, 108, 94, 76, 64, 50, 44, 40, 34, 41, 45, 51, 65, 77, 95, 109, //
    118, 113, 103, 92, 80, 68, 60, 56, 54, 57, 61, 69, 81, 93, 104, 114, //
    119, 116, 111, 106, 97, 88, 84, 74, 72, 75, 85, 89, 98, 107, 112, 117,
];

/// Port of `PixOrCopyMode` (the colour-cache mode is not produced by this port).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PixOrCopyMode {
    Literal,
    CacheIdx,
    Copy,
}

/// Port of `PixOrCopy`: a literal pixel, a colour-cache index or a copy of `len` pixels from
/// `distance` back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PixOrCopy {
    pub mode: PixOrCopyMode,
    pub len: u16,
    pub argb_or_distance: u32,
}

impl PixOrCopy {
    /// Port of `PixOrCopyCreateCopy`.
    #[must_use]
    pub fn copy(distance: u32, len: u32) -> Self {
        Self {
            mode: PixOrCopyMode::Copy,
            len: len as u16,
            argb_or_distance: distance,
        }
    }

    /// Port of `PixOrCopyCreateLiteral`.
    #[must_use]
    pub fn literal(argb: u32) -> Self {
        Self {
            mode: PixOrCopyMode::Literal,
            len: 1,
            argb_or_distance: argb,
        }
    }

    /// Port of `PixOrCopyCreateCacheIdx`: a colour-cache index of length 1.
    #[must_use]
    pub fn cache_idx(idx: u32) -> Self {
        Self {
            mode: PixOrCopyMode::CacheIdx,
            len: 1,
            argb_or_distance: idx,
        }
    }

    /// Port of `PixOrCopyLiteral(p, component)`: component 0 is blue, 1 green, 2 red, 3 alpha.
    #[must_use]
    pub fn literal_component(&self, component: u32) -> u32 {
        (self.argb_or_distance >> (component * 8)) & 0xff
    }

    /// Port of `PixOrCopyLength`.
    #[must_use]
    pub fn length(&self) -> u32 {
        u32::from(self.len)
    }

    /// Port of `PixOrCopyIsLiteral`.
    #[must_use]
    pub fn is_literal(&self) -> bool {
        self.mode == PixOrCopyMode::Literal
    }

    /// Port of `PixOrCopyIsCopy`.
    #[must_use]
    pub fn is_copy(&self) -> bool {
        self.mode == PixOrCopyMode::Copy
    }
}

/// Port of `VP8LBackwardRefs`: the references of one image, in order.
#[derive(Clone, Debug, Default)]
pub struct BackwardRefs {
    pub refs: Vec<PixOrCopy>,
}

impl BackwardRefs {
    /// Port of `VP8LClearBackwardRefs`.
    pub fn clear(&mut self) {
        self.refs.clear();
    }

    /// Port of `VP8LBackwardRefsCursorAdd`.
    pub fn push(&mut self, v: PixOrCopy) {
        self.refs.push(v);
    }
}

/// Port of `VP8LHashChain`: for each position, the best match as `(offset << 12) | length`.
#[derive(Clone, Debug, Default)]
pub struct HashChain {
    pub offset_length: Vec<u32>,
}

impl HashChain {
    /// Port of `VP8LHashChainInit`.
    #[must_use]
    pub fn new(size: usize) -> Self {
        Self {
            offset_length: vec![0; size],
        }
    }

    /// Port of `VP8LHashChainFindOffset`.
    #[must_use]
    pub fn find_offset(&self, base_position: usize) -> i32 {
        (self.offset_length[base_position] >> MAX_LENGTH_BITS) as i32
    }

    /// Port of `VP8LHashChainFindLength`.
    #[must_use]
    pub fn find_length(&self, base_position: usize) -> i32 {
        (self.offset_length[base_position] & ((1u32 << MAX_LENGTH_BITS) - 1)) as i32
    }

    /// Port of `VP8LHashChainFindCopy`.
    #[must_use]
    pub fn find_copy(&self, base_position: usize) -> (i32, i32) {
        (
            self.find_offset(base_position),
            self.find_length(base_position),
        )
    }
}

/// Port of `GetPixPairHash64`.
fn get_pix_pair_hash64(argb: &[u32]) -> u32 {
    let mut key = argb[1].wrapping_mul(HASH_MULTIPLIER_HI);
    key = key.wrapping_add(argb[0].wrapping_mul(HASH_MULTIPLIER_LO));
    key >> (32 - HASH_BITS)
}

/// Port of `GetMaxItersForQuality`.
fn get_max_iters_for_quality(quality: i32) -> i32 {
    8 + (quality * quality) / 128
}

/// Port of `GetWindowSizeForHashChain`.
fn get_window_size_for_hash_chain(quality: i32, xsize: i32) -> i32 {
    let max_window_size = if quality > 75 {
        WINDOW_SIZE
    } else if quality > 50 {
        xsize << 8
    } else if quality > 25 {
        xsize << 6
    } else {
        xsize << 4
    };
    if max_window_size > WINDOW_SIZE {
        WINDOW_SIZE
    } else {
        max_window_size
    }
}

/// Port of `MaxFindCopyLength`.
pub(super) fn max_find_copy_length(len: usize) -> usize {
    if len < MAX_LENGTH as usize {
        len
    } else {
        MAX_LENGTH as usize
    }
}

/// Port of `VP8LVectorMismatch` (`VectorMismatch_C`): the length of the common prefix, at most
/// `length`.
fn vector_mismatch(array1: &[u32], array2: &[u32], length: usize) -> usize {
    let mut match_len = 0;
    while match_len < length && array1[match_len] == array2[match_len] {
        match_len += 1;
    }
    match_len
}

/// Port of `FindMatchLength`.
pub(super) fn find_match_length(
    array1: &[u32],
    array2: &[u32],
    best_len_match: usize,
    max_limit: usize,
) -> usize {
    if array1[best_len_match] != array2[best_len_match] {
        return 0;
    }
    vector_mismatch(array1, array2, max_limit)
}

/// Port of `VP8LHashChainFill`: for every position, the longest match found by walking the hash
/// chain of equal pixel pairs (`iter_max` candidates, within the quality's window).
///
/// `argb` is `xsize * ysize` pixels, row-major with stride `xsize`. The chain must hold at least
/// that many entries.
pub fn hash_chain_fill(
    p: &mut HashChain,
    quality: i32,
    argb: &[u32],
    xsize: usize,
    ysize: usize,
    low_effort: bool,
) {
    let size = xsize * ysize;
    let iter_max = get_max_iters_for_quality(quality);
    let window_size = get_window_size_for_hash_chain(quality, xsize as i32);
    if size <= 2 {
        p.offset_length[0] = 0;
        p.offset_length[size - 1] = 0;
        return;
    }
    // The C code keeps the chain (int32) in the offset_length memory; read and write it through
    // the same storage here.
    let mut hash_to_first_index = vec![-1i32; HASH_SIZE];
    let mut argb_comp = argb[0] == argb[1];
    let mut pos: usize = 0;
    while pos < size - 2 {
        let argb_comp_next = argb[pos + 1] == argb[pos + 2];
        if argb_comp && argb_comp_next {
            let mut tmp = [0u32; 2];
            let mut len: u32 = 1;
            tmp[0] = argb[pos];
            while pos + len as usize + 2 < size && argb[pos + len as usize + 2] == argb[pos] {
                len += 1;
            }
            if len > MAX_LENGTH {
                for k in 0..(len - MAX_LENGTH) as usize {
                    p.offset_length[pos + k] = u32::MAX;
                }
                pos += (len - MAX_LENGTH) as usize;
                len = MAX_LENGTH;
            }
            while len != 0 {
                tmp[1] = len;
                len -= 1;
                let hash_code = get_pix_pair_hash64(&tmp) as usize;
                p.offset_length[pos] = hash_to_first_index[hash_code] as u32;
                hash_to_first_index[hash_code] = pos as i32;
                pos += 1;
            }
            argb_comp = false;
        } else {
            let hash_code = get_pix_pair_hash64(&argb[pos..]) as usize;
            p.offset_length[pos] = hash_to_first_index[hash_code] as u32;
            hash_to_first_index[hash_code] = pos as i32;
            pos += 1;
            argb_comp = argb_comp_next;
        }
    }
    let hash_code = get_pix_pair_hash64(&argb[pos..]) as usize;
    p.offset_length[pos] = hash_to_first_index[hash_code] as u32;

    p.offset_length[0] = 0;
    p.offset_length[size - 1] = 0;
    let mut base_position = size - 2;
    while base_position > 0 {
        let max_len = max_find_copy_length(size - 1 - base_position);
        let argb_start = base_position;
        let mut iter = iter_max;
        let mut best_length: usize = 0;
        let mut best_distance: u32 = 0;
        let min_pos: i64 = if base_position as i64 > i64::from(window_size) {
            base_position as i64 - i64::from(window_size)
        } else {
            0
        };
        let length_max = if max_len < 256 { max_len } else { 256 };
        let mut pos_i: i64 = p.offset_length[base_position] as i32 as i64;
        if !low_effort {
            if base_position >= xsize {
                let curr_length = find_match_length(
                    &argb[argb_start - xsize..],
                    &argb[argb_start..],
                    best_length,
                    max_len,
                );
                if curr_length > best_length {
                    best_length = curr_length;
                    best_distance = xsize as u32;
                }
                iter -= 1;
            }
            let curr_length = find_match_length(
                &argb[argb_start - 1..],
                &argb[argb_start..],
                best_length,
                max_len,
            );
            if curr_length > best_length {
                best_length = curr_length;
                best_distance = 1;
            }
            iter -= 1;
            if best_length == MAX_LENGTH as usize {
                pos_i = min_pos - 1;
            }
        }
        let mut best_argb = argb[argb_start + best_length];
        loop {
            if !(pos_i >= min_pos && {
                iter -= 1;
                iter != 0
            }) {
                break;
            }
            let pos = pos_i as usize;
            if argb[pos + best_length] == best_argb {
                let curr_length = vector_mismatch(&argb[pos..], &argb[argb_start..], max_len);
                if best_length < curr_length {
                    best_length = curr_length;
                    best_distance = (base_position - pos) as u32;
                    best_argb = argb[argb_start + best_length];
                    if best_length >= length_max {
                        break;
                    }
                }
            }
            pos_i = i64::from(p.offset_length[pos] as i32);
        }
        let mut max_base_position = base_position;
        loop {
            p.offset_length[base_position] =
                (best_distance << MAX_LENGTH_BITS) | (best_length as u32);
            base_position -= 1;
            if best_distance == 0 || base_position == 0 {
                break;
            }
            if base_position < best_distance as usize
                || argb[base_position - best_distance as usize] != argb[base_position]
            {
                break;
            }
            if best_length == MAX_LENGTH as usize
                && best_distance != 1
                && (base_position + MAX_LENGTH as usize) < max_base_position
            {
                break;
            }
            if best_length < MAX_LENGTH as usize {
                best_length += 1;
                max_base_position = base_position;
            }
        }
    }
}

/// Port of `BackwardReferencesLz77` with no colour cache: the greedy parse that takes the
/// hash-chain match at each position, and the lazily extended match when a later position
/// reaches further.
#[must_use]
pub fn backward_references_lz77(
    xsize: usize,
    ysize: usize,
    argb: &[u32],
    hash_chain: &HashChain,
) -> BackwardRefs {
    let pix_count = xsize * ysize;
    let mut refs = BackwardRefs::default();
    let mut i_last_check: isize = -1;
    let mut i: usize = 0;
    while i < pix_count {
        let (offset, mut len) = hash_chain.find_copy(i);
        if len >= MIN_LENGTH {
            let len_ini = len;
            let mut max_reach: usize = 0;
            let j_max = if i + len_ini as usize >= pix_count {
                pix_count - 1
            } else {
                i + len_ini as usize
            };
            i_last_check = if i as isize > i_last_check {
                i as isize
            } else {
                i_last_check
            };
            let mut j = (i_last_check + 1) as usize;
            while j <= j_max {
                let len_j = hash_chain.find_length(j);
                let reach = j + if len_j >= MIN_LENGTH {
                    len_j as usize
                } else {
                    1
                };
                if reach > max_reach {
                    len = (j - i) as i32;
                    max_reach = reach;
                    if max_reach >= pix_count {
                        break;
                    }
                }
                j += 1;
            }
        } else {
            len = 1;
        }
        if len == 1 {
            refs.push(PixOrCopy::literal(argb[i]));
        } else {
            refs.push(PixOrCopy::copy(offset as u32, len as u32));
        }
        i += len as usize;
    }
    refs
}

/// Port of `VP8LDistanceToPlaneCode`: maps a distance to the 2D-locality code.
#[must_use]
pub fn distance_to_plane_code(xsize: i32, dist: i32) -> i32 {
    let yoffset = dist / xsize;
    let xoffset = dist - yoffset * xsize;
    if xoffset <= 8 && yoffset < 8 {
        i32::from(PLANE_TO_CODE_LUT[(yoffset * 16 + 8 - xoffset) as usize]) + 1
    } else if xoffset > xsize - 8 && yoffset < 7 {
        i32::from(PLANE_TO_CODE_LUT[((yoffset + 1) * 16 + 8 + (xsize - xoffset)) as usize]) + 1
    } else {
        dist + 120
    }
}

/// Port of `BackwardReferences2DLocality`: rewrites every copy distance to its plane code.
pub(super) fn backward_references_2d_locality(xsize: i32, refs: &mut BackwardRefs) {
    for v in &mut refs.refs {
        if v.is_copy() {
            let dist = v.argb_or_distance as i32;
            v.argb_or_distance = distance_to_plane_code(xsize, dist) as u32;
        }
    }
}

/// Port of `GetBackwardReferencesLowEffort` followed by the copy into `refs[0]` that
/// `VP8LGetBackwardReferences` does for `low_effort`. Returns the references; the colour cache
/// bits are always 0 on this path.
#[must_use]
pub fn get_backward_references_low_effort(
    width: usize,
    height: usize,
    argb: &[u32],
    hash_chain: &HashChain,
) -> BackwardRefs {
    let mut refs = backward_references_lz77(width, height, argb, hash_chain);
    backward_references_2d_locality(width as i32, &mut refs);
    refs
}
