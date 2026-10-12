// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: modules/bentleyottmann/tests/EventQueueTest.cpp (chrome/m156)

#![cfg(test)]

use skia_rust_bentleyottmann::event_queue::{
    Cross, DeletionSegmentSet, Event, EventQueue, EventQueueInterface, EventQueueTestingPeer,
    EventType, InsertionSegmentSet, Queue, SweepLineInterface, Upper,
};
use skia_rust_bentleyottmann::point::Point;
use skia_rust_bentleyottmann::segment::{Crossing, Segment};

use crate::{Reporter, def_test, reporter_assert};

def_test!(BO_EventQueueOrdering, |reporter| {
    {
        // Check that event types are ordered correctly.
        let mut q = Queue::new();
        // Insert the events in reverse order.
        let event_point = Point::new(100, 100);
        let s = Segment::new(Point::new(100, 100), Point::new(200, 200));
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Upper(Upper { s }),
        });
        let s0 = Segment::new(Point::new(50, 50), Point::new(150, 150));
        let s1 = Segment::new(Point::new(150, 50), Point::new(50, 150));
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Cross(Cross { s0, s1 }),
        });
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Lower,
        });

        // Make sure that the events are in the right order.
        let mut cursor = q.iter();
        reporter_assert!(
            reporter,
            matches!(cursor.next().unwrap().event_type, EventType::Lower)
        );
        reporter_assert!(
            reporter,
            matches!(cursor.next().unwrap().event_type, EventType::Cross(_))
        );
        reporter_assert!(
            reporter,
            matches!(cursor.next().unwrap().event_type, EventType::Upper(_))
        );
    }
});

def_test!(BO_EventQueueBasic, |reporter| {
    {
        let q = Queue::new();
        let eq = EventQueue::new(q);
        reporter_assert!(reporter, !eq.has_more_events());
    }
    {
        let mut q = Queue::new();
        let event_point = Point::new(100, 100);
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Lower,
        });
        let mut eq = EventQueue::new(q);
        {
            reporter_assert!(reporter, eq.has_more_events());
            let e = EventQueueTestingPeer::next_event(&mut eq);
            reporter_assert!(reporter, e.where_ == event_point);
            reporter_assert!(reporter, !eq.has_more_events());
        }
    }
    {
        // Check that Lower events are de-duplicated.
        let mut q = Queue::new();
        let event_point = Point::new(100, 100);
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Lower,
        });
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Lower,
        });
        let mut eq = EventQueue::new(q);
        {
            // There should be only one lower because of queue de-duplication
            reporter_assert!(reporter, eq.has_more_events());
            let e = EventQueueTestingPeer::next_event(&mut eq);
            reporter_assert!(reporter, e.where_ == event_point);
            reporter_assert!(reporter, !eq.has_more_events());
        }
    }
    {
        // Check that Lower distinct Lower events are distinct.
        let mut q = Queue::new();
        let event_point1 = Point::new(100, 100);
        let event_point2 = Point::new(100, 101);
        q.insert(Event {
            where_: event_point1,
            event_type: EventType::Lower,
        });
        q.insert(Event {
            where_: event_point2,
            event_type: EventType::Lower,
        });
        let mut eq = EventQueue::new(q);
        {
            // There should be only one lower because of queue de-duplication
            reporter_assert!(reporter, eq.has_more_events());
            let e = EventQueueTestingPeer::next_event(&mut eq);
            reporter_assert!(reporter, e.where_ == event_point1);
        }
        {
            // There should be only one lower because of queue de-duplication
            reporter_assert!(reporter, eq.has_more_events());
            let e = EventQueueTestingPeer::next_event(&mut eq);
            reporter_assert!(reporter, e.where_ == event_point2);
            reporter_assert!(reporter, !eq.has_more_events());
        }
    }
    {
        // Check that non-Lower events are separate.
        let mut q = Queue::new();
        let s0 = Segment::new(Point::new(0, 0), Point::new(100, 100));
        let s1 = Segment::new(Point::new(0, 0), Point::new(-100, 100));
        q.insert(Event {
            where_: Point::new(0, 0),
            event_type: EventType::Upper(Upper { s: s0 }),
        });
        q.insert(Event {
            where_: Point::new(0, 0),
            event_type: EventType::Upper(Upper { s: s1 }),
        });
        let mut eq = EventQueue::new(q);
        {
            reporter_assert!(reporter, eq.has_more_events());
            let e = EventQueueTestingPeer::next_event(&mut eq);
            let upper_pt = Point::new(0, 0);
            reporter_assert!(reporter, e.where_ == upper_pt);
            reporter_assert!(reporter, e.event_type.index() == 2);
            let EventType::Upper(upper) = e.event_type else {
                panic!("std::get<Upper> on another alternative throws");
            };
            reporter_assert!(
                reporter,
                upper >= Upper { s: s1 } && Upper { s: s1 } >= upper
            );
            let e2 = EventQueueTestingPeer::next_event(&mut eq);
            reporter_assert!(reporter, e2.where_ == upper_pt);
            reporter_assert!(reporter, e2.event_type.index() == 2);
            let EventType::Upper(upper2) = e2.event_type else {
                panic!("std::get<Upper> on another alternative throws");
            };
            reporter_assert!(
                reporter,
                upper2 >= Upper { s: s0 } && Upper { s: s0 } >= upper2
            );
            reporter_assert!(reporter, !eq.has_more_events());
        }
    }
});

/// `HasDeletions`: whether the handleDeletions call should be made.
#[derive(Clone, Copy, PartialEq, Eq)]
enum HasDeletions {
    /// The handleDeletions call should be called.
    HasDeletions,
    /// The handleDeletions call should not be called.
    HasNoDeletions,
}

/// `TestEventHandler`: checks each sweep call against the expected segments and crossings.
struct TestEventHandler<'a> {
    r: &'a mut Reporter,
    candidate_event_point: Point,
    deletions: Vec<Segment>,
    insertions: Vec<Segment>,
    crossings: Vec<Crossing>,
    has_deletions: HasDeletions,
}

impl<'a> TestEventHandler<'a> {
    fn new(
        r: &'a mut Reporter,
        event_point: Point,
        deletions: Vec<Segment>,
        insertions: Vec<Segment>,
        crossings: Vec<Crossing>,
        has_deletions: HasDeletions,
    ) -> Self {
        Self {
            r,
            candidate_event_point: event_point,
            deletions,
            insertions,
            crossings,
            has_deletions,
        }
    }
}

impl SweepLineInterface for TestEventHandler<'_> {
    fn handle_deletions(&mut self, event_point: Point, removing: &DeletionSegmentSet) {
        if self.has_deletions == HasDeletions::HasNoDeletions {
            reporter_assert!(self.r, false, "There should be no deletions.");
            return;
        }
        reporter_assert!(self.r, event_point == self.candidate_event_point);
        reporter_assert!(self.r, removing.len() == self.deletions.len());
        for s in &self.deletions {
            reporter_assert!(self.r, removing.contains(s));
        }
    }

    fn handle_insertions_and_check_for_new_crossings(
        &mut self,
        event_point: Point,
        inserting: &InsertionSegmentSet,
        queue: &mut dyn EventQueueInterface,
    ) {
        reporter_assert!(self.r, event_point == self.candidate_event_point);
        reporter_assert!(self.r, inserting.len() == self.insertions.len());
        for s in &self.insertions {
            reporter_assert!(self.r, inserting.contains(s));
        }
        for crossing in &self.crossings {
            queue.add_crossing(crossing.crossing, &crossing.s0, &crossing.s1);
        }
    }
}

def_test!(BO_EventQueueHandlerInterface, |reporter| {
    {
        // Check that a Lower event is added while processing the Upper event.
        let mut q = Queue::new();
        let event_point = Point::new(100, 100);
        let end_point = Point::new(200, 200);
        let s = Segment::new(event_point, end_point);
        q.insert(Event {
            where_: event_point,
            event_type: EventType::Upper(Upper { s }),
        });
        let mut eq = EventQueue::new(q);

        reporter_assert!(reporter, eq.has_more_events());
        let mut eh1 = TestEventHandler::new(
            reporter,
            event_point,
            vec![],
            vec![s],
            vec![],
            HasDeletions::HasNoDeletions,
        );
        eq.handle_next_event_point(&mut eh1);
        let mut eh2 = TestEventHandler::new(
            reporter,
            end_point,
            vec![],
            vec![],
            vec![],
            HasDeletions::HasDeletions,
        );
        eq.handle_next_event_point(&mut eh2);
        reporter_assert!(reporter, !eq.has_more_events());
    }
    {
        // Check an entire crossing event.
        let mut q = Queue::new();
        let b0 = Point::new(100, 100);
        let e0 = Point::new(200, 200);
        let s0 = Segment::new(b0, e0);
        let b1 = Point::new(200, 100);
        let e1 = Point::new(100, 200);
        let s1 = Segment::new(b1, e1);
        let crossing_point = Point::new(150, 150);
        // Load crossing segments into the queue
        q.insert(Event {
            where_: b0,
            event_type: EventType::Upper(Upper { s: s0 }),
        });
        q.insert(Event {
            where_: b1,
            event_type: EventType::Upper(Upper { s: s1 }),
        });
        let mut eq = EventQueue::new(q);

        reporter_assert!(reporter, eq.has_more_events());
        let mut eh1 = TestEventHandler::new(
            reporter,
            b0,
            vec![],
            vec![s0],
            vec![],
            HasDeletions::HasNoDeletions,
        );
        eq.handle_next_event_point(&mut eh1);
        let mut eh2 = TestEventHandler::new(
            reporter,
            b1,
            vec![],
            vec![s1],
            vec![Crossing {
                s0,
                s1,
                crossing: crossing_point,
            }],
            HasDeletions::HasNoDeletions,
        );
        eq.handle_next_event_point(&mut eh2);
        let mut eh3 = TestEventHandler::new(
            reporter,
            crossing_point,
            vec![s0, s1],
            vec![s0, s1],
            vec![],
            HasDeletions::HasDeletions,
        );
        eq.handle_next_event_point(&mut eh3);
        let mut eh4 = TestEventHandler::new(
            reporter,
            e1,
            vec![],
            vec![],
            vec![],
            HasDeletions::HasDeletions,
        );
        eq.handle_next_event_point(&mut eh4);
        let mut eh5 = TestEventHandler::new(
            reporter,
            e0,
            vec![],
            vec![],
            vec![],
            HasDeletions::HasDeletions,
        );
        eq.handle_next_event_point(&mut eh5);
        reporter_assert!(reporter, !eq.has_more_events());
    }
});
