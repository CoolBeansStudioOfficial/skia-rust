// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// Oracle tests of the analytic AA scan converter and the AA rect scan converters. There is no
// upstream unit test for them (Skia checks them through GMs), so `oracle/scan-aaa` runs the cases
// of `scan_aaa_tests/cases.txt` through real Skia (chrome/m156) with a blitter that records every
// call, and stores the calls in `scan_aaa_tests/skia_dump.txt`. These tests run the same cases
// through skia-rust with `DumpBlitter` and compare the calls, exactly.

use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};
use skia_rust_core::rrect::RRect;

use crate::blitter_dump::DumpBlitter;
use crate::scan_anti_path::anti_fill_path_region;
use crate::scan_antihair::{anti_fill_rect, anti_fill_x_rect, anti_frame_rect};

const CASES: &str = include_str!("scan_aaa_tests/cases.txt");
const SKIA_DUMP: &str = include_str!("scan_aaa_tests/skia_dump.txt");

/// Runs one case of `cases.txt` (see `oracle/scan-aaa/scan_aaa.cpp` for the format) and returns
/// its blitter calls.
fn run_case(lines: &[&str]) -> String {
    let mut clip = Region::new();
    let mut no_clip = false;
    let mut op = "";
    let mut builder = PathBuilder::new();
    let mut operands: Vec<Vec<&str>> = Vec::new();
    let f = |s: &str| s.parse::<f32>().expect("float");
    let i = |s: &str| s.parse::<i32>().expect("int");
    for line in lines {
        let w: Vec<&str> = line.split_whitespace().collect();
        match w[0] {
            "clip" => {
                for r in w[1..].split(|s| *s == "+") {
                    clip.op_rect(IRect::new(i(r[0]), i(r[1]), i(r[2]), i(r[3])), Op::Union);
                }
            }
            "noclip" => no_clip = true,
            "fill" => {
                builder.set_fill_type(match w[1] {
                    "winding" => PathFillType::Winding,
                    "evenodd" => PathFillType::EvenOdd,
                    "invwinding" => PathFillType::InverseWinding,
                    _ => PathFillType::InverseEvenOdd,
                });
            }
            "op" => op = w[1],
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
                builder.add_oval(Rect::new(f(w[1]), f(w[2]), f(w[3]), f(w[4])), None, None);
            }
            "rrect" => {
                let r = Rect::new(f(w[1]), f(w[2]), f(w[3]), f(w[4]));
                builder.add_rrect(RRect::new_rect_xy(r, f(w[5]), f(w[6])), None, None);
            }
            "rect" | "xrect" | "frame" => operands.push(w),
            other => panic!("unknown directive {other}"),
        }
    }

    let mut blitter = DumpBlitter::new();
    let rgn = if no_clip { None } else { Some(&clip) };
    match op {
        "path" | "rle" => {
            let path = builder.detach();
            let Some(raw) = path_priv::raw(&path, ResolveConvexity::Yes) else {
                return "(no raw)\n".to_string();
            };
            // `SkScan::AntiFillPath(raw, SkRasterClip(clip), blitter)` for a BW raster clip is
            // `AntiFillPath(raw, clip, blitter, false)`.
            anti_fill_path_region(&raw, &clip, &mut blitter, op == "rle");
        }
        _ => {
            for o in &operands {
                match o[0] {
                    "rect" => {
                        let r = Rect::new(f(o[1]), f(o[2]), f(o[3]), f(o[4]));
                        anti_fill_rect(&r, rgn, &mut blitter);
                    }
                    "xrect" => {
                        let r = IRect::new(i(o[1]), i(o[2]), i(o[3]), i(o[4]));
                        anti_fill_x_rect(&r, rgn, &mut blitter);
                    }
                    _ => {
                        let r = Rect::new(f(o[1]), f(o[2]), f(o[3]), f(o[4]));
                        let stroke = Point::new(f(o[5]), f(o[6]));
                        anti_frame_rect(&r, &stroke, rgn, &mut blitter);
                    }
                }
            }
        }
    }
    blitter.oracle_text()
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
fn scan_aaa_matches_skia() {
    let cases = cases();
    let expected = skia_dump();
    assert_eq!(
        cases.len(),
        expected.len(),
        "rerun oracle/scan-aaa/build.ps1"
    );
    let mut failures = Vec::new();
    for ((name, lines), (skia_name, skia)) in cases.iter().zip(&expected) {
        assert_eq!(name, skia_name, "rerun oracle/scan-aaa/build.ps1");
        let ours = run_case(lines);
        if ours != *skia {
            let first = ours
                .lines()
                .zip(skia.lines())
                .position(|(a, b)| a != b)
                .unwrap_or_else(|| ours.lines().count().min(skia.lines().count()));
            failures.push(format!(
                "{name}: first difference at call line {first}\n  ours: {:?}\n  skia: {:?}",
                ours.lines().nth(first),
                skia.lines().nth(first)
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} cases differ:\n{}",
        failures.len(),
        cases.len(),
        failures.join("\n")
    );
}

// Hand-derived from `antifilldot8` (SkScan_Antihair.cpp): the rect [1.5, 3.5] x [1.5, 2.5] is
// FDot8 L = 384, T = 384, R = 896, B = 640. It spans two scanlines, each half covered
// (`do_scanline` with alpha 128): the half-covered end pixels get SkAlphaMul(128, 128) = 64.
#[test]
fn anti_fill_rect_by_hand() {
    let mut blitter = DumpBlitter::new();
    anti_fill_rect(&Rect::new(1.5, 1.5, 3.5, 2.5), None, &mut blitter);
    assert_eq!(
        blitter.oracle_text(),
        "blitV 1 1 1 64\nblitAntiH 2 1 128:1\nblitV 3 1 1 64\n\
         blitV 1 2 1 64\nblitAntiH 2 2 128:1\nblitV 3 2 1 64\n"
    );
}

// Hand-derived from `AAAFillPath`: a pixel-aligned 2x2 square is a rect, but not a fat one (width
// < 3), so it goes to the mask blitter. Its two vertical edges have dX == 0, so
// `aaa_walk_convex_edges` takes the rectangle branch and blits it with `blitAntiRect(0, 1, 2, 2,
// 0, 0)` into the mask (the zero-alpha sides are skipped); the mask is then blitted once.
#[test]
fn aaa_square_by_hand() {
    let mut builder = PathBuilder::new();
    builder.add_rect(Rect::new(1.0, 1.0, 3.0, 3.0), None, None);
    let path = builder.detach();
    let raw = path_priv::raw(&path, ResolveConvexity::Yes).expect("finite");
    let mut blitter = DumpBlitter::new();
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));
    anti_fill_path_region(&raw, &clip, &mut blitter, false);
    assert_eq!(
        blitter.oracle_text(),
        "blitMask A8 1 1 3 3 clip 1 1 3 3\n row 1: ff ff\n row 2: ff ff\n"
    );
}

// Clipped bounds beyond +-8191 px cannot be supersampled, so `AntiFillPath` falls back to the
// non-AA `FillPath` (`scan::fill_path`): both make the same calls, and they are not empty.
#[test]
fn anti_fill_path_beyond_8191_falls_back_to_fill_path() {
    let mut builder = PathBuilder::new();
    builder.add_rect(Rect::new(1.5, 1.5, 9000.0, 3.5), None, None);
    let path = builder.detach();
    let raw = path_priv::raw(&path, ResolveConvexity::Yes).expect("finite");
    let clip = Region::from_rect(IRect::new(0, 0, 10000, 10));

    let mut aa = DumpBlitter::new();
    anti_fill_path_region(&raw, &clip, &mut aa, false);
    let mut non_aa = DumpBlitter::new();
    crate::scan::fill_path(&raw, &clip, &mut non_aa);
    assert_ne!(non_aa.dump(), "");
    assert_eq!(aa.calls, non_aa.calls);
}
