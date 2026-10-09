// Copyright 2026 The skia-rust Authors.
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Decodes every BMP in Skia's resources, and truncated and corrupted copies of them, through the
//! full, scanline and skip paths. The test asserts that nothing panics. With `--nocapture` it also
//! prints a digest of each resource's RGBA decode, used to cross-check the pixels against an
//! independent decoder (see the report that accompanies this port).

use std::path::{Path, PathBuf};

use skia_rust_codec::{Codec, Options, ZeroInitialized, decoders};
use skia_rust_core::color_type::ColorType;
use skia_rust_core::stream::MemoryStream;

// Corrupted headers can ask for huge images. Those are decoded only up to this many pixel bytes.
const MAX_PIXEL_BYTES: usize = 64 << 20;

fn resources_dir() -> Option<PathBuf> {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../third_party/skia/resources");
    dir.is_dir().then_some(dir)
}

fn collect_bmps(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).expect("readable resources directory") {
        let path = entry.expect("readable directory entry").path();
        if path.is_dir() {
            collect_bmps(&path, out);
        } else if path
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("bmp"))
        {
            out.push(path);
        }
    }
}

// FNV-1a over the bytes of a decoded image: a simple, portable digest.
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325_u64, |hash, &b| {
        (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    })
}

// Decodes `data` in full with `color_type` and `zero`, and returns the pixels. `None` when the
// stream is not a BMP the decoder accepts, or the image is too large to decode here.
fn decode_full(data: &[u8], color_type: ColorType, zero: ZeroInitialized) -> Option<Vec<u8>> {
    let mut codec = Codec::make_from_stream(MemoryStream::make_copy(data), decoders()).ok()?;
    let info = codec.info().with_color_type(color_type);
    let row_bytes = info.min_row_bytes();
    let size = info.compute_byte_size(row_bytes);
    if size > MAX_PIXEL_BYTES {
        return None;
    }
    let mut pixels = vec![0u8; size];
    let opts = Options {
        zero_initialized: zero,
        ..Options::default()
    };
    codec.get_pixels(&info, &mut pixels, row_bytes, Some(&opts));
    Some(pixels)
}

// Walks the scanline API over `data`: start, one row at a time, a skip, then the rest.
#[allow(clippy::cast_sign_loss)] // rows count up from zero
fn exercise_scanlines(data: &[u8]) {
    let Ok(mut codec) = Codec::make_from_stream(MemoryStream::make_copy(data), decoders()) else {
        return;
    };
    let info = codec.info().with_color_type(ColorType::BGRA8888);
    let size = info.compute_byte_size(info.min_row_bytes());
    if size > MAX_PIXEL_BYTES {
        return;
    }
    let row_bytes = info.min_row_bytes();
    let mut pixels = vec![0u8; size];
    let height = info.height();
    if codec.start_scanline_decode(&info, None) != skia_rust_codec::Result::Success {
        return;
    }
    let mut row = 0;
    while row < height {
        let start = row as usize * row_bytes;
        let lines = codec.get_scanlines(&mut pixels[start..], 1, row_bytes);
        if lines != 1 {
            break;
        }
        row += 1;
        if row == height / 2 {
            codec.skip_scanlines(1);
            row += 1;
        }
    }
}

// Runs every decode path over one input, and returns the RGBA digest of the full decode.
fn exercise(data: &[u8]) -> Option<u64> {
    let rgba = decode_full(data, ColorType::RGBA8888, ZeroInitialized::No);
    // Other colour types and the zero-initialized path, for coverage.
    let _ = decode_full(data, ColorType::BGRA8888, ZeroInitialized::Yes);
    let _ = decode_full(data, ColorType::RGB565, ZeroInitialized::No);
    let _ = decode_full(data, ColorType::RGBAF16, ZeroInitialized::No);
    exercise_scanlines(data);
    rgba.map(|pixels| fnv1a(&pixels))
}

// Port-independent smoke test: every resource BMP, then truncated and corrupted copies.
#[test]
fn bmp_resources_never_panic() {
    let Some(dir) = resources_dir() else {
        eprintln!("skipping: third_party/skia/resources is not present");
        return;
    };
    let mut files = Vec::new();
    collect_bmps(&dir, &mut files);
    files.sort();
    assert!(!files.is_empty(), "no .bmp resources found");

    for path in &files {
        let data = std::fs::read(path).expect("readable resource");
        let name = path.file_name().unwrap_or_default().to_string_lossy();

        // The file as it is.
        let digest = exercise(&data);
        if let Some(digest) = digest {
            println!("DIGEST {name} {digest:016x}");
        } else {
            println!("NODECODE {name}");
        }

        // Every truncation of the first 256 bytes, then a sample of the rest.
        let step = (data.len() / 48).max(1);
        let lengths = (0..data.len().min(256)).chain((256..data.len()).step_by(step));
        for len in lengths {
            let _ = exercise(&data[..len]);
        }

        // Single-byte corruptions: zero, all ones and a high bit, in the header and then sampled.
        let positions = (0..data.len().min(96)).chain((96..data.len()).step_by(step));
        for pos in positions {
            for value in [0x00u8, 0xFF, 0x80] {
                let mut corrupted = data.clone();
                corrupted[pos] = value;
                let _ = exercise(&corrupted);
            }
        }
    }
}

// The header's dimension fields are the most dangerous corruption: check the extremes directly.
#[test]
fn bmp_extreme_dimensions_do_not_panic() {
    let Some(dir) = resources_dir() else {
        return;
    };
    let path = dir.join("images/randPixels.bmp");
    let Ok(data) = std::fs::read(&path) else {
        return;
    };
    for dim in [0u32, 1, 0x7FFF_FFFF, 0x8000_0000, 0xFFFF_FFFF, 65535, 65536] {
        let mut corrupted = data.clone();
        corrupted[18..22].copy_from_slice(&dim.to_le_bytes());
        corrupted[22..26].copy_from_slice(&dim.to_le_bytes());
        let _ = exercise(&corrupted);
    }
}
