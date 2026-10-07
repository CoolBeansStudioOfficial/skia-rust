// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Every tier of the memset kernels against the scalar twins.

#![allow(
    clippy::many_single_char_names,
    clippy::cast_possible_truncation,
    clippy::too_many_arguments
)] // test code: Skia's names, byte truncation of random words, kernel signatures

use super::*;
use crate::testing::test_selections;

#[test]
fn block_sizes() {
    assert_eq!(block_bytes(Tier::Ml3), 32);
    assert_eq!(block_bytes(Tier::Ml4), 32);
    for t in [Tier::Scalar, Tier::Sse2, Tier::Sse41, Tier::Neon] {
        assert_eq!(block_bytes(t), 16);
    }
}

macro_rules! memset_tests {
    ($t:ty, $name:ident, $with:ident, $scalar:ident, $rect_with:ident, $rect_scalar:ident, $value:expr) => {
        #[test]
        fn $name() {
            let value: $t = $value;
            for sel in test_selections() {
                for count in (0..100).step_by(if cfg!(miri) { 7 } else { 1 }) {
                    for offset in 0..9 {
                        let mut a = vec![1; 120];
                        let mut b = vec![1; 120];
                        $with(sel, &mut a[offset..], value, count);
                        $scalar(&mut b[offset..], value, count);
                        assert_eq!(a, b, "{sel}, count {count}, offset {offset}");
                    }
                }
                for (count, rows, pad) in [(0, 3, 0), (1, 1, 5), (7, 4, 0), (33, 5, 3), (64, 3, 1)]
                {
                    let stride = count + pad;
                    let rb = stride * size_of::<$t>();
                    let mut a = vec![2; stride * rows + 4];
                    let mut b = a.clone();
                    $rect_with(sel, &mut a, value, count, rb, rows);
                    $rect_scalar(&mut b, value, count, rb, rows);
                    assert_eq!(a, b, "{sel}, rect {count}x{rows}+{pad}");
                }
            }
        }
    };
}

memset_tests!(
    u16,
    memset16_all_tiers,
    memset16_with,
    memset16_scalar,
    rect_memset16_with,
    rect_memset16_scalar,
    0x1234
);
memset_tests!(
    u32,
    memset32_all_tiers,
    memset32_with,
    memset32_scalar,
    rect_memset32_with,
    rect_memset32_scalar,
    0x1234_5678
);
memset_tests!(
    u64,
    memset64_all_tiers,
    memset64_with,
    memset64_scalar,
    rect_memset64_with,
    rect_memset64_scalar,
    0x1234_5678_9ABC_DEF0
);

#[test]
fn default_selection_entry_points() {
    let mut a = [0u32; 70];
    memset32(&mut a, 7, 65);
    assert!(a[..65].iter().all(|&x| x == 7) && a[65..].iter().all(|&x| x == 0));
    let mut b = [0u16; 30];
    rect_memset16(&mut b, 9, 4, 12, 3); // rows 6 elements apart
    for (i, &x) in b.iter().enumerate() {
        let in_rect = i < 16 && i % 6 < 4;
        assert_eq!(x, if in_rect { 9 } else { 0 }, "{i}");
    }
}
