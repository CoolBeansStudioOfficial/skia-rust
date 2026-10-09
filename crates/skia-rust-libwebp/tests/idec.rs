//! Replays the incremental differential of `oracle/codec-diff/libwebp` (`webpidec.c`): each corpus
//! file is fed to `IDecoder::update` as a growing prefix in steps of 1, 7, 64 and 4096 bytes, in
//! every RGB output mode, and the status, the rows written and the pixels of those rows are
//! compared with the C reference after each call (`expected_idec.txt`).

// clippy::cast_sign_loss: the sizes and rows are non-negative, as in webpidec.c.
#![allow(clippy::cast_sign_loss)]

mod common;

use std::fmt::Write as _;

use common::{MODES, oracle_dir, status_code};
use skia_rust_libwebp::{CspMode, DecodeOptions, IDecoder, Status, get_features};

const CHUNKS: [usize; 4] = [1, 7, 64, 4096];

/// FNV-1a 64 over `bytes`, as `Fnv1a` in `webpidec.c`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

/// `HashRows` of `webpidec.c`: each row's FNV folded byte by byte into a running hash.
fn hash_rows(pixels: &[u8], stride: usize, row_bytes: usize, rows: i32) -> u64 {
    let mut acc: u64 = 1_469_598_103_934_665_603;
    for y in 0..rows as usize {
        let r = fnv1a(&pixels[y * stride..y * stride + row_bytes]);
        for b in 0..8 {
            acc ^= (r >> (8 * b)) & 0xff;
            acc = acc.wrapping_mul(1_099_511_628_211);
        }
    }
    acc
}

/// The lines `webpidec.c` prints for one file, mode and chunk size.
fn run_one(base: &str, data: &[u8], name: &str, mode: CspMode, bpp: usize, chunk: usize) -> String {
    let mut out = String::new();
    let Some(mut idec) = IDecoder::new(mode, DecodeOptions::default()) else {
        writeln!(
            out,
            "{base} {name} chunk={chunk} fed=0 status=invalid last_y=-1 fnv=0000000000000000"
        )
        .expect("write to String");
        return out;
    };
    let mut fed = 0usize;
    let mut last_status: i32 = -1;
    let mut last_y: i32 = -2;
    let mut have_out: i32 = -1;
    let mut status = Status::Suspended;
    while fed < data.len() {
        let n = (data.len() - fed).min(chunk);
        fed += n;
        status = idec.update(&data[..fed]);
        let (ly, rgb) = match idec.rgb() {
            Some(r) => (r.last_y, Some(r)),
            None => (-1, None),
        };
        let code = status_code(status);
        if status != Status::Suspended
            || code != last_status
            || ly != last_y
            || i32::from(rgb.is_some()) != have_out
        {
            let fnv = rgb.map_or(0, |r| {
                hash_rows(r.pixels, r.stride, r.width as usize * bpp, ly)
            });
            writeln!(
                out,
                "{base} {name} chunk={chunk} fed={fed} status={code} last_y={ly} fnv={fnv:016x}"
            )
            .expect("write to String");
            last_status = code;
            last_y = ly;
            have_out = i32::from(rgb.is_some());
        }
        if status != Status::Suspended {
            break;
        }
    }
    let (ly, rgb) = match idec.rgb() {
        Some(r) => (r.last_y, Some(r)),
        None => (-1, None),
    };
    let fnv = rgb.map_or(0, |r| {
        hash_rows(r.pixels, r.stride, r.width as usize * bpp, ly)
    });
    writeln!(
        out,
        "{base} {name} chunk={chunk} end fed={fed} status={} last_y={ly} fnv={fnv:016x}",
        status_code(status)
    )
    .expect("write to String");
    out
}

#[test]
fn idec_replay_matches_c_reference() {
    let dir = oracle_dir();
    let expected =
        std::fs::read_to_string(dir.join("expected_idec.txt")).expect("read expected_idec.txt");
    // The files, in the order they appear in the expected output.
    let mut files: Vec<&str> = Vec::new();
    for l in expected.lines() {
        let base = l.split(' ').next().expect("basename");
        if !files.contains(&base) {
            files.push(base);
        }
    }
    // The lossless (VP8L) incremental path is not ported yet (see `idec`): the lossy files are
    // compared here, and the lossless ones are left out until `DecodeVP8LData` lands.
    let mut lossy: Vec<&str> = Vec::new();
    let mut actual = String::new();
    for base in files {
        let data = std::fs::read(dir.join("corpus").join(base)).expect("read corpus file");
        if get_features(&data).map_or(true, |f| f.is_lossless) {
            continue;
        }
        lossy.push(base);
        for (name, mode, bpp) in MODES {
            for chunk in CHUNKS {
                actual.push_str(&run_one(base, &data, name, mode, bpp, chunk));
            }
        }
    }
    let mut expected_lossy = String::new();
    for l in expected
        .lines()
        .filter(|l| lossy.contains(&l.split(' ').next().unwrap_or("")))
    {
        expected_lossy.push_str(l);
        expected_lossy.push('\n');
    }
    assert_eq!(actual, expected_lossy);
}
