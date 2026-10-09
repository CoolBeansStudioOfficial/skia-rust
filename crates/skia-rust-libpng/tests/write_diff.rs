// Copyright (C) 1998-2026 Glenn Randers-Pehrson and the libpng contributors.
// Copyright (C) 2026 The skia-rust Authors.
// Use of this source code is governed by the libpng licence (libpng-2.0) in the LICENSE file.
// Port of: the call sequence and corpus of oracle/codec-diff/libpng/pngwdump.c (libpng 1.6.56,
// d5515b5b), replayed against the Rust write path. The expected output is
// oracle/codec-diff/libpng/expected.txt, printed by the C harness built from the pinned sources.

// The corpus generator and the case table mirror pngwdump.c line for line, so the C names and the
// C integer conversions are kept as they are.
#![allow(clippy::many_single_char_names)] // pngwdump.c's names
#![allow(clippy::cast_possible_truncation)] // C's unsigned byte and int conversions
#![allow(clippy::cast_sign_loss)] // C's size_t arithmetic on non-negative sizes
#![allow(clippy::cast_possible_wrap)] // C's int arithmetic on small sizes
#![allow(clippy::cast_lossless)] // C's implicit widening
#![allow(clippy::too_many_lines)] // one function per C case loop

use std::cell::RefCell;
use std::fmt::Write as _;
use std::fs;
use std::path::Path;
use std::rc::Rc;

use skia_rust_libpng::error::PngResult;
use skia_rust_libpng::{
    PNG_HANDLE_CHUNK_ALWAYS, PngColor8, PngInfo, PngStruct, TextCompression, UnknownChunk,
};

/// Port of `rng_state` and `rnd` of pngwdump.c: a 32-bit LCG, 15 bits per call.
struct Rng(u32);

impl Rng {
    fn rnd(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_103_515_245).wrapping_add(12345);
        (self.0 >> 16) & 0x7fff
    }
}

/// Port of `gen_pixels`: a gradient with noise. `i` is the byte index.
fn gen_pixels(n: usize, seed: u32) -> Vec<u8> {
    let mut rng = Rng(seed);
    let mut b = vec![0u8; n];
    for (i, byte) in b.iter_mut().enumerate() {
        let mut v = (i as u32).wrapping_mul(7);
        if rng.rnd().is_multiple_of(4) {
            v = v.wrapping_add(rng.rnd());
        }
        *byte = v as u8;
    }
    b
}

/// Port of `gen_profile`: a 132-byte ICC header-sized profile.
fn gen_profile() -> Vec<u8> {
    let n = 132usize;
    let mut p = vec![0u8; n];
    p[3] = 132;
    p[8] = 2;
    p[12] = b'm';
    p[13] = b'n';
    p[14] = b't';
    p[15] = b'r';
    for (i, byte) in p.iter_mut().enumerate().skip(128) {
        *byte = (i * 3) as u8;
    }
    p
}

/// Port of `fnv1a` of pngwdump.c.
fn fnv1a(p: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in p {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// Port of `struct Format` of pngwdump.c: colour type, depth, source channels, filler after.
struct Format {
    name: &'static str,
    color_type: u8,
    bit_depth: u8,
    src_channels: usize,
    filler_after: bool,
}

const FORMATS: [Format; 6] = [
    Format {
        name: "rgba8",
        color_type: 6,
        bit_depth: 8,
        src_channels: 4,
        filler_after: false,
    },
    Format {
        name: "rgb8x",
        color_type: 2,
        bit_depth: 8,
        src_channels: 4,
        filler_after: true,
    },
    Format {
        name: "gray8",
        color_type: 0,
        bit_depth: 8,
        src_channels: 1,
        filler_after: false,
    },
    Format {
        name: "graya8",
        color_type: 4,
        bit_depth: 8,
        src_channels: 2,
        filler_after: false,
    },
    Format {
        name: "rgba16",
        color_type: 6,
        bit_depth: 16,
        src_channels: 8,
        filler_after: false,
    },
    Format {
        name: "rgb16x",
        color_type: 2,
        bit_depth: 16,
        src_channels: 8,
        filler_after: true,
    },
];

const SIZES: [(usize, usize); 8] = [
    (1, 1),
    (1, 7),
    (9, 1),
    (3, 3),
    (17, 13),
    (64, 64),
    (200, 150),
    (300, 120),
];

const FILTERS: [u8; 7] = [0x00, 0x08, 0x10, 0x20, 0x40, 0x80, 0xf8];

const LEVELS: [i32; 3] = [-1, 1, 9];

/// Port of `encode` of pngwdump.c: the calls `SkPngEncoderImpl` makes. Returns the PNG bytes.
fn encode(
    f: &Format,
    w: usize,
    h: usize,
    filters: u8,
    level: i32,
    extras: bool,
) -> PngResult<Vec<u8>> {
    // sRGB for even widths, iCCP for odd ones, when the extras are on.
    let use_icc = !w.is_multiple_of(2);
    let src_bpp = f.src_channels * (f.bit_depth as usize / 8);
    let rowbytes_src = w * src_bpp;
    let seed = (w as i32 * 131 + h as i32 * 17 + i32::from(filters)) as u32;
    let pixels = gen_pixels(rowbytes_src * h, seed);

    let sink: Rc<RefCell<Vec<u8>>> = Rc::new(RefCell::new(Vec::new()));
    let writer = Rc::clone(&sink);
    let mut png = PngStruct::new_write(Box::new(move |data: &[u8]| {
        writer.borrow_mut().extend_from_slice(data);
        true
    }));
    let mut info = PngInfo::default();

    // setHeader
    let bits = f.bit_depth;
    let sig = if f.color_type == 0 || f.color_type == 4 {
        PngColor8 {
            gray: bits,
            alpha: if f.color_type == 4 { bits } else { 0 },
            ..PngColor8::default()
        }
    } else {
        PngColor8 {
            red: bits,
            green: bits,
            blue: bits,
            alpha: if f.color_type == 6 { bits } else { 0 },
            ..PngColor8::default()
        }
    };
    png.set_ihdr(
        &mut info,
        w as u32,
        h as u32,
        f.bit_depth,
        f.color_type,
        0,
        0,
        0,
    )?;
    png.set_sbit(&mut info, sig);
    png.set_filter(0, i32::from(filters))?;
    png.set_compression_level(level);
    if extras {
        png.set_text(&mut info, b"Comment", b"skia rust", TextCompression::None);
    }

    // setColorSpace
    if extras {
        if use_icc {
            let profile = gen_profile();
            png.set_iccp(&mut info, "Skia", 0, &profile);
        } else {
            png.set_srgb(&mut info, 0);
        }
    }

    // setHdrMetadata
    if extras {
        png.set_keep_unknown_chunks(
            PNG_HANDLE_CHUNK_ALWAYS,
            &[b"gmAP", b"gdAT", b"mDCV", b"cLLI"],
        );
        let chunk = UnknownChunk {
            name: [b'g', b'm', b'A', b'P', 0],
            data: vec![0, 0, 0, 1],
            location: 0x01, // PNG_HAVE_IHDR
        };
        png.set_unknown_chunks(&mut info, &[chunk])?;
    }

    // writeInfo
    png.write_info(&info)?;
    if f.filler_after {
        png.set_filler(0, true);
    }

    // onEncodeRow
    for y in 0..h {
        let row = &pixels[y * rowbytes_src..(y + 1) * rowbytes_src];
        if info.bit_depth == 16 {
            png.set_swap();
        }
        png.write_rows(&[row])?;
    }

    // onFinishEncoding
    png.write_end(&info)?;
    drop(png);
    let bytes = sink.borrow().clone();
    Ok(bytes)
}

/// Prints every case in the order of pngwdump.c's `main`.
fn dump() -> String {
    let mut out = String::new();
    let mut index = 0;
    for f in &FORMATS {
        for &(w, h) in &SIZES {
            for &filters in &FILTERS {
                for &level in &LEVELS {
                    let extras = level == -1;
                    let head = format!(
                        "case={index} fmt={} w={w} h={h} filter={filters:02x} level={level} extras={}",
                        f.name,
                        i32::from(extras)
                    );
                    match encode(f, w, h, filters, level, extras) {
                        Ok(bytes) => {
                            let _ = writeln!(
                                out,
                                "{head} len={} fnv={:016x}",
                                bytes.len(),
                                fnv1a(&bytes)
                            );
                        }
                        Err(_) => {
                            let _ = writeln!(out, "{head} status=err");
                        }
                    }
                    index += 1;
                }
            }
        }
    }
    out
}

#[test]
fn write_matches_libpng_corpus() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected_path = manifest.join("../../oracle/codec-diff/libpng/expected.txt");
    let expected = fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("{}: {e}", expected_path.display()));
    let got = dump();
    if got != expected {
        let first = match got.lines().zip(expected.lines()).find(|(g, e)| g != e) {
            Some((g, e)) => format!("expected: {e}\n  actual: {g}"),
            None => format!(
                "line counts differ: {} vs {}",
                got.lines().count(),
                expected.lines().count()
            ),
        };
        panic!("libpng write diverges from the C harness; first difference:\n{first}");
    }
}
