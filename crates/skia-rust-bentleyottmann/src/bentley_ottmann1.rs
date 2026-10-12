//! The Bentley-Ottmann sweep: finds all the crossings of a set of segments. Port of
//! `modules/bentleyottmann/include/BentleyOttmann1.h` and `src/BentleyOttmann1.cpp`
//! (chrome/m156).

use crate::event_queue::EventQueue;
use crate::segment::{Crossing, Segment};
use crate::sweep_line::SweepLine;

/// Takes in a list of segments, and returns intersection points found in the list of segments.
///
/// `None` means the data are out of range. An empty vector means there are no self intersections.
/// If `None` is returned, you could divide all your points by 2, and try again.
// Port of: modules/bentleyottmann/src/BentleyOttmann1.cpp#L12-L23 (chrome/m156)
#[must_use]
pub fn bentley_ottmann_1(segments: &[Segment]) -> Option<Vec<Crossing>> {
    let mut event_queue = EventQueue::make(segments)?;
    let mut sweep_line = SweepLine::new();
    while event_queue.has_more_events() {
        event_queue.handle_next_event_point(&mut sweep_line);
    }
    Some(event_queue.crossings())
}
