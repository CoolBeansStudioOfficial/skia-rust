// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScan.h, src/core/SkScan.cpp, src/core/SkScan_Path.cpp

//! The non-antialiased scan converter (`SkScan.h`, `SkScan.cpp`, `SkScan_Path.cpp`).
//!
//! Fills rectangles and paths into a [`Blitter`], clipped to a [`Region`]. The antialiased and
//! hairline scan converters (`SkScan_AntiPath.cpp`, `SkScan_Hairline.cpp`, ...) are separate
//! modules.
//!
//! # skia-rust deviations
//! - The `SkRasterClip` overloads (`FillIRect(r, SkRasterClip, ...)`, `FillPath(raw,
//!   SkRasterClip, ...)`, ...) wrap an `SkAAClip` blitter around the region versions; they come
//!   with `SkRasterClip` (task C5). Everything below takes the region (`SkRegion*` / `SkRegion&`)
//!   they delegate to; [`fill_triangle`] therefore takes the region of a BW raster clip.
//! - The edges of a path are stored in one `Vec` with the sorted-list sentinels (`headEdge`,
//!   `tailEdge`) appended; `fPrev`/`fNext` are indices ([`NIL`] is the null pointer).
//! - `ASSERT_RETURN` in `walk_simple_edges` returns from the function when its condition fails
//!   (what a release build of Skia does) without a debug-build abort.
//! - `SkXRect` is `SkIRect` with fixed-point coordinates ([`XRect`]).

use skia_rust_core::fdot6::FDOT6_ONE;
use skia_rust_core::fixed::{
    Fixed, fixed_ceil_to_int, fixed_floor_to_int, fixed_round_to_int, int_to_fixed, scalar_to_fixed,
};
use skia_rust_core::floating_point::double_saturate2int;
use skia_rust_core::path_raw::PathRaw;
use skia_rust_core::path_raw_shapes::triangle;
use skia_rust_core::path_types::PathFillType;
use skia_rust_core::point::Point;
use skia_rust_core::rect::{Contains, IRect, Rect, rect_priv};
use skia_rust_core::region::{Cliperator, Op, Region};
use skia_rust_core::safe32::can_overflow_add;

use crate::blitter::Blitter;
use crate::edge::{AnyEdge, Edge};
use crate::edge_builder::{BasicEdgeBuilder, EdgeBuilder};
use crate::scan_priv::{
    NIL, ScanClipper, backward_insert_edge_based_on_x, backward_insert_start, blit_above,
    blit_below, insert_edge_after, remove_edge,
};

/// Defines a fixed-point rectangle, identical to the integer [`IRect`], but its coordinates are
/// treated as `Fixed` rather than `i32`.
// Port of: src/core/SkScan.h#L23 (chrome/m156)
#[doc(alias = "SkXRect")]
pub type XRect = IRect;

/// Assigns an [`XRect`] from an [`IRect`], by promoting the src rect's coordinates from int to
/// `Fixed`. Does not check for overflow if the src coordinates exceed 32K.
// Port of: src/core/SkScan.h#L97-L104 (chrome/m156)
#[doc(alias = "XRect_set")]
#[must_use]
pub fn xrect_set_irect(src: &IRect) -> XRect {
    XRect {
        left: int_to_fixed(src.left),
        top: int_to_fixed(src.top),
        right: int_to_fixed(src.right),
        bottom: int_to_fixed(src.bottom),
    }
}

/// Assigns an [`XRect`] from a [`Rect`], by promoting the src rect's coordinates from scalar to
/// `Fixed`. Does not check for overflow if the src coordinates exceed 32K.
// Port of: src/core/SkScan.h#L106-L114 (chrome/m156)
#[doc(alias = "XRect_set")]
#[must_use]
pub fn xrect_set_rect(src: &Rect) -> XRect {
    XRect {
        left: scalar_to_fixed(src.left),
        top: scalar_to_fixed(src.top),
        right: scalar_to_fixed(src.right),
        bottom: scalar_to_fixed(src.bottom),
    }
}

/// Rounds the [`XRect`] coordinates, and returns the result as an [`IRect`].
// Port of: src/core/SkScan.h#L116-L122 (chrome/m156)
#[doc(alias = "XRect_round")]
#[must_use]
pub fn xrect_round(xr: &XRect) -> IRect {
    IRect {
        left: fixed_round_to_int(xr.left),
        top: fixed_round_to_int(xr.top),
        right: fixed_round_to_int(xr.right),
        bottom: fixed_round_to_int(xr.bottom),
    }
}

/// Rounds the [`XRect`] coordinates out (i.e. use floor for left/top, and ceiling for
/// right/bottom), and returns the result as an [`IRect`].
// Port of: src/core/SkScan.h#L124-L131 (chrome/m156)
#[doc(alias = "XRect_roundOut")]
#[must_use]
pub fn xrect_round_out(xr: &XRect) -> IRect {
    IRect {
        left: fixed_floor_to_int(xr.left),
        top: fixed_floor_to_int(xr.top),
        right: fixed_ceil_to_int(xr.right),
        bottom: fixed_ceil_to_int(xr.bottom),
    }
}

///////////////////////////////////////////////////////////////////////////////
// SkScan.cpp

// Port of: src/core/SkScan.cpp#L16-L18 (chrome/m156)
fn blitrect(blitter: &mut dyn Blitter, r: &IRect) {
    blitter.blit_rect(r.left, r.top, r.width(), r.height());
}

/// Fills `r`, clipped to `clip` if there is one (`SkScan::FillIRect(const SkIRect&, const
/// SkRegion*, SkBlitter*)`).
// Port of: src/core/SkScan.cpp#L20-L48 (chrome/m156)
#[doc(alias = "FillIRect")]
pub fn fill_irect(r: &IRect, clip: Option<&Region>, blitter: &mut dyn Blitter) {
    if !r.is_empty() {
        if let Some(clip) = clip {
            if clip.is_rect() {
                let clip_bounds = clip.bounds();

                if clip_bounds.contains(r) {
                    blitrect(blitter, r);
                } else if let Some(rr) = IRect::intersect(r, clip_bounds) {
                    blitrect(blitter, &rr);
                }
            } else {
                for rr in Cliperator::new(clip, r) {
                    blitrect(blitter, &rr);
                }
            }
        } else {
            blitrect(blitter, r);
        }
    }
}

/// Fills the rounded fixed-point rect (`SkScan::FillXRect`).
// Port of: src/core/SkScan.cpp#L50-L56 (chrome/m156)
#[doc(alias = "FillXRect")]
pub fn fill_xrect(xr: &XRect, clip: Option<&Region>, blitter: &mut dyn Blitter) {
    let r = xrect_round(xr);
    fill_irect(&r, clip, blitter);
}

/// Fills the rounded rect (`SkScan::FillRect`).
// Port of: src/core/SkScan.cpp#L58-L64 (chrome/m156)
#[doc(alias = "FillRect")]
pub fn fill_rect(r: &Rect, clip: Option<&Region>, blitter: &mut dyn Blitter) {
    let ir = r.round();
    fill_irect(&ir, clip, blitter);
}

///////////////////////////////////////////////////////////////////////////////
// SkScan_Path.cpp

const EDGE_HEAD_Y: i32 = i32::MIN;
const EDGE_TAIL_Y: i32 = i32::MAX;

// Port of: src/core/SkScan_Path.cpp#L47-L76 (chrome/m156)
fn insert_new_edges(edges: &mut [AnyEdge], new_edge: usize, curr_y: i32) {
    let mut new_edge = new_edge;
    if edges[new_edge].first_y != curr_y {
        return;
    }
    let prev = edges[new_edge].prev;
    if edges[prev].x <= edges[new_edge].x {
        return;
    }
    // find first x pos to insert
    let mut start = backward_insert_start(edges, prev, edges[new_edge].x);
    // insert the lot, fixing up the links as we go
    loop {
        let next = edges[new_edge].next;
        'next_edge: {
            loop {
                if edges[start].next == new_edge {
                    break 'next_edge;
                }
                let after = edges[start].next;
                if edges[after].x >= edges[new_edge].x {
                    break;
                }
                start = after;
            }
            remove_edge(edges, new_edge);
            insert_edge_after(edges, new_edge, start);
        }
        start = new_edge;
        new_edge = next;
        if edges[new_edge].first_y != curr_y {
            break;
        }
    }
}

/// The type of the pre/post scanline callbacks of [`walk_edges`] (`PrePostProc`).
// Port of: src/core/SkScan_Path.cpp#L100 (chrome/m156)
type PrePostProc<B> = fn(&mut B, i32, bool);
const PREPOST_START: bool = true;
const PREPOST_END: bool = false;

// Port of: src/core/SkScan_Path.cpp#L104-L186 (chrome/m156)
#[allow(clippy::too_many_arguments)] // mirrors the C++ signature
fn walk_edges<B: Blitter + ?Sized>(
    edges: &mut [AnyEdge],
    prev_head: usize,
    fill_type: PathFillType,
    blitter: &mut B,
    start_y: i32,
    stop_y: i32,
    proc: Option<PrePostProc<B>>,
    right_clip: i32,
) {
    let mut curr_y = start_y;
    let winding_mask: i32 = if fill_type.is_even_odd() { 1 } else { -1 };

    loop {
        let mut w: i32 = 0;
        let mut left: i32 = 0;
        let mut curr_e = edges[prev_head].next;
        let mut prev_x = edges[prev_head].x;

        if let Some(proc) = proc {
            proc(blitter, curr_y, PREPOST_START); // pre-proc
        }

        while edges[curr_e].first_y <= curr_y {
            debug_assert!(edges[curr_e].last_y >= curr_y);

            let x = fixed_round_to_int(edges[curr_e].x);

            if (w & winding_mask) == 0 {
                // we're starting interval
                left = x;
            }

            w = w.wrapping_add(i32::from(edges[curr_e].winding as i8));

            if (w & winding_mask) == 0 {
                // we finished an interval
                let width = x.wrapping_sub(left);
                debug_assert!(width >= 0);
                if width > 0 {
                    blitter.blit_h(left, curr_y, width);
                }
            }

            let next = edges[curr_e].next;
            // `Some(newX)` means "ripple currE backwards until it is x-sorted" (NEXT_X).
            let mut next_x: Option<Fixed> = None;

            if edges[curr_e].last_y == curr_y {
                // are we done with this edge?
                if edges[curr_e].has_next_segment() && edges[curr_e].next_segment() {
                    debug_assert_eq!(edges[curr_e].first_y, curr_y + 1);
                    next_x = Some(edges[curr_e].x);
                } else {
                    remove_edge(edges, curr_e);
                }
            } else {
                debug_assert!(edges[curr_e].last_y > curr_y);
                let new_x = edges[curr_e].x.wrapping_add(edges[curr_e].dx_dy);
                edges[curr_e].x = new_x;
                next_x = Some(new_x);
            }
            if let Some(new_x) = next_x {
                if new_x < prev_x {
                    // ripple currE backwards until it is x-sorted
                    backward_insert_edge_based_on_x(edges, curr_e);
                } else {
                    prev_x = new_x;
                }
            }
            curr_e = next;
            debug_assert_ne!(curr_e, NIL);
        }

        if (w & winding_mask) != 0 {
            // was our right-edge culled away?
            let width = right_clip.wrapping_sub(left);
            if width > 0 {
                blitter.blit_h(left, curr_y, width);
            }
        }

        if let Some(proc) = proc {
            proc(blitter, curr_y, PREPOST_END); // post-proc
        }

        curr_y += 1;
        if curr_y >= stop_y {
            break;
        }
        // now currE points to the first edge with a Yint larger than curr_y
        insert_new_edges(edges, curr_e, curr_y);
    }
}

// return true if we're NOT done with this edge
// Port of: src/core/SkScan_Path.cpp#L188-L202 (chrome/m156)
fn update_edge(edges: &mut [AnyEdge], edge: usize, last_y: i32) -> bool {
    debug_assert!(edges[edge].last_y >= last_y);
    if last_y != edges[edge].last_y {
        return true;
    }
    if !edges[edge].has_next_segment() {
        return false;
    }
    if edges[edge].next_segment() {
        debug_assert_eq!(edges[edge].first_y, last_y + 1);
        return true;
    }
    false
}

// Unexpected conditions for which we need to return
// Port of: src/core/SkScan_Path.cpp#L204-L211 (chrome/m156)
macro_rules! assert_return {
    ($cond:expr) => {
        if !($cond) {
            return;
        }
    };
}

// Needs Y to only change once (looser than convex in X)
// Port of: src/core/SkScan_Path.cpp#L213-L291 (chrome/m156)
fn walk_simple_edges(
    edges: &mut [AnyEdge],
    prev_head: usize,
    blitter: &mut dyn Blitter,
    start_y: i32,
    stop_y: i32,
) {
    let mut left_e = edges[prev_head].next;
    let mut rite_e = edges[left_e].next;
    let mut curr_e = edges[rite_e].next;

    // our edge choppers for curves can result in the initial edges
    // not lining up, so we take the max.
    let mut local_top = edges[left_e].first_y.max(edges[rite_e].first_y);
    assert_return!(local_top >= start_y);

    while local_top < stop_y {
        debug_assert!(edges[left_e].first_y <= stop_y);
        debug_assert!(edges[rite_e].first_y <= stop_y);

        let mut local_bot = edges[left_e].last_y.min(edges[rite_e].last_y);
        local_bot = local_bot.min(stop_y - 1);
        assert_return!(local_top <= local_bot);

        let mut left = edges[left_e].x;
        let d_left = edges[left_e].dx_dy;
        let mut rite = edges[rite_e].x;
        let d_rite = edges[rite_e].dx_dy;
        let mut count = local_bot - local_top;
        assert_return!(count >= 0);

        if d_left == 0 && d_rite == 0 {
            let mut l = fixed_round_to_int(left);
            let mut r = fixed_round_to_int(rite);
            if l > r {
                std::mem::swap(&mut l, &mut r);
            }
            if l < r {
                count += 1;
                blitter.blit_rect(l, local_top, r - l, count);
            }
            local_top = local_bot + 1;
        } else {
            loop {
                let mut l = fixed_round_to_int(left);
                let mut r = fixed_round_to_int(rite);
                if l > r {
                    std::mem::swap(&mut l, &mut r);
                }
                if l < r {
                    blitter.blit_h(l, local_top, r - l);
                }
                // Either/both of these might overflow, since we perform this step even if
                // (later) we determine that we are done with the edge, and so the computed
                // left or rite edge will not be used (see update_edge). Use this helper to
                // silence UBSAN when we perform the add.
                left = can_overflow_add(left, d_left);
                rite = can_overflow_add(rite, d_rite);
                local_top += 1;
                count -= 1;
                if count < 0 {
                    break;
                }
            }
        }

        edges[left_e].x = left;
        edges[rite_e].x = rite;

        if !update_edge(edges, left_e, local_bot) {
            if edges[curr_e].first_y >= stop_y {
                return; // we're done
            }
            left_e = curr_e;
            curr_e = edges[curr_e].next;
            assert_return!(edges[left_e].first_y == local_top);
        }
        if !update_edge(edges, rite_e, local_bot) {
            if edges[curr_e].first_y >= stop_y {
                return; // we're done
            }
            rite_e = curr_e;
            curr_e = edges[curr_e].next;
            assert_return!(edges[rite_e].first_y == local_top);
        }
    }
}

///////////////////////////////////////////////////////////////////////////////

// this overrides blitH, and will call its proxy blitter with the inverse
// of the spans it is given (clipped to the left/right of the cliprect)
//
// used to implement inverse filltypes on paths
// Port of: src/core/SkScan_Path.cpp#L293-L351 (chrome/m156)
struct InverseBlitter<'a> {
    blitter: &'a mut dyn Blitter,
    first_x: i32,
    last_x: i32,
    prev_x: i32,
}

impl<'a> InverseBlitter<'a> {
    // `setBlitter`.
    fn new(blitter: &'a mut dyn Blitter, clip: &IRect) -> Self {
        InverseBlitter {
            blitter,
            first_x: clip.left,
            last_x: clip.right,
            prev_x: 0,
        }
    }

    fn prepost(&mut self, y: i32, is_start: bool) {
        if is_start {
            self.prev_x = self.first_x;
        } else {
            let inv_width = self.last_x.wrapping_sub(self.prev_x);
            if inv_width > 0 {
                self.blitter.blit_h(self.prev_x, y, inv_width);
            }
        }
    }
}

impl Blitter for InverseBlitter<'_> {
    fn blit_h(&mut self, x: i32, y: i32, width: i32) {
        let inv_width = x.wrapping_sub(self.prev_x);
        if inv_width > 0 {
            self.blitter.blit_h(self.prev_x, y, inv_width);
        }
        self.prev_x = x.wrapping_add(width);
    }

    // we do not expect to get called with these entrypoints
    fn blit_anti_h(&mut self, _x: i32, _y: i32, _antialias: &mut [u8], _runs: &mut [i16]) {
        debug_assert!(false, "blitAntiH unexpected");
    }

    fn blit_v(&mut self, _x: i32, _y: i32, _height: i32, _alpha: u8) {
        debug_assert!(false, "blitV unexpected");
    }

    fn blit_rect(&mut self, _x: i32, _y: i32, _width: i32, _height: i32) {
        debug_assert!(false, "blitRect unexpected");
    }

    fn blit_mask(&mut self, _mask: &skia_rust_core::mask::Mask<'_>, _clip: &IRect) {
        debug_assert!(false, "blitMask unexpected");
    }

    fn blit_memory(&mut self) -> &mut crate::blitter::BlitMemory {
        self.blitter.blit_memory()
    }
}

// Port of: src/core/SkScan_Path.cpp#L353-L355 (chrome/m156)
fn prepost_inverse_blitter_proc(blitter: &mut InverseBlitter<'_>, y: i32, is_start: bool) {
    blitter.prepost(y, is_start);
}

///////////////////////////////////////////////////////////////////////////////

// Port of: src/core/SkScan_Path.cpp#L367-L373 (chrome/m156)
fn compare_edges(edges: &[AnyEdge], a: usize, b: usize) -> bool {
    if edges[a].first_y != edges[b].first_y {
        return edges[a].first_y < edges[b].first_y;
    }

    edges[a].x < edges[b].x
}

/// Sorts the edges of `list` (indices of `edges`) and links them in that order. Returns the
/// first and the last edge.
// Port of: src/core/SkScan_Path.cpp#L375-L385 (chrome/m156)
fn sort_edges(edges: &mut [AnyEdge], list: &mut [usize]) -> (usize, usize) {
    skia_rust_core::t_sort::t_q_sort(list, |&a, &b| compare_edges(edges, a, b));

    // now make the edges linked in sorted order
    for i in 1..list.len() {
        edges[list[i - 1]].next = list[i];
        edges[list[i]].prev = list[i - 1];
    }

    (list[0], list[list.len() - 1])
}

// Appends the `headEdge`/`tailEdge` sentinels around the sorted edge list `first..=last`.
// Returns the index of the head. The tail is the next index.
// Port of: src/core/SkScan_Path.cpp#L416-L428 (chrome/m156)
fn add_head_and_tail(edges: &mut Vec<AnyEdge>, first: usize, last: usize) -> usize {
    let head = edges.len();
    let tail = head + 1;

    let mut head_edge = Edge::default();
    head_edge.prev = NIL;
    head_edge.next = first;
    head_edge.first_y = EDGE_HEAD_Y;
    head_edge.x = i32::MIN;
    edges.push(AnyEdge::Line(head_edge));

    let mut tail_edge = Edge::default();
    tail_edge.prev = last;
    tail_edge.next = NIL;
    tail_edge.first_y = EDGE_TAIL_Y;
    edges.push(AnyEdge::Line(tail_edge));

    edges[first].prev = head;
    edges[last].next = tail;
    head
}

// Port of: src/core/SkScan_Path.cpp#L387-L441 (chrome/m156)
fn sk_fill_path(
    raw: &PathRaw<'_>,
    clip_rect: &IRect,
    blitter: &mut dyn Blitter,
    start_y: i32,
    stop_y: i32,
    path_contained_in_clip: bool,
) {
    let mut start_y = start_y;
    let mut stop_y = stop_y;

    let mut builder = BasicEdgeBuilder::new();
    let count = builder.build_edges(
        raw,
        if path_contained_in_clip {
            None
        } else {
            Some(clip_rect)
        },
    );
    let mut edges = builder.into_edges();
    debug_assert_eq!(count, edges.len());

    if 0 == count {
        if raw.is_inverse_fill_type() {
            // Since we are in inverse-fill, our caller has already drawn above our top
            // (start_y) and will draw below our bottom (stop_y). Thus we need to restrict our
            // drawing to the intersection of the clip and those two limits.
            let mut rect = *clip_rect;
            if rect.top < start_y {
                rect.top = start_y;
            }
            if rect.bottom > stop_y {
                rect.bottom = stop_y;
            }
            if !rect.is_empty() {
                blitter.blit_rect(rect.left, rect.top, rect.width(), rect.height());
            }
        }
        return;
    }

    // this returns the first and last edge after they're sorted into a dlink list
    let mut list: Vec<usize> = (0..count).collect();
    let (first_edge, last_edge) = sort_edges(&mut edges, &mut list);
    let head_edge = add_head_and_tail(&mut edges, first_edge, last_edge);

    // now edge is the head of the sorted linklist
    if !path_contained_in_clip && start_y < clip_rect.top {
        start_y = clip_rect.top;
    }
    if !path_contained_in_clip && stop_y > clip_rect.bottom {
        stop_y = clip_rect.bottom;
    }

    if raw.is_inverse_fill_type() {
        let mut ib = InverseBlitter::new(blitter, clip_rect);
        walk_edges(
            &mut edges,
            head_edge,
            raw.fill_type(),
            &mut ib,
            start_y,
            stop_y,
            Some(prepost_inverse_blitter_proc),
            clip_rect.right,
        );
    } else if raw.is_known_to_be_convex() && count >= 2 {
        // count >= 2 is required as the convex walker does not handle missing right edges
        walk_simple_edges(&mut edges, head_edge, blitter, start_y, stop_y);
    } else {
        walk_edges::<dyn Blitter>(
            &mut edges,
            head_edge,
            raw.fill_type(),
            blitter,
            start_y,
            stop_y,
            None,
            clip_rect.right,
        );
    }
}

///////////////////////////////////////////////////////////////////////////////

// Port of: src/core/SkScan_Path.cpp#L515-L525 (chrome/m156)
fn clip_to_limit(orig: &Region, reduced: &mut Region) -> bool {
    // need to limit coordinates such that the width/height of our rect can be represented
    // in SkFixed (16.16). See skbug.com/40039252
    let limit: i32 = 32767 >> 1;

    let limit_r = IRect::new(-limit, -limit, limit, limit);
    if limit_r.contains(orig.bounds()) {
        return false;
    }
    reduced.op_region_rect(orig, limit_r, Op::Intersect);
    true
}

// Bias used for conservative rounding of float rects to int rects, to nudge the irects a little
// larger, so we don't "think" a path's bounds are inside a clip, when (due to numeric drift in
// the scan-converter) we might walk beyond the predicted limits.
//
// This value has been determined trial and error: pick the smallest value (after the 0.5) that
// fixes any problematic cases (e.g. crbug.com/844457)
// NOTE: cubics appear to be the main reason for needing this slop. If we could (perhaps) have a
// more accurate walker for cubics, we may be able to reduce this fudge factor.
// Port of: src/core/SkScan_Path.cpp#L541 (chrome/m156)
#[allow(clippy::cast_lossless)] // const context: FDOT6_ONE is an i32 constant
const CONSERVATIVE_ROUND_BIAS: f64 = 0.5 + 1.5 / (FDOT6_ONE as f64);

/// Round the value down. This is used to round the top and left of a rectangle, and corresponds
/// to the way the scan converter treats the top and left edges. It has a slight bias to make the
/// "rounded" int smaller than a normal round, to create a more conservative int-bounds (larger)
/// from a float rect.
// Port of: src/core/SkScan_Path.cpp#L550-L554 (chrome/m156)
fn round_down_to_int(x: f32) -> i32 {
    let mut xx = f64::from(x);
    xx -= CONSERVATIVE_ROUND_BIAS;
    double_saturate2int(xx.ceil())
}

/// Round the value up. This is used to round the right and bottom of a rectangle. It has a slight
/// bias to make the "rounded" int smaller than a normal round, to create a more conservative
/// int-bounds (larger) from a float rect.
// Port of: src/core/SkScan_Path.cpp#L561-L565 (chrome/m156)
fn round_up_to_int(x: f32) -> i32 {
    let mut xx = f64::from(x);
    xx += CONSERVATIVE_ROUND_BIAS;
    double_saturate2int(xx.floor())
}

// Conservative rounding function, which effectively nudges the int-rect to be slightly larger
// than SkRect::round() might have produced. This is a safety-net for the scan-converter, which
// inspects the returned int-rect, and may disable clipping (for speed) if it thinks all of the
// edges will fit inside the clip's bounds. The scan-converter introduces slight numeric errors
// due to accumulated += of the slope, so this function is used to return a conservatively large
// int-bounds, and thus we will only disable clipping if we're sure the edges will stay in-bounds.
// Port of: src/core/SkScan_Path.cpp#L574-L581 (chrome/m156)
fn conservative_round_to_int(src: &Rect) -> IRect {
    IRect {
        left: round_down_to_int(src.left),
        top: round_down_to_int(src.top),
        right: round_up_to_int(src.right),
        bottom: round_up_to_int(src.bottom),
    }
}

/// Fills `raw` into `blitter`, clipped to `orig_clip` (`SkScan::FillPath(const SkPathRaw&, const
/// SkRegion& clip, SkBlitter*)`).
// Port of: src/core/SkScan_Path.cpp#L583-L645 (chrome/m156)
#[doc(alias = "FillPath")]
pub fn fill_path(raw: &PathRaw<'_>, orig_clip: &Region, blitter: &mut dyn Blitter) {
    if orig_clip.is_empty() {
        return;
    }

    // Our edges are fixed-point, and don't like the bounds of the clip to
    // exceed that. Here we trim the clip just so we don't overflow later on
    let mut finite_clip = Region::new();
    let clip: &Region = if clip_to_limit(orig_clip, &mut finite_clip) {
        if finite_clip.is_empty() {
            return;
        }
        &finite_clip
    } else {
        orig_clip
    };
    // don't reference "origClip" any more, just use clip

    let mut bounds = raw.bounds();
    let mut ir_pre_clipped = false;
    let large = rect_priv::make_large_s32();
    if !large.contains(&bounds) {
        if !bounds.intersect(large) {
            bounds.set_empty();
        }
        ir_pre_clipped = true;
    }

    let ir = conservative_round_to_int(&bounds);
    if ir.is_empty() {
        if raw.is_inverse_fill_type() {
            blitter.blit_region(clip);
        }
        return;
    }

    let mut clipper = ScanClipper::new(
        blitter,
        clip,
        &ir,
        raw.is_inverse_fill_type(),
        ir_pre_clipped,
    );

    let path_contained_in_clip = clipper.clip_rect().is_none();
    if let Some(blitter) = clipper.blitter() {
        // we have to keep our calls to blitter in sorted order, so we
        // must blit the above section first, then the middle, then the bottom.
        if raw.is_inverse_fill_type() {
            blit_above(blitter, &ir, clip);
        }
        sk_fill_path(
            raw,
            clip.bounds(),
            blitter,
            ir.top,
            ir.bottom,
            path_contained_in_clip,
        );
        if raw.is_inverse_fill_type() {
            blit_below(blitter, &ir, clip);
        }
    }
    // else: what does it mean to not have a blitter if path.isInverseFillType???
}

/// Paths of a certain size cannot be anti-aliased unless externally tiled (handled by `SkDraw`).
/// `SkBitmapDevice` automatically tiles, `SkAAClip` does not so `SkRasterClipStack` converts AA
/// clips to BW clips if that's the case. `SkRegion` uses this to know when to tile and union
/// smaller `SkRegion`s together.
// Port of: src/core/SkScan_Path.cpp#L647-L650 (chrome/m156)
#[doc(alias = "PathRequiresTiling")]
#[must_use]
pub fn path_requires_tiling(bounds: &IRect) -> bool {
    let mut out = Region::new(); // ignored
    clip_to_limit(&Region::from_rect(bounds), &mut out)
}

///////////////////////////////////////////////////////////////////////////////

// Port of: src/core/SkScan_Path.cpp#L654-L671 (chrome/m156)
fn build_tri_edges(pts: &[Point], clip_rect: Option<&IRect>) -> Vec<AnyEdge> {
    let mut edges = Vec::with_capacity(5);
    let mut edge = Edge::default();
    if edge.set_line_clipped(pts[0], pts[1], clip_rect) {
        edges.push(AnyEdge::Line(edge));
        edge = Edge::default();
    }
    if edge.set_line_clipped(pts[1], pts[2], clip_rect) {
        edges.push(AnyEdge::Line(edge));
        edge = Edge::default();
    }
    if edge.set_line_clipped(pts[2], pts[0], clip_rect) {
        edges.push(AnyEdge::Line(edge));
    }
    edges
}

// Port of: src/core/SkScan_Path.cpp#L674-L710 (chrome/m156)
fn sk_fill_triangle(
    pts: &[Point],
    clip_rect: Option<&IRect>,
    blitter: &mut dyn Blitter,
    ir: &IRect,
) {
    let mut edges = build_tri_edges(pts, clip_rect);
    let count = edges.len();
    if count < 2 {
        return;
    }

    // this returns the first and last edge after they're sorted into a dlink list
    let mut list: Vec<usize> = (0..count).collect();
    let (first_edge, last_edge) = sort_edges(&mut edges, &mut list);
    let head_edge = add_head_and_tail(&mut edges, first_edge, last_edge);

    // now edge is the head of the sorted linklist
    let mut stop_y = ir.bottom;
    if let Some(clip_rect) = clip_rect {
        stop_y = stop_y.min(clip_rect.bottom);
    }
    let mut start_y = ir.top;
    if let Some(clip_rect) = clip_rect {
        start_y = start_y.max(clip_rect.top);
    }
    walk_simple_edges(&mut edges, head_edge, blitter, start_y, stop_y);
}

/// Fills the triangle `pts[..3]`, clipped to `clip` (the region of a BW raster clip; see the
/// module documentation) (`SkScan::FillTriangle`).
// Port of: src/core/SkScan_Path.cpp#L712-L757 (chrome/m156)
#[doc(alias = "FillTriangle")]
pub fn fill_triangle(pts: &[Point], clip: &Region, blitter: &mut dyn Blitter) {
    if clip.is_empty() {
        return;
    }

    let Some(r) = Rect::bounds(&pts[..3]) else {
        return;
    };

    // If r is too large (larger than can easily fit in SkFixed) then we need perform geometric
    // clipping. This is a bit of work, so we just call the general FillPath() to handle it.
    // Use FixedMax/2 as the limit so we can subtract two edges and still store that in Fixed.
    #[allow(clippy::cast_precision_loss)] // SK_MaxS16 >> 1 is exact in f32
    let limit = (i32::from(i16::MAX) >> 1) as f32;
    if !Rect::new(-limit, -limit, limit, limit).contains(&r) {
        let tri = triangle(&pts[..3], &r);
        fill_path(&tri, clip, blitter);
        return;
    }

    let ir = conservative_round_to_int(&r);
    if ir.is_empty() || !IRect::intersects(&ir, clip.bounds()) {
        return;
    }

    let mut clipper = ScanClipper::new(blitter, clip, &ir, false, false);
    let clip_rect = clipper.clip_rect().copied();
    if let Some(blitter) = clipper.blitter() {
        sk_fill_triangle(pts, clip_rect.as_ref(), blitter, &ir);
    }
}
