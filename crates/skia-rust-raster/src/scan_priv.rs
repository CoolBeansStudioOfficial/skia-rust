// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScanPriv.h, src/core/SkScan_Path.cpp

//! Helpers shared by the scan converters (`SkScanPriv.h`): the clipper that picks the cheapest
//! clipping blitter, and the helpers that keep a doubly linked list of edges sorted by X.
//!
//! skia-rust: the C++ links edges with raw `fPrev`/`fNext` pointers and templates the list
//! helpers on the edge type. Here the links are indices into a slice of edges
//! ([`LinkedEdge`]), with [`NIL`] standing for the null pointer.

use skia_rust_core::fixed::Fixed;
use skia_rust_core::rect::IRect;
use skia_rust_core::region::Region;

use crate::blitter::{Blitter, RectClipBlitter, RgnClipBlitter};
use crate::edge::AnyEdge;

/// controls how much we super-sample (when we use that scan convertion)
// Port of: src/core/SkScanPriv.h#L17 (chrome/m156)
pub const SUPERSAMPLE_SHIFT: i32 = 2;

/// `(int)x` for a float, as the oracle's x86-64 build evaluates it (`cvttss2si`): truncation,
/// and `i32::MIN` for NaN and out-of-range values (undefined behaviour in C++). Used by the
/// analytic edges' fixed-point setup.
#[allow(clippy::cast_possible_truncation)] // the range is checked
pub(crate) fn float_to_int(v: f32) -> i32 {
    if v >= 2_147_483_648.0_f32 || v < -2_147_483_648.0_f32 || v.is_nan() {
        i32::MIN
    } else {
        v as i32
    }
}

/// The "null pointer" of an edge link.
pub const NIL: usize = usize::MAX;

/// An edge that can be linked into the sorted edge list of a scan converter.
pub trait LinkedEdge {
    /// `fPrev` (an index, or [`NIL`]).
    fn prev(&self) -> usize;
    /// `fNext` (an index, or [`NIL`]).
    fn next(&self) -> usize;
    /// Sets `fPrev`.
    fn set_prev(&mut self, prev: usize);
    /// Sets `fNext`.
    fn set_next(&mut self, next: usize);
    /// `fX`: the X where the edge currently starts scanning.
    fn x(&self) -> Fixed;
}

impl LinkedEdge for AnyEdge {
    fn prev(&self) -> usize {
        self.prev
    }
    fn next(&self) -> usize {
        self.next
    }
    fn set_prev(&mut self, prev: usize) {
        self.prev = prev;
    }
    fn set_next(&mut self, next: usize) {
        self.next = next;
    }
    fn x(&self) -> Fixed {
        self.x
    }
}

/// Picks the blitter to use for a path whose integer bounds are `bounds`, given the clip
/// (`SkScanClipper`). Holds the wrapper blitter it may need.
///
/// skia-rust: the C++ keeps both wrapper blitters as members and points `fBlitter` at one of
/// them; here the kinds are an enum. The debug-only `SkRectClipCheckBlitter` is not ported.
// Port of: src/core/SkScanPriv.h#L22-L41 (chrome/m156)
#[doc(alias = "SkScanClipper")]
#[derive(Debug)]
pub struct ScanClipper<'a> {
    blitter: ClipperBlitter<'a>,
    clip_rect: Option<IRect>,
}

#[derive(Debug)]
enum ClipperBlitter<'a> {
    /// `fBlitter == nullptr`: blit nothing. Holds the blitter that was passed in, which the C++
    /// caller still has a pointer to ([`ScanClipper::clipped_out_blitter`]).
    Nothing(&'a mut dyn Blitter),
    Plain(&'a mut dyn Blitter),
    Rect(RectClipBlitter<'a>),
    Rgn(RgnClipBlitter<'a>),
}

impl<'a> ScanClipper<'a> {
    /// If the caller is drawing an inverse-fill path, then it passes true for `skip_reject_test`,
    /// so we don't abort drawing just because the src bounds (`ir`) is outside of the clip.
    // Port of: src/core/SkScan_Path.cpp#L478-L513 (chrome/m156)
    #[must_use]
    pub fn new(
        blitter: &'a mut dyn Blitter,
        clip: &'a Region,
        ir: &IRect,
        skip_reject_test: bool,
        ir_pre_clipped: bool,
    ) -> Self {
        use skia_rust_core::rect::Contains;

        let clip_bounds = *clip.bounds();
        let mut clip_rect = Some(clip_bounds);
        if !skip_reject_test && !IRect::intersects(&clip_bounds, ir) {
            // completely clipped out
            return ScanClipper {
                blitter: ClipperBlitter::Nothing(blitter),
                clip_rect,
            };
        }

        let blitter = if clip.is_rect() {
            if !ir_pre_clipped && clip_bounds.contains(ir) {
                clip_rect = None;
                ClipperBlitter::Plain(blitter)
            } else {
                // only need a wrapper blitter if we're horizontally clipped
                if ir_pre_clipped || clip_bounds.left > ir.left || clip_bounds.right < ir.right {
                    ClipperBlitter::Rect(RectClipBlitter::new(blitter, clip_bounds))
                } else {
                    ClipperBlitter::Plain(blitter)
                }
            }
        } else {
            ClipperBlitter::Rgn(RgnClipBlitter::new(blitter, clip))
        };
        ScanClipper { blitter, clip_rect }
    }

    /// The blitter to draw with, or `None` to draw nothing (`getBlitter`).
    #[doc(alias = "getBlitter")]
    pub fn blitter(&mut self) -> Option<&mut dyn Blitter> {
        match &mut self.blitter {
            ClipperBlitter::Nothing(_) => None,
            ClipperBlitter::Plain(b) => Some(&mut **b),
            ClipperBlitter::Rect(b) => Some(b),
            ClipperBlitter::Rgn(b) => Some(b),
        }
    }

    /// The blitter that was passed in, when everything was clipped out (`blitter()` is `None`).
    ///
    /// skia-rust: the C++ caller keeps using its own `SkBlitter*` (`AntiFillPath` blits the
    /// clip region of a clipped-out inverse fill through it); here the clipper holds the borrow.
    pub fn clipped_out_blitter(&mut self) -> Option<&mut dyn Blitter> {
        match &mut self.blitter {
            ClipperBlitter::Nothing(b) => Some(&mut **b),
            _ => None,
        }
    }

    /// The rectangle the edges must be clipped to, or `None` if they need no clipping
    /// (`getClipRect`).
    #[doc(alias = "getClipRect")]
    #[must_use]
    pub fn clip_rect(&self) -> Option<&IRect> {
        self.clip_rect.as_ref()
    }
}

/// Blits the rects above `avoid`, clipped to `clip`.
// Port of: src/core/SkScan_Path.cpp#L433-L444 (chrome/m156)
#[doc(alias = "sk_blit_above")]
pub fn blit_above(blitter: &mut dyn Blitter, avoid: &IRect, clip: &Region) {
    let cr = clip.bounds();
    let tmp = IRect {
        left: cr.left,
        right: cr.right,
        top: cr.top,
        bottom: avoid.top,
    };
    if !tmp.is_empty() {
        blitter.blit_rect_region(&tmp, clip);
    }
}

/// Blits the rects below `avoid`, clipped to `clip`.
// Port of: src/core/SkScan_Path.cpp#L446-L457 (chrome/m156)
#[doc(alias = "sk_blit_below")]
pub fn blit_below(blitter: &mut dyn Blitter, avoid: &IRect, clip: &Region) {
    let cr = clip.bounds();
    let tmp = IRect {
        left: cr.left,
        right: cr.right,
        top: avoid.bottom,
        bottom: cr.bottom,
    };
    if !tmp.is_empty() {
        blitter.blit_rect_region(&tmp, clip);
    }
}

/// Unlinks `edge` from its neighbours.
// Port of: src/core/SkScanPriv.h#L43-L47 (chrome/m156)
pub fn remove_edge<E: LinkedEdge>(edges: &mut [E], edge: usize) {
    let (prev, next) = (edges[edge].prev(), edges[edge].next());
    edges[prev].set_next(next);
    edges[next].set_prev(prev);
}

/// Links `edge` into the list right after `after_me`.
// Port of: src/core/SkScanPriv.h#L49-L55 (chrome/m156)
pub fn insert_edge_after<E: LinkedEdge>(edges: &mut [E], edge: usize, after_me: usize) {
    let after_next = edges[after_me].next();
    edges[edge].set_prev(after_me);
    edges[edge].set_next(after_next);
    edges[after_next].set_prev(edge);
    edges[after_me].set_next(edge);
}

/// Moves `edge` backwards in the list until the list is sorted by X again.
// Port of: src/core/SkScanPriv.h#L57-L66 (chrome/m156)
pub fn backward_insert_edge_based_on_x<E: LinkedEdge>(edges: &mut [E], edge: usize) {
    let x = edges[edge].x();
    let mut prev = edges[edge].prev();
    while edges[prev].prev() != NIL && edges[prev].x() > x {
        prev = edges[prev].prev();
    }
    if edges[prev].next() != edge {
        remove_edge(edges, edge);
        insert_edge_after(edges, edge, prev);
    }
}

/// Start from the right side, searching backwards for the point to begin the new edge list
/// insertion, marching forwards from here. The implementation could have started from the left
/// of the prior insertion, and search to the right, or with some additional caching, binary
/// search the starting point. More work could be done to determine optimal new edge insertion.
// Port of: src/core/SkScanPriv.h#L68-L76 (chrome/m156)
#[must_use]
pub fn backward_insert_start<E: LinkedEdge>(edges: &[E], prev: usize, x: Fixed) -> usize {
    let mut prev = prev;
    while edges[prev].prev() != NIL && edges[prev].x() > x {
        prev = edges[prev].prev();
    }
    prev
}
