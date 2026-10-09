// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h (chrome/m156)

//! Generates a middle-out triangulation of a polygon. Conceptually, middle-out emits one large
//! triangle with vertices on both endpoints and a middle point, then recurses on both sides of the
//! new triangle. Middle-out produces drastically less work for the rasterizer as compared to a
//! linear triangle strip or fan.
//!
//! The triangulator does not know or store all the vertices in the polygon at once. The caller
//! pushes each vertex in linear order, and rather than relying on recursion, it manipulates an
//! O(log N) stack to determine the correct middle-out triangulation.

use skia_rust_core::path::Path;
use skia_rust_core::path_priv::{RangeIter, iterate, pts_in_iter};
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point;

/// `kStackPreallocCount`.
const K_STACK_PREALLOC_COUNT: usize = 32;

/// Internal representation of how we store vertices on our stack.
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L43-L58 (chrome/m156), `StackVertex`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct StackVertex {
    point: Point,
    // How many polygon vertices away is this vertex from the previous vertex on the stack? The
    // ith stack element's vertex index in the original polygon is the sum of the deltas of the
    // stack elements 1..=i. `vertex_idx_delta` of the bottom element is always 0.
    vertex_idx_delta: i32,
}

/// Generates a middle-out triangulation as vertices are pushed.
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L28-L168 (chrome/m156), `MiddleOutPolygonTriangulator`.
#[derive(Debug)]
pub struct MiddleOutPolygonTriangulator {
    vertex_stack: Vec<StackVertex>,
    // Index of the top element of `vertex_stack` (`fTop`).
    top: usize,
}

impl MiddleOutPolygonTriangulator {
    /// `maxPushVertexCalls` is an upper bound on the number of times the caller will call
    /// [`Self::push_vertex`]. The caller must not call it more times than this.
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L107-L122 (chrome/m156), constructor.
    #[must_use]
    pub fn new(max_push_vertex_calls: i32, start_point: Point) -> Self {
        debug_assert!(max_push_vertex_calls >= 0);
        // Determine the deepest our stack can ever go: SkNextLog2(maxPushVertexCalls) + 1.
        // (SkNextLog2(v) is 32 - clz(v - 1), with unsigned wrap-around as in the C++.)
        let max_stack_depth = (32
            - max_push_vertex_calls
                .cast_unsigned()
                .wrapping_sub(1)
                .leading_zeros()) as usize
            + 1;
        let alloc_count = max_stack_depth.max(K_STACK_PREALLOC_COUNT);
        let mut vertex_stack = vec![StackVertex::default(); alloc_count];
        // The stack will always contain a starting point. This is an implicit moveTo(0, 0)
        // initially, but will be overridden if moveTo() gets called before adding geometry.
        vertex_stack[0] = StackVertex {
            point: start_point,
            vertex_idx_delta: 0,
        };
        Self {
            vertex_stack,
            top: 0,
        }
    }

    /// Returns an RAII object that first allows the caller to iterate the triangles it will pop,
    /// pops those triangles, and finally pushes `pt` onto the vertex stack.
    ///
    /// Our topology wants triangles that have the same vertexIdxDelta on both sides: e.g., a run
    /// of 9 points should be triangulated as:
    ///
    /// ```text
    ///    [0, 1, 2], [2, 3, 4], [4, 5, 6], [6, 7, 8]  // vertexIdxDelta == 1
    ///    [0, 2, 4], [4, 6, 8]  // vertexIdxDelta == 2
    ///    [0, 4, 8]  // vertexIdxDelta == 4
    /// ```
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L124-L147 (chrome/m156), `pushVertex`.
    pub fn push_vertex(&mut self, pt: Point) -> PoppedTriangleStack<'_> {
        // Find as many new triangles as we can pop off the stack that have equal-delta sides.
        // (This is a stack-based implementation of the recursive example from the class comment.)
        let mut end_vertex = self.top;
        let mut vertex_idx_delta = 1;
        while self.vertex_stack[end_vertex].vertex_idx_delta == vertex_idx_delta {
            end_vertex -= 1;
            vertex_idx_delta *= 2;
        }

        // Once the above triangles are popped, push 'pt' to the top of the stack.
        let new_top_vertex = end_vertex + 1;
        let new_top_value = StackVertex {
            point: pt,
            vertex_idx_delta,
        };
        debug_assert!(new_top_vertex < self.vertex_stack.len()); // Is the stack big enough?
        PoppedTriangleStack {
            middle_out: Some(self),
            last_point: pt,
            end: end_vertex,
            new_top_vertex,
            new_top_value,
        }
    }

    /// Returns an RAII object that first allows the caller to iterate the remaining triangles,
    /// then resets the vertex stack with `new_start_point`.
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L149-L159 (chrome/m156), `closeAndMove`.
    pub fn close_and_move(&mut self, new_start_point: Point) -> PoppedTriangleStack<'_> {
        // Add an implicit line back to the starting point.
        let start_pt = self.vertex_stack[0].point;
        // Triangulate the rest of the polygon. Since we simply have to finish now, we can't be
        // picky anymore about getting a pure middle-out topology.
        let end_vertex = self.top.min(1);
        // Once every remaining triangle is popped, reset the vertex stack with newStartPoint.
        PoppedTriangleStack {
            middle_out: Some(self),
            last_point: start_pt,
            end: end_vertex,
            new_top_vertex: 0,
            new_top_value: StackVertex {
                point: new_start_point,
                vertex_idx_delta: 0,
            },
        }
    }

    /// Returns an RAII object that first allows the caller to iterate the remaining triangles,
    /// then resets the vertex stack with the same starting point as it had before.
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L161-L164 (chrome/m156), `close`.
    pub fn close(&mut self) -> PoppedTriangleStack<'_> {
        let start = self.vertex_stack[0].point;
        self.close_and_move(start)
    }
}

/// The triangles popped by one [`MiddleOutPolygonTriangulator::push_vertex`] (or `close`) call.
///
/// Iterate [`Self::iter`] to get the triangles; when this value is dropped the stack is
/// updated (`PoppedTriangleStack`'s destructor in the C++).
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L53-L104 (chrome/m156), `PoppedTriangleStack`.
#[must_use = "the popped triangles must be emitted before the stack is updated"]
#[derive(Debug)]
pub struct PoppedTriangleStack<'a> {
    // `None` once the stack update has been applied.
    middle_out: Option<&'a mut MiddleOutPolygonTriangulator>,
    last_point: Point,
    end: usize,
    new_top_vertex: usize,
    new_top_value: StackVertex,
}

impl PoppedTriangleStack<'_> {
    /// The popped triangles, from the top of the stack down.
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L80-L88 (chrome/m156), `begin` and `end`.
    #[must_use]
    pub fn iter(&self) -> PoppedTriangles<'_> {
        match self.middle_out.as_deref() {
            Some(middle_out) => PoppedTriangles {
                stack: &middle_out.vertex_stack,
                cur: middle_out.top,
                end: self.end,
                last_point: self.last_point,
            },
            // The update was already applied: nothing left to pop.
            None => PoppedTriangles {
                stack: &[],
                cur: self.end,
                end: self.end,
                last_point: self.last_point,
            },
        }
    }
}

impl<'s> IntoIterator for &'s PoppedTriangleStack<'_> {
    type Item = (Point, Point, Point);
    type IntoIter = PoppedTriangles<'s>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl Drop for PoppedTriangleStack<'_> {
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L62-L67 (chrome/m156), destructor.
    fn drop(&mut self) {
        if let Some(middle_out) = self.middle_out.take() {
            middle_out.top = self.new_top_vertex;
            middle_out.vertex_stack[self.new_top_vertex] = self.new_top_value;
        }
    }
}

/// Iterator over the triangles of a [`PoppedTriangleStack`]: `(stack[i-1], stack[i], last)`.
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L70-L79 (chrome/m156), `Iter`.
#[derive(Clone, Debug)]
pub struct PoppedTriangles<'s> {
    stack: &'s [StackVertex],
    cur: usize,
    end: usize,
    last_point: Point,
}

impl Iterator for PoppedTriangles<'_> {
    type Item = (Point, Point, Point);

    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L74-L77 (chrome/m156), `operator*` and `operator++`.
    fn next(&mut self) -> Option<Self::Item> {
        if self.cur == self.end {
            return None;
        }
        let triangle = (
            self.stack[self.cur - 1].point,
            self.stack[self.cur].point,
            self.last_point,
        );
        self.cur -= 1;
        Some(triangle)
    }
}

/// Transforms and pushes a path's inner fan vertices onto a [`MiddleOutPolygonTriangulator`].
/// Example usage:
///
/// ```text
/// let mut it = PathMiddleOutFanIter::new(&path);
/// while !it.done() {
///     let stack = it.next_stack();
///     for (p0, p1, p2) in &stack {
///         // emit the triangle
///     }
/// }
/// ```
// Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L184-L226 (chrome/m156), `PathMiddleOutFanIter`.
#[derive(Debug)]
pub struct PathMiddleOutFanIter<'a> {
    middle_out: MiddleOutPolygonTriangulator,
    path_iter: RangeIter<'a>,
    done: bool,
}

impl<'a> PathMiddleOutFanIter<'a> {
    /// `PathMiddleOutFanIter(const SkPath& path)`.
    ///
    /// # Panics
    ///
    /// Panics if the path has more verbs than `i32::MAX`.
    #[must_use]
    pub fn new(path: &'a Path) -> Self {
        Self {
            middle_out: MiddleOutPolygonTriangulator::new(
                // The caller never pushes more verbs than the path has.
                i32::try_from(path.count_verbs()).expect("path has too many verbs"),
                Point::new(0.0, 0.0),
            ),
            path_iter: iterate(path),
            done: false,
        }
    }

    /// Returns true once every verb has been consumed.
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L193 (chrome/m156), `done`.
    #[must_use]
    pub fn done(&self) -> bool {
        self.done
    }

    /// Consumes the next verb and returns the stack update it produces.
    ///
    /// # Panics
    ///
    /// Panics if called after [`Self::done`] returned true (a debug assertion in the C++).
    // Port of: src/gpu/tessellate/MiddleOutPolygonTriangulator.h#L195-L214 (chrome/m156), `nextStack`.
    pub fn next_stack(&mut self) -> PoppedTriangleStack<'_> {
        debug_assert!(!self.done);
        let Some((verb, pts, _weight)) = self.path_iter.next() else {
            self.done = true;
            return self.middle_out.close();
        };
        match verb {
            PathVerb::Move => self.middle_out.close_and_move(pts[0]),
            PathVerb::Line | PathVerb::Quad | PathVerb::Conic | PathVerb::Cubic => {
                let pt = pts[usize::try_from(pts_in_iter(verb) - 1).expect("non-negative")];
                self.middle_out.push_vertex(pt)
            }
            PathVerb::Close => self.middle_out.close(),
        }
    }
}
