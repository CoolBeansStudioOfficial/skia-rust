// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! Tests of the non-AA scan converter and `Region::set_path` against hand-derived traces of what
//! Skia's `SkScan::FillPath` / `SkRegion::setPath` emit (no oracle is needed: the expected spans
//! follow from the fixed-point edge arithmetic, which is worked out in the comments).

use skia_rust_core::color::Alpha;
use skia_rust_core::path::Path;
use skia_rust_core::path_builder::PathBuilder;
use skia_rust_core::path_enums::ResolveConvexity;
use skia_rust_core::path_priv;
use skia_rust_core::path_types::{PathDirection, PathFillType};
use skia_rust_core::point::Point;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::region::{Op, Region};

use crate::blitter::{BlitMemory, Blitter};
use crate::region_path::RegionExt;
use crate::scan;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum Call {
    H(i32, i32, i32),
    Rect(i32, i32, i32, i32),
}

// Records `blit_h` and `blit_rect`, which is all the non-AA scan converter calls.
#[derive(Default)]
struct Recorder {
    calls: Vec<Call>,
    mem: BlitMemory,
}

impl Blitter for Recorder {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        self.calls.push(Call::H(x, y, width));
    }
    fn blit_anti_h(&mut self, _x: i32, _y: i32, _aa: &mut [Alpha], _runs: &mut [i16]) {
        panic!("unexpected blit_anti_h");
    }
    fn blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        self.calls.push(Call::Rect(x, y, width, height));
    }
    fn blit_memory(&mut self) -> &mut BlitMemory {
        &mut self.mem
    }
}

fn fill(path: &Path, clip: &Region) -> Vec<Call> {
    let raw = path_priv::raw(path, ResolveConvexity::Yes).unwrap();
    let mut rec = Recorder::default();
    scan::fill_path(&raw, clip, &mut rec);
    rec.calls
}

fn rect_path(fill_type: PathFillType, rects: &[Rect]) -> Path {
    let mut b = PathBuilder::new_with_fill_type(fill_type);
    for r in rects {
        b.add_rect(r, PathDirection::CW, None);
    }
    b.detach()
}

#[test]
fn fill_irect_clips_to_rect_and_complex_regions() {
    let mut rec = Recorder::default();
    scan::fill_irect(&IRect::new(1, 1, 7, 3), None, &mut rec);
    assert_eq!(rec.calls, [Call::Rect(1, 1, 6, 2)]);

    // an empty rect draws nothing
    let mut rec = Recorder::default();
    scan::fill_irect(&IRect::new(3, 3, 3, 9), None, &mut rec);
    assert_eq!(rec.calls, []);

    // a rect region clips by intersecting
    let mut rec = Recorder::default();
    let clip = Region::from_rect(IRect::new(2, 2, 5, 9));
    scan::fill_irect(&IRect::new(1, 1, 7, 3), Some(&clip), &mut rec);
    assert_eq!(rec.calls, [Call::Rect(2, 2, 3, 1)]);

    // a complex region is walked with a Cliperator: one rect per intersecting region rect
    let mut clip = Region::new();
    clip.op_rect(IRect::new(0, 0, 3, 10), Op::Union);
    clip.op_rect(IRect::new(5, 0, 8, 10), Op::Union);
    let mut rec = Recorder::default();
    scan::fill_irect(&IRect::new(1, 1, 7, 3), Some(&clip), &mut rec);
    assert_eq!(rec.calls, [Call::Rect(1, 1, 2, 2), Call::Rect(5, 1, 2, 2)]);
}

#[test]
fn fill_rect_and_xrect_round() {
    let mut rec = Recorder::default();
    // round(1.5) = 2, round(2.5) = 3, round(4.4) = 4, round(5.5) = 6
    scan::fill_rect(&Rect::new(1.5, 2.5, 4.4, 5.5), None, &mut rec);
    assert_eq!(rec.calls, [Call::Rect(2, 3, 2, 3)]);

    let xr = scan::xrect_set_rect(&Rect::new(1.5, 2.5, 4.4, 5.5));
    assert_eq!(
        scan::xrect_round(&xr),
        IRect::new(2, 3, 4, 6),
        "SkFixedRoundToInt"
    );
    assert_eq!(scan::xrect_round_out(&xr), IRect::new(1, 2, 5, 6));
    let mut rec = Recorder::default();
    scan::fill_xrect(&xr, None, &mut rec);
    assert_eq!(rec.calls, [Call::Rect(2, 3, 2, 3)]);
}

#[test]
fn convex_rect_is_one_blit_rect() {
    // Both edges are vertical (dx/dy == 0), so walk_simple_edges blits the whole rect at once.
    let path = rect_path(PathFillType::Winding, &[Rect::new(2.0, 1.0, 6.0, 4.0)]);
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));
    assert_eq!(fill(&path, &clip), [Call::Rect(2, 1, 4, 3)]);
}

#[test]
fn rect_clipped_horizontally_and_vertically() {
    let path = rect_path(PathFillType::Winding, &[Rect::new(-3.0, -2.0, 6.0, 8.0)]);
    let clip = Region::from_rect(IRect::new(0, 1, 4, 5));
    // The edges are clipped to the rect (x to [0, 4) by the RectClipBlitter, y by the edge
    // builder), so the single rect that remains is the clip itself.
    assert_eq!(fill(&path, &clip), [Call::Rect(0, 1, 4, 4)]);
}

#[test]
fn convex_triangle_spans() {
    // (0,0) (4,4) (0,4): the vertical edge is at x = 0 and the diagonal is at x = y + 0.5 on the
    // center of row y, which SkFixedRoundToInt rounds up to y + 1.
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((4.0, 4.0))
        .line_to((0.0, 4.0))
        .close()
        .detach();
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));
    assert_eq!(
        fill(&path, &clip),
        [
            Call::H(0, 0, 1),
            Call::H(0, 1, 2),
            Call::H(0, 2, 3),
            Call::H(0, 3, 4)
        ]
    );
}

#[test]
fn fill_triangle_matches_fill_path() {
    let pts = [
        Point::new(0.0, 0.0),
        Point::new(4.0, 4.0),
        Point::new(0.0, 4.0),
    ];
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));
    let mut rec = Recorder::default();
    scan::fill_triangle(&pts, &clip, &mut rec);
    assert_eq!(
        rec.calls,
        [
            Call::H(0, 0, 1),
            Call::H(0, 1, 2),
            Call::H(0, 2, 3),
            Call::H(0, 3, 4)
        ]
    );
}

#[test]
fn even_odd_and_winding_nested_squares() {
    let outer = Rect::new(0.0, 0.0, 10.0, 10.0);
    let inner = Rect::new(3.0, 3.0, 7.0, 7.0);
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));

    // Even-odd: the inner square is a hole. Rows 3..7 have two spans.
    let mut expected = Vec::new();
    for y in 0..10 {
        if (3..7).contains(&y) {
            expected.push(Call::H(0, y, 3));
            expected.push(Call::H(7, y, 3));
        } else {
            expected.push(Call::H(0, y, 10));
        }
    }
    let path = rect_path(PathFillType::EvenOdd, &[outer, inner]);
    assert_eq!(fill(&path, &clip), expected);

    // Winding: both squares are clockwise, so the winding number is 2 inside the inner one and
    // everything is filled.
    let expected: Vec<Call> = (0..10).map(|y| Call::H(0, y, 10)).collect();
    let path = rect_path(PathFillType::Winding, &[outer, inner]);
    assert_eq!(fill(&path, &clip), expected);
}

#[test]
fn inverse_fill_blits_above_inside_and_below() {
    let path = rect_path(
        PathFillType::InverseWinding,
        &[Rect::new(2.0, 1.0, 6.0, 4.0)],
    );
    let clip = Region::from_rect(IRect::new(0, 0, 10, 6));
    assert_eq!(
        fill(&path, &clip),
        [
            // sk_blit_above: rows [0, 1)
            Call::Rect(0, 0, 10, 1),
            // the InverseBlitter blits the gaps left of and right of each span
            Call::H(0, 1, 2),
            Call::H(6, 1, 4),
            Call::H(0, 2, 2),
            Call::H(6, 2, 4),
            Call::H(0, 3, 2),
            Call::H(6, 3, 4),
            // sk_blit_below: rows [4, 6)
            Call::Rect(0, 4, 10, 2),
        ]
    );
}

#[test]
fn inverse_fill_of_a_path_outside_the_clip_fills_the_clip() {
    let path = rect_path(
        PathFillType::InverseWinding,
        &[Rect::new(100.0, 100.0, 110.0, 110.0)],
    );
    let clip = Region::from_rect(IRect::new(0, 0, 10, 6));
    // The path does not intersect the clip, but the inverse fill still skips the reject test.
    // Above: nothing (the path's top is below the clip). The edges are culled by the clip, so
    // the zero-edge branch of sk_fill_path fills the clip rect between start_y and stop_y,
    // which are limited to the path's rows: empty. Below: the whole clip.
    assert_eq!(fill(&path, &clip), [Call::Rect(0, 0, 10, 6)]);
}

#[test]
fn empty_bounds_inverse_fills_the_clip_region() {
    // A path that is empty after rounding: an inverse fill covers the whole clip.
    let path = PathBuilder::new_with_fill_type(PathFillType::InverseWinding)
        .move_to((1.0, 1.0))
        .line_to((1.0, 1.0))
        .detach();
    let mut clip = Region::new();
    clip.op_rect(IRect::new(0, 0, 3, 3), Op::Union);
    clip.op_rect(IRect::new(5, 0, 8, 3), Op::Union);
    // blit_region visits spans in Y -> X order, one row at a time when a row has several spans
    let expected: Vec<Call> = (0..3)
        .flat_map(|y| [Call::Rect(0, y, 3, 1), Call::Rect(5, y, 3, 1)])
        .collect();
    assert_eq!(fill(&path, &clip), expected);
}

#[test]
fn complex_clip_is_applied_through_the_region_blitter() {
    let path = rect_path(PathFillType::Winding, &[Rect::new(0.0, 0.0, 8.0, 2.0)]);
    let mut clip = Region::new();
    clip.op_rect(IRect::new(0, 0, 3, 2), Op::Union);
    clip.op_rect(IRect::new(5, 0, 8, 2), Op::Union);
    assert_eq!(
        fill(&path, &clip),
        [Call::Rect(0, 0, 3, 2), Call::Rect(5, 0, 3, 2)]
    );
}

#[test]
fn path_requires_tiling() {
    assert!(!scan::path_requires_tiling(&IRect::new(0, 0, 100, 100)));
    assert!(!scan::path_requires_tiling(&IRect::new(
        -16383, -16383, 16383, 16383
    )));
    assert!(scan::path_requires_tiling(&IRect::new(
        -16384, 0, 16383, 10
    )));
    assert!(scan::path_requires_tiling(&IRect::new(
        -45000, -45000, 45000, 45000
    )));
}

#[test]
fn region_set_path_rect_and_empty() {
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));
    let mut rgn = Region::new();

    let path = rect_path(PathFillType::Winding, &[Rect::new(2.0, 1.0, 6.0, 4.0)]);
    assert!(rgn.set_path(&path, &clip));
    assert!(rgn.is_rect());
    assert_eq!(*rgn.bounds(), IRect::new(2, 1, 6, 4));

    // a path outside of the clip gives an empty region, an inverse one gives the clip
    let far = rect_path(PathFillType::Winding, &[Rect::new(20.0, 20.0, 30.0, 30.0)]);
    assert!(!rgn.set_path(&far, &clip));
    assert!(rgn.is_empty());
    let far_inverse = rect_path(
        PathFillType::InverseWinding,
        &[Rect::new(20.0, 20.0, 30.0, 30.0)],
    );
    assert!(rgn.set_path(&far_inverse, &clip));
    assert_eq!(rgn, clip);

    // an empty clip gives an empty region
    assert!(!rgn.set_path(&path, &Region::new()));
    assert!(rgn.is_empty());
}

#[test]
fn region_set_path_triangle_is_a_staircase() {
    let path = PathBuilder::new()
        .move_to((0.0, 0.0))
        .line_to((4.0, 4.0))
        .line_to((0.0, 4.0))
        .close()
        .detach();
    let clip = Region::from_rect(IRect::new(0, 0, 10, 10));
    let mut rgn = Region::new();
    assert!(rgn.set_path(&path, &clip));

    let mut expected = Region::new();
    for y in 0..4 {
        expected.op_rect(IRect::new(0, y, y + 1, y + 1), Op::Union);
    }
    assert_eq!(rgn, expected);
    assert_eq!(*rgn.bounds(), IRect::new(0, 0, 4, 4));
}

#[test]
fn region_set_path_with_a_complex_clip_intersects() {
    let path = rect_path(PathFillType::Winding, &[Rect::new(0.0, 0.0, 8.0, 4.0)]);
    let mut clip = Region::new();
    clip.op_rect(IRect::new(0, 0, 3, 2), Op::Union);
    clip.op_rect(IRect::new(5, 1, 20, 3), Op::Union);
    let mut rgn = Region::new();
    assert!(rgn.set_path(&path, &clip));

    let mut expected = Region::new();
    expected.op_rect(IRect::new(0, 0, 3, 2), Op::Union);
    expected.op_rect(IRect::new(5, 1, 8, 3), Op::Union);
    assert_eq!(rgn, expected);
}

#[test]
fn region_set_path_even_odd_frame() {
    let path = rect_path(
        PathFillType::EvenOdd,
        &[
            Rect::new(0.0, 0.0, 10.0, 10.0),
            Rect::new(3.0, 3.0, 7.0, 7.0),
        ],
    );
    let clip = Region::from_rect(IRect::new(-5, -5, 15, 15));
    let mut rgn = Region::new();
    assert!(rgn.set_path(&path, &clip));

    let mut expected = Region::from_rect(IRect::new(0, 0, 10, 10));
    expected.op_rect(IRect::new(3, 3, 7, 7), Op::Difference);
    assert!(expected.is_complex());
    assert_eq!(rgn, expected);
}

#[test]
fn boundary_path_round_trips_through_set_path() {
    // a rect region's boundary path is a rect
    let rect = Region::from_rect(IRect::new(1, 2, 5, 9));
    let mut b = PathBuilder::new();
    assert!(rect.add_boundary_path(&mut b));
    let mut expected = PathBuilder::new();
    expected.add_rect(Rect::new(1.0, 2.0, 5.0, 9.0), None, None);
    assert_eq!(b.detach(), expected.detach());

    assert!(Region::new().boundary_path().is_empty());

    // Complex regions (an L, a frame with a hole, and separated pieces) survive the round trip:
    // the boundary path of the region fills back to the same region.
    let mut regions = Vec::new();

    let mut l_shape = Region::new();
    l_shape.op_rect(IRect::new(0, 0, 6, 2), Op::Union);
    l_shape.op_rect(IRect::new(0, 2, 2, 7), Op::Union);
    regions.push(l_shape);

    let mut frame = Region::from_rect(IRect::new(0, 0, 10, 10));
    frame.op_rect(IRect::new(3, 3, 7, 7), Op::Difference);
    regions.push(frame);

    let mut pieces = Region::new();
    pieces.op_rect(IRect::new(0, 0, 2, 2), Op::Union);
    pieces.op_rect(IRect::new(4, 1, 8, 5), Op::Union);
    pieces.op_rect(IRect::new(1, 6, 3, 9), Op::Union);
    regions.push(pieces);

    for region in regions {
        let path = region.boundary_path();
        assert!(!path.is_empty());
        let clip = Region::from_rect(IRect::new(-2, -2, 20, 20));
        let mut back = Region::new();
        assert!(back.set_path(&path, &clip));
        assert_eq!(back, region, "boundary path of {region:?}");
    }
}
