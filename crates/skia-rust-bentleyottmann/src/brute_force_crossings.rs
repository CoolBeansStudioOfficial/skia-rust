//! The quadratic reference: tests every pair of segments. Port of
//! `modules/bentleyottmann/include/BruteForceCrossings.h` and `src/BruteForceCrossings.cpp`
//! (chrome/m156).

use crate::segment::{Crossing, Segment, intersect};

/// Takes in a list of segments, and returns intersection points found in the list of segments.
/// An empty vector means there are no self intersections.
// Port of: modules/bentleyottmann/src/BruteForceCrossings.cpp#L11-L24 (chrome/m156)
#[must_use]
pub fn brute_force_crossings(segments: &[Segment]) -> Option<Vec<Crossing>> {
    let mut answer = Vec::new();
    if segments.len() >= 2 {
        for (i, s0) in segments[..segments.len() - 1].iter().enumerate() {
            for s1 in &segments[i + 1..] {
                if let Some(crossing) = intersect(s0, s1) {
                    answer.push(Crossing {
                        s0: *s0,
                        s1: *s1,
                        crossing,
                    });
                }
            }
        }
    }
    Some(answer)
}
