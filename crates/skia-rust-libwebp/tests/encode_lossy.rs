// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Differential replay of the lossy VP8 encoder against the reference build of the pinned libwebp
//! 1.4.0 sources (`oracle/codec-diff/libwebp/enc/encode_lossy.c`, built by `build.sh`).
//!
//! Each line is `name width height quality [rgbx] bytes fnv1a64`: the file that `WebPEncode`
//! writes for the image with `WebPConfigPreset(DEFAULT, quality)`, `method = 3`, `exact = 0`,
//! `use_sharp_yuv = 0`, imported as RGBA or RGBX. The images with transparency are not ported
//! (their ALPH chunk needs the VP8L encoder at method 3), so the replay covers the opaque cases.

mod common;

use common::lossy_corpus::{IMAGES, fnv1a64, make_rgba};
use skia_rust_libwebp::enc::vp8_lossy::encode_lossy;

/// Expected output of the C reference (`expected_lossy.txt`).
const EXPECTED: &str = include_str!("../../../oracle/codec-diff/libwebp/enc/expected_lossy.txt");

#[test]
fn lossy_encode_matches_c_reference_for_opaque_images() {
    let mut actual = String::new();
    for &(name, kind, w, h) in &IMAGES {
        // Transparent images are not ported (see the module comment).
        if kind == 4 || kind == 7 {
            continue;
        }
        for quality in [0u32, 50, 100] {
            for rgbx in [false, true] {
                let mut rgba = make_rgba(kind, w, h);
                if rgbx {
                    for px in rgba.chunks_exact_mut(4) {
                        px[3] = 255;
                    }
                }
                let bytes = encode_lossy(&rgba, w, h, quality as f32, rgbx).expect("encodes");
                let rgbx_tag = if rgbx { " rgbx" } else { "" };
                actual.push_str(&format!(
                    "{name} {w} {h} {quality}{rgbx_tag} {} {:016x}\n",
                    bytes.len(),
                    fnv1a64(&bytes)
                ));
            }
        }
    }
    let expected: String = EXPECTED
        .lines()
        .filter(|l| {
            let name = l.split(' ').next().unwrap_or("");
            !name.starts_with("noise_alpha") && !name.starts_with("alpha_ramp")
        })
        .map(|l| format!("{l}\n"))
        .collect();
    assert_eq!(actual, expected);
}
