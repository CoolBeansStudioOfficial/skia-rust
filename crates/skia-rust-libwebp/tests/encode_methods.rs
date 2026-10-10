// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Differential replay of the VP8L encoder at the methods `SkWebpEncoder` reaches (`method` 0
//! for lossless pictures, `method` 3 with `exact` for the alpha plane of lossy pictures), plus
//! methods 1, 2 and 4 for coverage of the same paths.
//!
//! The expected lines are the output of `oracle/codec-diff/libwebp/enc/encode_lossless_methods.c`
//! built by `build.sh` (`expected_methods.txt`): `name width height method exact quality bytes
//! fnv1a64`, or `... error <code>`. The generators and the LCG are the C ones.

// The generators mirror the C reference's `uint8_t` and `int` arithmetic, so their casts wrap as
// the C conversions do; the single-letter names are the C names (x, y, r, g, b, a).
#![allow(
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::cast_sign_loss
)]

mod common;

use skia_rust_libwebp::enc::encode_lossless_method;

/// The corpus generators of `encode_lossless_methods.c` (`make_image`), as ARGB pixels.
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
                8 => {
                    if ((x / 4) + (y / 2)) % 3 == 0 {
                        r = 255;
                        g = 255;
                        b = 255;
                        a = 0;
                    } else {
                        r = 16;
                        g = 32;
                        b = 48;
                    }
                }
                9 => {
                    r = ((x * 3 + y) as u32 + (lcg() & 15)) as u8;
                    g = ((x + y * 2) as u32 + (lcg() & 7)) as u8;
                    b = ((x * y) as u32 + (lcg() & 31)) as u8;
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

/// The corpus cases, in the order of `main` in `encode_lossless_methods.c`.
const CASES: &[(&str, u32, usize, usize)] = &[
    ("gradient_37x23", 0, 37, 23),
    ("solid_64x64", 1, 64, 64),
    ("five_colours_50x40", 2, 50, 40),
    ("noise_33x17", 3, 33, 17),
    ("noise_alpha_40x30", 4, 40, 30),
    ("colours200_120x120", 5, 120, 120),
    ("smooth_96x80", 6, 96, 80),
    ("alpha_ramp_64x8", 7, 64, 8),
    ("two_colours_41x23", 8, 41, 23),
    ("photo_64x48", 9, 64, 48),
    ("pixel_1x1", 0, 1, 1),
    ("row_2x1", 0, 2, 1),
    ("column_1x5", 2, 1, 5),
    ("gradient_300x200", 6, 300, 200),
];

const METHODS: [u32; 5] = [0, 1, 2, 3, 4];
const EXACTS: [bool; 2] = [false, true];
const QUALITIES: [i32; 3] = [0, 50, 100];

#[test]
fn encode_vp8l_methods_match_reference() {
    let text = std::fs::read_to_string(common::oracle_dir().join("enc/expected_methods.txt"))
        .expect("expected_methods.txt");
    let mut expected = text.lines();
    let mut mismatches = Vec::new();
    for &(name, kind, w, h) in CASES {
        let argb = make_image(kind, w, h);
        for method in METHODS {
            for exact in EXACTS {
                for quality in QUALITIES {
                    let line = expected.next().expect("one expected line per case");
                    let fields: Vec<&str> = line.split(' ').collect();
                    assert_eq!(fields[0], name);
                    let got = encode_lossless_method(w, h, &argb, method, quality, exact);
                    if fields.get(6) == Some(&"error") {
                        if got.is_some() {
                            mismatches.push(format!(
                                "{name} m{method} e{exact} q{quality}: expected error"
                            ));
                        }
                        continue;
                    }
                    let want_len: usize = fields[6].parse().expect("bytes");
                    let want_hash = fields[7];
                    let Some(bytes) = got else {
                        mismatches.push(format!("{name} m{method} e{exact} q{quality}: no output"));
                        continue;
                    };
                    let got_hash = format!("{:016x}", fnv1a64(&bytes));
                    if bytes.len() != want_len || got_hash != want_hash {
                        mismatches.push(format!(
                            "{name} m{method} e{exact} q{quality}: got {} bytes {got_hash}, want {want_len} bytes {want_hash}",
                            bytes.len()
                        ));
                    }
                }
            }
        }
    }
    assert!(
        mismatches.is_empty(),
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
