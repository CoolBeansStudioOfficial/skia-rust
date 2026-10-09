// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawOrder.h

//! Draw ordering for Graphite: monotonic sequences and the `DrawOrder` that aggregates them.

use std::marker::PhantomData;

// Port of: src/gpu/graphite/DrawOrder.h#L19-L50 (chrome/m156)
/// Helper to encapsulate an unsigned number and enforce that it can only be used to create a
/// monotonic sequence. The type argument `S` defines different sequences enforced by the
/// compiler. The entire sequence can be indexed by `u16`.
#[doc(alias = "skgpu::graphite::MonotonicValue")]
pub struct MonotonicValue<S> {
    index: u16,
    _sequence: PhantomData<S>,
}

impl<S> MonotonicValue<S> {
    /// `MonotonicValue::First()`.
    #[must_use]
    pub const fn first() -> Self {
        Self::from_index(0)
    }

    /// `MonotonicValue::Last()`.
    #[must_use]
    pub const fn last() -> Self {
        Self::from_index(0xffff)
    }

    // Private constructor, as in the C++.
    const fn from_index(index: u16) -> Self {
        Self {
            index,
            _sequence: PhantomData,
        }
    }

    /// `bits()`.
    #[must_use]
    pub const fn bits(&self) -> u16 {
        self.index
    }

    /// `next()`: the next value in the sequence after this one (wrapping at `u16`, as the C++
    /// implicit conversion does).
    #[must_use]
    pub const fn next(&self) -> Self {
        Self::from_index(self.index.wrapping_add(1))
    }
}

// Manual impls so that `S` need not implement these traits.
impl<S> Clone for MonotonicValue<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S> Copy for MonotonicValue<S> {}

impl<S> PartialEq for MonotonicValue<S> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<S> Eq for MonotonicValue<S> {}

impl<S> PartialOrd for MonotonicValue<S> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<S> Ord for MonotonicValue<S> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.index.cmp(&other.index)
    }
}

impl<S> std::fmt::Debug for MonotonicValue<S> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("MonotonicValue").field(&self.index).finish()
    }
}

/// The sequence marker of [`CompressedPaintersOrder`].
// Port of: src/gpu/graphite/DrawOrder.h#L59 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressedPaintersOrderSequence;

/// `CompressedPaintersOrder` is an ordinal number that allows draw commands to be re-ordered so
/// long as when they are executed, the read/writes to the color|depth attachments respect the
/// original painter's order. Logical draws with the same `CompressedPaintersOrder` can be assumed
/// to be executed in any order, however that may have been determined (e.g. `BoundsManager` or
/// relying on a depth test during rasterization).
// Port of: src/gpu/graphite/DrawOrder.h#L60 (chrome/m156)
pub type CompressedPaintersOrder = MonotonicValue<CompressedPaintersOrderSequence>;

/// The sequence marker of [`DisjointStencilIndex`].
// Port of: src/gpu/graphite/DrawOrder.h#L76 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisjointStencilIndexSequence;

/// Each `DisjointStencilIndex` specifies an implicit set of non-overlapping draws. Assuming that
/// two draws have the same `CompressedPaintersOrder` and the same `DisjointStencilIndex`, their
/// substeps for multi-pass rendering (stencil-then-cover, etc.) can be intermingled with each
/// other and produce the same results as if each draw's substeps were executed in order before
/// moving on to the next draw's.
// Port of: src/gpu/graphite/DrawOrder.h#L77 (chrome/m156)
pub type DisjointStencilIndex = MonotonicValue<DisjointStencilIndexSequence>;

/// The sequence marker of [`PaintersDepth`].
// Port of: src/gpu/graphite/DrawOrder.h#L87 (chrome/m156)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaintersDepthSequence;

/// Every draw has an associated depth value. The value is constant across the entire draw and is
/// not related to any varying Z coordinate induced by a 4x4 transform.
// Port of: src/gpu/graphite/DrawOrder.h#L88 (chrome/m156)
pub type PaintersDepth = MonotonicValue<PaintersDepthSequence>;

// Port of: src/gpu/graphite/DrawOrder.h#L107-L170 (chrome/m156)
/// `DrawOrder` aggregates the three separate sequences that Graphite uses to re-order draws and
/// their substeps as much as possible while preserving the painter's order semantics of the Skia
/// API.
#[doc(alias = "skgpu::graphite::DrawOrder")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DrawOrder {
    paint_order: CompressedPaintersOrder,
    stencil_index: DisjointStencilIndex,
    depth: PaintersDepth,
}

impl DrawOrder {
    /// The first `PaintersDepth` is reserved for clearing the depth attachment; any draw using
    /// this depth will always fail the depth test.
    pub const K_CLEAR_DEPTH: PaintersDepth = PaintersDepth::first();
    /// The first `CompressedPaintersOrder` is reserved to indicate there is no previous draw that
    /// must come before a draw.
    pub const K_NO_INTERSECTION: CompressedPaintersOrder = CompressedPaintersOrder::first();
    /// The last `DisjointStencilIndex` is reserved to indicate an unassigned stencil set.
    pub const K_UNASSIGNED: DisjointStencilIndex = DisjointStencilIndex::last();

    /// `DrawOrder(PaintersDepth originalOrder)`.
    #[must_use]
    pub const fn new(original_order: PaintersDepth) -> Self {
        Self {
            paint_order: Self::K_NO_INTERSECTION,
            stencil_index: Self::K_UNASSIGNED,
            depth: original_order,
        }
    }

    /// `DrawOrder(PaintersDepth originalOrder, CompressedPaintersOrder compressedOrder)`.
    #[must_use]
    pub const fn with_paint_order(
        original_order: PaintersDepth,
        compressed_order: CompressedPaintersOrder,
    ) -> Self {
        Self {
            paint_order: compressed_order,
            stencil_index: Self::K_UNASSIGNED,
            depth: original_order,
        }
    }

    /// `paintOrder()`.
    #[must_use]
    pub const fn paint_order(&self) -> CompressedPaintersOrder {
        self.paint_order
    }

    /// `stencilIndex()`.
    #[must_use]
    pub const fn stencil_index(&self) -> DisjointStencilIndex {
        self.stencil_index
    }

    /// `depth()`.
    #[must_use]
    pub const fn depth(&self) -> PaintersDepth {
        self.depth
    }

    /// While the `PaintersDepth` is a monotonically increasing value, the depth buffer prefers to
    /// use LESS and LEQUAL comparisons starting with a clear value of 1.f, so we normalize and
    /// flip the floating point value to count down from 1.0.
    // Port of: src/gpu/graphite/DrawOrder.h#L138-L140 (chrome/m156)
    #[must_use]
    pub fn depth_as_float(&self) -> f32 {
        1.0 - f32::from(self.depth.bits()) / f32::from(PaintersDepth::last().bits())
    }

    /// Coopt the stencil index to encode the draw's actual painter's depth in decreasing order,
    /// for use enforcing F2B order (since the compressed painter's order handles B2F).
    // Port of: src/gpu/graphite/DrawOrder.h#L144-L148 (chrome/m156)
    pub fn reverse_depth_as_stencil(&mut self) -> &mut Self {
        debug_assert_eq!(self.stencil_index, Self::K_UNASSIGNED); // can't have a real stencil index
        self.stencil_index = DisjointStencilIndex::from_index(
            DisjointStencilIndex::last().bits() - self.depth.bits(),
        );
        self
    }

    /// A draw must be ordered after all previous draws that it depends on.
    // Port of: src/gpu/graphite/DrawOrder.h#L150-L157 (chrome/m156)
    pub fn depends_on_painters_order(&mut self, prev_draw: CompressedPaintersOrder) -> &mut Self {
        let next = prev_draw.next();
        if self.paint_order < next {
            self.paint_order = next;
        }
        self
    }

    /// Stencil usage should only be set once.
    // Port of: src/gpu/graphite/DrawOrder.h#L159-L164 (chrome/m156)
    pub fn depends_on_stencil(&mut self, disjoint_set: DisjointStencilIndex) -> &mut Self {
        debug_assert_eq!(self.stencil_index, Self::K_UNASSIGNED);
        self.stencil_index = disjoint_set;
        self
    }
}
