// Differential check of the compressor against libjpeg-turbo 3.1.0 (the C library built by
// oracle/codec-diff/libjpeg/build.sh from `encode_diff.c`).
//
// Each line is `<case>|<bytes>|<fnv1a-64 of the JPEG bytes>`, the format of `encode_diff.c`. The
// corpus, case names and pixel generator are the same as in that file, so the port encodes the
// same images with the same calls (Skia's `SkJpegEncoderImpl` sequence). The expected text is
// committed in `tests/expected/encode.txt`; regenerate it after changing the corpus with
// `ENCODE_DIFF_WRITE=1 cargo test -p skia-rust-libjpeg --test encode_diff`.
//
// The byte count and FNV-1a hash must match exactly: there is no tolerance.

// The corpus mirrors encode_diff.c line by line: the same integer casts (uint32_t arithmetic is
// `wrapping`), and the same case table.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::too_many_arguments,
    clippy::doc_markdown,
    clippy::manual_let_else,
    clippy::unreadable_literal,
    clippy::too_many_lines,
    clippy::needless_range_loop
)]

use std::fmt::Write as _;
use std::path::PathBuf;

use skia_rust_libjpeg::{ColorSpace, Compress};

/// One pixel format of the corpus: its colour space and the components per pixel.
#[derive(Clone, Copy)]
struct Format {
    name: &'static str,
    in_cs: ColorSpace,
    nc: usize,
}

const FORMATS: [Format; 6] = [
    Format {
        name: "rgba",
        in_cs: ColorSpace::ExtRgba,
        nc: 4,
    },
    Format {
        name: "bgra",
        in_cs: ColorSpace::ExtBgra,
        nc: 4,
    },
    Format {
        name: "rgbx",
        in_cs: ColorSpace::ExtRgbx,
        nc: 4,
    },
    Format {
        name: "rgb",
        in_cs: ColorSpace::ExtRgb,
        nc: 3,
    },
    Format {
        name: "ycc",
        in_cs: ColorSpace::YCbCr,
        nc: 3,
    },
    Format {
        name: "gray",
        in_cs: ColorSpace::Grayscale,
        nc: 1,
    },
];

/// One case: the parameters of `encode_diff.c`'s `case_t`.
struct Case {
    name: String,
    fmt: Format,
    w: u32,
    h: u32,
    quality: i32,
    samp: i32,
    smoothing: i32,
    markers: bool,
    chunk: u32,
    kind: u32,
    seed: u32,
}

/// `mix` in encode_diff.c.
fn mix(mut h: u32) -> u32 {
    h ^= h >> 15;
    h = h.wrapping_mul(0x2C1B_3C6D);
    h ^= h >> 12;
    h = h.wrapping_mul(0x297A_2D39);
    h ^= h >> 15;
    h
}

/// `pixel` in encode_diff.c.
fn pixel(x: u32, y: u32, c: u32, seed: u32, kind: u32) -> u8 {
    let mut h = x.wrapping_mul(0x9E37_79B1)
        ^ y.wrapping_mul(0x85EB_CA77)
        ^ c.wrapping_mul(0xC2B2_AE3D)
        ^ seed.wrapping_mul(0x27D4_EB2F);
    h = mix(h);
    if kind == 1 {
        return (h & 255) as u8;
    }
    let grad = x
        .wrapping_mul(5)
        .wrapping_add(y.wrapping_mul(3))
        .wrapping_add(c.wrapping_mul(40))
        & 255;
    ((grad + (h & 31)) & 255) as u8
}

/// `add_case` in encode_diff.c, with the name format and the seed (the index plus one).
fn case(
    out: &mut Vec<Case>,
    fmt: Format,
    (w, h): (u32, u32),
    quality: i32,
    samp: i32,
    smoothing: i32,
    markers: bool,
    chunk: u32,
    kind: u32,
) {
    let name = format!(
        "{}/{}x{}/q{}/s{}/sm{}/m{}/c{}/k{}",
        fmt.name,
        w,
        h,
        quality,
        samp,
        smoothing,
        u32::from(markers),
        chunk,
        kind
    );
    let seed = out.len() as u32 + 1;
    out.push(Case {
        name,
        fmt,
        w,
        h,
        quality,
        samp,
        smoothing,
        markers,
        chunk,
        kind,
        seed,
    });
}

/// `build_cases` in encode_diff.c.
fn build_cases() -> Vec<Case> {
    const SIZES: [(u32, u32); 8] = [
        (1, 1),
        (3, 2),
        (8, 8),
        (9, 7),
        (16, 17),
        (33, 31),
        (64, 48),
        (101, 67),
    ];
    const QS: [i32; 3] = [1, 75, 100];
    const SAMPS: [i32; 3] = [420, 422, 444];
    let mut cs = Vec::new();
    for &size in &SIZES {
        for &fmt in &FORMATS {
            let ns = if fmt.in_cs == ColorSpace::Grayscale {
                1
            } else {
                3
            };
            for k in 0..ns {
                for &q in &QS {
                    let samp = if fmt.in_cs == ColorSpace::Grayscale {
                        0
                    } else {
                        SAMPS[k]
                    };
                    case(&mut cs, fmt, size, q, samp, 0, false, 0, 0);
                }
            }
        }
    }
    case(&mut cs, FORMATS[0], (33, 31), 75, 420, 0, true, 0, 0);
    case(&mut cs, FORMATS[4], (33, 31), 75, 420, 0, true, 0, 0);
    case(&mut cs, FORMATS[5], (33, 31), 90, 0, 0, true, 0, 0);
    case(&mut cs, FORMATS[0], (64, 48), 75, 420, 0, false, 3, 0);
    case(&mut cs, FORMATS[4], (64, 48), 75, 422, 0, false, 1, 0);
    case(&mut cs, FORMATS[1], (101, 67), 100, 444, 0, false, 7, 0);
    case(&mut cs, FORMATS[4], (16, 17), 75, 420, 40, false, 0, 0);
    case(&mut cs, FORMATS[4], (33, 31), 75, 444, 40, false, 0, 0);
    case(&mut cs, FORMATS[0], (33, 31), 75, 422, 100, false, 0, 0);
    case(&mut cs, FORMATS[4], (64, 48), 60, 420, 100, false, 0, 0);
    case(&mut cs, FORMATS[5], (64, 48), 75, 0, 40, false, 0, 0);
    case(&mut cs, FORMATS[0], (300, 200), 100, 420, 0, false, 0, 1);
    case(&mut cs, FORMATS[4], (300, 200), 1, 420, 0, false, 0, 1);
    case(&mut cs, FORMATS[5], (300, 200), 75, 0, 0, false, 0, 1);
    case(&mut cs, FORMATS[2], (300, 200), 100, 444, 0, false, 0, 1);
    cs
}

/// The metadata segments of encode_diff.c: (marker, body).
fn marker_segments() -> Vec<(u8, Vec<u8>)> {
    let icc: Vec<u8> = {
        let mut v = b"ACSP".to_vec();
        v.extend(0u8..36);
        v
    };
    let xmp: Vec<u8> = {
        let mut v = b"http://ns.adobe.com/xap/1.0/".to_vec();
        v.push(0);
        v.extend(b"xmpdata");
        v.extend(0u8..24);
        v
    };
    let exif: Vec<u8> = vec![
        b'E', b'x', b'i', b'f', 0, 0, b'M', b'M', 0, 42, 0, 0, 0, 8, 0, 1, 1, 0, 0, 3, 0, 0, 0, 1,
        0, 1, 0, 0, 0, 0,
    ];
    vec![(0xE2, icc), (0xE1, xmp), (0xE1, exif)]
}

/// The pixel bytes of one case, row by row.
fn image_rows(c: &Case) -> Vec<Vec<u8>> {
    let nc = c.fmt.nc;
    (0..c.h)
        .map(|y| {
            let mut row = vec![0u8; c.w as usize * nc];
            for x in 0..c.w {
                for cc in 0..nc {
                    row[x as usize * nc + cc] = pixel(x, y, cc as u32, c.seed, c.kind);
                }
            }
            row
        })
        .collect()
}

/// Encodes one case with Skia's sequence and returns the JPEG bytes.
fn encode(c: &Case) -> Result<Vec<u8>, skia_rust_libjpeg::Error> {
    let mut cinfo = Compress::new();
    cinfo.set_image(c.w, c.h, c.fmt.in_cs, c.fmt.nc as i32);
    cinfo.set_defaults()?;
    match c.samp {
        420 => cinfo.set_component_sampling(0, 2, 2)?,
        422 => cinfo.set_component_sampling(0, 2, 1)?,
        444 => cinfo.set_component_sampling(0, 1, 1)?,
        _ => {}
    }
    cinfo.set_smoothing_factor(c.smoothing)?;
    cinfo.set_optimize_coding(true)?;
    cinfo.set_quality(c.quality, true)?;
    cinfo.start_compress(true)?;
    if c.markers {
        for (marker, body) in marker_segments() {
            cinfo.write_marker(marker, &body)?;
        }
    }
    let rows = image_rows(c);
    let chunk = if c.chunk == 0 {
        c.h as usize
    } else {
        c.chunk as usize
    };
    let mut done = 0usize;
    while done < rows.len() {
        let n = (rows.len() - done).min(chunk);
        let refs: Vec<&[u8]> = rows[done..done + n].iter().map(Vec::as_slice).collect();
        let taken = cinfo.write_scanlines(&refs)?;
        assert_eq!(taken, n);
        done += n;
    }
    cinfo.finish_compress()?;
    Ok(cinfo.take_output())
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The text of every case, in encode_diff.c's order.
fn render() -> String {
    let mut out = String::new();
    for c in build_cases() {
        match encode(&c) {
            Ok(bytes) => {
                let _ = writeln!(out, "{}|{}|{:016x}", c.name, bytes.len(), fnv1a(&bytes));
            }
            Err(_) => {
                let _ = writeln!(out, "{}|err|0", c.name);
            }
        }
    }
    out
}

#[test]
fn encode_matches_libjpeg_turbo() {
    let text = render();
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/expected/encode.txt");
    if std::env::var_os("ENCODE_DIFF_WRITE").is_some() {
        std::fs::write(&path, &text).expect("write expected/encode.txt");
        return;
    }
    let expected = std::fs::read_to_string(&path).expect("read expected/encode.txt");
    assert_eq!(text.lines().count(), expected.lines().count(), "case count");
    for (got, want) in text.lines().zip(expected.lines()) {
        assert_eq!(got, want);
    }
}
