// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Differential replay of the VP8L encoder against the reference build of the pinned libwebp
//! 1.4.0 sources (`oracle/codec-diff/libwebp/enc/encode_lossless.c`, built by `build.sh`).
//!
//! Each line is `name width height quality bytes fnv1a64` for one image, encoded the way
//! `SkWebpEncoderImpl` encodes lossless: RGBA import, `method 0`, `exact 0`. The generators and
//! the LCG are the C ones; the expected lines are the C output (`oracle/.../enc/expected.txt`).

// The generators mirror the C reference's `uint8_t` and `int` arithmetic, so their casts wrap as
// the C conversions do; the single-letter names are the C names (x, y, r, g, b, a).
#![allow(
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::cast_sign_loss
)]

use skia_rust_libwebp::enc::encode_lossless;

/// Expected output of the C reference, one case per line (`expected.txt`).
const EXPECTED: &str = "\
gradient_37x23 37 23 0 158 9c4268dc7b6c9dad
gradient_37x23 37 23 50 158 e1cba68e48ea89b8
gradient_37x23 37 23 100 154 2b0f88c9def1d9cb
solid_64x64 64 64 0 36 ebb691d8f16d8f82
solid_64x64 64 64 50 36 ebb691d8f16d8f82
solid_64x64 64 64 100 36 ebb691d8f16d8f82
five_colours_50x40 50 40 0 122 94ead1e4352d9ae8
five_colours_50x40 50 40 50 124 f3014dac10b67171
five_colours_50x40 50 40 100 124 f3014dac10b67171
noise_33x17 33 17 0 1782 337f4d8bcdcd305a
noise_33x17 33 17 50 1782 337f4d8bcdcd305a
noise_33x17 33 17 100 1782 337f4d8bcdcd305a
noise_alpha_40x30 40 30 0 4942 37ced0336fc9a83c
noise_alpha_40x30 40 30 50 4942 37ced0336fc9a83c
noise_alpha_40x30 40 30 100 4942 37ced0336fc9a83c
colours200_120x120 120 120 0 11920 279e59031ce07fde
colours200_120x120 120 120 50 11920 279e59031ce07fde
colours200_120x120 120 120 100 11920 279e59031ce07fde
smooth_96x80 96 80 0 6228 c7232d4ef19bc5a1
smooth_96x80 96 80 50 6112 ac0aa3083538a735
smooth_96x80 96 80 100 6110 7bde50aa78de5181
alpha_ramp_64x8 64 8 0 120 87352e844b2d1656
alpha_ramp_64x8 64 8 50 120 87352e844b2d1656
alpha_ramp_64x8 64 8 100 120 87352e844b2d1656
pixel_1x1 1 1 0 34 ca2772b08d29a795
pixel_1x1 1 1 50 34 ca2772b08d29a795
pixel_1x1 1 1 100 34 ca2772b08d29a795
row_2x1 2 1 0 42 940e5644b227d8ba
row_2x1 2 1 50 42 940e5644b227d8ba
row_2x1 2 1 100 42 940e5644b227d8ba
column_1x5 1 5 0 40 57c76fb6f674f2e4
column_1x5 1 5 50 40 57c76fb6f674f2e4
column_1x5 1 5 100 40 57c76fb6f674f2e4
gradient_300x200 300 200 0 47822 642dcd2e25ddb500
gradient_300x200 300 200 50 46420 008f3a0794d8a3b9
gradient_300x200 300 200 100 45984 c4214aca47b2135f
";

/// The corpus generators of `encode_lossless.c` (`make_image`), as ARGB pixels.
fn make_image(kind: u32, w: usize, h: usize) -> Vec<u32> {
    let mut seed: u32 = 12345u32.wrapping_add(kind);
    let mut lcg = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) & 0x7fff
    };
    let mut out = Vec::with_capacity(w * h);
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut a) = (0u8, 0u8, 0u8, 255u8);
            match kind {
                0 => {
                    r = (x * 7) as u8;
                    g = (y * 11) as u8;
                    b = ((x + y) * 3) as u8;
                }
                1 => b = 255,
                2 => {
                    const PAL: [[u8; 3]; 5] = [
                        [255, 0, 0],
                        [0, 255, 0],
                        [0, 0, 255],
                        [10, 20, 30],
                        [200, 200, 0],
                    ];
                    let idx = ((x / 5) + (y / 3)) % 5;
                    [r, g, b] = PAL[idx];
                }
                3 => {
                    r = lcg() as u8;
                    g = lcg() as u8;
                    b = lcg() as u8;
                }
                4 => {
                    r = lcg() as u8;
                    g = lcg() as u8;
                    b = lcg() as u8;
                    a = lcg() as u8;
                }
                5 => {
                    let idx = (x * x + y * 3) % 200;
                    r = (idx * 37) as u8;
                    g = (idx * 91 + 7) as u8;
                    b = (idx * 13 + 100) as u8;
                }
                6 => {
                    r = ((x * 255) / if w > 1 { w - 1 } else { 1 }) as u8;
                    g = ((y * 255) / if h > 1 { h - 1 } else { 1 }) as u8;
                    b = (((x ^ y) as u32 + (lcg() & 3)) & 255) as u8;
                }
                7 => {
                    r = 40;
                    g = 90;
                    b = 200;
                    a = ((x * 255) / if w > 1 { w - 1 } else { 1 }) as u8;
                }
                _ => {}
            }
            out.push(
                (u32::from(a) << 24) | (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b),
            );
        }
    }
    out
}

/// Port of the reference's `fnv1a64`.
fn fnv1a64(data: &[u8]) -> u64 {
    let mut hash: u64 = 1_469_598_103_934_665_603;
    for &byte in data {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(1_099_511_628_211);
    }
    hash
}

/// The corpus cases, in the order of `main` in `encode_lossless.c`.
const CASES: &[(&str, u32, usize, usize)] = &[
    ("gradient_37x23", 0, 37, 23),
    ("solid_64x64", 1, 64, 64),
    ("five_colours_50x40", 2, 50, 40),
    ("noise_33x17", 3, 33, 17),
    ("noise_alpha_40x30", 4, 40, 30),
    ("colours200_120x120", 5, 120, 120),
    ("smooth_96x80", 6, 96, 80),
    ("alpha_ramp_64x8", 7, 64, 8),
    ("pixel_1x1", 0, 1, 1),
    ("row_2x1", 0, 2, 1),
    ("column_1x5", 2, 1, 5),
    ("gradient_300x200", 6, 300, 200),
];

#[test]
fn encode_lossless_matches_reference_corpus() {
    let mut expected = EXPECTED.lines();
    let mut mismatches = Vec::new();
    for &(name, kind, w, h) in CASES {
        for quality in [0, 50, 100] {
            let line = expected.next().expect("one expected line per case");
            let mut fields = line.split(' ');
            assert_eq!(fields.next(), Some(name));
            let _ = (fields.next(), fields.next(), fields.next());
            let want_len: usize = fields.next().expect("bytes").parse().expect("bytes");
            let want_hash = fields.next().expect("hash");
            let argb = make_image(kind, w, h);
            let bytes = encode_lossless(w, h, &argb, quality, false).expect("encodes");
            let got_hash = format!("{:016x}", fnv1a64(&bytes));
            if bytes.len() != want_len || got_hash != want_hash {
                mismatches.push(format!(
                    "{name} q{quality}: got {} bytes {got_hash}, want {want_len} bytes {want_hash}",
                    bytes.len()
                ));
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "mismatches:\n{}",
        mismatches.join("\n")
    );
}
