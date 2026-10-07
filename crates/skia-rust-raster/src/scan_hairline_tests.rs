// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

// Tests for the hairline scan converters (src/core/SkScan_Hairline.cpp and
// src/core/SkScan_Antihair.cpp, chrome/m156). No upstream unit test covers these without a
// canvas (`CappedHairlinesTest` needs the real `Canvas`, task D6), so each expectation is
// derived by hand from the C++ arithmetic; the comments show the derivation (FDot6 = 26.6
// fixed point, `Fixed` = 16.16).

use skia_rust_core::path_enums::PathConvexity;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_types::{PathFillType, PathVerb};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::Region;

use crate::blitter::Blitter;
use crate::blitter_dump::{BlitCall, DumpBlitter};
use crate::scan_antihair::{anti_fill_rect, anti_frame_rect, anti_hair_line_rgn, anti_hair_rect};
use crate::scan_hairline::{
    frame_rect, hair_line, hair_line_rgn, hair_path, hair_rect, hair_round_path, hair_square_path,
};

fn h(x: i32, y: i32, width: i32) -> BlitCall {
    BlitCall::H { x, y, width }
}

fn aa(x: i32, y: i32, runs: &[(i32, u8)]) -> BlitCall {
    BlitCall::AntiH {
        x,
        y,
        runs: runs.to_vec(),
    }
}

fn v(x: i32, y: i32, height: i32, alpha: u8) -> BlitCall {
    BlitCall::V {
        x,
        y,
        height,
        alpha,
    }
}

fn pt(x: f32, y: f32) -> Point {
    Point::new(x, y)
}

fn big_clip() -> Region {
    Region::from_rect(IRect::new(-100, -100, 100, 100))
}

fn line_path(points: &[Point]) -> (Vec<Point>, Vec<PathVerb>) {
    let mut verbs = vec![PathVerb::Move];
    verbs.extend(std::iter::repeat_n(PathVerb::Line, points.len() - 1));
    (points.to_vec(), verbs)
}

fn raw_of<'a>(points: &'a [Point], verbs: &'a [PathVerb]) -> PathRaw<'a> {
    PathRaw {
        points,
        verbs,
        conics: &[],
        bounds: Rect::bounds_or_empty(points),
        fill_type: PathFillType::Winding,
        convexity: PathConvexity::Unknown,
        segment_mask: 0,
    }
}

#[test]
fn dump_format() {
    let mut d = DumpBlitter::new();
    d.blit_h(3, 7, 5);
    let mut a = [128u8, 255, 0, 0, 0, 0];
    let mut runs = [1i16, 4, 0, 0, 0, 0];
    d.blit_anti_h(0, 2, &mut a, &mut runs);
    d.blit_v(1, 0, 3, 64);
    d.blit_rect(2, 2, 3, 1);
    d.blit_anti_h2(1, 1, 2, 3);
    let expected = [
        "blit_h(x=3, y=7, w=5)",
        "blit_anti_h(x=0, y=2, [1x128, 4x255])",
        "blit_v(x=1, y=0, h=3, a=64)",
        "blit_rect(x=2, y=2, w=3, h=1)",
        "blit_anti_h2(x=1, y=1, a0=2, a1=3)",
    ];
    assert_eq!(
        d.dump(),
        expected.join(
            "
"
        ) + "
"
    );
}

// (0.5, 0.5) -> (5.5, 0.5): x0 = 32, x1 = 352, y = 32 (FDot6). Mostly horizontal; ix0 =
// round(32) = 1, ix1 = round(352) = 6; slope = 0; startY = 32 << 10 = 32768, so y = 0.
#[test]
fn hair_line_horizontal() {
    let mut d = DumpBlitter::new();
    hair_line_rgn(&[pt(0.5, 0.5), pt(5.5, 0.5)], None, &mut d);
    assert_eq!(
        d.calls,
        [h(1, 0, 1), h(2, 0, 1), h(3, 0, 1), h(4, 0, 1), h(5, 0, 1)]
    );
}

// (0, 0) -> (4, 4): dx == dy, so "mostly vertical": iy0 = 0, iy1 = round(256) = 4, slope =
// 1.0 (65536), startX = 0 + (65536 * 32 >> 6) = 32768 -> x = 0, then +1 per row.
#[test]
fn hair_line_diagonal() {
    let mut d = DumpBlitter::new();
    hair_line_rgn(&[pt(0.0, 0.0), pt(4.0, 4.0)], None, &mut d);
    assert_eq!(d.calls, [h(0, 0, 1), h(1, 1, 1), h(2, 2, 1), h(3, 3, 1)]);
}

// A line that ends where it started draws nothing; a polyline draws each segment.
#[test]
fn hair_line_degenerate_and_polyline() {
    let mut d = DumpBlitter::new();
    hair_line_rgn(&[pt(2.0, 2.0), pt(2.0, 2.0)], None, &mut d);
    assert_eq!(d.dump(), "");
    hair_line_rgn(&[pt(1.0, 1.0), pt(3.0, 1.0), pt(3.0, 3.0)], None, &mut d);
    // (1,1)->(3,1): ix 1..3 at y = 1 (startY = 64 << 10 -> 1). (3,1)->(3,3): iy 1..3, x = 3.
    assert_eq!(d.calls, [h(1, 1, 1), h(2, 1, 1), h(3, 1, 1), h(3, 2, 1)]);
}

// clip (0,0,3,3): the line (-5, 1.5)->(5, 1.5) is first chopped to (0, 1.5)->(3, 1.5) in scalar
// space; the dot6 rect (0,96,192,96) outset by one pixel is not inside the clip, so the
// segment goes through a RectClipBlitter. ix0 = 0, ix1 = round(192) = 3, y = 96 << 10 >> 16 = 1.
#[test]
fn hair_line_clipped() {
    let clip = Region::from_rect(IRect::new(0, 0, 3, 3));
    let mut d = DumpBlitter::new();
    hair_line_rgn(&[pt(-5.0, 1.5), pt(5.0, 1.5)], Some(&clip), &mut d);
    assert_eq!(d.calls, [h(0, 1, 1), h(1, 1, 1), h(2, 1, 1)]);

    // A line entirely outside draws nothing.
    let mut d = DumpBlitter::new();
    hair_line_rgn(&[pt(-5.0, 7.5), pt(5.0, 7.5)], Some(&clip), &mut d);
    assert_eq!(d.dump(), "");
}

#[test]
fn hair_line_with_raster_clip() {
    let clip = big_clip();
    let mut d = DumpBlitter::new();
    hair_line(&[pt(0.0, 0.0), pt(4.0, 4.0)], &clip, &mut d);
    assert_eq!(d.calls, [h(0, 0, 1), h(1, 1, 1), h(2, 2, 1), h(3, 3, 1)]);
}

// rect (1,1,4,3) -> r = (floor 1, floor 1, floor(4+1) = 5, floor(3+1) = 4): 4x3, so four
// segments: top, left, right, bottom.
#[test]
fn hair_rect_outline() {
    let clip = big_clip();
    let mut d = DumpBlitter::new();
    hair_rect(&Rect::new(1.0, 1.0, 4.0, 3.0), &clip, &mut d);
    assert_eq!(
        d.calls,
        [
            h(1, 1, 4),
            BlitCall::Rect {
                x: 1,
                y: 2,
                width: 1,
                height: 1
            },
            BlitCall::Rect {
                x: 4,
                y: 2,
                width: 1,
                height: 1
            },
            h(1, 3, 4),
        ]
    );
}

// rect (1,1,1.5,10) -> r = (1, 1, 2, 11): width 1 <= 2, so one blitRect.
#[test]
fn hair_rect_thin() {
    let clip = big_clip();
    let mut d = DumpBlitter::new();
    hair_rect(&Rect::new(1.0, 1.0, 1.5, 10.0), &clip, &mut d);
    assert_eq!(
        d.calls,
        [BlitCall::Rect {
            x: 1,
            y: 1,
            width: 1,
            height: 10
        }]
    );
}

// Clipped to (0,0,3,3): r = (1,1,5,4) intersected with the clip outset by one, then drawn
// through a RectClipBlitter: the top row is cut to x < 3.
#[test]
fn hair_rect_clipped() {
    let clip = Region::from_rect(IRect::new(0, 0, 3, 3));
    let mut d = DumpBlitter::new();
    hair_rect(&Rect::new(1.0, 1.0, 4.0, 3.0), &clip, &mut d);
    // r = (1,1,5,4) ∩ (-1,-1,4,4) = (1,1,4,4): width 3, height 3. The clipper cuts to (0,0,3,3).
    // top blitH(1,1,3) -> (1,1,2); left blitRect(1,2,1,1); right blitRect(3,2,1,1) is outside;
    // bottom blitH(1,3,3) is outside (y == 3).
    assert_eq!(
        d.calls,
        [
            h(1, 1, 2),
            BlitCall::Rect {
                x: 1,
                y: 2,
                width: 1,
                height: 1
            },
        ]
    );
}

// (1,2)->(5,2): x0 = 64, x1 = 320, y = 128. istart = 1, istop = 5, HLine, fstart = 2.0.
// startCoverage = 64, stopCoverage = 0. drawCap: fy = 2.5 -> y = 2, a = 128; lower line
// 128 * 64 >> 6 = 128, upper 127. drawLine(2, 5) does the same for the three remaining pixels.
#[test]
fn anti_hair_horizontal() {
    let mut d = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(1.0, 2.0), pt(5.0, 2.0)], None, &mut d);
    assert_eq!(
        d.calls,
        [
            aa(1, 2, &[(1, 128)]),
            aa(1, 1, &[(1, 127)]),
            aa(2, 2, &[(3, 128)]),
            aa(2, 1, &[(3, 127)]),
        ]
    );
}

// (2,1)->(2,5): vertical, VLine; fx = 2.0 -> fx + 0.5: x = 2, a = 128.
#[test]
fn anti_hair_vertical() {
    let mut d = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(2.0, 1.0), pt(2.0, 5.0)], None, &mut d);
    assert_eq!(
        d.calls,
        [
            v(2, 1, 1, 128),
            v(1, 1, 1, 127),
            v(2, 2, 3, 128),
            v(1, 2, 3, 127)
        ]
    );
}

// (1,1)->(5,3): dx = 256, dy = 128 (dot6). slope = 128 << 16 / 256 = 0.5 (32768);
// dxToCenter = 32 - 0 = 32; fstart = 65536 + ((32768 * 32 + 32) >> 6) = 81920 (1.25).
// drawCap(1): fy = 1.75: lowerY = 1, a = 192 -> blitAntiV2(1, 0, 63, 192); returns 1.75.
// drawLine(2..5): fy = 2.25, 2.75, 3.25 -> (lowerY, a) = (2, 64), (2, 192), (3, 64).
#[test]
fn anti_hair_horish() {
    let mut d = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(1.0, 1.0), pt(5.0, 3.0)], None, &mut d);
    assert_eq!(
        d.calls,
        [
            BlitCall::AntiV2 {
                x: 1,
                y: 0,
                a0: 63,
                a1: 192
            },
            BlitCall::AntiV2 {
                x: 2,
                y: 1,
                a0: 191,
                a1: 64
            },
            BlitCall::AntiV2 {
                x: 3,
                y: 1,
                a0: 63,
                a1: 192
            },
            BlitCall::AntiV2 {
                x: 4,
                y: 2,
                a0: 191,
                a1: 64
            },
        ]
    );
}

// The transpose of the above: (1,1)->(3,5).
#[test]
fn anti_hair_vertish() {
    let mut d = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(1.0, 1.0), pt(3.0, 5.0)], None, &mut d);
    assert_eq!(
        d.calls,
        [
            BlitCall::AntiH2 {
                x: 0,
                y: 1,
                a0: 63,
                a1: 192
            },
            BlitCall::AntiH2 {
                x: 1,
                y: 2,
                a0: 191,
                a1: 64
            },
            BlitCall::AntiH2 {
                x: 1,
                y: 3,
                a0: 63,
                a1: 192
            },
            BlitCall::AntiH2 {
                x: 2,
                y: 4,
                a0: 191,
                a1: 64
            },
        ]
    );
}

// (1.5,2)->(3.25,2): x0 = 96, x1 = 208. istart = 1, istop = ceil(208) = 4; startCoverage =
// 64 - 32 = 32, stopCoverage = 16. Cap: 128 * 32 >> 6 = 64, 127 * 32 >> 6 = 63. One full span
// at x = 2. Stop cap at x = 3: 128 * 16 >> 6 = 32, 127 * 16 >> 6 = 31.
#[test]
fn anti_hair_partial_caps() {
    let mut d = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(1.5, 2.0), pt(3.25, 2.0)], None, &mut d);
    assert_eq!(
        d.calls,
        [
            aa(1, 2, &[(1, 64)]),
            aa(1, 1, &[(1, 63)]),
            aa(2, 2, &[(1, 128)]),
            aa(2, 1, &[(1, 127)]),
            aa(3, 2, &[(1, 32)]),
            aa(3, 1, &[(1, 31)]),
        ]
    );
}

// clip (0,2,3,3): the row above the clip (y = 1) is cut by the RectClipBlitter, and the line is
// chopped to x < 3 (the last pixel, x = 4, is outside).
#[test]
fn anti_hair_clipped() {
    let clip = Region::from_rect(IRect::new(0, 2, 3, 3));
    let mut d = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(1.0, 2.0), pt(5.0, 2.0)], Some(&clip), &mut d);
    assert_eq!(d.calls, [aa(1, 2, &[(1, 128)]), aa(2, 2, &[(1, 128)])]);
}

#[test]
fn anti_hair_rect_is_closed_polyline() {
    let clip = big_clip();
    let mut d = DumpBlitter::new();
    anti_hair_rect(&Rect::new(1.0, 2.0, 5.0, 2.0), &clip, &mut d);
    // The degenerate rect (zero height) is the line (1,2)->(5,2) drawn forwards, then the
    // zero-length vertical segments (nothing), then backwards (5,2)->(1,2): the same line.
    let mut line = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(1.0, 2.0), pt(5.0, 2.0)], None, &mut line);
    let mut back = DumpBlitter::new();
    anti_hair_line_rgn(&[pt(5.0, 2.0), pt(1.0, 2.0)], None, &mut back);
    let mut expected = line.calls;
    expected.extend(back.calls);
    assert_eq!(d.calls, expected);
}

// rect (1.5,1.5,3.5,2.5) in FDot8: L = 384, T = 384, R = 896, B = 640. Rows 1 and 2 are each a
// half-covered scanline (alpha 128) with a half-covered pixel on each side.
#[test]
fn anti_fill_rect_fractional() {
    let mut d = DumpBlitter::new();
    anti_fill_rect(&Rect::new(1.5, 1.5, 3.5, 2.5), None, &mut d);
    assert_eq!(
        d.calls,
        [
            v(1, 1, 1, 64),
            aa(2, 1, &[(1, 128)]),
            v(3, 1, 1, 64),
            v(1, 2, 1, 64),
            aa(2, 2, &[(1, 128)]),
            v(3, 2, 1, 64),
        ]
    );
}

// An integer-aligned rect: one blitRect.
#[test]
fn anti_fill_rect_aligned() {
    let mut d = DumpBlitter::new();
    anti_fill_rect(&Rect::new(1.0, 1.0, 4.0, 3.0), None, &mut d);
    assert_eq!(
        d.calls,
        [BlitCall::Rect {
            x: 1,
            y: 1,
            width: 3,
            height: 2
        }]
    );
}

// r = (2,2,6,6), stroke (1,1): outer hull L/T = 1.5 -> 384, R/B = 6.5 -> 1664 (FDot8); inner
// hull 640..1408. See the derivation in the test body.
#[test]
fn anti_frame_rect_outline() {
    let mut d = DumpBlitter::new();
    anti_frame_rect(&Rect::new(2.0, 2.0, 6.0, 6.0), &pt(1.0, 1.0), None, &mut d);
    assert_eq!(
        d.calls,
        [
            // outer hull, antifilldot8(384, 384, 1664, 1664, fillInner = false):
            // top scanline (T & 0xFF = 128 -> alpha 128), do_scanline(384, 1, 1664, 128)
            v(1, 1, 1, 64),
            aa(2, 1, &[(4, 128)]),
            v(6, 1, 1, 64),
            // left and right columns (height 4)
            v(1, 2, 4, 128),
            v(6, 2, 4, 128),
            // bottom scanline (B & 0xFF = 128)
            v(1, 6, 1, 64),
            aa(2, 6, &[(4, 128)]),
            v(6, 6, 1, 64),
            // the four fillcheckrects are empty (outer middle == inner == (2,2,6,6));
            // innerstrokedot8(640, 640, 1408, 1408): top scanline, inner alpha
            // 128 + 128 - round(128 * 128 / 255) = 192 at the left, 128 + 127 - 64 = 191 right
            v(2, 2, 1, 192),
            aa(3, 2, &[(2, 128)]),
            v(5, 2, 1, 191),
            // left/right columns (height 2): L & 0xFF = 128, ~R & 0xFF = 127
            v(2, 3, 2, 128),
            v(5, 3, 2, 127),
            // bottom scanline, alpha ~B & 0xFF = 127
            v(2, 5, 1, 191),
            aa(3, 5, &[(2, 127)]),
            v(5, 5, 1, 191),
        ]
    );
}

#[test]
fn frame_rect_uses_fill_rect() {
    let clip = big_clip();
    let mut d = DumpBlitter::new();
    let mut filled = Vec::new();
    frame_rect(
        &Rect::new(0.0, 0.0, 10.0, 10.0),
        &pt(2.0, 4.0),
        &clip,
        &mut d,
        &mut |r, _clip, _b| filled.push(*r),
    );
    // outer = (-1,-2,11,12); top (-1,-2,11,2), bottom (-1,8,11,12), left (-1,2,1,8), right
    // (9,2,11,8).
    assert_eq!(
        filled,
        [
            Rect::new(-1.0, -2.0, 11.0, 2.0),
            Rect::new(-1.0, 8.0, 11.0, 12.0),
            Rect::new(-1.0, 2.0, 1.0, 8.0),
            Rect::new(9.0, 2.0, 11.0, 8.0),
        ]
    );

    // Stroke wider than the rect: a single fill of the outset rect.
    let mut filled = Vec::new();
    frame_rect(
        &Rect::new(0.0, 0.0, 1.0, 10.0),
        &pt(2.0, 1.0),
        &clip,
        &mut d,
        &mut |r, _clip, _b| filled.push(*r),
    );
    assert_eq!(filled, [Rect::new(-1.0, -0.5, 2.0, 10.5)]);
}

// A butt-capped line (1,1)->(5,1): x0 = 64, x1 = 320 -> ix 1..5 at y = 1.
#[test]
fn hair_path_butt_square_round() {
    let clip = big_clip();
    let (points, verbs) = line_path(&[pt(1.0, 1.0), pt(5.0, 1.0)]);
    let raw = raw_of(&points, &verbs);

    let mut d = DumpBlitter::new();
    hair_path(&raw, &clip, &mut d);
    assert_eq!(d.calls, [h(1, 1, 1), h(2, 1, 1), h(3, 1, 1), h(4, 1, 1)]);

    // Square caps extend each end by 0.5: (0.5,1)->(5.5,1): ix0 = round(32) = 1, ix1 =
    // round(352) = 6.
    let mut d = DumpBlitter::new();
    hair_square_path(&raw, &clip, &mut d);
    assert_eq!(
        d.calls,
        [h(1, 1, 1), h(2, 1, 1), h(3, 1, 1), h(4, 1, 1), h(5, 1, 1)]
    );

    // Round caps extend by pi/8 = 0.3927: x0 = 38 (0.6073 * 64 = 38.87), x1 = 345 (5.3927 * 64):
    // ix0 = round(38) = 1, ix1 = round(345) = 5.
    let mut d = DumpBlitter::new();
    hair_round_path(&raw, &clip, &mut d);
    assert_eq!(d.calls, [h(1, 1, 1), h(2, 1, 1), h(3, 1, 1), h(4, 1, 1)]);
}

// A closed square contour with square caps is not extended (the caps are only for open
// contours); a lone moveTo/close dot is, in both directions.
#[test]
fn hair_path_closed_contour_has_no_caps() {
    let clip = big_clip();
    let points = [pt(1.0, 1.0), pt(4.0, 1.0), pt(4.0, 4.0)];
    let verbs = [
        PathVerb::Move,
        PathVerb::Line,
        PathVerb::Line,
        PathVerb::Close,
    ];
    let raw = raw_of(&points, &verbs);

    let mut square = DumpBlitter::new();
    hair_square_path(&raw, &clip, &mut square);
    let mut butt = DumpBlitter::new();
    hair_path(&raw, &clip, &mut butt);
    assert_ne!(butt.dump(), "");
    assert_eq!(square.calls, butt.calls);
}

// Anti-aliased path through the same entry points.
#[test]
fn anti_hair_path_draws_anti_lines() {
    use crate::scan_hairline::anti_hair_path;
    let clip = big_clip();
    let (points, verbs) = line_path(&[pt(1.0, 2.0), pt(5.0, 2.0)]);
    let raw = raw_of(&points, &verbs);
    let mut d = DumpBlitter::new();
    anti_hair_path(&raw, &clip, &mut d);
    assert_eq!(
        d.calls,
        [
            aa(1, 2, &[(1, 128)]),
            aa(1, 1, &[(1, 127)]),
            aa(2, 2, &[(3, 128)]),
            aa(2, 1, &[(3, 127)]),
        ]
    );
}

// Quads and cubics are flattened to line segments. A straight "quad" (control point at the
// middle) has compute_quad_dist = 0 -> level (33 - clz(0)) >> 1 = 0 -> one segment
// (the end points), so it draws exactly like the line.
#[test]
fn hair_path_straight_quad_is_a_line() {
    let clip = big_clip();
    let points = [pt(1.0, 1.0), pt(3.0, 1.0), pt(5.0, 1.0)];
    let verbs = [PathVerb::Move, PathVerb::Quad];
    let raw = raw_of(&points, &verbs);
    let mut d = DumpBlitter::new();
    hair_path(&raw, &clip, &mut d);
    assert_eq!(d.calls, [h(1, 1, 1), h(2, 1, 1), h(3, 1, 1), h(4, 1, 1)]);
}

#[test]
fn hair_path_straight_cubic_is_a_line() {
    let clip = big_clip();
    let points = [pt(1.0, 1.0), pt(2.0, 1.0), pt(4.0, 1.0), pt(5.0, 1.0)];
    let verbs = [PathVerb::Move, PathVerb::Cubic];
    let raw = raw_of(&points, &verbs);
    let mut d = DumpBlitter::new();
    hair_path(&raw, &clip, &mut d);
    assert_eq!(d.calls, [h(1, 1, 1), h(2, 1, 1), h(3, 1, 1), h(4, 1, 1)]);
}

// A path completely outside the clip draws nothing; one that is partly outside is clipped.
#[test]
fn hair_path_clipping() {
    let clip = Region::from_rect(IRect::new(0, 0, 3, 3));
    let (points, verbs) = line_path(&[pt(10.0, 1.0), pt(20.0, 1.0)]);
    let raw = raw_of(&points, &verbs);
    let mut d = DumpBlitter::new();
    hair_path(&raw, &clip, &mut d);
    assert_eq!(d.dump(), "");

    let (points, verbs) = line_path(&[pt(-3.0, 1.5), pt(8.0, 1.5)]);
    let raw = raw_of(&points, &verbs);
    let mut d = DumpBlitter::new();
    hair_path(&raw, &clip, &mut d);
    assert_eq!(d.calls, [h(0, 1, 1), h(1, 1, 1), h(2, 1, 1)]);
}
