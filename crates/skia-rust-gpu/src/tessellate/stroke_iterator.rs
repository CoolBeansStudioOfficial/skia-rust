// Copyright 2020 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/tessellate/StrokeIterator.h (chrome/m156)

//! Iterates the stroke geometry defined by a path and a stroke. Closes and square caps become
//! lines, round caps become circles, and every stroke comes with its previous stroke so the
//! caller knows the join.
//!
//! # Storage choice
//!
//! The C++ queue holds `const SkPoint*` pointers. Some of them point into the path's point array,
//! others into the iterator's own `fClosePts`, `fEndingCapPts` and `fBeginningCapPts`. Holding
//! those pointers in Rust would make the iterator self-referential. Instead every queue entry
//! copies its points (at most four, as `[Point; 4]`) and its optional weight when it is enqueued.
//! This keeps the same iteration order and emits the same verbs and points: the pointed-to data
//! is immutable for as long as the queue can reference it (the path is borrowed for the iterator's
//! lifetime, and the cap and close storage is only rewritten after the queue has drained a
//! contour). Consumers read the same points they would have read through the pointer. A
//! `kCircle` entry only ever reads its first point, so for it the copy holds that point in slot 0.

use skia_rust_core::matrix::Matrix;
use skia_rust_core::paint::Cap;
use skia_rust_core::path::Path;
use skia_rust_core::path_priv::{RangeIter, iterate};
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::{Point, Vector};
use skia_rust_core::stroke_rec::StrokeRec;

/// The verbs `StrokeIterator` reports. The first four mirror `SkPathVerb`; `Circle` is a
/// stroke-width circle drawn as a 180-degree point stroke. The last two notify callers to update
/// their own iteration state.
// Port of: src/gpu/tessellate/StrokeIterator.h#L38-L49 (chrome/m156), `StrokeIterator::Verb`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verb {
    Line,
    Quad,
    Conic,
    Cubic,
    /// A stroke-width circle drawn as a 180-degree point stroke.
    Circle,
    /// Helper verb: the next stroke moves within the contour and is not joined to the previous one.
    MoveWithinContour,
    /// Helper verb: the contour is finished.
    ContourFinished,
}

impl Verb {
    /// `IsVerbGeometric`: the verbs that describe stroke geometry.
    // Port of: src/gpu/tessellate/StrokeIterator.h#L51 (chrome/m156), `StrokeIterator::IsVerbGeometric`.
    #[must_use]
    pub fn is_geometric(self) -> bool {
        matches!(
            self,
            Self::Line | Self::Quad | Self::Conic | Self::Cubic | Self::Circle
        )
    }

    /// The index of a verb's last point, as `SkPathPriv::PtsInIter(verb) - 1`.
    fn last_point_index(self) -> usize {
        match self {
            Self::Line => 1,
            Self::Quad | Self::Conic => 2,
            Self::Cubic => 3,
            Self::Circle | Self::MoveWithinContour | Self::ContourFinished => {
                unreachable!("only Line, Quad, Conic and Cubic have a last point to cap")
            }
        }
    }
}

/// One queue entry: the verb, its points (copied, see the module docs) and its weight.
#[derive(Clone, Copy, Debug)]
struct Entry {
    verb: Verb,
    pts: [Point; 4],
    w: Option<f32>,
}

impl Entry {
    const EMPTY: Self = Self {
        verb: Verb::ContourFinished,
        pts: [Point { x: 0.0, y: 0.0 }; 4],
        w: None,
    };
}

/// Copies up to four points from `src`, the rest stay zero.
fn copy_pts(src: &[Point]) -> [Point; 4] {
    let mut pts = [Point::default(); 4];
    pts[..src.len()].copy_from_slice(src);
    pts
}

/// The points of a `kCircle` entry at `p`: only slot 0 is read by consumers.
fn circle_pts(p: Point) -> [Point; 4] {
    let mut pts = [Point::default(); 4];
    pts[0] = p;
    pts
}

/// Returns true when `pts` describes a zero-length segment of the given verb.
fn is_degenerate(verb: PathVerb, pts: &[Point]) -> bool {
    match verb {
        // `if (p3 == p2 && p2 == p1 && p1 == p0)`
        PathVerb::Cubic => pts[3] == pts[2] && pts[2] == pts[1] && pts[1] == pts[0],
        // `if (p2 == p1 && p1 == p0)`
        PathVerb::Conic | PathVerb::Quad => pts[2] == pts[1] && pts[1] == pts[0],
        // `if (p1 == p0)`
        PathVerb::Line => pts[1] == pts[0],
        PathVerb::Move | PathVerb::Close => false,
    }
}

/// Iterates the stroke geometry of a path. Usage:
///
/// ```ignore
/// let mut iter = StrokeIterator::new(&path, &stroke, Some(&view_matrix));
/// while iter.next() {  // Call next() first.
///     iter.verb();
///     iter.pts();
///     iter.w();
///     iter.prev_verb();
///     iter.prev_pts();
/// }
/// ```
// Port of: src/gpu/tessellate/StrokeIterator.h#L30-L378 (chrome/m156), `StrokeIterator`.
pub struct StrokeIterator<'a> {
    // For hairlines. Only read when the stroke is a hairline.
    view_matrix: Option<&'a Matrix>,
    stroke: &'a StrokeRec,
    // Info and iterators from the original path.
    iter: RangeIter<'a>,
    // Info for the current contour we are iterating.
    first_verb_in_contour: Verb,
    first_pts_in_contour: [Point; 4],
    first_w_in_contour: Option<f32>,
    last_degenerate_stroke_pt: Option<Point>,
    // The queue is implemented as a roll-over array with a floating front index.
    queue: [Entry; QUEUE_BUFFER_COUNT],
    queue_front_idx: usize,
    queue_count: usize,
}

const QUEUE_BUFFER_COUNT: usize = 8;

impl std::fmt::Debug for StrokeIterator<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StrokeIterator")
            .field("first_verb_in_contour", &self.first_verb_in_contour)
            .field("queue_count", &self.queue_count)
            .finish_non_exhaustive()
    }
}

impl<'a> StrokeIterator<'a> {
    /// `StrokeIterator(const SkPath& path, const SkStrokeRec* stroke, const SkMatrix* viewMatrix)`.
    // Port of: src/gpu/tessellate/StrokeIterator.h#L30-L36 (chrome/m156), `StrokeIterator::StrokeIterator`.
    #[must_use]
    pub fn new(path: &'a Path, stroke: &'a StrokeRec, view_matrix: Option<&'a Matrix>) -> Self {
        Self {
            view_matrix,
            stroke,
            iter: iterate(path),
            first_verb_in_contour: Verb::Line,
            first_pts_in_contour: [Point::default(); 4],
            first_w_in_contour: None,
            last_degenerate_stroke_pt: None,
            queue: [Entry::EMPTY; QUEUE_BUFFER_COUNT],
            queue_front_idx: 0,
            queue_count: 0,
        }
    }

    /// Must be called first. Loads the next pair of "prev" and "current" stroke. Returns false if
    /// iteration is complete.
    // Port of: src/gpu/tessellate/StrokeIterator.h#L60-L127 (chrome/m156), `StrokeIterator::next`.
    #[allow(clippy::should_implement_trait)] // the C++ name; the verbs are read through accessors
    pub fn next(&mut self) -> bool {
        if self.queue_count > 0 {
            debug_assert!(self.queue_count >= 2);
            self.pop_front();
            if self.queue_count >= 2 {
                return true;
            }
            debug_assert_eq!(self.queue_count, 1);
            if self.at_verb(0) == Verb::ContourFinished {
                // Don't let "kContourFinished" be prevVerb at the start of the next contour.
                self.queue_count = 0;
            }
        }
        while let Some((verb, pts, w)) = self.iter.next() {
            match verb {
                PathVerb::Move => {
                    if !self.finish_open_contour() {
                        continue;
                    }
                }
                PathVerb::Close => {
                    if self.queue_count == 0 {
                        self.last_degenerate_stroke_pt = Some(pts[0]);
                        continue;
                    }
                    if pts[0] != self.first_pts_in_contour[0] {
                        // Draw a line back to the contour's starting point.
                        let close_pts = [pts[0], self.first_pts_in_contour[0]];
                        self.enqueue(Verb::Line, copy_pts(&close_pts), None);
                    }
                    // Repeat the first verb, this time as the "current" stroke instead of the prev.
                    self.enqueue(
                        self.first_verb_in_contour,
                        self.first_pts_in_contour,
                        self.first_w_in_contour,
                    );
                    self.enqueue(Verb::ContourFinished, Entry::EMPTY.pts, None);
                    self.last_degenerate_stroke_pt = None;
                }
                PathVerb::Line | PathVerb::Quad | PathVerb::Conic | PathVerb::Cubic => {
                    if is_degenerate(verb, pts) {
                        self.last_degenerate_stroke_pt = Some(pts[0]);
                        continue;
                    }
                    let geometric = match verb {
                        PathVerb::Line => Verb::Line,
                        PathVerb::Quad => Verb::Quad,
                        PathVerb::Conic => Verb::Conic,
                        _ => Verb::Cubic,
                    };
                    let copied = copy_pts(pts);
                    self.enqueue(geometric, copied, w);
                    if self.queue_count == 1 {
                        // Defer the first verb until the end when we know what it's joined to.
                        self.first_verb_in_contour = geometric;
                        self.first_pts_in_contour = copied;
                        self.first_w_in_contour = w;
                        continue;
                    }
                }
            }
            debug_assert!(self.queue_count >= 2);
            return true;
        }
        self.finish_open_contour()
    }

    /// The verb of the previous stroke.
    #[must_use]
    pub fn prev_verb(&self) -> Verb {
        self.at_verb(0)
    }

    /// The points of the previous stroke.
    #[must_use]
    pub fn prev_pts(&self) -> &[Point; 4] {
        self.at_pts(0)
    }

    /// The verb of the current stroke.
    #[must_use]
    pub fn verb(&self) -> Verb {
        self.at_verb(1)
    }

    /// The points of the current stroke.
    #[must_use]
    pub fn pts(&self) -> &[Point; 4] {
        self.at_pts(1)
    }

    /// The weight of the current stroke. Only conics have one.
    #[must_use]
    pub fn w(&self) -> f32 {
        self.at_w(1)
    }

    /// The verb of the contour's first stroke.
    #[must_use]
    pub fn first_verb_in_contour(&self) -> Verb {
        debug_assert!(self.queue_count > 0);
        self.first_verb_in_contour
    }

    /// The points of the contour's first stroke.
    #[must_use]
    pub fn first_pts_in_contour(&self) -> &[Point; 4] {
        debug_assert!(self.queue_count > 0);
        &self.first_pts_in_contour
    }

    fn at_verb(&self, i: usize) -> Verb {
        debug_assert!(i < self.queue_count);
        self.queue[(self.queue_front_idx + i) & (QUEUE_BUFFER_COUNT - 1)].verb
    }

    fn back_verb(&self) -> Verb {
        self.at_verb(self.queue_count - 1)
    }

    fn at_pts(&self, i: usize) -> &[Point; 4] {
        debug_assert!(i < self.queue_count);
        &self.queue[(self.queue_front_idx + i) & (QUEUE_BUFFER_COUNT - 1)].pts
    }

    fn back_pts(&self) -> &[Point; 4] {
        self.at_pts(self.queue_count - 1)
    }

    fn at_w(&self, i: usize) -> f32 {
        debug_assert!(i < self.queue_count);
        self.queue[(self.queue_front_idx + i) & (QUEUE_BUFFER_COUNT - 1)]
            .w
            .expect("w() is only read for verbs that carry a weight")
    }

    fn enqueue(&mut self, verb: Verb, pts: [Point; 4], w: Option<f32>) {
        debug_assert!(self.queue_count < QUEUE_BUFFER_COUNT);
        let i = (self.queue_front_idx + self.queue_count) & (QUEUE_BUFFER_COUNT - 1);
        self.queue[i] = Entry { verb, pts, w };
        self.queue_count += 1;
    }

    fn pop_front(&mut self) {
        debug_assert!(self.queue_count > 0);
        self.queue_front_idx = (self.queue_front_idx + 1) & (QUEUE_BUFFER_COUNT - 1);
        self.queue_count -= 1;
    }

    /// Finishes the current contour without closing it. Enqueues any necessary caps as well as the
    /// contour's first stroke that we deferred at the beginning. Returns false and makes no changes
    /// if the current contour was already finished.
    // Port of: src/gpu/tessellate/StrokeIterator.h#L129-L226 (chrome/m156), `StrokeIterator::finishOpenContour`.
    #[allow(clippy::many_single_char_names)] // the hairline matrix terms keep the C++ names a, b, c, d
    fn finish_open_contour(&mut self) -> bool {
        if self.queue_count > 0 {
            debug_assert!(matches!(
                self.back_verb(),
                Verb::Line | Verb::Quad | Verb::Conic | Verb::Cubic
            ));
            match self.stroke.cap() {
                Cap::Butt => {
                    // There are no caps, but inject a "move" so the first stroke doesn't get joined
                    // with the end of the contour when it's processed.
                    self.enqueue(
                        Verb::MoveWithinContour,
                        self.first_pts_in_contour,
                        self.first_w_in_contour,
                    );
                }
                Cap::Round => {
                    // The "kCircle" verb serves as our barrier to prevent the first stroke from
                    // getting joined with the end of the contour. We just need to make sure that
                    // the first point of the contour goes last.
                    let back_idx = self.back_verb().last_point_index();
                    let back_pt = self.back_pts()[back_idx];
                    self.enqueue(Verb::Circle, circle_pts(back_pt), None);
                    self.enqueue(
                        Verb::Circle,
                        self.first_pts_in_contour,
                        self.first_w_in_contour,
                    );
                }
                Cap::Square => {
                    // Fills in the ending and beginning cap points.
                    let (ending_cap_pts, beginning_cap_pts) = self.fill_square_cap_points();
                    // Append the ending cap onto the current contour.
                    self.enqueue(Verb::Line, copy_pts(&ending_cap_pts), None);
                    // Move to the beginning cap and append it right before (and joined to) the
                    // first stroke (that we will add below).
                    self.enqueue(Verb::MoveWithinContour, copy_pts(&beginning_cap_pts), None);
                    self.enqueue(Verb::Line, copy_pts(&beginning_cap_pts), None);
                }
            }
        } else if let Some(last) = self.last_degenerate_stroke_pt {
            // queue_count=0 means this subpath is zero length. Generates caps on its location.
            //
            //   "Any zero length subpath ...  shall be stroked if the 'stroke-linecap' property has
            //   a value of round or square producing respectively a circle or a square."
            //
            //   (https://www.w3.org/TR/SVG11/painting.html#StrokeProperties)
            match self.stroke.cap() {
                Cap::Butt => {
                    // Zero-length contour with butt caps. There are no caps and no first stroke to
                    // generate.
                    return false;
                }
                Cap::Round => {
                    self.enqueue(Verb::Circle, circle_pts(last), None);
                    // Setting the "first" stroke as the circle causes it to be added again below,
                    // this time as the "current" stroke.
                    self.first_verb_in_contour = Verb::Circle;
                    self.first_pts_in_contour = circle_pts(last);
                    self.first_w_in_contour = None;
                }
                Cap::Square => {
                    let outset = if self.stroke.is_hairline_style() {
                        // If the stroke is hairline, draw a 1x1 device-space square instead. This
                        // is equivalent to using:
                        //
                        //   outset = inverse(fViewMatrix).mapVector(.5, 0)
                        //
                        // And since the matrix cannot have perspective, we only need to invert the
                        // upper 2x2 of the viewMatrix to achieve this.
                        let vm = self
                            .view_matrix
                            .expect("hairline strokes need a view matrix");
                        debug_assert!(!vm.has_perspective());
                        let a = vm.scale_x();
                        let b = vm.skew_x();
                        let c = vm.skew_y();
                        let d = vm.scale_y();
                        let det = a * d - b * c;
                        if det > 0.0 {
                            // outset = inverse(|a b|) * |.5|
                            //                  |c d|    | 0|
                            //
                            //     == 1/det * | d -b| * |.5|
                            //                |-c  a|   | 0|
                            //
                            //     == | d| * .5/det
                            //        |-c|
                            let s = 0.5 / det;
                            Vector::new(d * s, -c * s)
                        } else {
                            Vector::new(1.0, 0.0)
                        }
                    } else {
                        // Implement degenerate square caps as a stroke-width square in path space.
                        Vector::new(self.stroke.width() * 0.5, 0.0)
                    };
                    let ending = [last - outset, last + outset];
                    // Add the square first as the "prev" join.
                    self.enqueue(Verb::Line, copy_pts(&ending), None);
                    self.enqueue(Verb::MoveWithinContour, copy_pts(&ending), None);
                    // Setting the "first" stroke as the square causes it to be added again below,
                    // this time as the "current" stroke.
                    self.first_verb_in_contour = Verb::Line;
                    self.first_pts_in_contour = copy_pts(&ending);
                    self.first_w_in_contour = None;
                }
            }
        } else {
            // This contour had no lines, beziers, or "close" verbs. There are no caps and no first
            // stroke to generate.
            return false;
        }
        // Repeat the first verb, this time as the "current" stroke instead of the prev.
        self.enqueue(
            self.first_verb_in_contour,
            self.first_pts_in_contour,
            self.first_w_in_contour,
        );
        self.enqueue(Verb::ContourFinished, Entry::EMPTY.pts, None);
        self.last_degenerate_stroke_pt = None;
        true
    }

    /// We implement square caps as two extra "kLine" verbs. This method finds the endpoints for
    /// those lines, returning `(fEndingCapPts, fBeginningCapPts)`.
    // Port of: src/gpu/tessellate/StrokeIterator.h#L228-L290 (chrome/m156), `StrokeIterator::fillSquareCapPoints`.
    #[allow(clippy::if_not_else, clippy::many_single_char_names)] // C++ branch order; the hairline matrix terms keep the C++ names a, b, c, d
    fn fill_square_cap_points(&self) -> ([Point; 2], [Point; 2]) {
        let stroke = self.stroke;
        // Find the endpoints of the cap at the end of the contour.
        let last_pts = *self.back_pts();
        let last_verb = self.back_verb();
        let mut last_tangent = match last_verb {
            Verb::Cubic => {
                let t = last_pts[3] - last_pts[2];
                if !t.is_zero() {
                    t
                } else {
                    let t = last_pts[2] - last_pts[1];
                    if !t.is_zero() {
                        t
                    } else {
                        last_pts[1] - last_pts[0]
                    }
                }
            }
            Verb::Conic | Verb::Quad => {
                let t = last_pts[2] - last_pts[1];
                if !t.is_zero() {
                    t
                } else {
                    last_pts[1] - last_pts[0]
                }
            }
            Verb::Line => last_pts[1] - last_pts[0],
            Verb::Circle | Verb::MoveWithinContour | Verb::ContourFinished => {
                unreachable!("square caps are only computed for geometric verbs")
            }
        };
        if !stroke.is_hairline_style() {
            // Extend the cap by 1/2 stroke width.
            last_tangent = scale(last_tangent, (0.5 * stroke.width()) / last_tangent.length());
        } else {
            // Extend the cap by what will be 1/2 pixel after transformation.
            let vm = self
                .view_matrix
                .expect("hairline strokes need a view matrix");
            last_tangent = scale(last_tangent, 0.5 / vm.map_vector(last_tangent).length());
        }
        let last_point = last_pts[last_verb.last_point_index()];
        let ending = [last_point, last_point + last_tangent];

        // Find the endpoints of the cap at the beginning of the contour.
        let first_pts = self.first_pts_in_contour;
        let mut first_tangent = first_pts[1] - first_pts[0];
        if first_tangent.is_zero() {
            first_tangent = first_pts[2] - first_pts[0];
            if first_tangent.is_zero() {
                first_tangent = first_pts[3] - first_pts[0];
            }
        }
        if !stroke.is_hairline_style() {
            // Set the the cap back by 1/2 stroke width.
            first_tangent = scale(
                first_tangent,
                (-0.5 * stroke.width()) / first_tangent.length(),
            );
        } else {
            // Set the cap back by what will be 1/2 pixel after transformation.
            let vm = self
                .view_matrix
                .expect("hairline strokes need a view matrix");
            first_tangent = scale(first_tangent, -0.5 / vm.map_vector(first_tangent).length());
        }
        let beginning = [first_pts[0] + first_tangent, first_pts[0]];
        (ending, beginning)
    }
}

/// `SkVector::operator*=(scalar)`: scales both components.
fn scale(v: Vector, s: f32) -> Vector {
    Vector::new(v.x * s, v.y * s)
}

#[cfg(test)]
mod tests {
    use skia_rust_core::paint::{Cap, Join};
    use skia_rust_core::path_builder::PathBuilder;
    use skia_rust_core::stroke_rec::{InitStyle, StrokeRec};

    use super::{StrokeIterator, Verb};

    // An open polyline with butt caps: the iterator yields the (prev, current) pairs of the
    // deferred-first-verb walk: (A,B), (B,move), (move,A), (A,finished).
    #[test]
    fn open_polyline_butt_caps_walks_joins_in_order() {
        let mut builder = PathBuilder::new();
        builder
            .move_to((0.0, 0.0))
            .line_to((10.0, 0.0))
            .line_to((10.0, 10.0));
        let path = builder.detach();
        let mut stroke = StrokeRec::new(InitStyle::Hairline);
        stroke.set_stroke_style(2.0, false);
        stroke.set_stroke_params(Cap::Butt, Join::Miter, 4.0);

        let mut iter = StrokeIterator::new(&path, &stroke, None);
        let mut seen = Vec::new();
        while iter.next() {
            seen.push((iter.prev_verb(), iter.verb()));
        }
        assert_eq!(
            seen,
            vec![
                (Verb::Line, Verb::Line),
                (Verb::Line, Verb::MoveWithinContour),
                (Verb::MoveWithinContour, Verb::Line),
                (Verb::Line, Verb::ContourFinished),
            ]
        );
    }
}
