// Copyright 2014 Google Inc. All Rights Reserved.
// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Port of libwebp `src/utils/huffman_encode_utils.{c,h}`: optimal length-limited Huffman trees,
//! the run-length tokenisation of code lengths, and canonical code assignment.

/// Port of `HuffmanTreeToken`: a code-length symbol (0..=15, or the escapes 16, 17, 18) and its
/// extra bits.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HuffmanTreeToken {
    pub code: u8,
    pub extra_bits: u8,
}

/// Port of `HuffmanTreeCode`: the code lengths and the (bit-reversed) codes of one alphabet.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HuffmanTreeCode {
    pub code_lengths: Vec<u8>,
    pub codes: Vec<u16>,
}

impl HuffmanTreeCode {
    /// A code over `num_symbols` symbols, all lengths and codes zero.
    #[must_use]
    pub fn new(num_symbols: usize) -> Self {
        Self { code_lengths: vec![0; num_symbols], codes: vec![0; num_symbols] }
    }

    /// Port of `tree->num_symbols`.
    #[must_use]
    pub fn num_symbols(&self) -> usize {
        self.code_lengths.len()
    }
}

/// Port of `MAX_ALLOWED_CODE_LENGTH`.
pub const MAX_ALLOWED_CODE_LENGTH: usize = 15;

/// Port of `HuffmanTree`: a node of the tree being built. `pool_left`/`pool_right` index the
/// pool of internal nodes (`-1` for a leaf).
#[derive(Clone, Copy, Debug, Default)]
struct HuffmanTree {
    total_count: u32,
    value: i32,
    pool_left: i32,
    pool_right: i32,
}

/// Port of `ValuesShouldBeCollapsedToStrideAverage`.
fn values_should_be_collapsed_to_stride_average(a: i32, b: i32) -> bool {
    (a - b).abs() < 4
}

/// Port of `OptimizeHuffmanForRle`: marks runs in `good_for_rle` and rewrites `counts` with
/// their averages, so the tree is shaped for run-length coded code lengths.
fn optimize_huffman_for_rle(length: usize, good_for_rle: &mut [u8], counts: &mut [u32]) {
    // C: `for (; length >= 0; --length)`. The loop ends with `length` equal to the index one past
    // the last non-zero count, or returns when there are none.
    let mut length = length;
    loop {
        if length == 0 {
            return; // All zeros.
        }
        if counts[length - 1] != 0 {
            break;
        }
        length -= 1;
    }
    {
        let mut symbol = counts[0];
        let mut stride: usize = 0;
        for i in 0..length + 1 {
            if i == length || counts[i] != symbol {
                if (symbol == 0 && stride >= 5) || (symbol != 0 && stride >= 7) {
                    for k in 0..stride {
                        good_for_rle[i - k - 1] = 1;
                    }
                }
                stride = 1;
                if i != length {
                    symbol = counts[i];
                }
            } else {
                stride += 1;
            }
        }
    }
    {
        let mut stride: u32 = 0;
        let mut limit: u32 = counts[0];
        let mut sum: u32 = 0;
        for i in 0..length + 1 {
            if i == length
                || good_for_rle[i] != 0
                || (i != 0 && good_for_rle[i - 1] != 0)
                || !values_should_be_collapsed_to_stride_average(counts[i] as i32, limit as i32)
            {
                if stride >= 4 || (stride >= 3 && sum == 0) {
                    let mut count = (sum + stride / 2) / stride;
                    if count < 1 {
                        count = 1;
                    }
                    if sum == 0 {
                        count = 0;
                    }
                    for k in 0..stride as usize {
                        counts[i - k - 1] = count;
                    }
                }
                stride = 0;
                sum = 0;
                if i + 3 < length {
                    limit = (counts[i] + counts[i + 1] + counts[i + 2] + counts[i + 3] + 2) / 4;
                } else if i < length {
                    limit = counts[i];
                } else {
                    limit = 0;
                }
            }
            stride += 1;
            if i != length {
                sum += counts[i];
                if stride >= 4 {
                    limit = (sum + stride / 2) / stride;
                }
            }
        }
    }
}

/// Port of `SetBitDepths`: the depth of each leaf of the tree rooted at `tree`.
fn set_bit_depths(tree: &HuffmanTree, pool: &[HuffmanTree], bit_depths: &mut [u8], level: u8) {
    if tree.pool_left >= 0 {
        set_bit_depths(&pool[tree.pool_left as usize], pool, bit_depths, level + 1);
        set_bit_depths(&pool[tree.pool_right as usize], pool, bit_depths, level + 1);
    } else {
        bit_depths[tree.value as usize] = level;
    }
}

/// Port of `GenerateOptimalTree`: writes the code length of each used symbol of `histogram` into
/// `bit_depths`, with no code longer than `tree_depth_limit`.
fn generate_optimal_tree(
    histogram: &[u32],
    tree_depth_limit: u32,
    bit_depths: &mut [u8],
) {
    let tree_size_orig = histogram.iter().filter(|&&h| h != 0).count();
    if tree_size_orig == 0 {
        return; // pretty optimal already!
    }
    let mut count_min: u32 = 1;
    loop {
        // The leaves, sorted by count (descending) then value (ascending): the order of
        // `CompareHuffmanTrees`, a total order because the leaf values are distinct.
        let mut tree: Vec<HuffmanTree> = Vec::with_capacity(tree_size_orig);
        for (j, &h) in histogram.iter().enumerate() {
            if h != 0 {
                let count = if h < count_min { count_min } else { h };
                tree.push(HuffmanTree { total_count: count, value: j as i32, pool_left: -1, pool_right: -1 });
            }
        }
        tree.sort_by(|a, b| b.total_count.cmp(&a.total_count).then(a.value.cmp(&b.value)));
        let mut pool: Vec<HuffmanTree> = Vec::new();
        if tree.len() > 1 {
            // Normal case.
            while tree.len() > 1 {
                // Finish when we have only one root.
                let a = tree.pop().unwrap_or_default(); // tree[tree_size - 1]
                let b = tree.pop().unwrap_or_default(); // tree[tree_size - 2]
                pool.push(a);
                pool.push(b);
                let tps = pool.len() as i32;
                let count = a.total_count.wrapping_add(b.total_count);
                let k = tree.iter().position(|t| t.total_count <= count).unwrap_or(tree.len());
                tree.insert(
                    k,
                    HuffmanTree {
                        total_count: count,
                        value: -1,
                        pool_left: tps - 1,
                        pool_right: tps - 2,
                    },
                );
            }
            set_bit_depths(&tree[0], &pool, bit_depths, 0);
        } else if tree.len() == 1 {
            // Trivial case: only one element.
            bit_depths[tree[0].value as usize] = 1;
        }
        let mut max_depth = bit_depths[0];
        for &d in &bit_depths[1..histogram.len()] {
            if max_depth < d {
                max_depth = d;
            }
        }
        if u32::from(max_depth) <= tree_depth_limit {
            break;
        }
        count_min *= 2;
    }
}

/// Port of `CodeRepeatedValues`.
fn code_repeated_values(
    mut repetitions: usize,
    tokens: &mut Vec<HuffmanTreeToken>,
    value: u8,
    prev_value: u8,
) {
    if value != prev_value {
        tokens.push(HuffmanTreeToken { code: value, extra_bits: 0 });
        repetitions -= 1;
    }
    while repetitions >= 1 {
        if repetitions < 3 {
            for _ in 0..repetitions {
                tokens.push(HuffmanTreeToken { code: value, extra_bits: 0 });
            }
            break;
        } else if repetitions < 7 {
            tokens.push(HuffmanTreeToken { code: 16, extra_bits: (repetitions - 3) as u8 });
            break;
        } else {
            tokens.push(HuffmanTreeToken { code: 16, extra_bits: 3 });
            repetitions -= 6;
        }
    }
}

/// Port of `CodeRepeatedZeros`.
fn code_repeated_zeros(mut repetitions: usize, tokens: &mut Vec<HuffmanTreeToken>) {
    while repetitions >= 1 {
        if repetitions < 3 {
            for _ in 0..repetitions {
                tokens.push(HuffmanTreeToken { code: 0, extra_bits: 0 });
            }
            break;
        } else if repetitions < 11 {
            tokens.push(HuffmanTreeToken { code: 17, extra_bits: (repetitions - 3) as u8 });
            break;
        } else if repetitions < 139 {
            tokens.push(HuffmanTreeToken { code: 18, extra_bits: (repetitions - 11) as u8 });
            break;
        } else {
            tokens.push(HuffmanTreeToken { code: 18, extra_bits: 0x7f }); // 138 repeated 0s
            repetitions -= 138;
        }
    }
}

/// Port of `VP8LCreateCompressedHuffmanTree`: run-length tokens for the code lengths of `tree`.
#[must_use]
pub fn create_compressed_huffman_tree(tree: &HuffmanTreeCode) -> Vec<HuffmanTreeToken> {
    let depth_size = tree.num_symbols();
    let mut tokens = Vec::new();
    let mut prev_value: u8 = 8; // 8 is the initial value for rle.
    let mut i = 0;
    while i < depth_size {
        let value = tree.code_lengths[i];
        let mut k = i + 1;
        while k < depth_size && tree.code_lengths[k] == value {
            k += 1;
        }
        let runs = k - i;
        if value == 0 {
            code_repeated_zeros(runs, &mut tokens);
        } else {
            code_repeated_values(runs, &mut tokens, value, prev_value);
            prev_value = value;
        }
        i += runs;
    }
    tokens
}

const K_REVERSED_BITS: [u8; 16] = [
    0x0, 0x8, 0x4, 0xc, 0x2, 0xa, 0x6, 0xe, 0x1, 0x9, 0x5, 0xd, 0x3, 0xb, 0x7, 0xf,
];

/// Port of `ReverseBits`.
fn reverse_bits(num_bits: usize, bits: u32) -> u32 {
    let mut retval: u32 = 0;
    let mut bits = bits;
    let mut i: usize = 0;
    while i < num_bits {
        i += 4;
        retval |= u32::from(K_REVERSED_BITS[(bits & 0xf) as usize])
            << (MAX_ALLOWED_CODE_LENGTH + 1 - i);
        bits >>= 4;
    }
    retval >>= MAX_ALLOWED_CODE_LENGTH + 1 - num_bits;
    retval
}

/// Port of `ConvertBitDepthsToSymbols`: canonical codes from the code lengths.
fn convert_bit_depths_to_symbols(tree: &mut HuffmanTreeCode) {
    let len = tree.num_symbols();
    let mut depth_count = [0usize; MAX_ALLOWED_CODE_LENGTH + 1];
    for &code_length in &tree.code_lengths[..len] {
        depth_count[code_length as usize] += 1;
    }
    depth_count[0] = 0; // ignore unused symbol
    let mut next_code = [0u32; MAX_ALLOWED_CODE_LENGTH + 1];
    let mut code: u32 = 0;
    for i in 1..=MAX_ALLOWED_CODE_LENGTH {
        code = (code + depth_count[i - 1] as u32) << 1;
        next_code[i] = code;
    }
    for i in 0..len {
        let code_length = tree.code_lengths[i] as usize;
        tree.codes[i] = reverse_bits(code_length, next_code[code_length]) as u16;
        next_code[code_length] += 1;
    }
}

/// Port of `VP8LCreateHuffmanTree`: builds the code lengths and codes of `huff_code` for
/// `histogram` (which is modified, as in C).
pub fn create_huffman_tree(
    histogram: &mut [u32],
    tree_depth_limit: u32,
    huff_code: &mut HuffmanTreeCode,
) {
    let num_symbols = huff_code.num_symbols();
    let mut buf_rle = vec![0u8; num_symbols];
    optimize_huffman_for_rle(num_symbols, &mut buf_rle, histogram);
    generate_optimal_tree(
        histogram,
        tree_depth_limit,
        &mut huff_code.code_lengths,
    );
    convert_bit_depths_to_symbols(huff_code);
}
