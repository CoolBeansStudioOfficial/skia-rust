//! The Myers sweep: a sweep that finds the crossings of a set of segments, with the same
//! exact integer predicates as the Bentley-Ottmann sweep. Port of
//! `modules/bentleyottmann/include/Myers.h` and `src/Myers.cpp` (chrome/m156).
//!
//! This is the `myers` namespace of Skia, with its own `Point`, `Segment` and `Crossing`: the
//! types are not the `bentleyottmann` ones.
//!
//! Where the C++ sorts with `std::sort`, which is not stable, the port uses a stable sort. The
//! events that compare equal are then handled in input order, which is one of the orders the C++
//! may produce; the crossings found do not depend on that order.

use std::cmp::Ordering;

use crate::int96::{Int96, multiply_i32_i64};

// -- Point ----------------------------------------------------------------------------------------

/// A point of the Myers sweep. Points are ordered by `y` first, then `x`.
// Port of: modules/bentleyottmann/include/Myers.h#L17-L21 (chrome/m156)
#[doc(alias = "myers::Point")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Point {
    /// The x coordinate.
    pub x: i32,
    /// The y coordinate.
    pub y: i32,
}

impl Point {
    /// Creates a point from its coordinates.
    #[must_use]
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

impl PartialOrd for Point {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Point {
    /// `operator<`: `std::tie(p0.y, p0.x) < std::tie(p1.y, p1.x)`.
    // Port of: modules/bentleyottmann/include/Myers.h#L24-L26 (chrome/m156)
    fn cmp(&self, other: &Self) -> Ordering {
        (self.y, self.x).cmp(&(other.y, other.x))
    }
}

impl std::ops::Sub for Point {
    type Output = Self;

    /// `operator-(Point, Point)`, with C++ `int32_t` wrapping.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L23-L25 (chrome/m156)
    fn sub(self, rhs: Self) -> Self {
        Self {
            x: self.x.wrapping_sub(rhs.x),
            y: self.y.wrapping_sub(rhs.y),
        }
    }
}

// -- Segment --------------------------------------------------------------------------------------

/// An undirected edge whose points are stored in sweep order: the upper point is before the
/// lower one.
// Port of: modules/bentleyottmann/include/Myers.h#L36-L66 (chrome/m156)
#[doc(alias = "myers::Segment")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Segment {
    upper: Point,
    lower: Point,
}

impl Segment {
    /// Makes a segment from two points in either order.
    // Port of: modules/bentleyottmann/include/Myers.h#L40-L41 (chrome/m156)
    #[must_use]
    pub fn new(p0: Point, p1: Point) -> Self {
        // std::minmax(p0, p1)
        let (upper, lower) = if p1 < p0 { (p1, p0) } else { (p0, p1) };
        debug_assert_ne!(upper, lower);
        debug_assert!(upper < lower);
        Self { upper, lower }
    }

    /// The upper point.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L60-L62 (chrome/m156)
    #[must_use]
    pub fn upper(&self) -> Point {
        self.upper
    }

    /// The lower point.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L64-L66 (chrome/m156)
    #[must_use]
    pub fn lower(&self) -> Point {
        self.lower
    }

    /// The bounding box as `(left, top, right, bottom)`.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L68-L71 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> (i32, i32, i32, i32) {
        let (left, right) = (
            self.upper.x.min(self.lower.x),
            self.upper.x.max(self.lower.x),
        );
        (left, self.upper.y, right, self.lower.y)
    }

    /// True if the segment is horizontal.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L73-L75 (chrome/m156)
    #[must_use]
    pub fn is_horizontal(&self) -> bool {
        self.upper.y == self.lower.y
    }

    /// True if the segment is vertical.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L77-L79 (chrome/m156)
    #[must_use]
    pub fn is_vertical(&self) -> bool {
        self.upper.x == self.lower.x
    }
}

// -- Crossing -------------------------------------------------------------------------------------

/// A crossing of two segments, the higher one first.
// Port of: modules/bentleyottmann/include/Myers.h#L70-L86 (chrome/m156)
#[doc(alias = "myers::Crossing")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Crossing {
    higher: Segment,
    lower: Segment,
}

impl Crossing {
    /// Makes a crossing of two segments, ordered so the smaller one is first.
    // Port of: modules/bentleyottmann/include/Myers.h#L72 (chrome/m156)
    #[must_use]
    pub fn new(s0: Segment, s1: Segment) -> Self {
        // std::minmax(s0, s1)
        let (higher, lower) = if s1 < s0 { (s1, s0) } else { (s0, s1) };
        Self { higher, lower }
    }

    /// The first segment of the crossing, in segment order.
    #[must_use]
    pub fn higher(&self) -> Segment {
        self.higher
    }

    /// The second segment of the crossing, in segment order.
    #[must_use]
    pub fn lower(&self) -> Segment {
        self.lower
    }
}

// -- Predicates -----------------------------------------------------------------------------------

/// `cross(d0, d1)`: the 64-bit cross product of two vectors.
///
/// ```text
///    | d0x   d1x |
///    | d0y   d1y |
/// ```
// Port of: modules/bentleyottmann/src/Myers.cpp#L82-L88 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn cross(d0: Point, d1: Point) -> i64 {
    let (d0x, d0y) = (i64::from(d0.x), i64::from(d0.y));
    let (d1x, d1y) = (i64::from(d1.x), i64::from(d1.y));
    d0x.wrapping_mul(d1y).wrapping_sub(d1x.wrapping_mul(d0y))
}

/// Compares the slope of `s0` with the slope of `s1`, where `slope(s) = dx / dy` (the inverse of
/// the usual form), for segments that are not horizontal. Horizontal segments compare greater.
// Port of: modules/bentleyottmann/src/Myers.cpp#L90-L121 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn compare_slopes(s0: &Segment, s1: &Segment) -> i64 {
    // Handle cases involving horizontal segments.
    if s0.is_horizontal() || s1.is_horizontal() {
        if s0.is_horizontal() && s1.is_horizontal() {
            // slope(s0) == slope(s1)
            return 0;
        }
        if s0.is_horizontal() {
            // slope(s0) > slope(s1)
            return 1;
        }
        // slope(s0) < slope(s1)
        return -1;
    }

    let (u0, l0) = (s0.upper(), s0.lower());
    let (u1, l1) = (s1.upper(), s1.lower());
    let d0 = l0 - u0;
    let d1 = l1 - u1;

    // Since horizontal lines are handled separately and because of the ordering of points for a
    // segment, then d0y and d1y should always be positive.
    debug_assert!(d0.y > 0 && d1.y > 0);

    // The sign of the cross product is the sign of slope(s0) - slope(s1), since the delta ys are
    // positive.
    cross(d0, d1)
}

/// True if slope(s0) < slope(s1). See [`compare_slopes`].
// Port of: modules/bentleyottmann/src/Myers.cpp#L123-L126 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn slope_s0_less_than_slope_s1(s0: &Segment, s1: &Segment) -> bool {
    compare_slopes(s0, s1) < 0
}

/// Compares a point `p` with a segment `s` that spans `p` vertically: negative if `p` is left of
/// `s`, zero if on it, positive if right of it.
// Port of: modules/bentleyottmann/src/Myers.cpp#L128-L158 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn compare_point_to_segment(p: Point, s: &Segment) -> i64 {
    let (u, l) = (s.upper(), s.lower());

    // The segment must span p vertically.
    debug_assert!(u.y <= p.y && p.y <= l.y);

    // Check horizontal extents.
    {
        let (left, right) = (u.x.min(l.x), u.x.max(l.x));
        if p.x < left {
            return -1;
        }
        if right < p.x {
            return 1;
        }
    }

    // If s is horizontal, then p is on the interval [u.x, l.x].
    if s.is_horizontal() {
        return 0;
    }

    // The point p < s when (p - u) x (l - u) < 0. The other relations are the same cross product.
    let d_u_to_p = p - u;
    let d_s = l - u;
    debug_assert!(d_s.y > 0);
    cross(d_u_to_p, d_s)
}

/// Used with `partition_point` to find where `to_insert` goes in a sweep line ordered by
/// [`s0_less_than_s1_at_y`]: true when `segment` is before `to_insert`.
// Port of: modules/bentleyottmann/src/Myers.cpp#L160-L166 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn segment_less_than_upper_to_insert(segment: &Segment, to_insert: &Segment) -> bool {
    let compare = compare_point_to_segment(to_insert.upper(), segment);
    // compare > 0 when segment < to_insert.upper().
    (compare > 0) || ((compare == 0) && slope_s0_less_than_slope_s1(segment, to_insert))
}

/// True if `s0(y) < s1(y)`, or if they are equal there and the slope of `s0` is smaller.
// Port of: modules/bentleyottmann/src/Myers.cpp#L168-L212 (chrome/m156)
#[doc(hidden)]
#[must_use]
pub fn s0_less_than_s1_at_y(s0: &Segment, s1: &Segment, y: i32) -> bool {
    // Neither s0 nor s1 are horizontal because this is used during the sorting phase.
    debug_assert!(!s0.is_horizontal() && !s1.is_horizontal());

    let (u0, l0) = (s0.upper(), s0.lower());
    let (u1, l1) = (s1.upper(), s1.lower());

    let (left0, right0) = (u0.x.min(l0.x), u0.x.max(l0.x));
    let (left1, right1) = (u1.x.min(l1.x), u1.x.max(l1.x));
    if right0 < left1 {
        return true;
    } else if right1 < left0 {
        return false;
    }

    let d0 = l0 - u0;
    let d1 = l1 - u1;

    // Since horizontal lines are handled separately and the ordering of points for the segment,
    // then there should always be positive Dy.
    debug_assert!(d0.y > 0 && d1.y > 0);

    // Defining s0(y) = u0.x + (y - u0.y) * d0.x / d0.y, and likewise for s1, cross-multiplying the
    // denominators (which keeps their sign) gives
    //    d1.y * (u0.x * d0.y + (y - u0.y) * d0.x) < d0.y * (u1.x * d1.y + (y - u1.y) * d1.x)
    let lhs: Int96 = multiply_i32_i64(
        d1.y,
        i64::from(u0.x)
            .wrapping_mul(i64::from(d0.y))
            .wrapping_add(i64::from(y.wrapping_sub(u0.y)).wrapping_mul(i64::from(d0.x))),
    );
    let rhs: Int96 = multiply_i32_i64(
        d0.y,
        i64::from(u1.x)
            .wrapping_mul(i64::from(d1.y))
            .wrapping_add(i64::from(y.wrapping_sub(u1.y)).wrapping_mul(i64::from(d1.x))),
    );
    // If these are equal, then the slope breaks the tie.
    lhs < rhs || ((lhs == rhs) && slope_s0_less_than_slope_s1(s0, s1))
}

/// The intersection test from "Robust Plane Sweep for Intersecting Segments", page 10: true if
/// the two segments share a point.
// Port of: modules/bentleyottmann/src/Myers.cpp#L513-L570 (chrome/m156)
#[doc(hidden)]
#[must_use]
#[allow(clippy::similar_names)] // the names u0_to_u1, d0_x_u0_to_u1 and so on mirror the C++ vectors and cross products
pub fn s0_intersects_s1(s0: &Segment, s1: &Segment) -> bool {
    // Make sure that s0 upper is above s1 upper.
    if s1.upper().y < s0.upper().y
        || ((s1.upper().y == s0.upper().y) && (s1.lower().y > s0.lower().y))
    {
        // Swap to put in the right orientation.
        return s0_intersects_s1(s1, s0);
    }

    debug_assert!(s0.upper().y <= s1.upper().y);

    // If extents don't overlap then there is no intersection.
    {
        let (left0, top0, right0, bottom0) = s0.bounds();
        let (left1, top1, right1, bottom1) = s1.bounds();
        if right1 < left0 || right0 < left1 || bottom1 < top0 || bottom0 < top1 {
            return false;
        }
    }

    let (u0, l0) = (s0.upper(), s0.lower());
    let (u1, l1) = (s1.upper(), s1.lower());
    let d0 = l0 - u0;
    let d1 = l1 - u1;

    // If the vector from u0 to l0 (named D0) and the vector from u0 to u1 have an angle of 0
    // between them, then u1 is on the segment u0 to l0 (named s0).
    let u0_to_u1 = u1 - u0;
    let d0_x_u0_to_u1 = cross(d0, u0_to_u1);
    if d0_x_u0_to_u1 == 0 {
        // u1 is on s0.
        return true;
    }

    if l1.y <= l0.y {
        // S1 is between the upper and lower points of S0.
        let u0_to_l1 = l1 - u0;
        let d0_x_u0_to_l1 = cross(d0, u0_to_l1);
        if d0_x_u0_to_l1 == 0 {
            // l1 is on s0.
            return true;
        }
        // If U1 and L1 are on opposite sides of D0 then the segments cross.
        (d0_x_u0_to_u1 ^ d0_x_u0_to_l1) < 0
    } else {
        // S1 extends past S0. It could be that S1 crosses the line of S0 (not the bound segment)
        // beyond the endpoints of S0. Make sure that it crosses on the segment and not beyond.
        let u1_to_l0 = l0 - u1;
        let d1_x_u1_to_l0 = cross(d1, u1_to_l0);
        if d1_x_u1_to_l0 == 0 {
            return true;
        }
        // For D1 to cross D0, then D1 must be on the same side of U1toL0 as D0.
        (d0_x_u0_to_u1 ^ d1_x_u1_to_l0) >= 0
    }
}

// -- Event ----------------------------------------------------------------------------------------

/// An event: the segments that begin, are horizontal, and end at `y`. The slices borrow from the
/// [`EventQueue`].
// Port of: modules/bentleyottmann/src/Myers.cpp#L214-L224 (chrome/m156)
#[derive(Clone, Copy, Debug)]
pub struct Event<'a> {
    /// The y coordinate of the event.
    pub y: i32,
    /// The segments that begin at `y`.
    pub begin: &'a [Segment],
    /// The segments that are horizontal on `y`.
    pub horizontal: &'a [Segment],
    /// The segments that end on `y`.
    pub end: &'a [Segment],
}

// -- EventQueue -----------------------------------------------------------------------------------

/// The kind of a segment in the setup of the events.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum SetupKind {
    Begin = 0,
    Horizontal = 1,
    End = 2,
}

/// A segment tagged with the event it belongs to.
#[derive(Clone, Copy, Debug, PartialEq)]
struct SetupTuple {
    // The main ordering of events.
    y_ordering: i32,
    // Group together like event types together.
    kind: SetupKind,
    // Break ties if y_ordering is the same.
    x_tie_breaker: i32,
    // We want to sort the segments, but they are not part of the key.
    original_segment: Segment,
}

/// The events of an event queue, stored compactly: the segments of all events are in one vector
/// and each event holds the end indices of its three groups.
#[derive(Clone, Copy, Debug)]
struct CompactEvent {
    y: i32,
    start: usize,
    end_of_begin: usize,
    end_of_horizontal: usize,
    end_of_end: usize,
}

/// The events of the sweep, ordered by `y`. Events are never added after the queue is made.
// Port of: modules/bentleyottmann/src/Myers.cpp#L226-L330 (chrome/m156)
#[doc(alias = "myers::EventQueue")]
#[derive(Clone, Debug)]
pub struct EventQueue {
    events: Vec<CompactEvent>,
    segment_storage: Vec<Segment>,
}

impl EventQueue {
    /// Makes an event queue from a list of segments. Zero length segments are excluded.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L236-L313 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(segments: &[Segment]) -> Self {
        // A vector of SetupTuple when ordered will produce the events, and all the different
        // sets of segments (beginning, etc.).
        let mut event_ordering: Vec<SetupTuple> = Vec::new();
        for s in segments {
            // Exclude zero length segments.
            if s.upper() == s.lower() {
                continue;
            }

            if s.is_horizontal() {
                // Tag for the horizontal set.
                event_ordering.push(SetupTuple {
                    y_ordering: s.upper().y,
                    kind: SetupKind::Horizontal,
                    x_tie_breaker: s.upper().x.wrapping_neg(),
                    original_segment: *s,
                });
            } else {
                // Tag for the beginning and ending sets.
                event_ordering.push(SetupTuple {
                    y_ordering: s.upper().y,
                    kind: SetupKind::Begin,
                    x_tie_breaker: s.upper().x.wrapping_neg(),
                    original_segment: *s,
                });
                event_ordering.push(SetupTuple {
                    y_ordering: s.lower().y,
                    kind: SetupKind::End,
                    x_tie_breaker: s.lower().x.wrapping_neg(),
                    original_segment: *s,
                });
            }
        }

        // Order the tuples by y, then by set type, then by x value.
        event_ordering.sort_by(|l, r| {
            (l.y_ordering, l.kind, l.x_tie_breaker).cmp(&(r.y_ordering, r.kind, r.x_tie_breaker))
        });

        // Remove duplicate segments.
        event_ordering.dedup();

        let mut events = Vec::new();
        let mut segment_storage = Vec::with_capacity(event_ordering.len());
        let Some(first) = event_ordering.first() else {
            // No segment is longer than a point: there are no events.
            return Self {
                events,
                segment_storage,
            };
        };
        let mut current_y = first.y_ordering;
        let mut start = 0;
        let mut end_of_begin = 0;
        let mut end_of_horizontal = 0;
        let mut end_of_end = 0;
        for t in &event_ordering {
            // If this is a new y then create the compact event.
            if current_y != t.y_ordering {
                events.push(CompactEvent {
                    y: current_y,
                    start,
                    end_of_begin,
                    end_of_horizontal,
                    end_of_end,
                });
                // Every segment sets end_of_end below, so it needs no reset here.
                start = segment_storage.len();
                end_of_begin = start;
                end_of_horizontal = start;
                current_y = t.y_ordering;
            }

            segment_storage.push(t.original_segment);

            // Increment the various set indices. A beginning also ends the horizontal and end sets,
            // as the C++ switch falls through.
            let segment_count = segment_storage.len();
            match t.kind {
                SetupKind::Begin => {
                    end_of_begin = segment_count;
                    end_of_horizontal = segment_count;
                    end_of_end = segment_count;
                }
                SetupKind::Horizontal => {
                    end_of_horizontal = segment_count;
                    end_of_end = segment_count;
                }
                SetupKind::End => {
                    end_of_end = segment_count;
                }
            }
        }

        // Store the last event.
        events.push(CompactEvent {
            y: current_y,
            start,
            end_of_begin,
            end_of_horizontal,
            end_of_end,
        });

        Self {
            events,
            segment_storage,
        }
    }

    /// The event `i`, which must be less than [`EventQueue::len`].
    // Port of: modules/bentleyottmann/src/Myers.cpp#L266-L276 (chrome/m156)
    #[must_use]
    pub fn get(&self, i: usize) -> Event<'_> {
        let e = self.events[i];
        Event {
            y: e.y,
            begin: &self.segment_storage[e.start..e.end_of_begin],
            horizontal: &self.segment_storage[e.end_of_begin..e.end_of_horizontal],
            end: &self.segment_storage[e.end_of_horizontal..e.end_of_end],
        }
    }

    /// The events, in order of `y`.
    // Port of: modules/bentleyottmann/src/Myers.cpp#L277-L285 (chrome/m156) (`begin`/`end`)
    pub fn iter(&self) -> impl Iterator<Item = Event<'_>> + '_ {
        (0..self.len()).map(move |i| self.get(i))
    }

    /// The number of events.
    #[must_use]
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// True if there are no events.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

// -- CrossingAccumulator --------------------------------------------------------------------------

/// Collects all the crossings, rejecting endpoint-to-endpoint crossings, as those intersections
/// are already represented in the data.
// Port of: modules/bentleyottmann/src/Myers.cpp#L332-L353 (chrome/m156)
#[derive(Clone, Debug, Default)]
struct CrossingAccumulator {
    crossings: Vec<Crossing>,
}

impl CrossingAccumulator {
    fn record_crossing(&mut self, s0: &Segment, s1: &Segment) {
        // Endpoints with no possible interior overlap.
        if s0.upper() == s1.lower() || s0.lower() == s1.upper() {
            return;
        }

        // Segments don't overlap if they are not collinear.
        if (s0.upper() == s1.upper() || s0.lower() == s1.lower()) && compare_slopes(s0, s1) != 0 {
            return;
        }

        self.crossings.push(Crossing::new(*s0, *s1));
    }

    fn finish_and_release_crossings(&mut self) -> Vec<Crossing> {
        std::mem::take(&mut self.crossings)
    }
}

// -- SweepLine ------------------------------------------------------------------------------------

/// The sentinels of the status line. They are vertical segments at the extreme x values.
const LEFT_STATUS_SENTINEL: Segment = Segment {
    upper: Point {
        x: i32::MIN,
        y: i32::MIN,
    },
    lower: Point {
        x: i32::MIN,
        y: i32::MAX,
    },
};

/// The right sentinel of the status line.
const RIGHT_STATUS_SENTINEL: Segment = Segment {
    upper: Point {
        x: i32::MAX,
        y: i32::MIN,
    },
    lower: Point {
        x: i32::MAX,
        y: i32::MAX,
    },
};

/// The status line of the Myers sweep, with the crossings found so far.
// Port of: modules/bentleyottmann/src/Myers.cpp#L355-L505 (chrome/m156)
#[derive(Clone, Debug)]
struct SweepLine {
    status: Vec<Segment>,
    crossings: CrossingAccumulator,
}

impl SweepLine {
    fn new() -> Self {
        Self {
            status: vec![LEFT_STATUS_SENTINEL, RIGHT_STATUS_SENTINEL],
            crossings: CrossingAccumulator::default(),
        }
    }

    fn handle_event(&mut self, e: Event<'_>) {
        // Things could be out of order from last event.
        self.sort_and_record(e.y);
        self.handle_beginnings(e.begin);
        self.handle_horizontals(e.horizontal);
        self.handle_endings(e.end);
    }

    fn finish_and_release_crossings(&mut self) -> Vec<Crossing> {
        // Only the sentinels should be left.
        debug_assert!(self.status_empty());
        self.crossings.finish_and_release_crossings()
    }

    /// Only the sentinels are on the status line.
    fn status_empty(&self) -> bool {
        self.status.len() == 2
    }

    /// Sorts the status line by insertion sort. If items are swapped, then there is a crossing to
    /// record.
    fn sort_and_record(&mut self, y: i32) {
        // If there are only the sentinels or just 1 segment, then nothing to sort.
        if self.status.len() <= 3 {
            return;
        }

        // Skip the first and last sentinels.
        for i in 2..self.status.len() - 1 {
            let t = self.status[i];
            let mut j = i;
            while j > 1 && s0_less_than_s1_at_y(&t, &self.status[j - 1], y) {
                // While t < the thing before it move it down.
                self.crossings.record_crossing(&t, &self.status[j - 1]);
                self.status[j] = self.status[j - 1];
                j -= 1;
            }
            self.status[j] = t;
        }
    }

    /// When inserting a starting point (either a beginning or a horizontal) check the segments to
    /// the left and the right, checking nearby segments for crossings.
    fn check_crossings_left_and_right(
        &mut self,
        segment: &Segment,
        insertion_point: usize,
        check: impl Fn(&Segment) -> bool,
    ) {
        // Match to the left using the left sentinel to break the loop.
        let mut i = insertion_point;
        while i > 0 && check(&self.status[i - 1]) {
            self.crossings.record_crossing(segment, &self.status[i - 1]);
            i -= 1;
        }

        // Match to the right using the right sentinel to break the loop.
        let mut j = insertion_point;
        while j < self.status.len() && check(&self.status[j]) {
            self.crossings.record_crossing(segment, &self.status[j]);
            j += 1;
        }
    }

    /// Adds the segments that start on `y`, excluding horizontals.
    fn handle_beginnings(&mut self, inserting: &[Segment]) {
        for s in inserting {
            let insertion_point = self
                .status
                .partition_point(|x| segment_less_than_upper_to_insert(x, s));

            // Checking intersections left and right checks if the point s.upper() lies on the
            // nearby segment.
            let upper = s.upper();
            let check_intersect =
                |to_check: &Segment| compare_point_to_segment(upper, to_check) == 0;
            self.check_crossings_left_and_right(s, insertion_point, check_intersect);
            self.status.insert(insertion_point, *s);
        }
    }

    /// Horizontals on `y` are handled by checking for crossings by adding them, and then
    /// immediately removing them.
    fn handle_horizontals(&mut self, horizontals: &[Segment]) {
        for s in horizontals {
            let insertion_point = self
                .status
                .partition_point(|x| segment_less_than_upper_to_insert(x, s));

            // Check if the nearby segment crosses the horizontal line.
            let (upper, lower) = (s.upper(), s.lower());
            let check_intersection = |to_check: &Segment| {
                compare_point_to_segment(upper, to_check) <= 0
                    && compare_point_to_segment(lower, to_check) >= 0
            };
            self.check_crossings_left_and_right(s, insertion_point, check_intersection);
            self.status.insert(insertion_point, *s);
        }
        self.handle_endings(horizontals);
    }

    /// Removes all the segments ending on `y`.
    fn handle_endings(&mut self, removing: &[Segment]) {
        for s in removing {
            let before = self.status.len();
            self.status.retain(|x| x != s);
            debug_assert!(self.status.len() < before);
        }
    }
}

/// Finds all the crossings of `segments` with the Myers sweep.
// Port of: modules/bentleyottmann/src/Myers.cpp#L507-L515 (chrome/m156)
#[must_use]
pub fn myers_find_crossings(segments: &[Segment]) -> Vec<Crossing> {
    let event_queue = EventQueue::make(segments);
    let mut sweep_line = SweepLine::new();
    for event in event_queue.iter() {
        sweep_line.handle_event(event);
    }
    sweep_line.finish_and_release_crossings()
}

/// Finds all the crossings of `segments` by testing every pair. Zero length segments and
/// duplicates are removed first.
// Port of: modules/bentleyottmann/src/Myers.cpp#L572-L597 (chrome/m156)
#[must_use]
pub fn brute_force_crossings(segments: &[Segment]) -> Vec<Crossing> {
    let mut clean: Vec<Segment> = segments
        .iter()
        .filter(|segment| segment.upper() != segment.lower())
        .copied()
        .collect();
    clean.sort();
    clean.dedup();

    let mut crossings = CrossingAccumulator::default();
    if clean.len() >= 2 {
        for i in 0..clean.len() - 1 {
            for j in i + 1..clean.len() {
                if s0_intersects_s1(&clean[i], &clean[j]) {
                    crossings.record_crossing(&clean[i], &clean[j]);
                }
            }
        }
    }
    crossings.finish_and_release_crossings()
}
