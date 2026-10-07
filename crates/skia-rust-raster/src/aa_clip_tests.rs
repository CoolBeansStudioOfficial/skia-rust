// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// Oracle tests of `AAClip`, `RasterClip`, `AAClipBlitter` and the scan converters' raster clip
// overloads. `oracle/aaclip` runs the case script `aa_clip_tests/cases.txt` (format: the header of
// `oracle/aaclip/aaclip.cpp`) through real Skia (chrome/m156) and stores its output in
// `aa_clip_tests/skia_dump.txt`. These tests run the same script through skia-rust and compare
// the output, exactly.

// The driver converts small values from the script text; wrapping casts and short names are fine here.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::many_single_char_names
)]

use std::fmt::Write as _;
use std::panic::{AssertUnwindSafe, catch_unwind};

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::mask::{Mask, MaskBuilder, MaskFormat};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Iterator as RegionIterator, Op, Region};
use skia_rust_core::rrect::RRect;

use crate::aa_clip::{AAClip, AAClipBlitter};
use crate::blitter::Blitter;
use crate::blitter_dump::DumpBlitter;
use crate::raster_clip::RasterClip;
use crate::scan;
use crate::scan_anti_path::anti_fill_path_clip;
use crate::scan_antihair::{
    anti_fill_rect_clip, anti_fill_x_rect_clip, anti_frame_rect_clip, anti_hair_line,
    anti_hair_rect,
};
use crate::scan_hairline::{
    anti_hair_path, anti_hair_round_path, anti_hair_square_path, frame_rect, hair_line, hair_path,
    hair_rect, hair_round_path, hair_square_path,
};

const CASES: &str = include_str!("aa_clip_tests/cases.txt");
const SKIA_DUMP: &str = include_str!("aa_clip_tests/skia_dump.txt");

fn f(s: &str) -> f32 {
    s.parse::<f32>().expect("float")
}

fn i(s: &str) -> i32 {
    s.parse::<i32>().expect("int")
}

fn irect(w: &[&str]) -> IRect {
    IRect::new(i(w[0]), i(w[1]), i(w[2]), i(w[3]))
}

fn frect(w: &[&str]) -> Rect {
    Rect::new(f(w[0]), f(w[1]), f(w[2]), f(w[3]))
}

fn clip_op(s: &str) -> ClipOp {
    if s == "diff" {
        ClipOp::Difference
    } else {
        ClipOp::Intersect
    }
}

/// `<union|xor> l t r b [+ l t r b ...]`: the region and the number of tokens used.
fn spec(w: &[&str]) -> (Region, usize) {
    let op = if w[0] == "xor" { Op::XOR } else { Op::Union };
    let mut rgn = Region::new();
    let mut n = 1;
    loop {
        rgn.op_rect(irect(&w[n..n + 4]), op);
        n += 4;
        if n < w.len() && w[n] == "+" {
            n += 1;
        } else {
            break;
        }
    }
    (rgn, n)
}

fn matrix(w: &[&str]) -> (Matrix, usize) {
    if w[0] == "I" {
        return (Matrix::new_identity(), 1);
    }
    let v: Vec<f32> = w[1..10].iter().map(|s| f(s)).collect();
    (
        Matrix::new_all(v[0], v[1], v[2], v[3], v[4], v[5], v[6], v[7], v[8]),
        10,
    )
}

fn print_rows(out: &mut String, img: &[u8], b: &IRect, row_bytes: usize) {
    let w = usize::try_from(i64::from(b.right) - i64::from(b.left)).expect("width");
    let h = i64::from(b.bottom) - i64::from(b.top);
    for y in 0..usize::try_from(h).expect("height") {
        let _ = write!(out, "row {}:", i64::from(b.top) + y as i64);
        for x in 0..w {
            let _ = write!(out, " {:02x}", img[y * row_bytes + x]);
        }
        out.push('\n');
    }
}

fn print_aa_mask(out: &mut String, aa: &AAClip) {
    if aa.is_empty() {
        return;
    }
    let b = aa.bounds();
    let area = (i64::from(b.right) - i64::from(b.left)) * (i64::from(b.bottom) - i64::from(b.top));
    if area > 65536 {
        out.push_str("mask skipped\n");
        return;
    }
    let mask = aa.copy_to_mask();
    print_rows(out, &mask.image, &mask.bounds, mask.row_bytes as usize);
}

fn print_aa(out: &mut String, aa: &AAClip, ret: bool) {
    let _ = writeln!(out, "ret {}", i32::from(ret));
    let b = aa.bounds();
    let _ = writeln!(
        out,
        "clip {} {} {} {} {} {}",
        i32::from(aa.is_empty()),
        i32::from(aa.is_rect()),
        b.left,
        b.top,
        b.right,
        b.bottom
    );
    print_aa_mask(out, aa);
}

fn print_rc(out: &mut String, rc: &RasterClip, ret: bool) {
    let _ = writeln!(out, "ret {}", i32::from(ret));
    let b = *rc.bounds();
    let _ = writeln!(
        out,
        "rc {} {} {} {} {} {} {} {}",
        i32::from(rc.is_bw()),
        i32::from(rc.is_empty()),
        i32::from(rc.is_rect()),
        i32::from(rc.is_complex()),
        b.left,
        b.top,
        b.right,
        b.bottom
    );
    if rc.is_empty() {
        return;
    }
    if !rc.is_bw() {
        print_aa_mask(out, rc.aa_rgn());
        return;
    }
    let w = i64::from(b.right) - i64::from(b.left);
    let h = i64::from(b.bottom) - i64::from(b.top);
    if w * h > 65536 {
        out.push_str("mask skipped\n");
        return;
    }
    let w = usize::try_from(w).expect("width");
    let mut img = vec![0u8; w * usize::try_from(h).expect("height")];
    let mut it = RegionIterator::new(rc.bw_rgn());
    while !it.is_done() {
        let r = *it.rect();
        for y in r.top..r.bottom {
            for x in r.left..r.right {
                img[(y - b.top) as usize * w + (x - b.left) as usize] = 0xFF;
            }
        }
        it.next();
    }
    print_rows(out, &img, &b, w);
}

fn slot<'a>(slots: &'a mut [AAClip; 2], name: &str) -> &'a mut AAClip {
    &mut slots[usize::from(name == "aa2")]
}

fn blit(out: &mut String, aa: &AAClip, w: &[&str]) {
    if aa.is_empty() {
        out.push_str("skip empty\n");
        return;
    }
    let mut dump = DumpBlitter::new();
    {
        let mut blitter = AAClipBlitter::new(&mut dump, aa);
        match w[1] {
            "h" => blitter.blit_h(i(w[2]), i(w[3]), i(w[4])),
            "antih" => {
                let (x, y) = (i(w[2]), i(w[3]));
                let mut alphas = Vec::new();
                let mut counts = Vec::new();
                let mut total = 0;
                for tok in &w[4..] {
                    let (a, n) = tok.split_once(':').expect("alpha:run");
                    alphas.push(u8::try_from(i(a)).expect("alpha"));
                    counts.push(i(n));
                    total += i(n);
                }
                let total = usize::try_from(total).expect("total");
                let mut aa = vec![0u8; total + 1];
                let mut runs = vec![0i16; total + 1];
                let mut pos = 0usize;
                for (a, n) in alphas.iter().zip(&counts) {
                    runs[pos] = i16::try_from(*n).expect("run");
                    aa[pos] = *a;
                    pos += usize::try_from(*n).expect("run");
                }
                runs[total] = 0;
                blitter.blit_anti_h(x, y, &mut aa, &mut runs);
            }
            "v" => blitter.blit_v(
                i(w[2]),
                i(w[3]),
                i(w[4]),
                u8::try_from(i(w[5])).expect("alpha"),
            ),
            "rect" => blitter.blit_rect(i(w[2]), i(w[3]), i(w[4]), i(w[5])),
            "mask" => {
                let a8 = w[2] == "a8";
                let (x, y, mw, mh) = (i(w[3]), i(w[4]), i(w[5]), i(w[6]));
                let clip = irect(&w[7..11]);
                let mut img: Vec<u8> = w[11..]
                    .iter()
                    .map(|t| u8::from_str_radix(t, 16).expect("hex"))
                    .collect();
                let row_bytes = if a8 { mw } else { (mw + 7) / 8 };
                img.resize((row_bytes * mh) as usize, 0);
                let mask = Mask::new(
                    &img,
                    IRect::new(x, y, x + mw, y + mh),
                    row_bytes as u32,
                    if a8 { MaskFormat::A8 } else { MaskFormat::BW },
                );
                blitter.blit_mask(&mask, &clip);
            }
            other => panic!("bad blit command {other}"),
        }
    }
    out.push_str(&dump.oracle_text());
}

fn scan_cmd(out: &mut String, rc: &RasterClip, path: &Path, w: &[&str]) {
    let mut d = DumpBlitter::new();
    let raw: PathRaw<'_> = path_priv::raw(path, ResolveConvexity::Yes)
        .unwrap_or_else(|| PathRaw::empty(path.fill_type()));
    match w[1] {
        "fillpath" => scan::fill_path_clip(&raw, rc, &mut d),
        "antifillpath" => anti_fill_path_clip(&raw, rc, &mut d),
        "fillirect" => scan::fill_irect_clip(&irect(&w[2..]), rc, &mut d),
        "fillxrect" => scan::fill_xrect_clip(&irect(&w[2..]), rc, &mut d),
        "fillrect" => scan::fill_rect_clip(&frect(&w[2..]), rc, &mut d),
        "antifillrect" => anti_fill_rect_clip(&frect(&w[2..]), rc, &mut d),
        "antifillxrect" => anti_fill_x_rect_clip(&irect(&w[2..]), rc, &mut d),
        "filltriangle" => {
            let pts = [
                Point::new(f(w[2]), f(w[3])),
                Point::new(f(w[4]), f(w[5])),
                Point::new(f(w[6]), f(w[7])),
            ];
            scan::fill_triangle(&pts, rc, &mut d);
        }
        "hairline" | "antihairline" => {
            let pts: Vec<Point> = w[2..]
                .chunks(2)
                .map(|c| Point::new(f(c[0]), f(c[1])))
                .collect();
            if w[1] == "hairline" {
                hair_line(&pts, rc, &mut d);
            } else {
                anti_hair_line(&pts, rc, &mut d);
            }
        }
        "hairrect" => hair_rect(&frect(&w[2..]), rc, &mut d),
        "antihairrect" => anti_hair_rect(&frect(&w[2..]), rc, &mut d),
        "hairpath" => hair_path(&raw, rc, &mut d),
        "antihairpath" => anti_hair_path(&raw, rc, &mut d),
        "hairsquarepath" => hair_square_path(&raw, rc, &mut d),
        "antihairsquarepath" => anti_hair_square_path(&raw, rc, &mut d),
        "hairroundpath" => hair_round_path(&raw, rc, &mut d),
        "antihairroundpath" => anti_hair_round_path(&raw, rc, &mut d),
        "framerect" => frame_rect(&frect(&w[2..]), &Point::new(f(w[6]), f(w[7])), rc, &mut d),
        "antiframerect" => {
            anti_frame_rect_clip(&frect(&w[2..]), &Point::new(f(w[6]), f(w[7])), rc, &mut d);
        }
        other => panic!("bad scan command {other}"),
    }
    out.push_str(&d.oracle_text());
}

/// Runs one case and returns its output.
#[allow(clippy::too_many_lines)] // one arm per directive of the script
fn run_case(lines: &[&str]) -> String {
    let mut out = String::new();
    let mut slots = [AAClip::new(), AAClip::new()];
    let mut rc = RasterClip::new();
    let mut builder = PathBuilder::new();
    for line in lines {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w[0] {
            "path" => {
                builder.reset();
                builder.set_fill_type(match w[1] {
                    "winding" => PathFillType::Winding,
                    "evenodd" => PathFillType::EvenOdd,
                    "invwinding" => PathFillType::InverseWinding,
                    _ => PathFillType::InverseEvenOdd,
                });
            }
            "M" => {
                builder.move_to((f(w[1]), f(w[2])));
            }
            "L" => {
                builder.line_to((f(w[1]), f(w[2])));
            }
            "Q" => {
                builder.quad_to((f(w[1]), f(w[2])), (f(w[3]), f(w[4])));
            }
            "C" => {
                builder.cubic_to((f(w[1]), f(w[2])), (f(w[3]), f(w[4])), (f(w[5]), f(w[6])));
            }
            "K" => {
                builder.conic_to((f(w[1]), f(w[2])), (f(w[3]), f(w[4])), f(w[5]));
            }
            "Z" => {
                builder.close();
            }
            "circle" | "circleccw" => {
                let dir = if w[0] == "circle" {
                    PathDirection::CW
                } else {
                    PathDirection::CCW
                };
                builder.add_circle((f(w[1]), f(w[2])), f(w[3]), dir);
            }
            "oval" => {
                builder.add_oval(frect(&w[1..]), None, None);
            }
            "rrect" => {
                let r = frect(&w[1..]);
                builder.add_rrect(RRect::new_rect_xy(r, f(w[5]), f(w[6])), None, None);
            }
            "rect" => {
                builder.add_rect(frect(&w[1..]), None, None);
            }
            "aa" | "aa2" => {
                if w[1] == "qc" {
                    let c = slot(&mut slots, w[0]);
                    let _ = writeln!(out, "qc {}", i32::from(c.quick_contains(&irect(&w[2..]))));
                    continue;
                }
                if w[1] == "opaa" || w[1] == "copy" || w[1] == "translateto" {
                    let other_name = w[2];
                    let this_name = w[0];
                    match w[1] {
                        "opaa" => {
                            let other = slot(&mut slots, other_name).clone();
                            let c = slot(&mut slots, this_name);
                            let ret = c.op_aa_clip(&other, clip_op(w[3]));
                            print_aa(&mut out, c, ret);
                        }
                        "copy" => {
                            let other = slot(&mut slots, other_name).clone();
                            let c = slot(&mut slots, this_name);
                            *c = other;
                            print_aa(&mut out, c, true);
                        }
                        _ => {
                            let this = slot(&mut slots, this_name).clone();
                            let o = slot(&mut slots, other_name);
                            let ret = this.translate(i(w[3]), i(w[4]), o);
                            print_aa(&mut out, o, ret);
                        }
                    }
                    continue;
                }
                let c = slot(&mut slots, w[0]);
                let ret = match w[1] {
                    "empty" => c.set_empty(),
                    "rect" => c.set_rect(&irect(&w[2..])),
                    "region" => c.set_region(&spec(&w[2..]).0),
                    "path" => c.set_path(&builder.snapshot(), &irect(&w[3..]), w[2] == "aa"),
                    "pathbounds" => {
                        let p = builder.snapshot();
                        let b: IRect = skia_rust_core::rect::RoundOut::round_out(p.bounds());
                        c.set_path(&p, &b, w[2] == "aa")
                    }
                    "opi" => c.op_irect(&irect(&w[2..]), clip_op(w[6])),
                    "opf" => c.op_rect(&frect(&w[2..]), clip_op(w[6]), w[7] == "aa"),
                    "translate" => c.translate_in_place(i(w[2]), i(w[3])),
                    other => panic!("bad aa command {other}"),
                };
                print_aa(&mut out, c, ret);
            }
            "rc" => {
                let mut ret = true;
                match w[1] {
                    "new" => match w[2] {
                        "empty" => rc = RasterClip::new(),
                        "irect" => rc = RasterClip::from_rect(&irect(&w[3..])),
                        "region" => rc = RasterClip::from_region(&spec(&w[3..]).0),
                        _ => {
                            rc = RasterClip::from_path(
                                &builder.snapshot(),
                                &irect(&w[4..]),
                                w[3] == "aa",
                            );
                        }
                    },
                    "setempty" => ret = rc.set_empty(),
                    "setrect" => ret = rc.set_rect(&irect(&w[2..])),
                    "opi" => ret = rc.op_irect(&irect(&w[2..]), clip_op(w[6])),
                    "opregion" => {
                        let (r, n) = spec(&w[2..]);
                        ret = rc.op_region(&r, clip_op(w[2 + n]));
                    }
                    "oprect" => {
                        let r = frect(&w[2..]);
                        let (m, n) = matrix(&w[6..]);
                        ret = rc.op_rect(&r, &m, clip_op(w[6 + n]), w[7 + n] == "aa");
                    }
                    "oprrect" => {
                        let r = frect(&w[2..]);
                        let rr = RRect::new_rect_xy(r, f(w[6]), f(w[7]));
                        let (m, n) = matrix(&w[8..]);
                        ret = rc.op_rrect(&rr, &m, clip_op(w[8 + n]), w[9 + n] == "aa");
                    }
                    "oppath" => {
                        let (m, n) = matrix(&w[2..]);
                        ret = rc.op_path(
                            &builder.snapshot(),
                            &m,
                            clip_op(w[2 + n]),
                            w[3 + n] == "aa",
                        );
                    }
                    "translate" => {
                        let mut tmp = RasterClip::new();
                        rc.translate(i(w[2]), i(w[3]), &mut tmp);
                        rc = tmp;
                    }
                    other => panic!("bad rc command {other}"),
                }
                print_rc(&mut out, &rc, ret);
            }
            "blit" => blit(&mut out, &slots[0], &w),
            "scan" => scan_cmd(&mut out, &rc, &builder.snapshot(), &w),
            other => panic!("unknown directive {other}"),
        }
    }
    out
}

/// The cases of `cases.txt` as (name, directive lines).
fn cases() -> Vec<(&'static str, Vec<&'static str>)> {
    let mut out = Vec::new();
    let mut current: Option<(&str, Vec<&str>)> = None;
    for line in CASES.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(name) = line.strip_prefix("case ") {
            current = Some((name, Vec::new()));
        } else if line == "end" {
            out.push(current.take().expect("case before end"));
        } else {
            current.as_mut().expect("case").1.push(line);
        }
    }
    out
}

/// Skia's output, split per case.
fn skia_dump() -> Vec<(&'static str, String)> {
    let mut out: Vec<(&str, String)> = Vec::new();
    for line in SKIA_DUMP.lines() {
        if let Some(name) = line.strip_prefix("== ") {
            out.push((name, String::new()));
        } else {
            let s = &mut out.last_mut().expect("case header").1;
            s.push_str(line);
            s.push('\n');
        }
    }
    out
}

#[test]
fn aa_clip_matches_skia() {
    let cases = cases();
    let expected = skia_dump();
    assert_eq!(cases.len(), expected.len(), "rerun oracle/aaclip/build.ps1");
    let mut failures = Vec::new();
    for ((name, lines), (skia_name, skia)) in cases.iter().zip(&expected) {
        assert_eq!(name, skia_name, "rerun oracle/aaclip/build.ps1");
        let result = catch_unwind(AssertUnwindSafe(|| run_case(lines)));
        match result {
            Err(_) => failures.push(format!("{name}: panicked")),
            Ok(ours) if ours != *skia => {
                let first = ours
                    .lines()
                    .zip(skia.lines())
                    .position(|(a, b)| a != b)
                    .unwrap_or_else(|| ours.lines().count().min(skia.lines().count()));
                failures.push(format!(
                    "{name}: first difference at output line {first}\n  ours: {:?}\n  skia: {:?}",
                    ours.lines().nth(first),
                    skia.lines().nth(first)
                ));
            }
            Ok(_) => {}
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures
            .iter()
            .take(40)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// A region clip expands to 0xFF inside and 0 outside.
#[test]
fn copy_to_mask_of_a_region_clip() {
    let mut aa = AAClip::new();
    let mut rgn = Region::new();
    rgn.op_rect(IRect::new(0, 0, 2, 2), Op::Union);
    rgn.op_rect(IRect::new(2, 2, 4, 4), Op::Union);
    assert!(aa.set_region(&rgn));
    let m: MaskBuilder = aa.copy_to_mask();
    assert_eq!(m.bounds, IRect::new(0, 0, 4, 4));
    assert_eq!(
        m.image,
        [
            0xFF, 0xFF, 0, 0, 0xFF, 0xFF, 0, 0, 0, 0, 0xFF, 0xFF, 0, 0, 0xFF, 0xFF
        ]
    );
}
