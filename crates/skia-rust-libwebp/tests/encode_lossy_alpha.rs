// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Differential replay of the lossy encoder with transparency (the `ALPH` chunk, the VP8X
//! header and the transparent-area cleanup), against `oracle/codec-diff/libwebp/enc/
//! encode_lossy_alpha.c` built by `build.sh` (`expected_lossy_alpha.txt`): `name width height
//! quality bytes fnv1a64`, or `... error <code>`. The generators and the LCG are the C ones.

// The generators mirror the C reference's `uint8_t` and `int` arithmetic, so their casts wrap as
// the C conversions do; the single-letter names are the C names (x, y, r, g, b, a).
#![allow(
    clippy::cast_possible_truncation,
    clippy::many_single_char_names,
    clippy::cast_sign_loss
)]

mod common;

use skia_rust_libwebp::enc::vp8_lossy::encode_lossy;

/// The generators of `encode_lossy_alpha.c` (`make_image`), as RGBA bytes.
fn make_rgba(kind: u32, w: usize, h: usize) -> Vec<u8> {
    let mut seed: u32 = 12345u32.wrapping_add(kind);
    let mut lcg = || {
        seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (seed >> 16) & 0x7fff
    };
    let mut out = Vec::with_capacity(w * h * 4);
    for y in 0..h {
        for x in 0..w {
            let (mut r, mut g, mut b, mut a) = (0u8, 0u8, 0u8, 255u8);
            match kind {
                4 => {
                    r = lcg() as u8;
                    g = lcg() as u8;
                    b = lcg() as u8;
                    a = lcg() as u8;
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
                    a = ((x * 9 + y * 4) & 255) as u8;
                }
                10 => {
                    let (bx, by) = (x / 8, y / 8);
                    if (bx + by) % 3 == 0 {
                        r = lcg() as u8;
                        g = lcg() as u8;
                        b = lcg() as u8;
                        a = 0;
                    } else {
                        r = (x * 5) as u8;
                        g = (y * 7) as u8;
                        b = (bx * 11) as u8;
                        a = 255;
                    }
                }
                _ => {}
            }
            out.extend_from_slice(&[r, g, b, a]);
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

/// The corpus cases, in the order of `main` in `encode_lossy_alpha.c`.
const CASES: &[(&str, u32, usize, usize)] = &[
    ("noise_alpha_40x30", 4, 40, 30),
    ("alpha_ramp_64x8", 7, 64, 8),
    ("two_colours_alpha_41x23", 8, 41, 23),
    ("photo_alpha_64x48", 9, 64, 48),
    ("blocks_alpha_72x40", 10, 72, 40),
    ("blocks_alpha_75x37", 10, 75, 37),
    ("pixel_alpha_1x1", 4, 1, 1),
    ("row_alpha_5x1", 7, 5, 1),
];

const QUALITIES: [u32; 3] = [0, 50, 100];

#[test]
fn encode_lossy_alpha_matches_reference() {
    let text = std::fs::read_to_string(common::oracle_dir().join("enc/expected_lossy_alpha.txt"))
        .expect("expected_lossy_alpha.txt");
    let mut expected = text.lines();
    let mut mismatches = Vec::new();
    for &(name, kind, w, h) in CASES {
        let rgba = make_rgba(kind, w, h);
        for quality in QUALITIES {
            let line = expected.next().expect("one expected line per case");
            let fields: Vec<&str> = line.split(' ').collect();
            assert_eq!(fields[0], name);
            let got = encode_lossy(&rgba, w, h, quality as f32, false);
            if fields.get(4) == Some(&"error") {
                if got.is_some() {
                    mismatches.push(format!("{name} q{quality}: expected error"));
                }
                continue;
            }
            let want_len: usize = fields[4].parse().expect("bytes");
            let want_hash = fields[5];
            let Some(bytes) = got else {
                mismatches.push(format!("{name} q{quality}: no output"));
                continue;
            };
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
        "{} mismatches:\n{}",
        mismatches.len(),
        mismatches.join("\n")
    );
}
