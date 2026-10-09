//! Helpers shared by the differential replay tests, which mirror `webpdump.c`.

// clippy::cast_sign_loss: the decoded width and height are non-negative sizes, as in webpdump.c.
// dead_code: each test crate uses a subset of these helpers.
#![allow(dead_code, clippy::cast_sign_loss)]

use std::fmt::Write as _;
use std::path::PathBuf;

use skia_rust_libwebp::{CspMode, DecodeOptions, IDecoder, Status, decode_webp, get_features};

/// The output modes, in the order of `kModes` in `webpdump.c`.
pub const MODES: [(&str, CspMode, usize); 9] = [
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
pub fn status_code(s: Status) -> i32 {
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

/// One `webpdump` output line for `data` decoded in `mode`.
pub fn line(base: &str, name: &str, mode: CspMode, bpp: usize, data: &[u8]) -> String {
    let Ok(features) = get_features(data) else {
        // WebPDecode fails in its GetFeatures step with the same status; the empty buffer is
        // never written.
        let s = decode_webp(data, mode, &mut [], 0)
            .err()
            .unwrap_or(Status::BitstreamError);
        return format!(
            "{base} {name} status={} w=0 h=0 fnv=0000000000000000",
            status_code(s)
        );
    };
    let (w, h) = (features.width, features.height);
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

/// The chunk sizes of the incremental differential (`CHUNKS` in `webpidec.c`).
pub const CHUNKS: [usize; 4] = [1, 7, 64, 4096];

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

/// The hash of the rows written so far, `0` when there is no output buffer (`webpidec.c`).
fn output_hash(idec: &IDecoder, bpp: usize) -> u64 {
    match idec.rgb() {
        Some(r) if !r.pixels.is_empty() => {
            hash_rows(r.pixels, r.stride, r.width as usize * bpp, r.last_y)
        }
        _ => 0,
    }
}

/// The lines `webpidec.c` prints for `data` fed to `IDecoder::update` as a growing prefix in
/// steps of `chunk` bytes, in output mode `mode`. A line is printed when the status, the last row
/// or the output buffer's availability changes, and once more at the end.
pub fn idec_lines(
    base: &str,
    data: &[u8],
    name: &str,
    mode: CspMode,
    bpp: usize,
    chunk: usize,
) -> String {
    let mut out = String::new();
    let Some(mut idec) = IDecoder::new(mode, DecodeOptions::default()) else {
        writeln!(
            out,
            "{base} {name} chunk={chunk} fed=0 status=invalid last_y=-1 fnv=0000000000000000"
        )
        .expect("write to String");
        return out;
    };
    // The state `webpidec.c` reads after each call: the last row (`-1` before the decoder exists)
    // and whether the output buffer is there.
    let observe = |idec: &IDecoder| match idec.rgb() {
        None => (-1, false),
        Some(r) => (r.last_y, !r.pixels.is_empty()),
    };
    let mut fed = 0usize;
    let mut last_status: i32 = -1;
    let mut last_y: i32 = -2;
    let mut have_out = false;
    let mut status = Status::Suspended;
    while fed < data.len() {
        let n = (data.len() - fed).min(chunk);
        fed += n;
        status = idec.update(&data[..fed]);
        let (ly, have) = observe(&idec);
        let code = status_code(status);
        if status != Status::Suspended || code != last_status || ly != last_y || have != have_out {
            let fnv = output_hash(&idec, bpp);
            writeln!(
                out,
                "{base} {name} chunk={chunk} fed={fed} status={code} last_y={ly} fnv={fnv:016x}"
            )
            .expect("write to String");
            last_status = code;
            last_y = ly;
            have_out = have;
        }
        if status != Status::Suspended {
            break;
        }
    }
    let (ly, _) = observe(&idec);
    let fnv = output_hash(&idec, bpp);
    writeln!(
        out,
        "{base} {name} chunk={chunk} end fed={fed} status={} last_y={ly} fnv={fnv:016x}",
        status_code(status)
    )
    .expect("write to String");
    out
}

pub fn oracle_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/codec-diff/libwebp")
}
