// Copyright (C) 2025 The skia-rust Authors.
//
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE
// file. Additional intellectual property rights are granted by the PATENTS file (libwebp).

//! Differential replay of the lossy encoder's YUV import (`enc::picture::import_rgba`) against
//! the reference build of the pinned libwebp 1.4.0 sources
//! (`oracle/codec-diff/libwebp/enc/yuv_import.c`, built by `build.sh`).
//!
//! Each line is `name width height rgbx y_fnv1a64 u_fnv1a64 v_fnv1a64 a_fnv1a64|-`: the planes
//! of `WebPPictureImportRGBA` (or `WebPPictureImportRGBX` for `rgbx = 1`, opaque) with
//! `use_argb = 0`, hashed as the C reference hashes them.

mod common;

use common::lossy_corpus::{IMAGES, fnv1a64, make_rgba};
use skia_rust_libwebp::enc::picture::import_rgba;

/// Expected output of the C reference (`expected_yuv.txt`).
const EXPECTED: &str = include_str!("../../../oracle/codec-diff/libwebp/enc/expected_yuv.txt");

#[test]
fn yuv_import_matches_c_reference() {
    let mut actual = String::new();
    for &(name, kind, w, h) in &IMAGES {
        for rgbx in [false, true] {
            // The C reference skips the RGBX variant of the alpha images.
            if rgbx && (kind == 4 || kind == 7) {
                continue;
            }
            let mut rgba = make_rgba(kind, w, h);
            if rgbx {
                for px in rgba.chunks_exact_mut(4) {
                    px[3] = 255;
                }
            }
            let pic = import_rgba(&rgba, 4 * w, w, h, false, !rgbx).expect("import");
            let mut line = format!("{name} {w} {h} {}", u8::from(rgbx));
            for plane in [Some(&pic.y), Some(&pic.u), Some(&pic.v), pic.a.as_ref()] {
                match plane {
                    Some(p) => line.push_str(&format!(" {:016x}", fnv1a64(p))),
                    None => line.push_str(" -"),
                }
            }
            actual.push_str(&line);
            actual.push('\n');
        }
    }
    assert_eq!(actual, EXPECTED);
}
