// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/DrawListTypes.h

//! The types `DrawListLayer` collects draws in: [`Layer`]s of [`BindingList`]s of [`Draw`]s.
//!
//! Skia allocates these from an arena and links them with intrusive lists and raw pointers. Here
//! the draws, the binding lists and the layers live in index-addressed arenas
//! ([`BindingArena`] and a `Vec<Layer>`), and the links are indices. A layer's binding lists are
//! a doubly linked chain over the shared binding arena ([`BindingChain`]). Indices are valid
//! until the owning draw list is reset.

use std::sync::Arc;

use bitflags::bitflags;

use crate::graphite::draw_order::CompressedPaintersOrder;
use crate::graphite::geom::rect::{ComplementRect, Rect};
use crate::graphite::render_step::RenderStep;

bitflags! {
    /// What a draw blocks, and what it must be tested against (`BoundsFlags`).
    // Port of: src/gpu/graphite/DrawListTypes.h#L34-L47 (chrome/m156)
    #[doc(alias = "skgpu::graphite::BoundsFlags")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct BoundsFlags: u8 {
        /// `kNone`: no need to test against draws; only respect the stop layer when searching.
        const NONE = 0x0;
        /// `kStencil`: cannot intersect with anything that uses stencil; but could draw out of
        /// order.
        const STENCIL = 0x1;
        /// `kColor`: cannot intersect with anything that uses color, and cannot be ordered
        /// earlier.
        const COLOR = 0x2;
        /// `kMustBeDisjoint`: adds a requirement that draws within the same `BindingList` must
        /// be disjoint from each other, even if there wasn't otherwise a stencil or color
        /// dependency. This arises in two cases: blending requires barriers so we can't just
        /// rely on the GPU's rasterization order to handling the painter's order for us; and the
        /// draw belongs to a multi-step renderer and we don't want the intermediate steps to
        /// contaminate other draws within the same layer and renderer.
        const MUST_BE_DISJOINT = 0x4;
    }
}

bitflags! {
    /// The result of testing a draw against a layer (`BoundsTestResult`).
    // Port of: src/gpu/graphite/DrawListTypes.h#L49-L53 (chrome/m156)
    #[doc(alias = "skgpu::graphite::BoundsTestResult")]
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct BoundsTestResult: u8 {
        /// `kBlocked`: the draw must go in a layer after the tested layer.
        const BLOCKED = 0x0;
        /// `kAllowedInLayer`: the draw can go in the layer.
        const ALLOWED_IN_LAYER = 0x1;
        /// `kAllowedBeforeLayer`: the draw can go before the tested layer.
        const ALLOWED_BEFORE_LAYER = 0x2;
    }
}

/// Identifies a `BindingList`: the pipeline, textures and (without SSBOs) uniforms its draws
/// share (`LayerKey`).
// Port of: src/gpu/graphite/DrawListTypes.h#L55-L81 (chrome/m156)
#[doc(alias = "skgpu::graphite::LayerKey")]
#[derive(Clone, Copy, Debug)]
pub struct LayerKey {
    /// `fPipelineIndex`.
    pub pipeline_index: u32,
    /// `fTextureIndex`.
    pub texture_index: u32,
    /// `fUniformIndex`: set to invalid for `BindingList`s when SSBOs are used; for SSBOs, each
    /// draw's uniform index is stored on the `Draw` itself.
    pub uniform_index: u32,
    /// `fFlags`: new draws with a testMask that overlaps with `fFlags` must be checked for
    /// bounds intersections with the draws in the `BindingList` for this key.
    pub flags: BoundsFlags,
}

impl LayerKey {
    /// `performsShading()`.
    #[must_use]
    pub fn performs_shading(&self) -> bool {
        self.flags.contains(BoundsFlags::COLOR)
    }

    /// `usesStencil()`.
    #[must_use]
    pub fn uses_stencil(&self) -> bool {
        self.flags.contains(BoundsFlags::STENCIL)
    }

    /// `isDepthOnly()`.
    #[must_use]
    pub fn is_depth_only(&self) -> bool {
        self.flags == BoundsFlags::NONE
    }

    /// `isSimpleShading()`.
    #[must_use]
    pub fn is_simple_shading(&self) -> bool {
        self.flags == BoundsFlags::COLOR
    }

    /// `isEqual(other)`.
    #[must_use]
    pub fn is_equal(&self, other: &LayerKey) -> bool {
        // The pipeline defines the layer key's flags, so if the pipeline index is the same the
        // flags should be too and we skip checking them as part of isEqual.
        debug_assert!(self.pipeline_index != other.pipeline_index || self.flags == other.flags);
        self.pipeline_index == other.pipeline_index
            && self.texture_index == other.texture_index
            && self.uniform_index == other.uniform_index
    }
}

/// An index into a draw list's `DrawParams`; returned by `recordDraw` for the clip stack, which
/// updates a clip draw's params after the fact. Valid until the list is reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DrawParamsId(pub(crate) u32);

/// An index into a draw list's layers: the layer a draw was recorded in. Valid until the list is
/// reset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayerId(pub(crate) u32);

/// The index of a `BindingList` in its [`BindingArena`].
pub type BindingId = u32;

/// The index of a `Draw` in its [`BindingArena`].
pub type DrawId = u32;

/// One draw of one render step in a binding list (`Draw`).
// Port of: src/gpu/graphite/DrawListTypes.h#L83-L94 (chrome/m156)
#[doc(alias = "skgpu::graphite::Draw")]
#[derive(Clone, Copy, Debug)]
pub struct Draw {
    /// `fDrawParams`.
    pub draw_params: DrawParamsId,
    /// `fUniformIndex`.
    pub uniform_index: u32,
    /// `fNext`.
    pub next: Option<DrawId>,
}

/// The draws that share a `LayerKey`, in a singly linked list that is prepended to for
/// front-to-back rendering or appended to for back-to-front rendering (`BindingList`).
// Port of: src/gpu/graphite/DrawListTypes.h#L96-L170 (chrome/m156)
#[doc(alias = "skgpu::graphite::BindingList")]
#[derive(Clone, Debug)]
pub struct BindingList {
    /// `fBounds`.
    pub bounds: Rect,
    /// `fStep`.
    pub step: Arc<dyn RenderStep>,
    /// `fKey`.
    pub key: LayerKey,
    /// `fHead`.
    pub head: Option<DrawId>,
    /// `fTail`.
    pub tail: Option<DrawId>,
    // The links of the layer's chain (`SK_DECLARE_INTERNAL_LLIST_INTERFACE(BindingList)`).
    prev: Option<BindingId>,
    next: Option<BindingId>,
}

impl BindingList {
    /// `BindingList(step, key)`.
    #[must_use]
    pub fn new(step: Arc<dyn RenderStep>, key: LayerKey) -> Self {
        Self {
            bounds: Rect::infinite_inverted(),
            step,
            key,
            head: None,
            tail: None,
            prev: None,
            next: None,
        }
    }

    /// The previous binding list of the layer (`fPrev`).
    #[must_use]
    pub fn prev(&self) -> Option<BindingId> {
        self.prev
    }

    /// The next binding list of the layer (`fNext`).
    #[must_use]
    pub fn next(&self) -> Option<BindingId> {
        self.next
    }

    /// `isBetterMatch(key, existingMatch)`.
    // Port of: src/gpu/graphite/DrawListTypes.h#L120-L152 (chrome/m156)
    #[must_use]
    pub fn is_better_match(&self, key: &LayerKey, existing_match: Option<&BindingList>) -> bool {
        if key.pipeline_index != self.key.pipeline_index {
            // Any partial match must still share the same pipeline
            return false;
        }
        let Some(existing_match) = existing_match else {
            return true; // Any pipeline match is better than no match
        };

        // Otherwise we need to rank based on similarities, preferring texture matches to
        // uniform matches.
        debug_assert_eq!(existing_match.key.pipeline_index, key.pipeline_index);
        let existing_texture_match = existing_match.key.texture_index == key.texture_index;
        let new_texture_match = self.key.texture_index == key.texture_index;
        if existing_texture_match != new_texture_match {
            // If `newTextureMatch` is true, then the new BindingList is definitely the better
            // match (prioritizing textures over UBO changes). If it's false, then the old
            // match was better since it had a texture match.
            return new_texture_match;
        } // else either both match on the texture, or neither match so equal preference.

        let existing_uniform_match = existing_match.key.uniform_index == key.uniform_index;
        let new_uniform_match = self.key.uniform_index == key.uniform_index;
        if existing_uniform_match != new_uniform_match {
            // Like above, if `newUniformMatch` is true, it's the better match.
            return new_uniform_match;
        } // else either both match on the uniform, or neither match so equal preference.

        // At this point, they are equivalent, so prefer the new list as it's deeper
        true
    }
}

/// The arena of draws and binding lists of a `DrawListLayer`.
#[derive(Debug, Default)]
pub struct BindingArena {
    /// All the binding lists, of all the layers.
    pub bindings: Vec<BindingList>,
    /// All the draws, of all the binding lists.
    pub draws: Vec<Draw>,
}

impl BindingArena {
    /// Removes everything (`fStorage.reset()`).
    pub fn reset(&mut self) {
        self.bindings.clear();
        self.draws.clear();
    }

    /// `BindingList::addDraw(alloc, draw, uniformIndex, backToFront)`.
    // Port of: src/gpu/graphite/DrawListTypes.h#L154-L170 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn add_draw(
        &mut self,
        binding: BindingId,
        draw_params: DrawParamsId,
        draw_bounds: Rect,
        uniform_index: u32,
        back_to_front: bool,
    ) {
        self.bindings[binding as usize].bounds.join(draw_bounds);
        let draw_id = u32::try_from(self.draws.len()).expect("draw count fits");
        let draw = Draw {
            draw_params,
            uniform_index,
            next: None,
        };
        let list = &mut self.bindings[binding as usize];
        if let (Some(head), Some(tail)) = (list.head, list.tail) {
            if back_to_front {
                self.draws.push(draw);
                self.draws[tail as usize].next = Some(draw_id);
                self.bindings[binding as usize].tail = Some(draw_id);
            } else {
                let mut draw = draw;
                draw.next = Some(head);
                self.draws.push(draw);
                self.bindings[binding as usize].head = Some(draw_id);
            }
        } else {
            self.draws.push(draw);
            list.head = Some(draw_id);
            list.tail = Some(draw_id);
        }
    }
}

/// The ordered binding lists of one layer: a doubly linked list over the arena
/// (`SkTInternalLList<BindingList>`).
#[derive(Clone, Copy, Debug, Default)]
pub struct BindingChain {
    head: Option<BindingId>,
    tail: Option<BindingId>,
}

impl BindingChain {
    /// `isEmpty()`.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.head.is_none()
    }

    /// `head()`.
    #[must_use]
    pub fn head(&self) -> Option<BindingId> {
        self.head
    }

    /// `tail()`.
    #[must_use]
    pub fn tail(&self) -> Option<BindingId> {
        self.tail
    }

    /// `addToHead(list)`.
    pub fn add_to_head(&mut self, arena: &mut BindingArena, list: BindingId) {
        arena.bindings[list as usize].prev = None;
        arena.bindings[list as usize].next = self.head;
        if let Some(head) = self.head {
            arena.bindings[head as usize].prev = Some(list);
        } else {
            self.tail = Some(list);
        }
        self.head = Some(list);
    }

    /// `addToTail(list)`.
    pub fn add_to_tail(&mut self, arena: &mut BindingArena, list: BindingId) {
        arena.bindings[list as usize].next = None;
        arena.bindings[list as usize].prev = self.tail;
        if let Some(tail) = self.tail {
            arena.bindings[tail as usize].next = Some(list);
        } else {
            self.head = Some(list);
        }
        self.tail = Some(list);
    }

    /// `addBefore(newList, existingList)`.
    pub fn add_before(
        &mut self,
        arena: &mut BindingArena,
        new_list: BindingId,
        existing: BindingId,
    ) {
        let prev = arena.bindings[existing as usize].prev;
        arena.bindings[new_list as usize].next = Some(existing);
        arena.bindings[new_list as usize].prev = prev;
        arena.bindings[existing as usize].prev = Some(new_list);
        match prev {
            Some(prev) => arena.bindings[prev as usize].next = Some(new_list),
            None => self.head = Some(new_list),
        }
    }

    /// `remove(list)`.
    pub fn remove(&mut self, arena: &mut BindingArena, list: BindingId) {
        let prev = arena.bindings[list as usize].prev;
        let next = arena.bindings[list as usize].next;
        match prev {
            Some(prev) => arena.bindings[prev as usize].next = next,
            None => self.head = next,
        }
        match next {
            Some(next) => arena.bindings[next as usize].prev = prev,
            None => self.tail = prev,
        }
        arena.bindings[list as usize].prev = None;
        arena.bindings[list as usize].next = None;
    }

    /// `isInList(list)` (`SK_DEBUG`).
    #[must_use]
    pub fn is_in_list(&self, arena: &BindingArena, list: BindingId) -> bool {
        let mut current = self.head;
        while let Some(id) = current {
            if id == list {
                return true;
            }
            current = arena.bindings[id as usize].next;
        }
        false
    }

    /// The binding lists in order, for iteration.
    #[must_use]
    pub fn ids(&self, arena: &BindingArena) -> Vec<BindingId> {
        let mut ids = Vec::new();
        let mut current = self.head;
        while let Some(id) = current {
            ids.push(id);
            current = arena.bindings[id as usize].next;
        }
        ids
    }
}

/// Helper struct to aggregate the bounds of all draws in a `Layer` into a smaller set of aligned
/// bounding boxes (`BoundsBlock`).
// Port of: src/gpu/graphite/DrawListTypes.h#L172-L222 (chrome/m156)
#[doc(alias = "skgpu::graphite::BoundsBlock")]
#[derive(Clone, Debug)]
pub struct BoundsBlock {
    // Overall bounds
    bounds: Rect,
    rects: [Rect; Self::N],
    // The index into fRects that will consume the next recorded draw's bounds. Stochastically
    // this works about as well as trying to minify the area increase when adding a draw's bounds
    // but is much faster since there is no search.
    join_index: usize,
}

impl Default for BoundsBlock {
    // Initializing these Rects to infinite inverted makes the first call to join() equivalent
    // to just assigning the new rect.
    fn default() -> Self {
        Self {
            bounds: Rect::infinite_inverted(),
            rects: [Rect::infinite_inverted(); Self::N],
            join_index: 0,
        }
    }
}

impl BoundsBlock {
    // This is both performance and space sensitive. A value of 8 makes Layers smaller and can
    // lead to faster CPU collection for layers that have lots of draws, but it starts to hurt
    // the GPU batching. A value of 32 makes Layers larger, which slows down creating lots of
    // low-draw count layers and increases the bounds testing time, but helps GPU batching. More
    // than 32 starts to have diminishing returns for GPU batching. 16 seems to be a good sweet
    // spot in local benchmarking.
    //
    // NOTE: As long as a Layer has fewer than N draws recorded in it, its bounds testing is
    // exact.
    const N: usize = 16;

    /// `intersects(test)`.
    #[must_use]
    pub fn intersects(&self, test: ComplementRect) -> bool {
        if !self.bounds.intersects_complement(test) {
            return false;
        }

        let count = Self::N.min(self.join_index);
        for rect in &self.rects[..count] {
            if rect.intersects_complement(test) {
                return true;
            }
        }

        false
    }

    /// `add(rect)`.
    pub fn add(&mut self, rect: Rect) {
        self.bounds.join(rect);
        let index = self.join_index % Self::N;
        self.join_index += 1;
        self.rects[index].join(rect);
    }
}

/// A set of binding lists whose draws can be reordered freely among themselves; layers are
/// executed in order (`Layer`).
// Port of: src/gpu/graphite/DrawListTypes.h#L224-L350 (chrome/m156)
#[doc(alias = "skgpu::graphite::Layer")]
#[derive(Clone, Debug)]
pub struct Layer {
    /// `fColorBounds`.
    pub color_bounds: BoundsBlock,
    /// `fStencilBounds`.
    pub stencil_bounds: BoundsBlock,
    /// `fOrder`.
    pub order: CompressedPaintersOrder,
    /// `fBindings`.
    pub bindings: BindingChain,
}

impl Layer {
    /// `Layer(order)`.
    #[must_use]
    pub fn new(order: CompressedPaintersOrder) -> Self {
        Self {
            color_bounds: BoundsBlock::default(),
            stencil_bounds: BoundsBlock::default(),
            order,
            bindings: BindingChain::default(),
        }
    }

    /// `searchBinding(key, startList, forForwardMerge)`: performs no bounds checks, so can only
    /// be used when checks have already confirmed the `Layer` is valid for adding a new draw
    /// into. This searches backwards from `start_list` (exclusive) or the tail `BindingList` if
    /// `None`.
    ///
    /// Returns a `BindingList` matching `key` if one exists in the layer (or exists in the layer
    /// at or before `start_list`). If an exact match is not found, it attempts to return a
    /// `BindingList` that has the same pipeline index.
    // Port of: src/gpu/graphite/DrawListTypes.h#L249-L277 (chrome/m156)
    #[must_use]
    pub fn search_binding(
        &self,
        arena: &BindingArena,
        key: &LayerKey,
        start_list: Option<BindingId>,
    ) -> Option<BindingId> {
        // `startList` is exclusive, so if it's non-null the loop starts with fPrev.
        let mut pipeline_match: Option<BindingId> = None;
        let mut list = match start_list {
            Some(start) => arena.bindings[start as usize].prev,
            None => self.bindings.tail(),
        };
        while let Some(id) = list {
            let binding = &arena.bindings[id as usize];
            if binding.key.is_equal(key) {
                return Some(id);
            } else if binding
                .is_better_match(key, pipeline_match.map(|m| &arena.bindings[m as usize]))
            {
                // Save pipeline while continuing to search for an exact match
                debug_assert_eq!(binding.key.flags, key.flags);
                pipeline_match = Some(id);
            } else if key.performs_shading() && !binding.key.performs_shading() {
                // The BindingLists are split in two sections: a latter half with color (that is
                // check first because we start at the tail) and then anything else that is
                // non-shading (possibly with stencil). The depth-only and the stencil-only get
                // intermingled so we can't early out for those, but if `key` has color and
                // `list` does not, then a match is no longer possible.
                break;
            }
            list = binding.prev;
        }

        // Even though an exact match wasn't found, any non-null pipelineMatch can be used to
        // place a new binding list, which helps shift from a pipeline switch to just a dynamic
        // state.
        pipeline_match
    }

    /// `test(drawBounds, testMask)`: tests the draw with the given bounds and `LayerKey` against
    /// the draws already collected in this `Layer`, limiting checks to those that overlap with
    /// `test_mask`. Returns whether or not the draw is allowed in the layer, allowed before the
    /// layer, or must be in a later layer.
    // Port of: src/gpu/graphite/DrawListTypes.h#L279-L294 (chrome/m156)
    #[must_use]
    pub fn test(&self, draw_bounds: ComplementRect, test_mask: BoundsFlags) -> BoundsTestResult {
        let mut result = BoundsTestResult::BLOCKED;
        if !test_mask.contains(BoundsFlags::COLOR) || !self.color_bounds.intersects(draw_bounds) {
            result |= BoundsTestResult::ALLOWED_BEFORE_LAYER;
            if !test_mask.contains(BoundsFlags::STENCIL)
                || !self.stencil_bounds.intersects(draw_bounds)
            {
                result |= BoundsTestResult::ALLOWED_IN_LAYER;
            }
        }

        result
    }

    /// `addNewBinding(alloc, insertBefore, key, step)`.
    // Port of: src/gpu/graphite/DrawListTypes.h#L296-L333 (chrome/m156)
    #[allow(clippy::missing_panics_doc)] // the panics are SkASSERT-style invariants of the C++
    pub fn add_new_binding(
        &mut self,
        arena: &mut BindingArena,
        insert_before: Option<BindingId>,
        key: LayerKey,
        step: Arc<dyn RenderStep>,
    ) -> BindingId {
        debug_assert!(insert_before.is_none_or(|b| self.bindings.is_in_list(arena, b)));

        let list = u32::try_from(arena.bindings.len()).expect("binding count fits");
        arena.bindings.push(BindingList::new(step, key));

        if self.bindings.is_empty() {
            debug_assert!(insert_before.is_none());
            self.bindings.add_to_head(arena, list);
            return list;
        }

        // We need to insert the new list in the right place to keep fBindings organized with all
        // non-shading layers before shading layers, while also ensuring that the new `list`
        // comes before `insertBefore` (when non-null).
        if let Some(before) = insert_before
            && key.performs_shading() == arena.bindings[before as usize].key.performs_shading()
        {
            // Since both keys' shading state matches, putting the new list right in front of
            // `insertBefore` will not split the two sections (regardless of whether it was in
            // the shading or non-shading section).
            self.bindings.add_before(arena, list, before);
        } else if key.performs_shading() {
            // Since a new shading binding can only be inserted before other shading bindings,
            // the only way to get to this branch is to not have an insertBefore target. As such,
            // the simplest way to maintain keeping shading bindings in the latter half is to add
            // to the tail.
            debug_assert!(insert_before.is_none());
            self.bindings.add_to_tail(arena, list);
        } else {
            // A non-shading draw can have an `insertBefore` target that is a shading binding
            // (e.g. where the final shading step was inserted in the layer). In that case,
            // addBefore() would possibly split the shading bindings section of `fBindings`.
            // Adding it to the head of the bindings' list preserves the guarantee that all
            // non-shading bindings are at the start and satisfies adding it before the
            // `insertBefore` (if it were non-null).
            debug_assert!(!key.performs_shading());
            debug_assert!(
                insert_before.is_none_or(|b| arena.bindings[b as usize].key.performs_shading())
            );
            self.bindings.add_to_head(arena, list);
        }

        list
    }

    /// `transfer(binding, newLayer)`.
    // Port of: src/gpu/graphite/DrawListTypes.h#L335-L355 (chrome/m156)
    pub fn transfer(
        &mut self,
        arena: &mut BindingArena,
        binding: BindingId,
        new_layer: &mut Layer,
        draw_bounds_of: &dyn Fn(DrawParamsId) -> Rect,
    ) {
        debug_assert!(self.bindings.is_in_list(arena, binding));
        self.bindings.remove(arena, binding);
        new_layer.bindings.add_to_head(arena, binding);

        // The new layer needs to initialize its bounds array to match what's now in it.
        // `transfer` is only called for forward-merges so there's no need to update the stencil
        // bounds, since only simple-shading draws can be moved. There is also little need to try
        // and remove the binding's bounds from this layer's fColorBounds. Any new draw that
        // would have overlapped with `binding`'s bounds will get caught by `newLayer` instead.
        // Draws that make it past `newLayer` might get caught by a slot that was grown to be the
        // union of a `binding` draw and a different draw, but that would require fully
        // re-iterating all of this layer's draw's bounds and in practice this risk does not seem
        // to hurt batching.
        debug_assert!(arena.bindings[binding as usize].key.is_simple_shading());
        let mut draw = arena.bindings[binding as usize].head;
        while let Some(id) = draw {
            let d = arena.draws[id as usize];
            new_layer.color_bounds.add(draw_bounds_of(d.draw_params));
            draw = d.next;
        }
    }

    /// `updateForDraw(bounds, flags)`.
    // Port of: src/gpu/graphite/DrawListTypes.h#L357-L364 (chrome/m156)
    pub fn update_for_draw(&mut self, bounds: Rect, flags: BoundsFlags) {
        if flags.contains(BoundsFlags::COLOR) {
            self.color_bounds.add(bounds);
        }
        if flags.contains(BoundsFlags::STENCIL) {
            self.stencil_bounds.add(bounds);
        }
    }
}
