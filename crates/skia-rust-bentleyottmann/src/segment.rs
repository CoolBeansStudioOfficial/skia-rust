//! Segments of integer points and the exact predicates the sweep uses on them. Port of
//! `modules/bentleyottmann/include/Segment.h` and `src/Segment.cpp` (chrome/m156).

use std::cmp::Ordering;

use crate::int96::{Int96, multiply};
use crate::point::Point;

/// A segment between two integer points. The endpoints are kept as given; [`Segment::upper`]
/// and [`Segment::lower`] give them in sweep order.
#[doc(alias = "bentleyottmann::Segment")]
#[derive(Clone, Copy, Debug)]
pub struct Segment {
    /// The first endpoint as given.
    pub p0: Point,
    /// The second endpoint as given.
    pub p1: Point,
}

impl Segment {
    /// Creates a segment from its endpoints, which are kept in the order given.
    #[must_use]
    pub const fn new(p0: Point, p1: Point) -> Self {
        Self { p0, p1 }
    }

    /// The higher point: the upper end. For a horizontal segment it is the left most one.
    // Port of: modules/bentleyottmann/src/Segment.cpp#L18-L20 (chrome/m156)
    #[must_use]
    pub fn upper(&self) -> Point {
        // std::min(p0, p1) returns p1 only when p1 < p0.
        if self.p1 < self.p0 { self.p1 } else { self.p0 }
    }

    /// The lower point: the lower end. For a horizontal segment it is the right most one.
    // Port of: modules/bentleyottmann/src/Segment.cpp#L22-L24 (chrome/m156)
    #[must_use]
    pub fn lower(&self) -> Point {
        // std::max(p0, p1) returns p0 only when p0 < p1 is false.
        if self.p0 < self.p1 { self.p1 } else { self.p0 }
    }

    /// The bounding box as `(left, top, right, bottom)`.
    // Port of: modules/bentleyottmann/src/Segment.cpp#L27-L31 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let (l, r) = (self.p0.x.min(self.p1.x), self.p0.x.max(self.p1.x));
        let (t, b) = (self.p0.y.min(self.p1.y), self.p0.y.max(self.p1.y));
        (l, t, r, b)
    }
}

impl PartialEq for Segment {
    /// `operator==`: equal upper and lower points, so the orientation does not matter.
    // Port of: modules/bentleyottmann/src/Segment.cpp#L33-L35 (chrome/m156)
    fn eq(&self, other: &Self) -> bool {
        self.upper() == other.upper() && self.lower() == other.lower()
    }
}

impl Eq for Segment {}

impl PartialOrd for Segment {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Segment {
    /// `operator<`: `std::make_tuple(upper, lower)` lexicographic.
    // Port of: modules/bentleyottmann/src/Segment.cpp#L37-L39 (chrome/m156)
    fn cmp(&self, other: &Self) -> Ordering {
        (self.upper(), self.lower()).cmp(&(other.upper(), other.lower()))
    }
}

/// A crossing of two segments at a point. Port of `bentleyottmann::Crossing`.
#[doc(alias = "bentleyottmann::Crossing")]
#[derive(Clone, Copy, Debug)]
pub struct Crossing {
    /// The first crossing segment.
    pub s0: Segment,
    /// The second crossing segment.
    pub s1: Segment,
    /// The point where they cross.
    pub crossing: Point,
}

/// True if the bounding boxes of `s0` and `s1` do not overlap in their interiors: the boxes touch
/// or are apart, so the segments cannot cross.
// Port of: modules/bentleyottmann/src/Segment.cpp#L41-L49 (chrome/m156)
#[must_use]
pub fn no_intersection_by_bounding_box(s0: &Segment, s1: &Segment) -> bool {
    let (left0, top0, right0, bottom0) = s0.bounds();
    let (left1, top1, right1, bottom1) = s1.bounds();
    // If the sides of the box touch, then there is no new intersection.
    right0 <= left1 || right1 <= left0 || bottom0 <= top1 || bottom1 <= top0
}

/// The 64-bit cross product `v0 x v1`, with C++ `int64_t` wrapping.
#[must_use]
fn cross_s64(v0: Point, v1: Point) -> i64 {
    let (x0, y0) = (i64::from(v0.x), i64::from(v0.y));
    let (x1, y1) = (i64::from(v1.x), i64::from(v1.y));
    x0.wrapping_mul(y1).wrapping_sub(y0.wrapping_mul(x1))
}

// The derivation of the intersection point is in Skia's Segment.cpp: the parameters t and s of the
// two segments are the quotients of cross products, so the crossing test is exact in 64 bits and
// only the final point is rounded.
/// Finds the intersection of `s0` and `s1`. Returns `None` if there is no intersection.
///
/// Note this intersection assumes that line segments do not include their end points.
// Port of: modules/bentleyottmann/src/Segment.cpp#L92-L166 (chrome/m156)
#[must_use]
#[allow(clippy::cast_precision_loss)] // mirrors static_cast<double>(int64_t) in the C++
#[allow(clippy::cast_possible_truncation)] // mirrors the C++ double -> int32_t conversion
#[allow(clippy::many_single_char_names)] // the points and bounds keep the C++ names x0..y3, l, t, r, b
pub fn intersect(s0: &Segment, s1: &Segment) -> Option<Point> {
    // Check if the bounds intersect.
    if no_intersection_by_bounding_box(s0, s1) {
        return None;
    }

    // Create the end Points for s0 and s1.
    let p0 = s0.upper();
    let p1 = s0.lower();
    let p2 = s1.upper();
    let p3 = s1.lower();

    if p0 == p2 || p1 == p3 || p1 == p2 || p3 == p0 {
        // Lines don't intersect if they share an end point.
        return None;
    }

    // Create the Q, R, and T.
    let q = p1 - p0;
    let r = p2 - p0;
    let t = p3 - p2;

    // Calculate the cross products needed for calculating s and t.
    let qxr = cross_s64(q, r);
    let txr = cross_s64(t, r);
    let txq = cross_s64(t, q);

    if txq == 0 {
        // Both t and s are either < 0 or > 1 because the denominator is 0.
        return None;
    }

    // t = (Q x R) / (T x Q). s = (T x R) / (T x Q). Check that t & s are on [0, 1].
    if (qxr ^ txq) < 0 || (txr ^ txq) < 0 {
        // The division is negative and t or s < 0.
        return None;
    }

    if txq > 0 {
        if qxr > txq || txr > txq {
            // t or s is greater than 1.
            return None;
        }
    } else if qxr < txq || txr < txq {
        // t or s is greater than 1.
        return None;
    }

    // Calculate the intersection using doubles.
    let t = qxr as f64 / txq as f64;
    debug_assert!((0.0..=1.0).contains(&t));
    let x = (t * f64::from(p3.x.wrapping_sub(p2.x)) + f64::from(p2.x)).round() as i32;
    let y = (t * f64::from(p3.y.wrapping_sub(p2.y)) + f64::from(p2.y)).round() as i32;
    Some(Point::new(x, y))
}

// The comparison is:
//     x0 + (y - y0)(x1 - x0) / (y1 - y0) <? x2 + (y - y2)(x3 - x2) / (y3 - y2)
// Removing the divides by cross multiplying gives a 64-bit int times a 32-bit int, which is a
// 96-bit int, so `Int96` is used. Because y0 <= y1 and y2 <= y3, the differences (y1 - y0) and
// (y3 - y2) are positive, so the multiplication does not change any sign.
/// Compares `s0` and `s1` at the sweep line `y`. It is an error to pass segments that do not
/// intersect the horizontal line at `y`.
// Port of: modules/bentleyottmann/src/Segment.cpp#L168-L201 (chrome/m156)
#[must_use]
pub fn less_than_at(s0: &Segment, s1: &Segment, y: i32) -> bool {
    let (l0, t0, r0, b0) = s0.bounds();
    let (l1, t1, r1, b1) = s1.bounds();
    debug_assert!(t0 <= y && y <= b0);
    debug_assert!(t1 <= y && y <= b1);

    // Return true if the bounding box of s0 is fully to the left of s1.
    if r0 < l1 {
        return true;
    }

    // Return false if the bounding box of s0 is fully to the right of s1.
    if r1 < l0 {
        return false;
    }

    // Check the x intercepts along the horizontal line at y.
    // Make s0 be (x0, y0) -> (x1, y1) and s1 be (x2, y2) -> (x3, y3).
    let Point { x: x0, y: y0 } = s0.upper();
    let Point { x: x1, y: y1 } = s0.lower();
    let Point { x: x2, y: y2 } = s1.upper();
    let Point { x: x3, y: y3 } = s1.lower();

    let s0_y_diff = i64::from(y.wrapping_sub(y0));
    let s1_y_diff = i64::from(y.wrapping_sub(y2));
    let s0_y_delta = i64::from(y1.wrapping_sub(y0));
    let s1_y_delta = i64::from(y3.wrapping_sub(y2));
    let x0_offset = i64::from(x0)
        .wrapping_mul(s0_y_delta)
        .wrapping_add(s0_y_diff.wrapping_mul(i64::from(x1.wrapping_sub(x0))));
    let x2_offset = i64::from(x2)
        .wrapping_mul(s1_y_delta)
        .wrapping_add(s1_y_diff.wrapping_mul(i64::from(x3.wrapping_sub(x2))));

    let s0_factor: Int96 = multiply(x0_offset, y3.wrapping_sub(y2));
    let s1_factor: Int96 = multiply(x2_offset, y1.wrapping_sub(y0));
    s0_factor < s1_factor
}

/// Given a horizontal line defined by `p.y`, is `p.x` < the x value where the horizontal line
/// passes `segment`.
// Port of: modules/bentleyottmann/src/Segment.cpp#L203-L232 (chrome/m156)
#[must_use]
#[allow(clippy::many_single_char_names)] // the points and bounds keep the C++ names x0..y3, l, t, r, b
pub fn point_less_than_segment_in_x(p: Point, segment: &Segment) -> bool {
    let (l, t, r, b) = segment.bounds();
    // Ensure that the segment intersects the horizontal sweep line.
    debug_assert!(t <= p.y && p.y <= b);

    // Fast answers using bounding boxes.
    if p.x < l {
        return true;
    } else if p.x >= r {
        return false;
    }

    let Point { x: x0, y: y0 } = segment.upper();
    let Point { x: x1, y: y1 } = segment.lower();
    let Point { x: x2, y: y2 } = p;

    // For a point and a segment the comparison is:
    //    (x2 - x0)(y1 - y0) < (x1 - x0)(y2 - y0)
    i64::from(x2.wrapping_sub(x0)).wrapping_mul(i64::from(y1.wrapping_sub(y0)))
        < i64::from(y2.wrapping_sub(y0)).wrapping_mul(i64::from(x1.wrapping_sub(x0)))
}

// The design of the rounded comparisons allows their use with `partition_point`: they are true
// for the segments that lie left of the rounded point, and false from the first one that does not.
// The rounding definition is `x < floor(s(y) + 1/2)`, which expands to `(x - 1/2) <= s(y) < (x + 1/2)`.

/// The rounding lower bound: `s(y)` is at least `x - 1/2`. The result is true when `s(y) < x - 1/2`.
///
/// The ordering of the parameters facilitates using `partition_point`, the counterpart of
/// `std::lower_bound`.
// Port of: modules/bentleyottmann/src/Segment.cpp#L234-L270 (chrome/m156)
#[must_use]
#[allow(clippy::many_single_char_names)] // the points and bounds keep the C++ names x0..y3, l, t, r, b
pub fn rounded_point_less_than_segment_in_x_lower(s: &Segment, p: Point) -> bool {
    let (l, t, r, b) = s.bounds();
    let Point { x, y } = p;

    // Ensure that the segment intersects the horizontal sweep line.
    debug_assert!(t <= y && y <= b);

    // In the comparisons below, x is really x - 1/2.
    if r < x {
        // s is entirely < p.
        return true;
    } else if x <= l {
        // s is entirely > p. This also handles vertical lines, so we don't have to handle them
        // below.
        return false;
    }

    let Point { x: x0, y: y0 } = s.upper();
    let Point { x: x1, y: y1 } = s.lower();

    // Horizontal - from the guards above we know that p is on s.
    if y0 == y1 {
        return false;
    }

    // s is not horizontal or vertical.
    debug_assert!(x0 != x1 && y0 != y1);

    // (x1 - x0)(y - y0) < (x - x0 - 1/2)(y1 - y0), doubled to stay in integers:
    // 2(x1 - x0)(y - y0) < (2(x - x0) - 1)(y1 - y0)
    2i64.wrapping_mul(i64::from(x1.wrapping_sub(x0)))
        .wrapping_mul(i64::from(y.wrapping_sub(y0)))
        < (2i64
            .wrapping_mul(i64::from(x.wrapping_sub(x0)))
            .wrapping_sub(1))
        .wrapping_mul(i64::from(y1.wrapping_sub(y0)))
}

/// The rounding upper bound: `s(y) < x + 1/2`.
///
/// The result is true when `s(y) < x + 1/2`, the counterpart of `std::lower_bound` with
/// `rounded_point_less_than_segment_in_x_upper`.
// Port of: modules/bentleyottmann/src/Segment.cpp#L272-L305 (chrome/m156)
#[must_use]
#[allow(clippy::many_single_char_names)] // the points and bounds keep the C++ names x0..y3, l, t, r, b
pub fn rounded_point_less_than_segment_in_x_upper(s: &Segment, p: Point) -> bool {
    let (l, t, r, b) = s.bounds();
    let Point { x, y } = p;

    // Ensure that the segment intersects the horizontal sweep line.
    debug_assert!(t <= y && y <= b);

    // In the comparisons below, x is really x + 1/2.
    if r <= x {
        // s is entirely < p.
        return true;
    } else if x < l {
        // s is entirely > p. This also handles vertical lines, so we don't have to handle them
        // below.
        return false;
    }

    let Point { x: x0, y: y0 } = s.upper();
    let Point { x: x1, y: y1 } = s.lower();

    // Horizontal - from the guards above we know that p is on s.
    if y0 == y1 {
        return false;
    }

    // s is not horizontal or vertical.
    debug_assert!(x0 != x1 && y0 != y1);

    // (x1 - x0)(y - y0) < (x - x0 + 1/2)(y1 - y0), doubled to stay in integers:
    // 2(x1 - x0)(y - y0) < (2(x - x0) + 1)(y1 - y0)
    2i64.wrapping_mul(i64::from(x1.wrapping_sub(x0)))
        .wrapping_mul(i64::from(y.wrapping_sub(y0)))
        < (2i64
            .wrapping_mul(i64::from(x.wrapping_sub(x0)))
            .wrapping_add(1))
        .wrapping_mul(i64::from(y1.wrapping_sub(y0)))
}

/// Compares the slopes of two segments. A horizontal slope is greater than all other slopes,
/// or equal to another horizontal one. The slope of non-horizontal segments increases from the
/// smallest along the negative x-axis, counterclockwise, to the largest along the positive x-axis.
///
/// Returns `-1` if `slope(s0) < slope(s1)`, `0` if they are equal, and `1` if `slope(s0) >
/// slope(s1)`.
// Port of: modules/bentleyottmann/src/Segment.cpp#L307-L335 (chrome/m156)
#[must_use]
pub fn compare_slopes(s0: &Segment, s1: &Segment) -> i32 {
    let s0_delta = s0.lower() - s0.upper();
    let s1_delta = s1.lower() - s1.upper();

    // Handle the horizontal cases to avoid dealing with infinities.
    if s0_delta.y == 0 || s1_delta.y == 0 {
        // A non horizontal s0 is less than a horizontal s1, and a horizontal s0 is greater.
        return if s0_delta.y != 0 {
            -1
        } else {
            i32::from(s1_delta.y != 0)
        };
    }

    // Compare s0Delta.x / s0Delta.y ? s1Delta.x / s1Delta.y using the alternate slope form: the
    // delta ys are always positive, so the cross multiply does not change the sign.
    let lhs = i64::from(s0_delta.x).wrapping_mul(i64::from(s1_delta.y));
    let rhs = i64::from(s1_delta.x).wrapping_mul(i64::from(s0_delta.y));
    match lhs.cmp(&rhs) {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Greater => 1,
        std::cmp::Ordering::Equal => 0,
    }
}
