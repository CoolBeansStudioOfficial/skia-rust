// Differential check against libjpeg-turbo 3.1.0 (the C library built by oracle/codec-diff/libjpeg).
//
// Each line is `<file>|<case>|<status>|<rows>|<fnv1a-64 of the output bytes>`, the same format as
// `oracle/codec-diff/libjpeg/jpeg_diff.c`. The expected text is committed in
// `tests/expected/libjpeg.txt`, so this test needs no C compiler. Regenerate it after changing a
// case with `JPEG_DIFF_WRITE=1 cargo test -p skia-rust-libjpeg --test diff`.
//
// `JPEG_DIFF_DUMP=<dir>` writes every output's bytes to `<dir>/<file>.<case>.bin`, for `cmp`
// against the dumps of the C harness.

// The harness mirrors jpeg_diff.c line by line (its FNV constants, its C-typed counters, and its
// one-function-per-case run loop), so the casts and long function are kept as the C has them.
#![allow(
    clippy::assigning_clones,
    clippy::case_sensitive_file_extension_comparisons,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::doc_markdown,
    clippy::manual_let_else,
    clippy::redundant_closure_for_method_calls,
    clippy::too_many_lines,
    clippy::unreadable_literal
)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use skia_rust_libjpeg::{
    ColorSpace, ConsumeResult, DctMethod, Decompress, DitherMode, HeaderResult, JpegSource, SrcBuf,
};

/// Skia's memory source: the whole buffer is available at once, and running out is a suspension.
struct MemSource {
    data: Vec<u8>,
}

impl JpegSource for MemSource {
    fn init_source(&mut self, buf: &mut SrcBuf) {
        buf.data = self.data.clone();
        buf.next = 0;
        buf.bytes_in_buffer = self.data.len();
    }

    fn fill_input_buffer(&mut self, buf: &mut SrcBuf) -> bool {
        buf.next = 0;
        buf.bytes_in_buffer = 0;
        false
    }

    fn skip_input_bytes(&mut self, bytes_to_skip: usize, buf: &mut SrcBuf) -> bool {
        if bytes_to_skip > buf.bytes_in_buffer {
            return false;
        }
        buf.next += bytes_to_skip;
        buf.bytes_in_buffer -= bytes_to_skip;
        true
    }
}

const FNV_OFFSET: u64 = 14695981039346656037;

/// FNV-1a 64, the same hash as the C harness.
struct Hasher {
    h: u64,
    dump: Option<Vec<u8>>,
}

impl Hasher {
    fn new(dump: bool) -> Self {
        Hasher {
            h: FNV_OFFSET,
            dump: dump.then(Vec::new),
        }
    }

    fn bytes(&mut self, p: &[u8]) {
        for &b in p {
            self.h ^= u64::from(b);
            self.h = self.h.wrapping_mul(1099511628211);
        }
        if let Some(d) = self.dump.as_mut() {
            d.extend_from_slice(p);
        }
    }
}

#[derive(Clone, Copy)]
struct CaseDef {
    name: &'static str,
    scale_num: u32,
    out_cs: Option<ColorSpace>,
    raw: bool,
}

const CASES: &[CaseDef] = &[
    CaseDef {
        name: "rgba/s1",
        scale_num: 1,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s2",
        scale_num: 2,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s3",
        scale_num: 3,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s4",
        scale_num: 4,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s5",
        scale_num: 5,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s6",
        scale_num: 6,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s7",
        scale_num: 7,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "rgba/s8",
        scale_num: 8,
        out_cs: Some(ColorSpace::ExtRgba),
        raw: false,
    },
    CaseDef {
        name: "bgra/s8",
        scale_num: 8,
        out_cs: Some(ColorSpace::ExtBgra),
        raw: false,
    },
    CaseDef {
        name: "rgb/s8",
        scale_num: 8,
        out_cs: Some(ColorSpace::Rgb),
        raw: false,
    },
    CaseDef {
        name: "gray/s8",
        scale_num: 8,
        out_cs: Some(ColorSpace::Grayscale),
        raw: false,
    },
    CaseDef {
        name: "cmyk/s8",
        scale_num: 8,
        out_cs: Some(ColorSpace::Cmyk),
        raw: false,
    },
    CaseDef {
        name: "rgb565/s8",
        scale_num: 8,
        out_cs: Some(ColorSpace::Rgb565),
        raw: false,
    },
    CaseDef {
        name: "raw/s8",
        scale_num: 8,
        out_cs: None,
        raw: true,
    },
];

/// Result of one decode: `status`, the rows produced, and the hash.
struct Outcome {
    status: &'static str,
    rows: u32,
    hash: u64,
    dump: Option<Vec<u8>>,
}

/// Mirrors `run_case` in jpeg_diff.c, including which fields a failure reports.
fn run_case(data: &[u8], case: &CaseDef, want_dump: bool) -> Outcome {
    let mut hs = Hasher::new(want_dump);
    let fail = |hs: &Hasher, status: &'static str| Outcome {
        status,
        rows: 0,
        hash: hs.h,
        dump: hs.dump.clone(),
    };
    let mut d = Decompress::new(Box::new(MemSource {
        data: data.to_vec(),
    }));
    match d.read_header(true) {
        Err(_) => return fail(&hs, "err"),
        Ok(HeaderResult::Suspended) => return fail(&hs, "suspended"),
        Ok(_) => {}
    }
    // Arithmetic coding is not decoded; progressive images go through the buffered-image path
    // Skia uses, and the raw YUV path skips them.
    if d.arith_code || (d.progressive_mode && case.raw) {
        return fail(&hs, "skip");
    }
    d.scale_num = case.scale_num;
    d.scale_denom = 8;
    if case.raw {
        d.raw_data_out = true;
    } else if let Some(cs) = case.out_cs {
        d.out_color_space = cs;
        // SkJpegCodec.cpp#L320-L330: RGB565 output is decoded with JDITHER_NONE.
        if cs == ColorSpace::Rgb565 {
            d.dither_mode = DitherMode::None;
        }
    }
    if d.progressive_mode {
        d.buffered_image = true;
    }
    let started = match d.start_decompress() {
        Err(_) => return fail(&hs, "err"),
        Ok(s) => s,
    };
    if !started {
        return fail(&hs, "suspended");
    }
    if d.progressive_mode {
        // SkJpegCodec.cpp#L508-L540: keep consuming input until it stops, then output the last
        // complete scan.
        let mut last_scan = 0i32;
        while !d.input_complete() {
            match d.consume_input() {
                Err(_) => return fail(&hs, "err"),
                Ok(ConsumeResult::Suspended) => break,
                Ok(ConsumeResult::ScanCompleted) => last_scan = d.input_scan_number,
                Ok(_) => {}
            }
        }
        if last_scan == 0 {
            return fail(&hs, "suspended");
        }
        if d.start_output(last_scan).is_err() {
            return fail(&hs, "err");
        }
    }
    let mut total_rows: u32 = 0;
    if case.raw {
        // Planes: per component, v_samp * DCTSIZE rows of width_in_blocks * DCTSIZE samples.
        let mut planes: Vec<Vec<Vec<u8>>> = d
            .comp_info
            .iter()
            .map(|c| {
                let lines = (c.v_samp_factor * 8) as usize;
                let width = c.width_in_blocks as usize * 8;
                vec![vec![0u8; width]; lines]
            })
            .collect();
        let nrows: Vec<usize> = planes.iter().map(Vec::len).collect();
        loop {
            let got = match d.read_raw_data(&mut planes) {
                Err(_) => return fail_partial(&hs, total_rows, "err"),
                Ok(g) => g,
            };
            if got == 0 {
                break;
            }
            for (ci, plane) in planes.iter().enumerate() {
                for row in plane.iter().take(nrows[ci]) {
                    hs.bytes(row);
                }
            }
            total_rows += got as u32;
        }
        let status = if d.output_scanline() >= d.output_height() {
            match d.finish_decompress() {
                // C records the rows before finishing, so a failure here keeps them.
                Err(_) => {
                    return Outcome {
                        status: "err",
                        rows: total_rows,
                        hash: hs.h,
                        dump: hs.dump,
                    };
                }
                Ok(_) => "ok",
            }
        } else {
            "partial"
        };
        return Outcome {
            status,
            rows: total_rows,
            hash: hs.h,
            dump: hs.dump,
        };
    }
    // RGB565 rows are two bytes per pixel, whatever `output_components` says.
    let width = if case.out_cs == Some(ColorSpace::Rgb565) {
        d.output_width as usize * 2
    } else {
        d.output_width as usize * d.output_components as usize
    };
    let mut row = vec![0u8; width];
    while d.output_scanline() < d.output_height() {
        let got = {
            let mut rows: [&mut [u8]; 1] = [row.as_mut_slice()];
            match d.read_scanlines(&mut rows) {
                Err(_) => return fail_partial(&hs, 0, "err"),
                Ok(g) => g,
            }
        };
        if got == 0 {
            break;
        }
        hs.bytes(&row);
        total_rows += got as u32;
    }
    let status = if d.output_scanline() == d.output_height() {
        if d.progressive_mode {
            // The return value is ignored, as in the C harness.
            let _ = d.finish_output();
        }
        match d.finish_decompress() {
            Err(_) => {
                return Outcome {
                    status: "err",
                    rows: total_rows,
                    hash: hs.h,
                    dump: hs.dump,
                };
            }
            Ok(_) => "ok",
        }
    } else {
        "partial"
    };
    Outcome {
        status,
        rows: total_rows,
        hash: hs.h,
        dump: hs.dump,
    }
}

/// An error after some output: libjpeg's longjmp leaves `rows` at 0 and keeps the hash.
fn fail_partial(hs: &Hasher, _rows: u32, status: &'static str) -> Outcome {
    Outcome {
        status,
        rows: 0,
        hash: hs.h,
        dump: hs.dump.clone(),
    }
}

/// The JPEG files of Skia's `resources/images`, sorted by name.
fn jpeg_files() -> Vec<(String, PathBuf)> {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../third_party/skia/resources/images");
    let mut out: Vec<(String, PathBuf)> = std::fs::read_dir(&dir)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", dir.display()))
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".jpg") || n.ends_with(".jpeg"))
        .map(|n| (n.clone(), dir.join(n)))
        .collect();
    out.sort_by(|a, b| a.0.as_bytes().cmp(b.0.as_bytes()));
    out
}

/// Produces the report, in the same order as `jpeg_diff.c`.
fn report() -> String {
    let dump_dir = std::env::var_os("JPEG_DIFF_DUMP").map(PathBuf::from);
    let mut s = String::new();
    let only = std::env::var("JPEG_DIFF_ONLY").ok();
    for (name, path) in jpeg_files() {
        if only.as_ref().is_some_and(|o| !name.contains(o.as_str())) {
            continue;
        }
        let data =
            std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        for case in CASES {
            let o = run_case(&data, case, dump_dir.is_some());
            if let (Some(dir), Some(bytes)) = (dump_dir.as_ref(), o.dump.as_ref()) {
                let cn = case.name.replace('/', "_");
                std::fs::write(dir.join(format!("{name}.{cn}.bin")), bytes).expect("dump write");
            }
            let _ = writeln!(
                s,
                "{name}|{}|{}|{}|{:016x}",
                case.name, o.status, o.rows, o.hash
            );
        }
        // Truncations: 1/16 .. 15/16 of the file, decoded as rgba/s8.
        let rgba8 = CASES[7];
        for k in 1..=15usize {
            let tl = data.len() * k / 16;
            let o = run_case(&data[..tl], &rgba8, false);
            let _ = writeln!(
                s,
                "{name}#trunc{k}|{}|{}|{}|{:016x}",
                rgba8.name, o.status, o.rows, o.hash
            );
        }
        // Corruptions: one byte XOR 0x5A at len * k / 8.
        for k in 1..=7usize {
            let mut copy = data.clone();
            let pos = data.len() * k / 8;
            copy[pos] ^= 0x5A;
            let o = run_case(&copy, &rgba8, false);
            let _ = writeln!(
                s,
                "{name}#flip{k}|{}|{}|{}|{:016x}",
                rgba8.name, o.status, o.rows, o.hash
            );
        }
    }
    s
}

#[test]
fn matches_libjpeg_oracle() {
    let got = report();
    if let Some(out) = std::env::var_os("JPEG_DIFF_OUT") {
        std::fs::write(out, &got).expect("write JPEG_DIFF_OUT");
    }
    if std::env::var_os("JPEG_DIFF_ONLY").is_some() {
        // A filtered run is a debugging aid: print the report, compare nothing.
        println!("{got}");
        return;
    }
    let expected_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/expected/libjpeg.txt");
    if std::env::var_os("JPEG_DIFF_WRITE").is_some() {
        std::fs::create_dir_all(expected_path.parent().expect("parent")).expect("mkdir");
        std::fs::write(&expected_path, &got).expect("write expected");
        return;
    }
    let expected = std::fs::read_to_string(&expected_path)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", expected_path.display()));
    let mut diffs = Vec::new();
    for (i, (g, e)) in got.lines().zip(expected.lines()).enumerate() {
        if g != e {
            diffs.push(format!("line {}: port `{g}` vs C `{e}`", i + 1));
        }
    }
    if got.lines().count() != expected.lines().count() {
        diffs.push(format!(
            "line count: port {} vs C {}",
            got.lines().count(),
            expected.lines().count()
        ));
    }
    assert!(
        diffs.is_empty(),
        "{} differing lines, first:\n{}",
        diffs.len(),
        diffs
            .iter()
            .take(20)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn baseline_decode_matches_header_geometry() {
    // A tiny sanity check that does not need the C oracle: the dimensions of a known file.
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../third_party/skia/resources/images/color_wheel.jpg");
    let data = std::fs::read(path).expect("color_wheel.jpg");
    let mut d = Decompress::new(Box::new(MemSource { data }));
    assert_eq!(d.read_header(true).expect("header"), HeaderResult::Ok);
    assert!(d.image_width > 0 && d.image_height > 0);
    assert_eq!(d.dct_method(), DctMethod::IsLow);
}
