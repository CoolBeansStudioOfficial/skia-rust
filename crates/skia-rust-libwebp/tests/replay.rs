// Copyright (C) 2025 The skia-rust Authors.
// Use of this source code is governed by the BSD-3-Clause licence in the LICENSE file.
//
//! Replays the differential corpus of `oracle/codec-diff/libwebp` through the Rust decoder and
//! compares the result line by line with `expected.txt`, which is the output of the C reference
//! (`webpdump.c`, built from the pinned libwebp 1.4.0 by `build.sh`).

// clippy::cast_sign_loss: the decoder's width and height are non-negative and are converted to
// `usize` buffer sizes, as the C driver does.
#![allow(clippy::cast_sign_loss)]

use std::fmt::Write as _;
use std::path::PathBuf;

use skia_rust_libwebp::{CspMode, Status, decode_webp, get_features};

/// The output modes, in the order of `kModes` in `webpdump.c`.
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

/// FNV-1a 64 over `bytes`, as `Fnv1a` in `webpdump.c`.
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
        Status::NotEnoughData => 7,
    }
}

/// One `webpdump` output line for `data` decoded in `mode`.
fn line(base: &str, name: &str, mode: CspMode, bpp: usize, data: &[u8]) -> String {
    let (w, h) = match get_features(data) {
        Ok(f) => (f.width, f.height),
        Err(s) => {
            return format!(
                "{base} {name} status={} w=0 h=0 fnv=0000000000000000",
                status_code(s)
            );
        }
    };
    let stride = w as usize * bpp;
    let mut out = vec![0u8; stride * h as usize];
    match decode_webp(data, mode, &mut out, stride) {
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
            format!("{base} {name} status=0 w={w} h={h} fnv={acc:016x}")
        }
        Err(s) => format!(
            "{base} {name} status={} w=0 h=0 fnv=0000000000000000",
            status_code(s)
        ),
    }
}

fn oracle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/codec-diff/libwebp")
}

#[test]
fn replay_matches_c_reference() {
    let dir = oracle_dir();
    let expected = std::fs::read_to_string(dir.join("expected.txt")).expect("read expected.txt");
    // The files, in the order they appear in the expected output.
    let mut files: Vec<&str> = Vec::new();
    for l in expected.lines() {
        let base = l.split(' ').next().expect("basename");
        if !files.contains(&base) {
            files.push(base);
        }
    }
    let mut actual = String::new();
    for base in files {
        let data = std::fs::read(dir.join("corpus").join(base)).expect("read corpus file");
        for (name, mode, bpp) in MODES {
            writeln!(actual, "{}", line(base, name, mode, bpp, &data)).expect("write to String");
        }
    }
    assert_eq!(actual, expected);
}
