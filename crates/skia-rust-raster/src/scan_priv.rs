// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkScanPriv.h, src/core/SkScan_Path.cpp (SkScanClipper,
// sk_blit_above, sk_blit_below)

//! Helpers shared by the scan converters (`SkScanPriv.h`).
//!
//! skia-rust: the edge lists of the scan converters are doubly linked through indices into the
//! array that owns the edges (the C++ links raw pointers); [`NO_EDGE`] stands for null. The
//! list templates (`remove_edge`, `insert_edge_after`, ...) work on any edge type implementing
//! [`LinkedEdge`]. `SkScanClipper`, `sk_blit_above` and `sk_blit_below` are declared here and
//! defined in `SkScan_Path.cpp`; they live here so both the non-AA and the AA scan converters can
//! use them.

use skia_rust_core::fixed::Fixed;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::region::Region;

use crate::blitter::{Blitter, RectClipBlitter, RgnClipBlitter};

/// Controls how much we super-sample (when we use that scan conversion).
// Port of: src/core/SkScanPriv.h#L15-L16 (chrome/m156)
#[doc(alias = "SK_SUPERSAMPLE_SHIFT")]
pub const SUPERSAMPLE_SHIFT: i32 = 2;

/// `(int)x` for a float, as the oracle's x86-64 build evaluates it (`cvttss2si`): truncation,
/// and `i32::MIN` for NaN and out-of-range values (undefined behaviour in C++).
#[allow(clippy::cast_possible_truncation)] // the range is checked
pub(crate) fn float_to_int(v: f32) -> i32 {
    if v >= 2_147_483_648.0_f32 || v < -2_147_483_648.0_f32 || v.is_nan() {
        i32::MIN
    } else {
        v as i32
    }
}

/// The "null pointer" of the scan converters' linked edge lists.
pub const NO_EDGE: usize = usize::MAX;

/// An edge in a scan converter's doubly linked edge list (`fNext`, `fPrev`, `fX`).
pub trait LinkedEdge {
    /// `fNext`.
    fn next(&self) -> usize;
    /// `fPrev`.
    fn prev(&self) -> usize;
    /// Sets `fNext`.
    fn set_next(&mut self, next: usize);
    /// Sets `fPrev`.
    fn set_prev(&mut self, prev: usize);
    /// `fX`.
    fn x(&self) -> Fixed;
}

/// Unlinks `edge` from its list.
// Port of: src/core/SkScanPriv.h#L40-L44 (chrome/m156)
pub fn remove_edge<E: LinkedEdge>(edges: &mut [E], edge: usize) {
    let prev = edges[edge].prev();
    let next = edges[edge].next();
    edges[prev].set_next(next);
    edges[next].set_prev(prev);
}

/// Links `edge` into the list after `after_me`.
// Port of: src/core/SkScanPriv.h#L46-L52 (chrome/m156)
pub fn insert_edge_after<E: LinkedEdge>(edges: &mut [E], edge: usize, after_me: usize) {
    let after_next = edges[after_me].next();
    edges[edge].set_prev(after_me);
    edges[edge].set_next(after_next);
    edges[after_next].set_prev(edge);
    edges[after_me].set_next(edge);
}

/// Moves `edge` backwards in the list until the list is sorted by x again.
// Port of: src/core/SkScanPriv.h#L54-L65 (chrome/m156)
pub fn backward_insert_edge_based_on_x<E: LinkedEdge>(edges: &mut [E], edge: usize) {
    let x = edges[edge].x();
    let mut prev = edges[edge].prev();
    while edges[prev].prev() != NO_EDGE && edges[prev].x() > x {
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
// Port of: src/core/SkScanPriv.h#L67-L77 (chrome/m156)
pub fn backward_insert_start<E: LinkedEdge>(edges: &[E], mut prev: usize, x: Fixed) -> usize {
    while edges[prev].prev() != NO_EDGE && edges[prev].x() > x {
        prev = edges[prev].prev();
    }
    prev
}

/// What [`ScanClipper::apply`] decided.
#[derive(Debug)]
pub enum ScanClip<'b> {
    /// Completely clipped out (`getBlitter() == nullptr`); holds the blitter passed in.
    ClippedOut(&'b mut dyn Blitter),
    /// Blit through `blitter`; `clip_rect` is `getClipRect()` (`None` when no clipping is
    /// needed).
    Blit {
        /// `getBlitter()`.
        blitter: &'b mut dyn Blitter,
        /// `getClipRect()`.
        clip_rect: Option<IRect>,
    },
}

/// Sets up the wrapper blitter (if any) that applies a clip region to a scan conversion
/// (`SkScanClipper`).
///
/// skia-rust: the C++ constructor is [`ScanClipper::apply`], which returns what `getBlitter()`
/// and `getClipRect()` would. The `SK_DEBUG`-only `SkRectClipCheckBlitter` is not ported.
// Port of: src/core/SkScanPriv.h#L18-L34 (chrome/m156)
#[doc(alias = "SkScanClipper")]
#[derive(Debug, Default)]
pub struct ScanClipper<'a> {
    rect_blitter: Option<RectClipBlitter<'a>>,
    rgn_blitter: Option<RgnClipBlitter<'a>>,
}

impl<'a> ScanClipper<'a> {
    /// Creates a clipper.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// If the caller is drawing an inverse-fill path, then it passes true for
    /// `skip_reject_test`, so we don't abort drawing just because the src bounds (`ir`) is
    /// outside of the clip.
    // Port of: src/core/SkScan_Path.cpp#L493-L536 (chrome/m156)
    pub fn apply<'b>(
        &'b mut self,
        blitter: &'a mut dyn Blitter,
        clip: Option<&'a Region>,
        ir: &IRect,
        skip_reject_test: bool,
        ir_pre_clipped: bool,
    ) -> ScanClip<'b>
    where
        'a: 'b,
    {
        let Some(clip) = clip else {
            return ScanClip::Blit {
                blitter,
                clip_rect: None,
            };
        };
        let clip_rect = *clip.bounds();
        if !skip_reject_test && !IRect::intersects(&clip_rect, ir) {
            // completely clipped out
            return ScanClip::ClippedOut(blitter);
        }

        if clip.is_rect() {
            if !ir_pre_clipped && clip_rect.contains(ir) {
                ScanClip::Blit {
                    blitter,
                    clip_rect: None,
                }
            } else if ir_pre_clipped || clip_rect.left > ir.left || clip_rect.right < ir.right {
                // only need a wrapper blitter if we're horizontally clipped
                ScanClip::Blit {
                    blitter: self
                        .rect_blitter
                        .insert(RectClipBlitter::new(blitter, clip_rect)),
                    clip_rect: Some(clip_rect),
                }
            } else {
                ScanClip::Blit {
                    blitter,
                    clip_rect: Some(clip_rect),
                }
            }
        } else {
            ScanClip::Blit {
                blitter: self.rgn_blitter.insert(RgnClipBlitter::new(blitter, clip)),
                clip_rect: Some(clip_rect),
            }
        }
    }
}

/// Blits the rects above `ir`, clipped to `clip`.
// Port of: src/core/SkScan_Path.cpp#L466-L477 (chrome/m156)
#[doc(alias = "sk_blit_above")]
pub fn blit_above(blitter: &mut dyn Blitter, ir: &IRect, clip: &Region) {
    let cr = clip.bounds();
    let tmp = IRect {
        left: cr.left,
        right: cr.right,
        top: cr.top,
        bottom: ir.top,
    };
    if !tmp.is_empty() {
        blitter.blit_rect_region(&tmp, clip);
    }
}

/// Blits the rects below `ir`, clipped to `clip`.
// Port of: src/core/SkScan_Path.cpp#L479-L490 (chrome/m156)
#[doc(alias = "sk_blit_below")]
pub fn blit_below(blitter: &mut dyn Blitter, ir: &IRect, clip: &Region) {
    let cr = clip.bounds();
    let tmp = IRect {
        left: cr.left,
        right: cr.right,
        top: ir.bottom,
        bottom: cr.bottom,
    };
    if !tmp.is_empty() {
        blitter.blit_rect_region(&tmp, clip);
    }
}
