//! Contours of a path as integer points, the input of the sweeps. Port of
//! `modules/bentleyottmann/include/Contour.h` and `src/Contour.cpp` (chrome/m156).
//!
//! The points are the path's line endpoints rounded to 1/1024: each contour is closed by repeating
//! its first point, as the C++ does on a close verb. Curves are not supported, as in Skia (the C++
//! aborts on them).

use skia_rust_core::floating_point::{double_to_float, float_round2int};
use skia_rust_core::path::{Iter as PathIter, Path};
use skia_rust_core::path_types::PathVerb;
use skia_rust_core::point::Point as SkPoint;
use skia_rust_core::rect::IRect;

// Port of: modules/bentleyottmann/include/Contour.h#L23-L26 (chrome/m156)

/// An integer point of a contour, in units of 1/[`Contours::SCALE_FACTOR`].
#[doc(alias = "contour::Point")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Point {
    /// The x coordinate.
    pub x: i32,
    /// The y coordinate.
    pub y: i32,
}

/// One contour: its points and the bounds of those points.
#[doc(alias = "contour::Contour")]
#[derive(Clone, Copy, Debug)]
pub struct Contour<'a> {
    /// The points of the contour, borrowed from [`Contours`].
    pub points: &'a [Point],
    /// The bounding box of the points.
    pub bounds: IRect,
}

/// A contour as stored: its bounds and the end index of its points.
#[derive(Clone, Copy, Debug)]
struct CompactContour {
    bounds: IRect,
    end: i32,
}

/// The contours of a path, stored compactly.
#[doc(alias = "contour::Contours")]
#[derive(Clone, Debug)]
pub struct Contours {
    contour_start: Point,
    contour_bounds: IRect,
    points: Vec<Point>,
    compact: Vec<CompactContour>,
}

/// The bounds of a contour that has no points yet: any point extends it.
// Port of: modules/bentleyottmann/include/Contour.h#L70 (chrome/m156) (`kEmptyRect`)
const EMPTY_RECT: IRect = IRect::from_ltrb(i32::MAX, i32::MAX, i32::MIN, i32::MIN);

/// `extend_rect`: the smallest rectangle containing `r` and `p`.
// Port of: modules/bentleyottmann/src/Contour.cpp#L36-L42 (chrome/m156)
#[must_use]
fn extend_rect(r: IRect, p: Point) -> IRect {
    let left = p.x.min(r.left);
    let top = p.y.min(r.top);
    let right = p.x.max(r.right);
    let bottom = p.y.max(r.bottom);
    IRect::from_ltrb(left, top, right, bottom)
}

impl Contours {
    /// The scale of the points: coordinates are multiplied by this and rounded to integers.
    // Port of: modules/bentleyottmann/include/Contour.h#L56 (chrome/m156) (`kScaleFactor`)
    pub const SCALE_FACTOR: f64 = 1024.0;

    /// Splits `path` into contours. Line verbs and closes are kept, moves start a contour, and
    /// curves panic (`SK_ABORT("Not implemented")` in C++).
    ///
    /// # Panics
    ///
    /// If `path` has a conic, quad or cubic verb.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L12-L40 (chrome/m156)
    #[must_use]
    pub fn make(path: &Path) -> Self {
        let mut contours = Self {
            contour_start: Point::default(),
            contour_bounds: EMPTY_RECT,
            points: Vec::new(),
            compact: Vec::new(),
        };
        let mut iter = PathIter::new(path, false);
        while let Some(rec) = iter.next_rec() {
            let pts = rec.points();
            match rec.verb() {
                PathVerb::Conic | PathVerb::Quad | PathVerb::Cubic => {
                    panic!("Not implemented");
                }
                PathVerb::Move => {
                    contours.close_contour_if_needed();
                    contours.move_to_start_of_contour(pts[0]);
                }
                PathVerb::Line => {
                    contours.add_point_to_current_contour(pts[1]);
                }
                PathVerb::Close => {
                    contours.close_contour_if_needed();
                }
            }
        }

        // Close the remaining contour.
        contours.close_contour_if_needed();
        contours
    }

    /// The contour `i`, which must be less than [`Contours::len`].
    // Port of: modules/bentleyottmann/include/Contour.h#L44-L51 (chrome/m156)
    #[must_use]
    pub fn get(&self, i: usize) -> Contour<'_> {
        let compact = self.compact[i];
        let start = if i == 0 { 0 } else { self.compact[i - 1].end };
        Contour {
            points: &self.points[to_index(start)..to_index(compact.end)],
            bounds: compact.bounds,
        }
    }

    /// The contours, in order.
    // Port of: modules/bentleyottmann/include/Contour.h#L60-L67 (chrome/m156) (`begin`/`end`)
    pub fn iter(&self) -> impl Iterator<Item = Contour<'_>> + '_ {
        (0..self.len()).map(move |i| self.get(i))
    }

    /// The number of contours.
    // Port of: modules/bentleyottmann/include/Contour.h#L68 (chrome/m156)
    #[must_use]
    pub fn len(&self) -> usize {
        self.compact.len()
    }

    /// True if there are no contours.
    // Port of: modules/bentleyottmann/include/Contour.h#L72 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.compact.is_empty()
    }

    /// The segments of the contours. Not implemented, as in Skia (`SK_ABORT`).
    ///
    /// # Panics
    ///
    /// Always.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L42-L44 (chrome/m156)
    #[must_use]
    #[allow(clippy::unused_self)] // mirrors the C++ member, which aborts
    pub fn segments(&self) -> Vec<crate::myers::Segment> {
        panic!("Not implemented");
    }

    /// `RoundSkPoint`: scales `p` and rounds it to the nearest integers.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L46-L48 (chrome/m156)
    #[must_use]
    fn round_sk_point(p: SkPoint) -> Point {
        // The C++ multiplies in double and narrows to SkScalar for the rounding.
        Point {
            x: float_round2int(double_to_float(f64::from(p.x) * Self::SCALE_FACTOR)),
            y: float_round2int(double_to_float(f64::from(p.y) * Self::SCALE_FACTOR)),
        }
    }

    /// True if the current (last) contour has no points yet.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L53-L56 (chrome/m156)
    #[must_use]
    fn current_contour_is_empty(&self) -> bool {
        let last_end = self.compact.last().map_or(0, |c| c.end);
        last_end == to_i32(self.points.len())
    }

    /// Adds `p` to the current contour, first adding the contour start if the contour is empty.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L58-L66 (chrome/m156)
    fn add_point_to_current_contour(&mut self, p: SkPoint) {
        if self.current_contour_is_empty() {
            self.points.push(self.contour_start);
            self.contour_bounds = extend_rect(self.contour_bounds, self.contour_start);
        }

        let point = Self::round_sk_point(p);
        self.points.push(point);
        self.contour_bounds = extend_rect(self.contour_bounds, point);
    }

    /// Starts a new contour at `p`.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L68-L70 (chrome/m156)
    fn move_to_start_of_contour(&mut self, p: SkPoint) {
        self.contour_start = Self::round_sk_point(p);
    }

    /// Records the current contour, unless it is empty.
    // Port of: modules/bentleyottmann/src/Contour.cpp#L72-L81 (chrome/m156)
    fn close_contour_if_needed(&mut self) {
        if self.current_contour_is_empty() {
            // The current contour is empty. Don't record it.
            return;
        }

        self.compact.push(CompactContour {
            bounds: self.contour_bounds,
            end: to_i32(self.points.len()),
        });
        self.contour_bounds = EMPTY_RECT;
    }
}

/// `SkToS32` of a length: the C++ code stores point counts as `int32_t`.
#[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // paths with more than 2^31 points are out of scope
#[must_use]
fn to_i32(n: usize) -> i32 {
    n as i32
}

/// Converts a stored `int32_t` index to a `usize`.
#[allow(clippy::cast_sign_loss)] // stored ends are never negative
#[must_use]
fn to_index(n: i32) -> usize {
    n as usize
}
