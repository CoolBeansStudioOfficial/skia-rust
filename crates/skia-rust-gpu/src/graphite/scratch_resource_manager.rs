// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ScratchResourceManager.h,
//                   src/gpu/graphite/ScratchResourceManager.cpp

//! `ScratchResourceManager`: reuse of scratch resources *within* a recording.
//!
//! skia-rust: Skia's manager keeps a `ResourceProvider*` for its lifetime. To keep lifetimes off
//! public types, the provider is passed to [`ScratchResourceManager::get_scratch_texture`]
//! instead (the manager only lives for one `prepareResources()` pass, during which the recorder
//! owns the provider).

use std::collections::HashMap;
use std::sync::Arc;

use skia_rust_core::size::ISize;

use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_cache::ScratchResourceSet;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::texture::Texture;
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::texture_proxy::TextureProxy;

// `const TextureProxy*` as a map key: the proxy's address.
fn proxy_key(proxy: &TextureProxy) -> usize {
    std::ptr::from_ref(proxy) as usize
}

/// `ProxyReadCountMap`: pending read counts per proxy.
///
/// NOTE: This is temporary in Skia while atlas management requires flushing an entire Recorder.
// Port of: src/gpu/graphite/ScratchResourceManager.h#L30-L63 (chrome/m156)
#[derive(Debug, Default)]
pub struct ProxyReadCountMap {
    counts: HashMap<usize, i32>,
}

impl ProxyReadCountMap {
    /// An empty map.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `increment()`.
    // Port of: src/gpu/graphite/ScratchResourceManager.h#L34-L40 (chrome/m156)
    pub fn increment(&mut self, proxy: &TextureProxy) {
        *self.counts.entry(proxy_key(proxy)).or_insert(0) += 1;
    }

    /// `decrement()`: returns true if the count reached zero; it must have been positive.
    ///
    /// # Panics
    /// If `proxy` has no pending reads.
    // Port of: src/gpu/graphite/ScratchResourceManager.h#L42-L47 (chrome/m156)
    pub fn decrement(&mut self, proxy: &TextureProxy) -> bool {
        let count = self
            .counts
            .get_mut(&proxy_key(proxy))
            .expect("decrementing a proxy without pending reads");
        debug_assert!(*count > 0);
        *count -= 1;
        *count == 0
    }

    /// `get()`.
    // Port of: src/gpu/graphite/ScratchResourceManager.h#L49-L52 (chrome/m156)
    #[must_use]
    pub fn get(&self, proxy: &TextureProxy) -> i32 {
        self.counts.get(&proxy_key(proxy)).copied().unwrap_or(0)
    }

    /// `hasPendingReads()`.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L17-L25 (chrome/m156)
    #[doc(alias = "hasPendingReads")]
    #[must_use]
    pub fn has_pending_reads(&self) -> bool {
        self.counts.values().any(|&c| c > 0)
    }
}

/// A listener invoked when the resources a task prepared are consumed (`PendingUseListener`).
// Port of: src/gpu/graphite/ScratchResourceManager.h#L128-L132 (chrome/m156)
pub trait PendingUseListener: Send + Sync {
    /// `onUseCompleted()`.
    #[doc(alias = "onUseCompleted")]
    fn on_use_completed(&self, manager: &mut ScratchResourceManager);
}

/// Coordinates the reuse of scratch resources within a recording, which the resource cache would
/// not otherwise return because the recorder holds usage refs on them.
// Port of: src/gpu/graphite/ScratchResourceManager.h#L65-L194 (chrome/m156)
#[doc(alias = "skgpu::graphite::ScratchResourceManager")]
pub struct ScratchResourceManager {
    // The un-returned scratch resources fetched from the provider, which must be considered
    // unavailable when making additional requests from the cache with compatible keys.
    unavailable: ScratchResourceSet,
    // A stack of sublists of listeners; `None` marks the start of a scope.
    listener_stack: Vec<Option<Arc<dyn PendingUseListener>>>,
    proxy_read_counts: ProxyReadCountMap,
}

impl std::fmt::Debug for ScratchResourceManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ScratchResourceManager")
            .field("unavailable", &self.unavailable)
            .field("listener_stack_len", &self.listener_stack.len())
            .field("proxy_read_counts", &self.proxy_read_counts)
            .finish()
    }
}

impl ScratchResourceManager {
    /// `ScratchResourceManager(resourceProvider, proxyCounts)`.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L27-L33 (chrome/m156)
    #[must_use]
    pub fn new(proxy_read_counts: ProxyReadCountMap) -> Self {
        Self {
            unavailable: ScratchResourceSet::new(),
            listener_stack: Vec::new(),
            proxy_read_counts,
        }
    }

    /// `getScratchTexture()`: a scratch texture that will not be handed out again by this manager
    /// until it is [returned](Self::return_texture). If no compatible texture is available, the
    /// provider finds or creates one.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L40-L51 (chrome/m156)
    #[doc(alias = "getScratchTexture")]
    pub fn get_scratch_texture(
        &mut self,
        resource_provider: &mut ResourceProvider,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        let scratch_texture = resource_provider.find_or_create_scratch_texture(
            dimensions,
            info,
            label,
            &self.unavailable,
        );
        // Store the returned scratch texture into `unavailable` so that it is filtered from the
        // ResourceCache when going through *this* ScratchResourceManager. But the scratch texture
        // will remain visible to other Recorders.
        if let Some(texture) = &scratch_texture {
            let inserted = self.unavailable.insert(texture.base().unique_id());
            debug_assert!(inserted);
        }
        scratch_texture
    }

    /// `returnTexture()`: marks the texture as available for reuse. It must have come from
    /// [`get_scratch_texture`](Self::get_scratch_texture).
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L53-L57 (chrome/m156)
    #[doc(alias = "returnTexture")]
    pub fn return_texture(&mut self, texture: &ResourceRef<Texture>) {
        // Fails if trying to return a resource that didn't come from getScratchTexture()
        let removed = self.unavailable.remove(&texture.base().unique_id());
        debug_assert!(removed);
    }

    /// `pushScope()`.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L59-L62 (chrome/m156)
    #[doc(alias = "pushScope")]
    pub fn push_scope(&mut self) {
        // Push a null to mark the beginning of the list of listeners in the next depth
        self.listener_stack.push(None);
    }

    /// `popScope()`: pops the current scope without invoking its pending listeners.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L64-L78 (chrome/m156)
    #[doc(alias = "popScope")]
    pub fn pop_scope(&mut self) {
        // Must have at least the null element to start the scope being popped
        debug_assert!(!self.listener_stack.is_empty());

        // TODO: Assert that the current sublist is empty but for now skip over them and leave
        // them un-invoked to keep the unconsumed scratch resources out of the pool.
        let n = self
            .listener_stack
            .iter()
            .rev()
            .take_while(|l| l.is_some())
            .count();
        debug_assert!(n < self.listener_stack.len());
        // Remove all non-null listeners after the most recent null entry AND the null entry
        let new_len = self.listener_stack.len() - (n + 1);
        self.listener_stack.truncate(new_len);
    }

    /// `notifyResourcesConsumed()`: invokes and removes every pending listener of the current
    /// scope.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L80-L95 (chrome/m156)
    #[doc(alias = "notifyResourcesConsumed")]
    pub fn notify_resources_consumed(&mut self) {
        // Should only be called inside a scope
        debug_assert!(!self.listener_stack.is_empty());

        let listeners: Vec<Arc<dyn PendingUseListener>> = self
            .listener_stack
            .iter()
            .rev()
            .map_while(Clone::clone)
            .collect();
        for listener in &listeners {
            listener.on_use_completed(self);
        }
        let n = listeners.len();
        debug_assert!(n < self.listener_stack.len());
        // Remove all non-null listeners that were just invoked, but do not remove the null entry
        // that marks the start of this scope boundary.
        if n > 0 {
            let new_len = self.listener_stack.len() - n;
            self.listener_stack.truncate(new_len);
        }
    }

    /// `markResourceInUse()`: registers a listener invoked on the next
    /// [`notify_resources_consumed`](Self::notify_resources_consumed) in the current scope.
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L97-L101 (chrome/m156)
    #[doc(alias = "markResourceInUse")]
    pub fn mark_resource_in_use(&mut self, listener: Arc<dyn PendingUseListener>) {
        // Should only be called inside a scope
        debug_assert!(!self.listener_stack.is_empty());
        self.listener_stack.push(Some(listener));
    }

    /// `pendingReadCount()`.
    #[doc(alias = "pendingReadCount")]
    #[must_use]
    pub fn pending_read_count(&self, proxy: &TextureProxy) -> i32 {
        self.proxy_read_counts.get(proxy)
    }

    /// `removePendingRead()`: true if the read count reached zero.
    #[doc(alias = "removePendingRead")]
    pub fn remove_pending_read(&mut self, proxy: &TextureProxy) -> bool {
        self.proxy_read_counts.decrement(proxy)
    }
}

impl Drop for ScratchResourceManager {
    // Port of: src/gpu/graphite/ScratchResourceManager.cpp#L35-L38 (chrome/m156)
    fn drop(&mut self) {
        if !std::thread::panicking() {
            debug_assert!(self.unavailable.is_empty());
            debug_assert!(!self.proxy_read_counts.has_pending_reads());
        }
    }
}
