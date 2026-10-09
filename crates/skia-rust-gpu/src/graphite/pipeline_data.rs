// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PipelineData.h (uniform parts; see the module docs)

//! Uniform data blocks, the de-duplicating uniform cache and the gatherer that collects a draw's
//! uniforms (`PipelineData.h`).
//!
//! Not ported yet: `TextureDataBlock`, `TextureDataCache` and the texture half of
//! `PipelineDataGatherer`. They hold `TextureProxy`s and `SamplerDesc`s, which come with the
//! Graphite texture layer. The uniform cache's entries also lack Skia's `BindBufferInfo`
//! (`fBufferBinding`), which comes with the buffer manager.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::graphite::resource_types::Layout;
#[cfg(debug_assertions)]
use crate::graphite::uniform::Uniform;
use crate::graphite::uniform_manager::UniformManager;

/// Index into a [`DenseBiMap`] (`DenseBiMap::Index`).
// Port of: src/gpu/graphite/PipelineData.h#L166-L168 (chrome/m156)
pub type Index = u32;

/// An index that never refers to an entry (`DenseBiMap::kInvalidIndex`).
// Port of: src/gpu/graphite/PipelineData.h#L172-L173 (chrome/m156)
#[doc(alias = "kInvalidIndex")]
pub const K_INVALID_INDEX: Index = 4096;

/// Wraps the bytes of uniform data, which are aligned to match some uniform interface declaration
/// that consumes them once they are copied to the GPU (`UniformDataBlock`).
///
/// Skia's block is a view of the storage of a [`UniformManager`] or of an arena. Here the block
/// shares its bytes through an `Arc`, so it can outlive the manager's next reset.
// Port of: src/gpu/graphite/PipelineData.h#L42-L94 (chrome/m156)
#[doc(alias = "skgpu::graphite::UniformDataBlock")]
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct UniformDataBlock {
    data: Arc<[u8]>,
}

impl Default for UniformDataBlock {
    // Port of: src/gpu/graphite/PipelineData.h#L46 (chrome/m156), `constexpr UniformDataBlock()`
    fn default() -> Self {
        Self {
            data: Arc::from(Vec::new()),
        }
    }
}

impl UniformDataBlock {
    /// Wraps the finished accumulated uniform data of `uniforms` (`UniformDataBlock::Wrap`).
    // Port of: src/gpu/graphite/PipelineData.h#L57-L59 (chrome/m156)
    #[doc(alias = "Wrap")]
    pub fn wrap(uniforms: &mut UniformManager) -> Self {
        Self {
            data: Arc::from(uniforms.finish()),
        }
    }

    /// Wraps the non-shading uniform data of `uniforms` (`UniformDataBlock::WrapNonShading`).
    // Port of: src/gpu/graphite/PipelineData.h#L61-L63 (chrome/m156)
    #[doc(alias = "WrapNonShading")]
    pub fn wrap_non_shading(uniforms: &mut UniformManager) -> Self {
        Self {
            data: Arc::from(uniforms.finish_marked()),
        }
    }

    /// Whether the block holds no bytes.
    // Port of: src/gpu/graphite/PipelineData.h#L69-L70 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    /// The bytes of the block.
    // Port of: src/gpu/graphite/PipelineData.h#L72 (chrome/m156), `data()`
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data
    }

    /// The number of bytes in the block.
    // Port of: src/gpu/graphite/PipelineData.h#L73 (chrome/m156), `size()`
    #[must_use]
    pub fn size(&self) -> usize {
        self.data.len()
    }
}

/// Maps each distinct key to a dense index, and each index to its value (`DenseBiMap`).
///
/// Skia's `persist` storage hook is not needed: keys are owned, so inserting a key stores it.
// Port of: src/gpu/graphite/PipelineData.h#L166-L231 (chrome/m156)
#[derive(Debug)]
pub struct DenseBiMap<K: Eq + Hash, V> {
    data_to_index: HashMap<K, Index>,
    index_to_data: Vec<V>,
}

impl<K: Eq + Hash, V> Default for DenseBiMap<K, V> {
    fn default() -> Self {
        Self {
            data_to_index: HashMap::new(),
            index_to_data: Vec::new(),
        }
    }
}

impl<K: Eq + Hash + Clone, V: From<K>> DenseBiMap<K, V> {
    /// Returns the index of `data`, inserting it (and constructing its value) on first sight.
    ///
    /// # Panics
    ///
    /// Panics if the map would hold more than `u32::MAX` entries.
    // Port of: src/gpu/graphite/PipelineData.h#L172-L185 (chrome/m156), `insert`
    pub fn insert(&mut self, data: K) -> Index {
        if let Some(&index) = self.data_to_index.get(&data) {
            return index;
        }
        // First time we've seen this piece of data.
        let index = Index::try_from(self.index_to_data.len()).expect("DenseBiMap index fits");
        debug_assert!(index < K_INVALID_INDEX);
        self.data_to_index.insert(data.clone(), index);
        self.index_to_data.push(V::from(data));
        index
    }

    /// Whether `data` has been inserted (`contains`).
    // Port of: src/gpu/graphite/PipelineData.h#L187 (chrome/m156)
    #[must_use]
    pub fn contains(&self, data: &K) -> bool {
        self.data_to_index.contains_key(data)
    }
}

impl<K: Eq + Hash, V> DenseBiMap<K, V> {
    /// The value at `index` (`lookup`).
    // Port of: src/gpu/graphite/PipelineData.h#L189 (chrome/m156)
    #[must_use]
    pub fn lookup(&self, index: Index) -> &V {
        &self.index_to_data[index as usize]
    }

    /// The value at `index`, mutably (`lookup`).
    // Port of: src/gpu/graphite/PipelineData.h#L191 (chrome/m156)
    pub fn lookup_mut(&mut self, index: Index) -> &mut V {
        &mut self.index_to_data[index as usize]
    }

    /// All values, in index order (`get`).
    // Port of: src/gpu/graphite/PipelineData.h#L195 (chrome/m156)
    #[must_use]
    pub fn get(&self) -> &[V] {
        &self.index_to_data
    }

    /// Takes the values out, leaving the map empty (`detach`).
    // Port of: src/gpu/graphite/PipelineData.h#L193 (chrome/m156)
    pub fn detach(&mut self) -> Vec<V> {
        std::mem::take(&mut self.index_to_data)
    }

    /// Removes every entry (`reset`).
    // Port of: src/gpu/graphite/PipelineData.h#L202-L207 (chrome/m156)
    pub fn reset(&mut self) {
        self.index_to_data.clear();
        self.data_to_index.clear();
    }

    /// The number of distinct entries (`count`).
    // Port of: src/gpu/graphite/PipelineData.h#L212 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.index_to_data.len()
    }
}

/// The uniform data of one draw, and where it lives once uploaded (`UniformDataCache::Entry`).
///
/// Skia's entry also has `fBufferBinding`, the buffer and offset the data was uploaded to. It
/// comes with the buffer manager and is not here yet.
// Port of: src/gpu/graphite/PipelineData.h#L241-L250 (chrome/m156)
#[doc(alias = "UniformDataCache::Entry")]
#[derive(Clone, Debug)]
pub struct UniformDataCacheEntry {
    cpu_data: UniformDataBlock,
}

impl UniformDataCacheEntry {
    /// The uniform data on the CPU (`fCpuData`).
    // Port of: src/gpu/graphite/PipelineData.h#L243 (chrome/m156)
    #[must_use]
    pub fn cpu_data(&self) -> &UniformDataBlock {
        &self.cpu_data
    }
}

impl From<UniformDataBlock> for UniformDataCacheEntry {
    // Port of: src/gpu/graphite/PipelineData.h#L248 (chrome/m156), `Entry(UniformDataBlock)`
    fn from(cpu_data: UniformDataBlock) -> Self {
        Self { cpu_data }
    }
}

/// De-duplicates the uniform data blocks uploaded to uniform or storage buffers for a draw pass
/// pipeline (`UniformDataCache`).
// Port of: src/gpu/graphite/PipelineData.h#L237-L277 (chrome/m156)
#[doc(alias = "skgpu::graphite::UniformDataCache")]
#[derive(Debug, Default)]
pub struct UniformDataCache {
    uniforms: DenseBiMap<UniformDataBlock, UniformDataCacheEntry>,
}

impl UniformDataCache {
    /// Creates an empty cache.
    // Port of: src/gpu/graphite/PipelineData.h#L262 (chrome/m156)
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes every entry.
    // Port of: src/gpu/graphite/PipelineData.h#L263 (chrome/m156)
    pub fn reset(&mut self) {
        self.uniforms.reset();
    }

    /// The index of `data_block`, inserting it on first sight (`insert`).
    // Port of: src/gpu/graphite/PipelineData.h#L265 (chrome/m156)
    pub fn insert(&mut self, data_block: UniformDataBlock) -> Index {
        self.uniforms.insert(data_block)
    }

    /// The entry at `index` (`lookup`).
    // Port of: src/gpu/graphite/PipelineData.h#L267 (chrome/m156)
    #[must_use]
    pub fn lookup(&self, index: Index) -> &UniformDataCacheEntry {
        self.uniforms.lookup(index)
    }

    /// The entry at `index`, mutably (`lookup`).
    // Port of: src/gpu/graphite/PipelineData.h#L269 (chrome/m156)
    pub fn lookup_mut(&mut self, index: Index) -> &mut UniformDataCacheEntry {
        self.uniforms.lookup_mut(index)
    }
}

/// Collects the uniforms of a draw: the paint uniforms, then those of the render step, with the
/// rewinding a render step needs to share the paint uniforms (`PipelineDataGatherer`).
///
/// The texture side of Skia's gatherer is not ported yet (see the module docs).
// Port of: src/gpu/graphite/PipelineData.h#L352-L395 (chrome/m156)
#[derive(Debug)]
pub struct PipelineDataGatherer {
    uniform_manager: UniformManager,
}

impl PipelineDataGatherer {
    /// Creates a gatherer that writes uniforms with `layout`.
    // Port of: src/gpu/graphite/PipelineData.h#L355 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        Self {
            uniform_manager: UniformManager::new(layout),
        }
    }

    /// Fully resets the uniforms (paint and render step) to the start of a draw.
    // Port of: src/gpu/graphite/PipelineData.h#L358-L362 (chrome/m156), `resetForDraw`
    pub fn reset_for_draw(&mut self) {
        self.uniform_manager.reset();
    }

    /// Marks the end of the paint uniforms. A non-shading render step aligns its uniforms to
    /// `required_alignment`; a shading one continues the paint uniforms unaligned.
    // Port of: src/gpu/graphite/PipelineData.h#L382-L389 (chrome/m156), `markOffsetAndAlign`
    #[doc(alias = "markOffsetAndAlign")]
    pub fn mark_offset_and_align(&mut self, performs_shading: bool, required_alignment: i32) {
        self.uniform_manager.mark_offset();
        if !performs_shading {
            self.uniform_manager
                .align_for_non_shading(required_alignment);
        }
    }

    /// Rewinds to collect the data of another render step with the same paint data
    /// (`rewindForRenderStep`).
    // Port of: src/gpu/graphite/PipelineData.h#L392 (chrome/m156)
    pub fn rewind_for_render_step(&mut self) {
        self.uniform_manager.rewind_to_mark();
    }

    /// Returns the uniform data written since the last reset: the paint and render step data
    /// when `performs_shading`, else only the render step data (`endCombinedData`'s uniform half).
    // Port of: src/gpu/graphite/PipelineData.h#L397-L408 (chrome/m156), `endCombinedData`
    #[doc(alias = "endCombinedData")]
    pub fn end_combined_uniforms(&mut self, performs_shading: bool) -> UniformDataBlock {
        if performs_shading {
            UniformDataBlock::wrap(&mut self.uniform_manager)
        } else {
            UniformDataBlock::wrap_non_shading(&mut self.uniform_manager)
        }
    }

    /// The uniform manager that the draw's uniforms are written through.
    // Port of: src/gpu/graphite/PipelineData.h#L434 (chrome/m156), `uniformManager()`
    #[must_use]
    pub fn uniform_manager(&mut self) -> &mut UniformManager {
        &mut self.uniform_manager
    }

    /// Gives back capacity the uniforms no longer need.
    // Port of: src/gpu/graphite/PipelineData.h#L412-L415 (chrome/m156), `tryShrinkCapacity`
    pub fn try_shrink_capacity(&mut self) {
        self.uniform_manager.try_shrink_capacity();
    }
}

/// Declares the uniforms that the writes of its scope must match, and ends the declaration when
/// dropped (`UniformExpectationsValidator`). Debug builds only.
// Port of: src/gpu/graphite/PipelineData.h#L459-L480 (chrome/m156)
#[cfg(debug_assertions)]
#[derive(Debug)]
pub struct UniformExpectationsValidator<'a> {
    manager: &'a mut UniformManager,
}

#[cfg(debug_assertions)]
impl<'a> UniformExpectationsValidator<'a> {
    /// Starts declaring `expected_uniforms` on `gatherer`'s uniform manager.
    // Port of: src/gpu/graphite/PipelineData.h#L462-L467 (chrome/m156)
    pub fn new(
        gatherer: &'a mut PipelineDataGatherer,
        expected_uniforms: &[Uniform],
        is_substruct: bool,
    ) -> Self {
        let manager = gatherer.uniform_manager();
        manager.set_expected_uniforms(expected_uniforms, is_substruct);
        Self { manager }
    }
}

#[cfg(debug_assertions)]
impl Drop for UniformExpectationsValidator<'_> {
    // Port of: src/gpu/graphite/PipelineData.h#L469-L471 (chrome/m156), the destructor
    fn drop(&mut self) {
        self.manager.done_with_expected_uniforms();
    }
}
