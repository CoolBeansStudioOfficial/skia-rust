// Copyright (C) 2025 The skia-rust Authors.
// Use of this source code is governed by the BSD-3-Clause licence in the LICENSE file.
//
//! Prints the decode of each file in every output mode, in the format of
//! `oracle/codec-diff/libwebp/webpdump.c`, so the two outputs can be compared line by line:
//!
//! ```text
//! cargo run -p skia-rust-libwebp --example webpdump -- <files...>
//! ```

// clippy::cast_sign_loss: the decoder's width and height are non-negative and are converted to
// `usize` buffer sizes, as the C driver does.
#![allow(clippy::cast_sign_loss)]

use skia_rust_libwebp::{CspMode, Status, decode_webp, get_features};

/// The output modes, with the names used by the C driver.
const MODES: [(&str, CspMode, usize); 9] = [
    ("RGB", CspMode::Rgb, 3),
    ("RGBA", CspMode::Rgba, 4),
    ("BGR", CspMode::Bgr, 3),
    ("BGRA", CspMode::Bgra, 4),
    ("ARGB", CspMode::Argb, 4),
    ("rgbA", CspMode::RgbA, 4),
    ("bgrA", CspMode::BgrA, 4),
    ("Argb", CspMode::ArgbPremultiplied, 4),
    ("RGB565", CspMode::Rgb565, 2),
];

/// FNV-1a 64 over `bytes`, as the C driver computes it.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

/// The `VP8StatusCode` numbering of `webp/decode.h`.
fn status_code(s: Status) -> i32 {
    match s {
        Status::Ok => 0,
        Status::OutOfMemory => 1,
        Status::InvalidParam => 2,
        Status::BitstreamError => 3,
        Status::UnsupportedFeature => 4,
        Status::Suspended => 5,
        Status::UserAbort => 6,
        Status::NotEnoughData => 7,
    }
}

fn main() {
    for path in std::env::args().skip(1) {
        let base = path.rsplit('/').next().unwrap_or(&path).to_string();
        let Ok(data) = std::fs::read(&path) else {
            println!("{base} unreadable");
            continue;
        };
        for (name, mode, bpp) in MODES {
            // Size first (WebPGetFeatures), then decode into a tightly packed buffer.
            let Ok(features) = get_features(&data) else {
                // WebPDecode fails in its GetFeatures step with the same status; the empty
                // buffer is never written.
                let s = decode_webp(&data, mode, &mut [], 0)
                    .err()
                    .unwrap_or(Status::BitstreamError);
                println!(
                    "{base} {name} status={} w=0 h=0 fnv=0000000000000000",
                    status_code(s)
                );
                continue;
            };
            let (w, h) = (features.width, features.height);
            let stride = w as usize * bpp;
            let mut out = vec![0u8; stride * h as usize];
            match decode_webp(&data, mode, &mut out, stride) {
                Ok((w, h)) => {
                    // Fold each row's hash into a running hash, byte by byte (as the C driver).
                    let mut acc: u64 = 1_469_598_103_934_665_603;
                    for y in 0..h as usize {
                        let r = fnv1a(&out[y * stride..(y + 1) * stride]);
                        for b in 0..8 {
                            acc ^= (r >> (8 * b)) & 0xff;
                            acc = acc.wrapping_mul(1_099_511_628_211);
                        }
                    }
                    println!("{base} {name} status=0 w={w} h={h} fnv={acc:016x}");
                }
                Err(s) => println!(
                    "{base} {name} status={} w=0 h=0 fnv=0000000000000000",
                    status_code(s)
                ),
            }
        }
    }
}
