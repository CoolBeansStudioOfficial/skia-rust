//! Replays the option-variant corpus of `oracle/codec-diff/libwebp` (cropping, scaling, bypass of
//! the loop filter, no fancy upsampling) through `decode_with_options` and compares the result
//! line by line with `expected_options.txt`, the output of `webpopts.c` (built from the pinned
//! libwebp 1.4.0 by `build.sh`).

// clippy::cast_sign_loss: the decoded sizes are non-negative, as in webpopts.c.
#![allow(clippy::cast_sign_loss)]

mod common;

use std::fmt::Write as _;

use common::{MODES, oracle_dir, status_code};
use skia_rust_libwebp::{
    CspMode, DecodeOptions, Status, decode_with_options, get_features, rescaler,
};

/// The modes `webpopts.c` decodes into, by name, with their bytes per pixel.
const OPTION_MODES: [&str; 6] = ["RGBA", "BGRA", "rgbA", "bgrA", "RGB", "RGB565"];

/// Parses a variant string written by `webpopts.c`: `none`, `crop=L,T,W,H`, `scale=W,H`,
/// `bypass`, `nofancy`, joined by `+`.
fn parse_variant(variant: &str) -> DecodeOptions {
    let mut opts = DecodeOptions::default();
    for part in variant.split('+') {
        match part {
            "none" => {}
            "nofancy" => opts.no_fancy_upsampling = true,
            "bypass" => opts.bypass_filtering = true,
            _ => {
                if let Some(v) = part.strip_prefix("crop=") {
                    let n: Vec<i32> = v.split(',').map(|x| x.parse().expect("crop int")).collect();
                    opts.crop = Some((n[0], n[1], n[2], n[3]));
                } else if let Some(v) = part.strip_prefix("scale=") {
                    let n: Vec<i32> = v
                        .split(',')
                        .map(|x| x.parse().expect("scale int"))
                        .collect();
                    opts.scale = Some((n[0], n[1]));
                } else {
                    panic!("unknown variant part {part}");
                }
            }
        }
    }
    opts
}

/// One line of `webpopts` output for `data` decoded with `variant` into `mode`.
fn option_line(
    base: &str,
    variant: &str,
    name: &str,
    mode: CspMode,
    bpp: usize,
    data: &[u8],
) -> String {
    let opts = parse_variant(variant);
    let Ok(features) = get_features(data) else {
        let s = decode_with_options(data, mode, &mut [], 0, &opts)
            .err()
            .unwrap_or(Status::BitstreamError);
        return format!(
            "{base} {variant} {name} status={} w=0 h=0 fnv=0000000000000000",
            status_code(s)
        );
    };
    // The window the decoder will produce, sized as WebPAllocateDecBuffer sizes it.
    let (mut w, mut h) = match opts.crop {
        Some((_, _, cw, ch)) => (cw, ch),
        None => (features.width, features.height),
    };
    if let Some((sw, sh)) = opts.scale {
        let (mut sw, mut sh) = (sw, sh);
        if rescaler::get_scaled_dimensions(w, h, &mut sw, &mut sh) {
            (w, h) = (sw, sh);
        } else {
            (w, h) = (0, 0);
        }
    }
    let stride = w.max(1) as usize * bpp;
    let mut out = vec![0u8; stride * h.max(1) as usize];
    let result = if w > 0 && h > 0 {
        decode_with_options(data, mode, &mut out, stride, &opts)
    } else {
        decode_with_options(data, mode, &mut [], 0, &opts)
    };
    match result {
        Ok((w, h)) => {
            // Fold each row's hash into a running hash, byte by byte (as webpopts.c does).
            let row_bytes = w as usize * bpp;
            let mut acc: u64 = 1_469_598_103_934_665_603;
            for y in 0..h as usize {
                let r = fnv1a(&out[y * stride..y * stride + row_bytes]);
                for b in 0..8 {
                    acc ^= (r >> (8 * b)) & 0xff;
                    acc = acc.wrapping_mul(1_099_511_628_211);
                }
            }
            format!("{base} {variant} {name} status=0 w={w} h={h} fnv={acc:016x}")
        }
        Err(s) => format!(
            "{base} {variant} {name} status={} w=0 h=0 fnv=0000000000000000",
            status_code(s)
        ),
    }
}

/// FNV-1a 64 over `bytes`, as `Fnv1a` in `webpopts.c`.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 1_469_598_103_934_665_603;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(1_099_511_628_211);
    }
    h
}

#[test]
fn options_replay_matches_c_reference() {
    let dir = oracle_dir();
    let expected = std::fs::read_to_string(dir.join("expected_options.txt"))
        .expect("read expected_options.txt");
    // The files and variants, in the order they appear in the expected output.
    let mut files: Vec<&str> = Vec::new();
    let mut variants: Vec<&str> = Vec::new();
    for l in expected.lines() {
        let mut it = l.split(' ');
        let base = it.next().expect("basename");
        let variant = it.next().expect("variant");
        if !files.contains(&base) {
            files.push(base);
        }
        if !variants.contains(&variant) {
            variants.push(variant);
        }
    }
    let mut actual = String::new();
    for base in files {
        let data = std::fs::read(dir.join("corpus").join(base)).expect("read corpus file");
        for variant in &variants {
            for name in OPTION_MODES {
                let (_, mode, bpp) = *MODES
                    .iter()
                    .find(|(n, _, _)| *n == name)
                    .expect("mode name");
                writeln!(
                    actual,
                    "{}",
                    option_line(base, variant, name, mode, bpp, &data)
                )
                .expect("write to String");
            }
        }
    }
    let expected_lines: Vec<&str> = expected.lines().collect();
    let actual_lines: Vec<&str> = actual.lines().collect();
    let mismatches: Vec<String> = expected_lines
        .iter()
        .zip(actual_lines.iter())
        .filter(|(e, a)| e != a)
        .take(20)
        .map(|(e, a)| format!("expected: {e}\n  actual: {a}"))
        .collect();
    assert!(
        mismatches.is_empty() && expected_lines.len() == actual_lines.len(),
        "{} of {} lines differ (first 20 shown):\n{}",
        expected_lines.len().max(actual_lines.len()) - mismatches.len(),
        expected_lines.len(),
        mismatches.join("\n")
    );
}
