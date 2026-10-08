//! Differential check of the IDCTs against libjpeg-turbo's own (`oracle/codec-diff/libjpeg/idct_check.c`).
//!
//! The expected text is `tests/expected/idct.txt`, generated from the C tool. The inputs come
//! from the same 64-bit LCG as the C tool.

use crate::idct::{RangeLimit, idct_for_size};
use crate::tables::DCTSIZE2;

/// The same 64-bit LCG as `idct_check.c`.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
}

#[test]
fn idct_matches_libjpeg_c() {
    let expected = include_str!("../tests/expected/idct.txt");
    let rl = RangeLimit::new();
    let mut got = String::new();
    for size in 1..=8i32 {
        let mut lcg = Lcg(12345 + size as u64);
        for blk in 0..200 {
            let mut coef = [0i16; DCTSIZE2];
            let mut quant = [0i32; DCTSIZE2];
            for i in 0..DCTSIZE2 {
                let v = lcg.next();
                coef[i] = if v & 1 != 0 { (((v >> 1) % 512) as i32 - 256) as i16 } else { 0 };
                quant[i] = (lcg.next() % 16) as i32 + 1;
            }
            let out = idct_for_size(size, &coef, &quant, &rl).expect("size");
            got.push_str(&format!("size {size} block {blk}:"));
            for row in out.iter().take(size as usize) {
                for v in row.iter().take(size as usize) {
                    got.push_str(&format!(" {v}"));
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
