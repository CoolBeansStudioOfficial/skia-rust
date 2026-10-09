//! Differential check of the IDCTs against libjpeg-turbo's own (`oracle/codec-diff/libjpeg/idct_check.c`).
//!
//! The expected text is `tests/expected/idct.txt`, generated from the C tool. The inputs come
//! from the same 64-bit LCG as the C tool.

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

use std::fmt::Write as _;

use crate::idct::{RangeLimit, idct_for_size};
use crate::tables::DCTSIZE2;

/// The same 64-bit LCG as `idct_check.c`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
}

#[test]
fn idct_matches_libjpeg_c() {
    let expected = include_str!("../tests/expected/idct.txt");
    let rl = RangeLimit::new();
    let mut got = String::new();
    for size in 1..=16i32 {
        let mut lcg = Lcg(12345 + size as u64);
        for blk in 0..200 {
            let mut coef = [0i16; DCTSIZE2];
            let mut quant = [0i32; DCTSIZE2];
            for i in 0..DCTSIZE2 {
                let v = lcg.next();
                coef[i] = if v & 1 != 0 {
                    (((v >> 1) % 512) as i32 - 256) as i16
                } else {
                    0
                };
                quant[i] = (lcg.next() % 16) as i32 + 1;
            }
            let out = idct_for_size(size, &coef, &quant, &rl).expect("size");
            write!(got, "size {size} block {blk}:").expect("write to String");
            for row in out.iter().take(size as usize) {
                for v in row.iter().take(size as usize) {
                    write!(got, " {v}").expect("write to String");
                }
            }
            got.push('\n');
        }
    }
    for (i, (g, e)) in got.lines().zip(expected.lines()).enumerate() {
        assert_eq!(g, e, "line {}", i + 1);
    }
    assert_eq!(got.lines().count(), expected.lines().count());
}
