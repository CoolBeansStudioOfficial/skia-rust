//! The event queue of the Bentley-Ottmann sweep. Port of `modules/bentleyottmann/include/
//! EventQueue.h`, `include/EventQueueInterface.h` and `src/EventQueue.cpp` (chrome/m156).
//!
//! The C++ `std::set`s become [`BTreeSet`]s ordered by the same comparisons, so equivalent
//! elements are dropped on insertion exactly as `std::set::insert` drops them. The insertion set
//! is ordered by slope, not by segment, so it has its own type ([`InsertionSegmentSet`]).

use std::cmp::Ordering;
use std::collections::BTreeSet;

use crate::point::Point;
use crate::segment::{Crossing, Segment, compare_slopes};

/// An `EventQueueInterface` implementation must be able to add crossing events into the queue.
// Port of: modules/bentleyottmann/include/EventQueueInterface.h#L17-L25 (chrome/m156)
#[doc(alias = "bentleyottmann::EventQueueInterface")]
pub trait EventQueueInterface {
    /// Adds the crossing of `s0` and `s1` at `crossing_point` to the queue.
    fn add_crossing(&mut self, crossing_point: Point, s0: &Segment, s1: &Segment);
}

/// The set of segments to remove from the sweep line, ordered by `operator<` of [`Segment`].
// Port of: modules/bentleyottmann/include/EventQueueInterface.h#L27
#[doc(alias = "bentleyottmann::DeletionSegmentSet")]
pub type DeletionSegmentSet = BTreeSet<Segment>;

/// `OrderBySlope`: segments compare by slope, so the insertion set holds one segment per slope.
// Port of: modules/bentleyottmann/include/EventQueueInterface.h#L29-L31 (chrome/m156)
#[doc(alias = "bentleyottmann::OrderBySlope")]
#[derive(Clone, Copy, Debug, Default)]
pub struct OrderBySlope;

impl OrderBySlope {
    /// `OrderBySlope::operator()`: true when `s0` has a smaller slope than `s1`.
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L125-L128 (chrome/m156)
    #[must_use]
    pub fn less(s0: &Segment, s1: &Segment) -> bool {
        compare_slopes(s0, s1) < 0
    }
}

/// The set of insertion segments, ordered by slope. All the lines pass through the same point,
/// so the slope of each line must be ordered from smallest to largest to keep the segment order
/// correct in the sweep line. Two segments with the same slope are equivalent, so only the first
/// one inserted is kept.
///
/// This is a sorted vector: the set is small (the segments that start or cross at one event
/// point), and the vector keeps the `std::set<Segment, OrderBySlope>` semantics without a custom
/// comparator.
// Port of: modules/bentleyottmann/include/EventQueueInterface.h#L37-L38 (chrome/m156)
#[doc(alias = "bentleyottmann::InsertionSegmentSet")]
#[derive(Clone, Debug, Default)]
pub struct InsertionSegmentSet {
    segments: Vec<Segment>,
}

impl InsertionSegmentSet {
    /// An empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Inserts `s` unless a segment with the same slope is already present. Returns true if it
    /// was inserted, like `std::set::insert(...).second`.
    pub fn insert(&mut self, s: Segment) -> bool {
        let position = self.segments.partition_point(|x| OrderBySlope::less(x, &s));
        if self
            .segments
            .get(position)
            .is_some_and(|x| compare_slopes(x, &s) == 0)
        {
            return false;
        }
        self.segments.insert(position, s);
        true
    }

    /// True if a segment with the same slope as `s` is in the set (`std::set::find`).
    #[must_use]
    pub fn contains(&self, s: &Segment) -> bool {
        let position = self.segments.partition_point(|x| OrderBySlope::less(x, s));
        self.segments
            .get(position)
            .is_some_and(|x| compare_slopes(x, s) == 0)
    }

    /// The number of segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    /// True if there are no segments.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    /// Removes all segments.
    pub fn clear(&mut self) {
        self.segments.clear();
    }

    /// The segment with the smallest slope (`begin()`).
    #[must_use]
    pub fn first(&self) -> Option<&Segment> {
        self.segments.first()
    }

    /// The segment with the largest slope (`rbegin()`).
    #[must_use]
    pub fn last(&self) -> Option<&Segment> {
        self.segments.last()
    }

    /// The segments in slope order.
    pub fn iter(&self) -> std::slice::Iter<'_, Segment> {
        self.segments.iter()
    }
}

impl<'a> IntoIterator for &'a InsertionSegmentSet {
    type Item = &'a Segment;
    type IntoIter = std::slice::Iter<'a, Segment>;

    fn into_iter(self) -> Self::IntoIter {
        self.segments.iter()
    }
}

/// The `Lower` event: the end of a segment. All `Lower` events are equal.
// Port of: modules/bentleyottmann/include/EventQueue.h#L21-L25 (chrome/m156)
#[doc(alias = "bentleyottmann::Lower")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Lower;

/// The `Upper` event: the start of a segment. Ordered arbitrarily, by its raw endpoints, so the
/// queue keeps one event per segment.
// Port of: modules/bentleyottmann/include/EventQueue.h#L26-L33 (chrome/m156)
#[doc(alias = "bentleyottmann::Upper")]
#[derive(Clone, Copy, Debug)]
pub struct Upper {
    /// The segment starting at the event point.
    pub s: Segment,
}

impl PartialEq for Upper {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Upper {}

impl PartialOrd for Upper {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Upper {
    /// `std::tie(u0.s.p0, u0.s.p1) < std::tie(u1.s.p0, u1.s.p1)`.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.s.p0, self.s.p1).cmp(&(other.s.p0, other.s.p1))
    }
}

/// The `Cross` event: two segments that cross at the event point. Ordered arbitrarily, by their
/// raw endpoints.
// Port of: modules/bentleyottmann/include/EventQueue.h#L34-L41 (chrome/m156)
#[doc(alias = "bentleyottmann::Cross")]
#[derive(Clone, Copy, Debug)]
pub struct Cross {
    /// The first crossing segment.
    pub s0: Segment,
    /// The second crossing segment.
    pub s1: Segment,
}

impl PartialEq for Cross {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for Cross {}

impl PartialOrd for Cross {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cross {
    /// `std::tie(c0.s0.p0, c0.s0.p1, c0.s1.p0, c0.s1.p1)` lexicographic.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.s0.p0, self.s0.p1, self.s1.p0, self.s1.p1).cmp(&(
            other.s0.p0,
            other.s0.p1,
            other.s1.p0,
            other.s1.p1,
        ))
    }
}

/// The type of an event. The variants are ordered `Lower < Cross < Upper`, the order of the
/// C++ `std::variant<Lower, Cross, Upper>`.
// Port of: modules/bentleyottmann/include/EventQueue.h#L42 (chrome/m156)
#[doc(alias = "bentleyottmann::EventType")]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventType {
    /// A segment ends at the event point.
    Lower,
    /// Two segments cross at the event point.
    Cross(Cross),
    /// A segment starts at the event point.
    Upper(Upper),
}

impl EventType {
    /// The index of the active alternative, as `std::variant::index()`.
    #[must_use]
    pub fn index(&self) -> usize {
        match self {
            Self::Lower => 0,
            Self::Cross(_) => 1,
            Self::Upper(_) => 2,
        }
    }
}

/// An event at a point. Events are ordered by point first, then by type and contents.
// Port of: modules/bentleyottmann/include/EventQueue.h#L43-L49 (chrome/m156)
#[doc(alias = "bentleyottmann::Event")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    /// The point where the event happens (`where` in C++).
    pub where_: Point,
    /// What happens there.
    pub event_type: EventType,
}

impl PartialOrd for Event {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Event {
    /// `std::tie(e0.where, e0.type) < std::tie(e1.where, e1.type)`.
    fn cmp(&self, other: &Self) -> Ordering {
        (self.where_, self.event_type).cmp(&(other.where_, other.event_type))
    }
}

/// The sweep line operations the event queue calls. Port of `SweepLineInterface`.
// Port of: modules/bentleyottmann/include/EventQueueInterface.h#L49-L64 (chrome/m156)
#[doc(alias = "bentleyottmann::SweepLineInterface")]
pub trait SweepLineInterface {
    /// These are the segments to remove from the sweep line.
    fn handle_deletions(&mut self, event_point: Point, removing: &DeletionSegmentSet);

    /// Inserts `inserting` into the sweep line. Checks the inserting segments against the
    /// existing sweep line segments and reports any crossings using `queue`.
    fn handle_insertions_and_check_for_new_crossings(
        &mut self,
        event_point: Point,
        inserting: &InsertionSegmentSet,
        queue: &mut dyn EventQueueInterface,
    );
}

/// The queue of events, ordered by point, type and contents. The contents ordering is arbitrary
/// but makes the events unique in the queue.
pub type Queue = BTreeSet<Event>;

/// The event queue of the sweep. It produces the crossings found while processing the events.
// Port of: modules/bentleyottmann/include/EventQueue.h#L62-L86 (chrome/m156)
#[doc(alias = "bentleyottmann::EventQueue")]
#[derive(Debug)]
pub struct EventQueue {
    last_event_point: Point,
    deletion_set: DeletionSegmentSet,
    insertion_set: InsertionSegmentSet,
    queue: Queue,
    crossings: Vec<Crossing>,
}

impl Default for EventQueue {
    /// `EventQueue() = default;`: an empty queue.
    fn default() -> Self {
        Self::new(Queue::new())
    }
}

impl EventQueue {
    /// Builds a queue from the given events (`explicit EventQueue(Queue&&)`).
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L51 (chrome/m156)
    #[must_use]
    pub fn new(queue: Queue) -> Self {
        Self {
            last_event_point: Point::smallest(),
            deletion_set: DeletionSegmentSet::new(),
            insertion_set: InsertionSegmentSet::new(),
            queue,
            crossings: Vec::new(),
        }
    }

    /// Makes an event queue with one `Upper` event for each segment, or `None` if the data are
    /// out of range: the bounding box of the segments must not overflow the point differences.
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L14-L31 (chrome/m156)
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(segments: &[Segment]) -> Option<Self> {
        let mut queue = Queue::new();
        let mut left = Point::largest().x;
        let mut top = Point::largest().y;
        let mut right = Point::smallest().x;
        let mut bottom = Point::smallest().y;
        for s in segments {
            let (l, t, r, b) = s.bounds();
            left = l.min(left);
            top = t.min(top);
            right = r.max(right);
            bottom = b.max(bottom);
            queue.insert(Event {
                where_: s.upper(),
                event_type: EventType::Upper(Upper { s: *s }),
            });
        }

        // If min max difference is too large, fail.
        if Point::difference_too_big(Point::new(left, top), Point::new(right, bottom)) {
            return None;
        }

        Some(Self::new(queue))
    }

    /// Adds `event`, which must be upstream of the current event point.
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L56-L60 (chrome/m156)
    fn add(&mut self, event: Event) {
        // New events must be up stream from the current event.
        debug_assert!(self.last_event_point < event.where_);
        self.queue.insert(event);
    }

    /// True if there are events left to handle.
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L66-L68 (chrome/m156)
    #[must_use]
    pub fn has_more_events(&self) -> bool {
        !self.queue.is_empty()
    }

    /// Handles all the events at the next event point, then calls `handler` for the deletions and
    /// the insertions (with crossing checks) at that point.
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L76-L124 (chrome/m156)
    pub fn handle_next_event_point(&mut self, handler: &mut dyn SweepLineInterface) {
        debug_assert!(!self.queue.is_empty());

        // Clear temp segment buffers.
        self.deletion_set.clear();
        self.insertion_set.clear();

        // An events that are Lower points.
        let mut has_lower = false;

        let event_point = self
            .queue
            .first()
            .map_or(Point::smallest(), |event| event.where_);

        // We must make forward progress.
        debug_assert!(self.last_event_point < event_point);
        self.last_event_point = event_point;

        // Accumulate changes for all events with the same event point. Events added while
        // handling them are upstream of event_point, so they are not part of this batch.
        let batch: Vec<Event> = self
            .queue
            .iter()
            .take_while(|event| event.where_ == event_point)
            .copied()
            .collect();
        for event in &batch {
            match event.event_type {
                EventType::Lower => has_lower = true,
                EventType::Cross(c) => {
                    // Crossing segments must be deleted and re-inserted in the sweep line.
                    self.deletion_set.insert(c.s0);
                    self.deletion_set.insert(c.s1);
                    self.insertion_set.insert(c.s0);
                    self.insertion_set.insert(c.s1);
                }
                EventType::Upper(u) => {
                    // Upper events are added to the sweep line, and a lower event is added to the
                    // event queue.
                    self.insertion_set.insert(u.s);

                    // Add the delete event for the inserted segment. Make sure we are not adding
                    // more events on this eventPoint.
                    debug_assert_ne!(u.s.lower(), u.s.upper());
                    self.add(Event {
                        where_: u.s.lower(),
                        event_type: EventType::Lower,
                    });
                }
            }
        }

        // Remove all accumulated events with the same event point.
        for event in &batch {
            self.queue.remove(event);
        }

        if has_lower || !self.deletion_set.is_empty() {
            // There are segments to delete.
            handler.handle_deletions(event_point, &self.deletion_set);
        }

        if has_lower || !self.deletion_set.is_empty() || !self.insertion_set.is_empty() {
            // If there are insertions then insert them. If there are no insertions, but there
            // were deletions we need to check for new crossings. The insertion set is taken out
            // while the handler gets `self` as its queue.
            let inserting = std::mem::take(&mut self.insertion_set);
            handler.handle_insertions_and_check_for_new_crossings(event_point, &inserting, self);
            self.insertion_set = inserting;
        }
    }

    /// The crossings added so far.
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L131 (chrome/m156)
    #[must_use]
    pub fn crossings(&self) -> Vec<Crossing> {
        self.crossings.clone()
    }
}

impl EventQueueInterface for EventQueue {
    // Port of: modules/bentleyottmann/src/EventQueue.cpp#L62-L65 (chrome/m156)
    fn add_crossing(&mut self, crossing_point: Point, s0: &Segment, s1: &Segment) {
        self.add(Event {
            where_: crossing_point,
            event_type: EventType::Cross(Cross { s0: *s0, s1: *s1 }),
        });
        self.crossings.push(Crossing {
            s0: *s0,
            s1: *s1,
            crossing: crossing_point,
        });
    }
}

/// Port of `EventQueueTestingPeer`: reads the queue of an [`EventQueue`] from the tests.
#[doc(hidden)]
#[derive(Debug)]
pub struct EventQueueTestingPeer;

impl EventQueueTestingPeer {
    /// Extracts the event at the beginning of the queue.
    ///
    /// # Panics
    ///
    /// If the queue is empty.
    // Port of: modules/bentleyottmann/tests/EventQueueTest.cpp#L8-L18 (chrome/m156)
    #[must_use]
    pub fn next_event(eq: &mut EventQueue) -> Event {
        debug_assert!(eq.has_more_events());
        // Extract event at the beginning of the queue and remove it from the queue.
        eq.queue
            .pop_first()
            .expect("NextEvent needs an event in the queue")
    }
}
