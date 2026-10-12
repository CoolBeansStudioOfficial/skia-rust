//! The sweep line of the Bentley-Ottmann algorithm: the segments crossing the current event
//! point, in order. Port of `modules/bentleyottmann/include/SweepLine.h` and `src/SweepLine.cpp`
//! (chrome/m156).

use crate::event_queue::{
    DeletionSegmentSet, EventQueueInterface, InsertionSegmentSet, SweepLineInterface,
};
use crate::point::Point;
use crate::segment::{Segment, intersect, point_less_than_segment_in_x};

/// The sweep line: the segments that cross the sweep, ordered left to right. It is bounded by a
/// left and a right sentinel, so the neighbours of any inserted segment always exist.
// Port of: modules/bentleyottmann/include/SweepLine.h#L14-L31 (chrome/m156)
#[doc(alias = "bentleyottmann::SweepLine")]
#[derive(Clone, Debug)]
pub struct SweepLine {
    sweep_line: Vec<Segment>,
}

impl Default for SweepLine {
    fn default() -> Self {
        Self::new()
    }
}

impl SweepLine {
    /// A sweep line holding only the two sentinels.
    // Port of: modules/bentleyottmann/src/SweepLine.cpp#L14-L26 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        let left_sentinel = Segment::new(
            Point::new(-i32::MAX, -i32::MAX),
            Point::new(-i32::MAX, i32::MAX),
        );
        let right_sentinel = Segment::new(
            Point::new(i32::MAX, -i32::MAX),
            Point::new(i32::MAX, i32::MAX),
        );
        let mut sweep_line = Vec::with_capacity(8);
        sweep_line.push(left_sentinel);
        sweep_line.push(right_sentinel);
        Self { sweep_line }
    }

    /// Asserts that the sweep line is ordered at `y`.
    // Port of: modules/bentleyottmann/src/SweepLine.cpp#L28-L35 (chrome/m156)
    fn verify(&self, y: i32) {
        for pair in self.sweep_line.windows(2) {
            debug_assert!(crate::segment::less_than_at(&pair[0], &pair[1], y));
        }
    }
}

impl SweepLineInterface for SweepLine {
    // Port of: modules/bentleyottmann/src/SweepLine.cpp#L37-L52 (chrome/m156)
    fn handle_deletions(&mut self, event_point: Point, removing: &DeletionSegmentSet) {
        if removing.is_empty() {
            // Remove ending segments.
            self.sweep_line.retain(|s| s.lower() != event_point);
        } else {
            // Remove all ending and crossing segments.
            self.sweep_line
                .retain(|s| !(s.lower() == event_point || removing.contains(s)));
        }
    }

    // Port of: modules/bentleyottmann/src/SweepLine.cpp#L54-L94 (chrome/m156)
    fn handle_insertions_and_check_for_new_crossings(
        &mut self,
        event_point: Point,
        inserting: &InsertionSegmentSet,
        queue: &mut dyn EventQueueInterface,
    ) {
        // The insertion set is ordered by slope so these segments are in the right order for
        // insertion. lower_bound: the first segment the point is not left of.
        let right_of_insertion = self
            .sweep_line
            .partition_point(|s| !point_less_than_segment_in_x(event_point, s));
        debug_assert!(right_of_insertion != 0);
        let left_of_insertion = right_of_insertion - 1;

        if inserting.is_empty() {
            // There were deletions, but no insertions, so check if the two segments at the
            // insertion point cross.
            let left = self.sweep_line[left_of_insertion];
            let right = self.sweep_line[right_of_insertion];
            if let Some(crossing_point) = intersect(&left, &right) {
                queue.add_crossing(crossing_point, &left, &right);
            }
        } else {
            // Safe: the set is not empty.
            let first = *inserting.first().expect("inserting is not empty");
            let last = *inserting.last().expect("inserting is not empty");

            // Check if the left most inserted segment crosses the segment immediately to the left
            // of the insertion cursor.
            let left = self.sweep_line[left_of_insertion];
            if let Some(crossing_point) = intersect(&left, &first) {
                queue.add_crossing(crossing_point, &left, &first);
            }

            // Check if the right most inserted segment crosses the segment immediately to the
            // right of the insertion cursor.
            let right = self.sweep_line[right_of_insertion];
            if let Some(crossing_point) = intersect(&last, &right) {
                queue.add_crossing(crossing_point, &last, &right);
            }

            // Insert the set in the sweep line.
            self.sweep_line.splice(
                right_of_insertion..right_of_insertion,
                inserting.iter().copied(),
            );
        }
    }
}

/// Port of `SweepLineTestingPeer`: reads and edits the sweep line from the tests. The peer takes
/// the sweep line as an argument, so a test can still call the sweep line directly.
#[doc(hidden)]
#[derive(Debug)]
pub struct SweepLineTestingPeer;

impl SweepLineTestingPeer {
    /// Checks the sweep line is ordered at `y`.
    // Port of: modules/bentleyottmann/tests/SweepLineTest.cpp#L13-L15 (chrome/m156)
    pub fn verify_sweep_line(sl: &SweepLine, y: i32) {
        sl.verify(y);
    }

    /// Inserts `s` at position `i` of the sweep line.
    // Port of: modules/bentleyottmann/tests/SweepLineTest.cpp#L16-L19 (chrome/m156)
    pub fn insert_segment(sl: &mut SweepLine, i: usize, s: Segment) {
        sl.sweep_line.insert(i, s);
    }

    /// The number of segments, sentinels included.
    // Port of: modules/bentleyottmann/tests/SweepLineTest.cpp#L20-L22 (chrome/m156)
    #[must_use]
    pub fn size(sl: &SweepLine) -> usize {
        sl.sweep_line.len()
    }

    /// The segments, sentinels included.
    // Port of: modules/bentleyottmann/tests/SweepLineTest.cpp#L23-L25 (chrome/m156)
    #[must_use]
    pub fn sweep_line(sl: &SweepLine) -> &[Segment] {
        &sl.sweep_line
    }
}
