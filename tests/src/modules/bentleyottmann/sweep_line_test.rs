// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/SweepLineTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::event_queue::{
    DeletionSegmentSet, EventQueueInterface, InsertionSegmentSet, SweepLineInterface,
};
use skia_rust_bentleyottmann::point::Point;
use skia_rust_bentleyottmann::segment::{
    Crossing, Segment, rounded_point_less_than_segment_in_x_lower,
    rounded_point_less_than_segment_in_x_upper,
};
use skia_rust_bentleyottmann::sweep_line::{SweepLine, SweepLineTestingPeer as TP};

use crate::{def_test, reporter_assert};

def_test!(BO_SweepLineSearch, |reporter| {
    {
        let mut sweep_line = SweepLine::new();
        let p0 = Point::new(-100, -100);
        let p1 = Point::new(100, 100);
        let p2 = Point::new(100, -100);
        let p3 = Point::new(-100, 100);
        let s0 = Segment::new(p0, p1);
        let s1 = Segment::new(p2, p3);
        TP::insert_segment(&mut sweep_line, 1, s0);
        TP::insert_segment(&mut sweep_line, 2, s1);
        let sl = TP::sweep_line(&sweep_line);
        let crossing = Point::new(0, 0);
        let l = sl.partition_point(|s| rounded_point_less_than_segment_in_x_lower(s, crossing));
        let r = l + sl[l..]
            .partition_point(|s| rounded_point_less_than_segment_in_x_upper(s, crossing));
        // Remember that the index is off by 1 because of the left sentinel.
        reporter_assert!(reporter, l == 1);
        reporter_assert!(reporter, r == 3);
    }
    {
        let mut sweep_line = SweepLine::new();
        // No longer cross at {0, 0}, but still round to {0, 0}.
        let p0 = Point::new(-100, -100);
        let p1 = Point::new(99, 100);
        let p2 = Point::new(100, -100);
        let p3 = Point::new(-101, 100);
        let s0 = Segment::new(p0, p1);
        let s1 = Segment::new(p2, p3);
        TP::insert_segment(&mut sweep_line, 1, s0);
        TP::insert_segment(&mut sweep_line, 2, s1);
        let sl = TP::sweep_line(&sweep_line);
        let crossing = Point::new(0, 0);
        let l = sl.partition_point(|s| rounded_point_less_than_segment_in_x_lower(s, crossing));
        let r = l + sl[l..]
            .partition_point(|s| rounded_point_less_than_segment_in_x_upper(s, crossing));
        // Remember that the index is off by 1 because of the left sentinel.
        reporter_assert!(reporter, l == 1);
        reporter_assert!(reporter, r == 3);
    }
});

/// `FailIfEventQueueAdd`: an event queue that must not receive crossings.
struct FailIfEventQueueAdd;

impl EventQueueInterface for FailIfEventQueueAdd {
    fn add_crossing(&mut self, _crossing_point: Point, _s0: &Segment, _s1: &Segment) {
        panic!("There should be no crossings.");
    }
}

/// `CollectCrossings`: an event queue that records the crossings it is given.
#[derive(Default)]
struct CollectCrossings {
    f_crossing: Vec<Crossing>,
}

impl EventQueueInterface for CollectCrossings {
    fn add_crossing(&mut self, crossing_point: Point, s0: &Segment, s1: &Segment) {
        self.f_crossing.push(Crossing {
            s0: *s0,
            s1: *s1,
            crossing: crossing_point,
        });
    }
}

/// Builds an insertion set from segments, inserting them in order as the C++ initializer list
/// does.
fn insertion_set(segments: &[Segment]) -> InsertionSegmentSet {
    let mut set = InsertionSegmentSet::new();
    for s in segments {
        set.insert(*s);
    }
    set
}

/// Builds a deletion set from segments, inserting them in order as the C++ initializer list does.
fn deletion_set(segments: &[Segment]) -> DeletionSegmentSet {
    let mut set = DeletionSegmentSet::new();
    for s in segments {
        set.insert(*s);
    }
    set
}

def_test!(BO_SweepLineInsert, |reporter| {
    {
        let sweep_line = SweepLine::new();
        TP::verify_sweep_line(&sweep_line, 0);
    }
    {
        // Handle insert
        let mut sweep_line = SweepLine::new();
        let mut event_queue = FailIfEventQueueAdd;
        let mut insertions = InsertionSegmentSet::new();
        let s = Segment::new(Point::new(0, -100), Point::new(0, 100));
        insertions.insert(s);
        TP::verify_sweep_line(&sweep_line, -99);
        sweep_line.handle_insertions_and_check_for_new_crossings(
            Point::new(0, -100),
            &insertions,
            &mut event_queue,
        );
        reporter_assert!(reporter, TP::size(&sweep_line) == 3);
        TP::verify_sweep_line(&sweep_line, -99);
    }
    {
        // Handle 3 segments where removing middle segment introduces crossing
        let mut sweep_line = SweepLine::new();
        let p0 = Point::new(-100, -100);
        let p1 = Point::new(100, 100);
        let p2 = Point::new(100, -100);
        let p3 = Point::new(-100, 100);
        let p4 = Point::new(0, -100);
        let p5 = Point::new(0, -50);
        let s0 = Segment::new(p0, p1);
        let s1 = Segment::new(p2, p3);
        let s2 = Segment::new(p4, p5);
        let mut event_queue = CollectCrossings::default();
        {
            // Simulate handling Upper s0
            let insertions = insertion_set(&[s0]);
            TP::verify_sweep_line(&sweep_line, -99);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                p0,
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, TP::size(&sweep_line) == 3);
            reporter_assert!(reporter, event_queue.f_crossing.is_empty());
            TP::verify_sweep_line(&sweep_line, -99);
        }
        {
            // Simulate handling Upper s2
            let insertions = insertion_set(&[s2]);
            TP::verify_sweep_line(&sweep_line, -99);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                p4,
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, TP::size(&sweep_line) == 4);
            reporter_assert!(reporter, event_queue.f_crossing.is_empty());
            TP::verify_sweep_line(&sweep_line, -99);
        }
        {
            // Simulate handling Upper s1
            let insertions = insertion_set(&[s1]);
            TP::verify_sweep_line(&sweep_line, -99);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                p2,
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, TP::size(&sweep_line) == 5);
            reporter_assert!(reporter, event_queue.f_crossing.is_empty());
            TP::verify_sweep_line(&sweep_line, -99);
        }
        {
            // Simulate handling Lower s2 which introduces a crossing
            let deletions = DeletionSegmentSet::new(); // empty set because this will be a lower event
            let insertions = InsertionSegmentSet::new();
            TP::verify_sweep_line(&sweep_line, -51);
            sweep_line.handle_deletions(p5, &deletions);
            reporter_assert!(reporter, TP::size(&sweep_line) == 4);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                p5,
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, event_queue.f_crossing.len() == 1);
            TP::verify_sweep_line(&sweep_line, -51);
        }
        {
            // Handle crossing
            // The deletions and insertions are both s0 and s1.
            let deletions = deletion_set(&[s0, s1]);
            let insertions = insertion_set(&[s0, s1]);
            TP::verify_sweep_line(&sweep_line, -1); // Check above the crossing
            sweep_line.handle_deletions(Point::new(0, 0), &deletions);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                Point::new(0, 0),
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, TP::size(&sweep_line) == 4);
            reporter_assert!(reporter, event_queue.f_crossing.len() == 1);
            TP::verify_sweep_line(&sweep_line, 1); // Make sure things are correct after deletion
        }
        {
            // Handle deletion s1
            let deletions = DeletionSegmentSet::new(); // empty set because this will be a lower event
            let insertions = InsertionSegmentSet::new();
            TP::verify_sweep_line(&sweep_line, 99); // Check above the crossing
            sweep_line.handle_deletions(p3, &deletions);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                p3,
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, TP::size(&sweep_line) == 3);
            reporter_assert!(reporter, event_queue.f_crossing.len() == 1);
            TP::verify_sweep_line(&sweep_line, 99); // Make sure sentinels are correct.
        }
        {
            // Handle deletion s0
            let deletions = DeletionSegmentSet::new(); // empty set because this will be a lower event
            let insertions = InsertionSegmentSet::new();
            TP::verify_sweep_line(&sweep_line, 99); // Check above the crossing
            sweep_line.handle_deletions(p1, &deletions);
            sweep_line.handle_insertions_and_check_for_new_crossings(
                p1,
                &insertions,
                &mut event_queue,
            );
            reporter_assert!(reporter, TP::size(&sweep_line) == 2);
            reporter_assert!(reporter, event_queue.f_crossing.len() == 1);
            TP::verify_sweep_line(&sweep_line, 99); // Make sure sentinels are correct.
        }
    }
});
