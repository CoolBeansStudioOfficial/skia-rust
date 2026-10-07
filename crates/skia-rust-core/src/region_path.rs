// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRegion_path.cpp (the part that does not need the scan converter)

//! `SkRegion::addBoundaryPath` and `SkRegion::getBoundaryPath`.
//!
//! `SkRegion::setPath` is in `skia_rust_raster::region_path`: it needs the scan converter, which
//! depends on this crate.

use crate::path::Path;
use crate::path_builder::PathBuilder;
use crate::rect::Rect;
use crate::region::{Iterator, Region};
use crate::t_sort::t_q_sort;

// Port of: src/core/SkRegion_path.cpp#L330-L350 (chrome/m156)
const Y0_LINK: u8 = 0x01;
const Y1_LINK: u8 = 0x02;
const COMPLETE_LINK: u8 = Y0_LINK | Y1_LINK;

// `Edge` in SkRegion_path.cpp; `fNext` is an index into the edge array.
// Port of: src/core/SkRegion_path.cpp#L335-L363 (chrome/m156)
#[derive(Copy, Clone, Debug)]
struct Edge {
    x: i32,
    y0: i32,
    y1: i32,
    flags: u8,
    next: Option<usize>,
}

impl Edge {
    fn new(x: i32, y0: i32, y1: i32) -> Self {
        debug_assert_ne!(y0, y1);
        Edge {
            x,
            y0,
            y1,
            flags: 0,
            next: None,
        }
    }

    fn top(&self) -> i32 {
        self.y0.min(self.y1)
    }
}

// Port of: src/core/SkRegion_path.cpp#L365-L406 (chrome/m156)
fn find_link(edges: &mut [Edge], base: usize) {
    if edges[base].flags == COMPLETE_LINK {
        debug_assert!(edges[base].next.is_some());
        return;
    }

    debug_assert!(base + 1 < edges.len());

    let y0 = edges[base].y0;
    let y1 = edges[base].y1;

    let mut e = base;
    if (edges[base].flags & Y0_LINK) == 0 {
        loop {
            e += 1;
            if (edges[e].flags & Y1_LINK) == 0 && y0 == edges[e].y1 {
                debug_assert!(edges[e].next.is_none());
                edges[e].next = Some(base);
                edges[e].flags |= Y1_LINK;
                break;
            }
        }
    }

    e = base;
    if (edges[base].flags & Y1_LINK) == 0 {
        loop {
            e += 1;
            if (edges[e].flags & Y0_LINK) == 0 && y1 == edges[e].y0 {
                debug_assert!(edges[base].next.is_none());
                edges[base].next = Some(e);
                edges[e].flags |= Y0_LINK;
                break;
            }
        }
    }

    edges[base].flags = COMPLETE_LINK;
}

// Port of: src/core/SkRegion_path.cpp#L408-L440 (chrome/m156)
#[allow(clippy::cast_precision_loss)] // mirrors SkIntToScalar
fn extract_path(edges: &mut [Edge], start: usize, builder: &mut PathBuilder) -> usize {
    let mut edge = start;
    while 0 == edges[edge].flags {
        edge += 1; // skip over "used" edges
    }

    debug_assert!(edge < edges.len());

    let base = edge;
    let mut prev = edge;
    edge = edges[edge].next.expect("edges are linked");
    debug_assert_ne!(edge, base);

    let mut count = 1;
    builder.move_to((edges[prev].x as f32, edges[prev].y0 as f32));
    edges[prev].flags = 0;
    loop {
        let (p, e) = (edges[prev], edges[edge]);
        if p.x != e.x || p.y1 != e.y0 {
            // skip collinear
            builder.line_to((p.x as f32, p.y1 as f32)); // V
            builder.line_to((e.x as f32, e.y0 as f32)); // H
        }
        prev = edge;
        edge = edges[edge].next.expect("edges are linked");
        count += 1;
        edges[prev].flags = 0;
        if edge == base {
            break;
        }
    }
    builder.line_to((edges[prev].x as f32, edges[prev].y1 as f32)); // V
    builder.close();
    count
}

// `EdgeLT`.
// Port of: src/core/SkRegion_path.cpp#L442-L446 (chrome/m156)
fn edge_lt(a: &Edge, b: &Edge) -> bool {
    if a.x == b.x {
        a.top() < b.top()
    } else {
        a.x < b.x
    }
}

impl Region {
    /// Appends the outline of the region to `builder`. Returns true if the region is not empty
    /// (and so something was added).
    // Port of: src/core/SkRegion_path.cpp#L448-L504 (chrome/m156)
    #[doc(alias = "addBoundaryPath")]
    pub fn add_boundary_path(&self, builder: &mut PathBuilder) -> bool {
        if self.is_empty() {
            return false;
        }

        let bounds = self.bounds();

        if self.is_rect() {
            let r = Rect::from(*bounds); // this converts the ints to scalars
            builder.add_rect(r, None, None);
            return true;
        }

        let mut edges: Vec<Edge> = Vec::new();
        for r in Iterator::new(self) {
            edges.push(Edge::new(r.left, r.bottom, r.top));
            edges.push(Edge::new(r.right, r.top, r.bottom));
        }

        let mut count = edges.len();
        t_q_sort(&mut edges, edge_lt);

        for e in 0..count {
            find_link(&mut edges, e);
        }

        if cfg!(debug_assertions) {
            for e in &edges {
                debug_assert!(e.next.is_some());
                debug_assert_eq!(e.flags, COMPLETE_LINK);
            }
        }

        // `incReserve(count << 1)` reserves that many points and verbs.
        let reserve = i32::try_from(count << 1).unwrap_or(i32::MAX);
        builder.inc_reserve(reserve, reserve, 0);
        loop {
            debug_assert!(count > 1);
            count -= extract_path(&mut edges, 0, builder);
            if count == 0 {
                break;
            }
        }

        true
    }

    /// Returns the outline of the region as a [`Path`] (empty if the region is empty).
    // Port of: src/core/SkRegion_path.cpp#L506-L510 (chrome/m156)
    #[doc(alias = "getBoundaryPath")]
    #[must_use]
    pub fn boundary_path(&self) -> Path {
        let mut builder = PathBuilder::new();
        let _ = self.add_boundary_path(&mut builder);
        builder.detach()
    }
}
