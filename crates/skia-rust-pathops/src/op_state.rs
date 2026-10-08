// Some ported helpers have no caller yet: the Op pieces that use them (tight bounds,
// the builder) are later slices. Remove this allow as those callers are ported.
#![allow(dead_code)]
// Copyright 2012 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/pathops/SkPathOpsTypes.h (SkOpGlobalState, SkOpPhase),
// src/pathops/SkOpCoincidence.h (SkOpCoincidence fields), and the arenas that replace
// Skia's SkArenaAlloc.

//! The state of one boolean operation (`SkOpGlobalState`) and the arenas of its op graph.
//!
//! Skia's op graph is pointer linked: contours own segments, segments own spans, spans link
//! to their points (`SkOpPtT`) and angles, and points are linked in rings. Here every object
//! lives in a `Vec` owned by [`OpState`], and every pointer is a typed index. Objects are never
//! freed during an operation (Skia's arena does not free them either), so an index stays valid
//! for the whole operation. The graph methods are `impl OpState` blocks spread over the modules
//! of the C++ files they come from.

use crate::op_angle::Angle;
use crate::op_coincidence::{CoinSet, CoincidentSpans};
use crate::op_contour::Contour;
use crate::op_segment::Segment;
use crate::op_span::{PtT, Span};

macro_rules! arena_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
        pub struct $name(pub(crate) usize);
    };
}

arena_id!(
    /// Index of a [`Contour`] in an [`OpState`]: Skia's `SkOpContour*`.
    ContourId
);
arena_id!(
    /// Index of a [`Segment`] in an [`OpState`]: Skia's `SkOpSegment*`.
    SegId
);
arena_id!(
    /// Index of a [`Span`] in an [`OpState`]: Skia's `SkOpSpanBase*` / `SkOpSpan*`.
    SpanId
);
arena_id!(
    /// Index of a [`PtT`] in an [`OpState`]: Skia's `SkOpPtT*`.
    PtTId
);
arena_id!(
    /// Index of an [`Angle`] in an [`OpState`]: Skia's `SkOpAngle*`.
    AngleId
);
arena_id!(
    /// Index of a [`CoincidentSpans`] record in an [`OpState`]: Skia's `SkCoincidentSpans*`.
    CoinId
);
arena_id!(
    /// Index of a [`CoinSet`] (one `SkOpCoincidence` object) in an [`OpState`].
    CoinSetId
);

/// `SkOpPhase`: which stage of the operation is running.
// Port of: src/pathops/SkPathOpsTypes.h#L29-L34 (chrome/m156)
#[doc(alias = "SkOpPhase")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum OpPhase {
    /// `kNoChange`: `set_phase` ignores this value.
    NoChange,
    /// `kIntersecting`.
    Intersecting,
    /// `kWalking`.
    Walking,
    /// `kFixWinding`.
    FixWinding,
}

/// `SkOpGlobalState::kMaxWindingTries`.
// Port of: src/pathops/SkPathOpsTypes.h#L47 (chrome/m156)
pub(crate) const MAX_WINDING_TRIES: i32 = 10;

/// `SK_MinS32` (`-SK_MaxS32`): the "not yet computed" value of winding sums.
// Port of: include/private/SkMath.h#L22 (chrome/m156)
pub(crate) const SK_MIN_S32: i32 = i32::MIN + 1;

/// `SkOpGlobalState` together with the arenas of its op graph, and the fields of the
/// `SkOpCoincidence` that the operation owns.
// Port of: src/pathops/SkPathOpsTypes.h#L36-L85 (chrome/m156)
#[doc(alias = "SkOpGlobalState")]
#[derive(Debug, Default)]
pub struct OpState {
    /// Every contour; `ContourId(0)` is the `SkOpContourHead`.
    pub(crate) contours: Vec<Contour>,
    /// Every segment (`SkOpSegment`).
    pub(crate) segments: Vec<Segment>,
    /// Every span (`SkOpSpanBase` and `SkOpSpan`), in one arena.
    pub(crate) spans: Vec<Span>,
    /// Every point-and-t record (`SkOpPtT`).
    pub(crate) ptts: Vec<PtT>,
    /// Every angle (`SkOpAngle`).
    pub(crate) angles: Vec<Angle>,
    /// Every coincident span record (`SkCoincidentSpans`).
    pub(crate) coin_spans: Vec<CoincidentSpans>,
    /// Every `SkOpCoincidence` object (its `fHead` and `fTop` lists).
    pub(crate) coin_sets: Vec<CoinSet>,
    /// The object `globalState()->coincidence()` returns (the last one constructed).
    pub(crate) coin_global: Option<CoinSetId>,
    /// `SkOpGlobalState::fContourHead`.
    pub(crate) contour_head: Option<ContourId>,
    /// `SkOpGlobalState::fNested`.
    pub(crate) nested: i32,
    /// `SkOpGlobalState::fAllocatedOpSpan`.
    pub(crate) allocated_op_span: bool,
    /// `SkOpGlobalState::fWindingFailed`.
    pub(crate) winding_failed: bool,
    /// `SkOpGlobalState::fPhase`.
    pub(crate) phase: Option<OpPhase>,
}

impl OpState {
    /// `SkOpGlobalState(SkOpContourHead* head, SkArenaAlloc* allocator)`, with the head
    /// contour already created as `ContourId(0)`.
    // Port of: src/pathops/SkPathOpsTypes.cpp (SkOpGlobalState::SkOpGlobalState) (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        let mut state = Self {
            contour_head: None,
            phase: Some(OpPhase::NoChange),
            ..Self::default()
        };
        state.contours.push(Contour::default());
        state.contour_head = Some(ContourId(0));
        state
    }

    /// `SkOpGlobalState::setPhase`: `kNoChange` is ignored.
    // Port of: src/pathops/SkPathOpsTypes.h#L63-L70 (chrome/m156)
    pub(crate) fn set_phase(&mut self, phase: OpPhase) {
        if phase == OpPhase::NoChange {
            return;
        }
        self.phase = Some(phase);
    }

    /// `SkOpGlobalState::phase()`.
    #[must_use]
    pub(crate) fn phase(&self) -> OpPhase {
        self.phase.unwrap_or(OpPhase::NoChange)
    }

    /// `SkOpGlobalState::setWindingFailed()`.
    // Port of: src/pathops/SkPathOpsTypes.h#L111-L113 (chrome/m156)
    pub(crate) fn set_winding_failed(&mut self) {
        self.winding_failed = true;
    }

    /// `SkOpGlobalState::windingFailed()`.
    #[must_use]
    pub(crate) fn winding_failed(&self) -> bool {
        self.winding_failed
    }

    /// `SkOpGlobalState::contourHead()`.
    #[must_use]
    pub(crate) fn contour_head(&self) -> ContourId {
        self.contour_head.unwrap_or(ContourId(0))
    }

    /// `SkOpGlobalState::setAllocatedOpSpan()`: an arena span was created by this call.
    pub(crate) fn set_allocated_op_span(&mut self) {
        self.allocated_op_span = true;
    }

    /// `SkOpGlobalState::resetAllocatedOpSpan()`.
    pub(crate) fn reset_allocated_op_span(&mut self) {
        self.allocated_op_span = false;
    }

    /// `SkOpGlobalState::allocatedOpSpan()`.
    #[must_use]
    pub(crate) fn allocated_op_span(&self) -> bool {
        self.allocated_op_span
    }

    /// `SkOpGlobalState::bumpNested()`.
    pub(crate) fn bump_nested(&mut self) {
        self.nested += 1;
    }

    /// `SkOpGlobalState::clearNested()`.
    pub(crate) fn clear_nested(&mut self) {
        self.nested = 0;
    }

    /// `SkOpGlobalState::nested()`.
    #[must_use]
    pub(crate) fn nested(&self) -> i32 {
        self.nested
    }

    /// `new SkOpSpan` / `SkOpSpanBase` allocation: a span whose embedded `SkOpPtT` is its
    /// own record. Fields are set by `init_base` or `init`, as in C++.
    pub(crate) fn alloc_span(&mut self) -> SpanId {
        let span_id = SpanId(self.spans.len());
        let ptt_id = PtTId(self.ptts.len());
        self.ptts.push(PtT::new(span_id));
        self.spans.push(Span::new(ptt_id));
        span_id
    }

    /// Allocates a `SkOpPtT` record (`SkOpSegment::addT` and friends use these).
    pub(crate) fn alloc_ptt(&mut self, span: SpanId) -> PtTId {
        let id = PtTId(self.ptts.len());
        self.ptts.push(PtT::new(span));
        id
    }

    /// Appends a contour (`SkOpContourHead::appendContour`) and returns its id.
    pub(crate) fn alloc_contour(&mut self) -> ContourId {
        let id = ContourId(self.contours.len());
        self.contours.push(Contour::default());
        id
    }

    /// Appends an angle record (`allocator()->make<SkOpAngle>()`).
    pub(crate) fn alloc_angle(&mut self) -> AngleId {
        let id = AngleId(self.angles.len());
        self.angles.push(Angle::default());
        id
    }

    /// Appends a segment record (`allocator()->make<SkOpSegment>()`).
    pub(crate) fn alloc_segment(&mut self) -> SegId {
        let id = SegId(self.segments.len());
        self.segments.push(Segment::default());
        id
    }

    /// Appends a coincident span record (`SkCoincidentSpans` from the allocator).
    pub(crate) fn alloc_coin(&mut self) -> CoinId {
        let id = CoinId(self.coin_spans.len());
        self.coin_spans.push(CoincidentSpans::default());
        id
    }

    /// `SkOpSegment::insert(SkOpSpan* prev)`: links a new span after `prev`.
    // Port of: src/pathops/SkOpSegment.h#L233-L244 (chrome/m156)
    pub(crate) fn insert_span(&mut self, prev: SpanId) -> SpanId {
        self.set_allocated_op_span();
        let result = self.alloc_span();
        let next = self.spans[prev.0].next;
        self.spans[result.0].prev = Some(prev);
        self.spans[prev.0].next = Some(result);
        self.spans[result.0].next = next;
        if let Some(next) = next {
            self.spans[next.0].prev = Some(result);
        }
        result
    }
}
