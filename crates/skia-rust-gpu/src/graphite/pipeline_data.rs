// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PipelineData.h (uniform parts; see the module docs)

//! Uniform data blocks, the de-duplicating uniform cache and the gatherer that collects a draw's
//! uniforms (`PipelineData.h`).
//!
//! The texture half is here too: `TextureDataBlock` (the sampled textures of a draw),
//! `TextureDataCache` (de-duplicated bindings and the unique proxies they reference) and the
//! gatherer's `add` / `endCombinedData` / `rewindForRenderStep`. Textures are `(proxy, sampler)`
//! pairs; a proxy is `None` only on the pre-compile path.

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::graphite::buffer::BindBufferInfo;
use crate::graphite::resource_types::{Layout, SamplerDesc};
use crate::graphite::texture_proxy::TextureProxy;
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
    /// The index of `data`, if it has been inserted (the `find` half of `insert`).
    // Port of: src/gpu/graphite/PipelineData.h#L173-L175 (chrome/m156), `fDataToIndex.find`
    #[must_use]
    pub fn index_of(&self, data: &K) -> Option<Index> {
        self.data_to_index.get(data).copied()
    }

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
// Port of: src/gpu/graphite/PipelineData.h#L241-L250 (chrome/m156)
#[doc(alias = "UniformDataCache::Entry")]
#[derive(Clone, Debug)]
pub struct UniformDataCacheEntry {
    cpu_data: UniformDataBlock,
    // `fBufferBinding`: the buffer and offset the data was uploaded to; empty until the draw
    // pass writes the data.
    buffer_binding: BindBufferInfo,
}

impl UniformDataCacheEntry {
    /// The uniform data on the CPU (`fCpuData`).
    // Port of: src/gpu/graphite/PipelineData.h#L243 (chrome/m156)
    #[must_use]
    pub fn cpu_data(&self) -> &UniformDataBlock {
        &self.cpu_data
    }

    /// Where the data was uploaded to (`fBufferBinding`).
    // Port of: src/gpu/graphite/PipelineData.h#L244 (chrome/m156)
    #[must_use]
    pub fn buffer_binding(&self) -> &BindBufferInfo {
        &self.buffer_binding
    }

    /// Records where the data was uploaded to (`fBufferBinding = ...`).
    // Port of: src/gpu/graphite/PipelineData.h#L244 (chrome/m156)
    pub fn set_buffer_binding(&mut self, binding: BindBufferInfo) {
        self.buffer_binding = binding;
    }
}

impl From<UniformDataBlock> for UniformDataCacheEntry {
    // Port of: src/gpu/graphite/PipelineData.h#L248 (chrome/m156), `Entry(UniformDataBlock)`
    fn from(cpu_data: UniformDataBlock) -> Self {
        Self {
            cpu_data,
            buffer_binding: BindBufferInfo::default(),
        }
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

    /// The number of distinct entries (`count`).
    // Port of: src/gpu/graphite/PipelineData.h#L276 (chrome/m156)
    #[must_use]
    pub fn count(&self) -> usize {
        self.uniforms.count()
    }
}

/// One sampled texture of a draw: the proxy (`None` only on the pre-compile path) and the sampler
/// it is read with (`TextureDataBlock::SampledTexture`).
// Port of: src/gpu/graphite/PipelineData.h#L98 (chrome/m156)
pub type SampledTexture = (Option<Arc<TextureProxy>>, SamplerDesc);

/// Compares two optional proxies by identity, as `sk_sp` equality does.
fn same_proxy(a: Option<&Arc<TextureProxy>>, b: Option<&Arc<TextureProxy>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

/// The address of a proxy, or 0 for none (`reinterpret_cast<uintptr_t>(proxy.get())`).
fn proxy_address(proxy: Option<&Arc<TextureProxy>>) -> usize {
    proxy.map_or(0, |p| Arc::as_ptr(p) as usize)
}

/// The sampled textures a draw binds, as a shared immutable list (`TextureDataBlock`).
///
/// Skia's block is a span into the gatherer's or an arena's storage. Here it shares its entries
/// through an `Arc`, so the proxies stay alive for as long as the block does.
// Port of: src/gpu/graphite/PipelineData.h#L96-L164 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextureDataBlock")]
#[derive(Clone, Debug, Default)]
pub struct TextureDataBlock {
    textures: Arc<[SampledTexture]>,
}

impl TextureDataBlock {
    /// A block holding one texture (`TextureDataBlock(const SampledTexture&)`).
    // Port of: src/gpu/graphite/PipelineData.h#L110 (chrome/m156)
    #[must_use]
    pub fn from_texture(texture: SampledTexture) -> Self {
        Self {
            textures: Arc::from(vec![texture]),
        }
    }

    /// The block made from `textures`, in order (`TextureDataBlock::Make`).
    // Port of: src/gpu/graphite/PipelineData.h#L103-L106 (chrome/m156)
    #[must_use]
    pub fn make(textures: &[SampledTexture]) -> Self {
        Self {
            textures: Arc::from(textures),
        }
    }

    /// Whether the block holds no textures (`empty`).
    // Port of: src/gpu/graphite/PipelineData.h#L115 (chrome/m156)
    #[must_use]
    pub fn empty(&self) -> bool {
        self.textures.is_empty()
    }

    /// The number of textures (`numTextures`).
    // Port of: src/gpu/graphite/PipelineData.h#L117 (chrome/m156)
    #[must_use]
    pub fn num_textures(&self) -> usize {
        self.textures.len()
    }

    /// The texture at `index` (`texture`).
    // Port of: src/gpu/graphite/PipelineData.h#L118 (chrome/m156)
    #[must_use]
    pub fn texture(&self, index: usize) -> &SampledTexture {
        &self.textures[index]
    }

    /// All textures, in binding order.
    #[must_use]
    pub fn textures(&self) -> &[SampledTexture] {
        &self.textures
    }
}

impl PartialEq for TextureDataBlock {
    // Port of: src/gpu/graphite/PipelineData.h#L120-L135 (chrome/m156), `operator==`
    fn eq(&self, other: &Self) -> bool {
        if self.textures.len() != other.textures.len() {
            return false;
        }
        if std::ptr::eq(self.textures.as_ptr(), other.textures.as_ptr()) {
            return true; // shortcut for the same span
        }
        self.textures.iter().zip(other.textures.iter()).all(
            |((tex, sampler), (other_tex, other_sampler))| {
                same_proxy(tex.as_ref(), other_tex.as_ref()) && sampler == other_sampler
            },
        )
    }
}

impl Eq for TextureDataBlock {}

impl Hash for TextureDataBlock {
    // Port of: src/gpu/graphite/PipelineData.h#L138-L155 (chrome/m156), `TextureDataBlock::Hash`
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // The proxies are hashed by address: a TextureDataCache lives for one recording, and its
        // blocks hold refs on their proxies.
        for (proxy, sampler) in self.textures.iter() {
            sampler.hash(state);
            proxy_address(proxy.as_ref()).hash(state);
        }
    }
}

/// A proxy keyed by its identity (`TextureProxyCache`'s key: `TextureProxy*` compared by address).
#[derive(Clone, Debug)]
struct ProxyIdentity(Arc<TextureProxy>);

impl PartialEq for ProxyIdentity {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for ProxyIdentity {}

impl Hash for ProxyIdentity {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.0).hash(state);
    }
}

impl From<ProxyIdentity> for Arc<TextureProxy> {
    fn from(identity: ProxyIdentity) -> Self {
        identity.0
    }
}

/// De-duplicates sets of texture bindings and collects the list of unique texture proxies that
/// are referenced by all inserted bindings (`TextureDataCache`).
// Port of: src/gpu/graphite/PipelineData.h#L281-L337 (chrome/m156)
#[doc(alias = "skgpu::graphite::TextureDataCache")]
#[derive(Debug, Default)]
pub struct TextureDataCache {
    textures: DenseBiMap<TextureDataBlock, TextureDataBlock>,
    unique_textures: DenseBiMap<ProxyIdentity, Arc<TextureProxy>>,
}

impl TextureDataCache {
    /// Creates an empty cache.
    // Port of: src/gpu/graphite/PipelineData.h#L316 (chrome/m156), the default constructor
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Removes every binding and unique texture.
    // Port of: src/gpu/graphite/PipelineData.h#L318 (chrome/m156)
    pub fn reset(&mut self) {
        self.textures.reset();
        self.unique_textures.reset();
    }

    /// The index of `data_block`, inserting it on first sight. The first sight also records each
    /// of its textures as unique to hand off to the `DrawPass` (`TextureCopier::persist`).
    // Port of: src/gpu/graphite/PipelineData.h#L320 (chrome/m156), `insert`
    pub fn insert(&mut self, data_block: TextureDataBlock) -> Index {
        if let Some(index) = self.textures.index_of(&data_block) {
            return index;
        }
        // Insert every referenced texture into the unique set to hand off to DrawPass.
        for (proxy, _) in data_block.textures() {
            if let Some(proxy) = proxy {
                self.unique_textures.insert(ProxyIdentity(proxy.clone()));
            }
        }
        self.textures.insert(data_block)
    }

    /// The binding at `index` (`lookup`).
    // Port of: src/gpu/graphite/PipelineData.h#L322 (chrome/m156)
    #[must_use]
    pub fn lookup(&self, index: Index) -> TextureDataBlock {
        self.textures.lookup(index).clone()
    }

    /// Whether `texture` is one of the unique textures (`hasTexture`).
    // Port of: src/gpu/graphite/PipelineData.h#L324-L331 (chrome/m156)
    #[must_use]
    pub fn has_texture(&self, texture: &Arc<TextureProxy>) -> bool {
        self.unique_textures
            .contains(&ProxyIdentity(texture.clone()))
    }

    /// Takes the unique textures out, leaving the set empty (`detachTextures`).
    // Port of: src/gpu/graphite/PipelineData.h#L333-L335 (chrome/m156)
    #[must_use]
    pub fn detach_textures(&mut self) -> Vec<Arc<TextureProxy>> {
        self.unique_textures.detach()
    }

    /// All the bindings, in index order (`getBindings`).
    // Port of: src/gpu/graphite/PipelineData.h#L337-L339 (chrome/m156)
    #[must_use]
    pub fn get_bindings(&self) -> &[TextureDataBlock] {
        self.textures.get()
    }

    /// The number of distinct bindings (`bindingCount`).
    // Port of: src/gpu/graphite/PipelineData.h#L343 (chrome/m156)
    #[must_use]
    pub fn binding_count(&self) -> usize {
        self.textures.count()
    }

    /// The number of unique textures (`uniqueTextureCount`).
    // Port of: src/gpu/graphite/PipelineData.h#L344 (chrome/m156)
    #[must_use]
    pub fn unique_texture_count(&self) -> usize {
        self.unique_textures.count()
    }
}

/// Collects the uniforms and textures of a draw: the paint data, then those of the render step,
/// with the rewinding a render step needs to share the paint data (`PipelineDataGatherer`).
// Port of: src/gpu/graphite/PipelineData.h#L352-L395 (chrome/m156)
#[derive(Debug)]
pub struct PipelineDataGatherer {
    uniform_manager: UniformManager,
    textures: Vec<SampledTexture>,
    paint_texture_count: usize,
}

impl PipelineDataGatherer {
    /// Creates a gatherer that writes uniforms with `layout`.
    // Port of: src/gpu/graphite/PipelineData.h#L355 (chrome/m156)
    #[must_use]
    pub fn new(layout: Layout) -> Self {
        Self {
            uniform_manager: UniformManager::new(layout),
            textures: Vec::new(),
            paint_texture_count: 0,
        }
    }

    /// Fully resets the uniforms (paint and render step) and the textures to the start of a draw.
    // Port of: src/gpu/graphite/PipelineData.h#L358-L363 (chrome/m156), `resetForDraw`
    pub fn reset_for_draw(&mut self) {
        self.uniform_manager.reset();
        self.textures.clear();
        self.paint_texture_count = 0;
    }

    /// Checks that the gatherer is back in its initial state (`checkReset`). Debug builds only.
    // Port of: src/gpu/graphite/PipelineData.h#L368-L373 (chrome/m156)
    #[cfg(debug_assertions)]
    pub fn check_reset(&self) {
        debug_assert!(self.textures.is_empty());
        debug_assert!(self.uniform_manager.is_reset());
        debug_assert_eq!(self.paint_texture_count, 0);
    }

    /// Checks that the textures are back at the end of the paint data (`checkRewind`).
    // Port of: src/gpu/graphite/PipelineData.h#L375-L377 (chrome/m156)
    #[cfg(debug_assertions)]
    pub fn check_rewind(&self) {
        debug_assert_eq!(self.textures.len(), self.paint_texture_count);
    }

    /// Checks that `other` collected the same data for the same key (`checkEquivalent`).
    ///
    /// The textures are not compared by identity: picture shaders and non-Graphite images can
    /// make new proxies on a second `toKey()`, and as long as their properties and samplers
    /// match, they are equivalent. Debug builds only.
    // Port of: src/gpu/graphite/PipelineData.h#L379-L407 (chrome/m156)
    #[cfg(debug_assertions)]
    pub fn check_equivalent(&self, other: &PipelineDataGatherer) {
        // We don't call finish() here because we don't want to modify any of the required
        // alignment and offsets that UniformManager is tracking for being able to rewind and
        // be combined with RenderStep data.
        debug_assert_eq!(
            self.uniform_manager.storage(),
            other.uniform_manager.storage()
        );

        debug_assert_eq!(self.textures.len(), other.textures.len());
        for ((tex, sampler), (o_tex, o_sampler)) in self.textures.iter().zip(&other.textures) {
            match (tex, o_tex) {
                (Some(tex), Some(o_tex)) => {
                    debug_assert_eq!(tex.dimensions(), o_tex.dimensions());
                    debug_assert_eq!(tex.texture_info(), o_tex.texture_info());
                }
                (None, None) => {}
                _ => debug_assert!(false, "a texture is missing on one side"),
            }
            debug_assert_eq!(sampler, o_sampler);
        }
    }

    /// If a renderstep performs shading, then alignment should occur on the combined
    /// paint+renderstep, so no alignment is required and we simply mark the end of the paints. Else
    /// we need to align whatever is currently stored to the renderstep's uniform alignment.
    // Port of: src/gpu/graphite/PipelineData.h#L382-L389 (chrome/m156), `markOffsetAndAlign`
    #[doc(alias = "markOffsetAndAlign")]
    pub fn mark_offset_and_align(&mut self, performs_shading: bool, required_alignment: i32) {
        self.paint_texture_count = self.textures.len();
        self.uniform_manager.mark_offset();
        if !performs_shading {
            self.uniform_manager
                .align_for_non_shading(required_alignment);
        }
    }

    /// Rewinds to collect the data of another render step with the same paint data
    /// (`rewindForRenderStep`).
    // Port of: src/gpu/graphite/PipelineData.h#L392-L395 (chrome/m156)
    pub fn rewind_for_render_step(&mut self) {
        self.textures.truncate(self.paint_texture_count);
        self.uniform_manager.rewind_to_mark();
    }

    /// Marks the end of extracting the uniforms and textures of a render step: the paint and
    /// render step data when `performs_shading`, else only the render step data
    /// (`endCombinedData`).
    // Port of: src/gpu/graphite/PipelineData.h#L397-L408 (chrome/m156), `endCombinedData`
    #[doc(alias = "endCombinedData")]
    pub fn end_combined_data(
        &mut self,
        performs_shading: bool,
    ) -> (UniformDataBlock, TextureDataBlock) {
        if performs_shading {
            // Return paint AND renderstep uniforms written since the last resetForDraw.
            (
                UniformDataBlock::wrap(&mut self.uniform_manager),
                TextureDataBlock::make(&self.textures),
            )
        } else {
            // Return only the renderstep uniforms and textures.
            (
                UniformDataBlock::wrap_non_shading(&mut self.uniform_manager),
                TextureDataBlock::make(&self.textures[self.paint_texture_count..]),
            )
        }
    }

    /// Appends a sampled texture (`add`). A `None` proxy is only valid on the pre-compile path.
    // Port of: src/gpu/graphite/PipelineData.h#L411-L413 (chrome/m156)
    pub fn add(&mut self, proxy: Option<Arc<TextureProxy>>, sampler_desc: SamplerDesc) {
        self.textures.push((proxy, sampler_desc));
    }

    /// Gives back capacity the uniforms no longer need.
    // Port of: src/gpu/graphite/PipelineData.h#L415-L418 (chrome/m156), `tryShrinkCapacity`
    pub fn try_shrink_capacity(&mut self) {
        #[cfg(debug_assertions)]
        self.check_reset();
        self.uniform_manager.try_shrink_capacity();
    }

    /// The uniform manager that the draw's uniforms are written through.
    // Port of: src/gpu/graphite/PipelineData.h#L434 (chrome/m156), `uniformManager()`
    #[must_use]
    pub fn uniform_manager(&mut self) -> &mut UniformManager {
        &mut self.uniform_manager
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
