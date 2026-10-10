// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ProxyCache.h, src/gpu/graphite/ProxyCache.cpp

//! `ProxyCache`: the recorder-local cache of utility texture proxies, keyed by `UniqueKey`.
//!
//! The entries live in a port of `THashMap`, because the order in which `freeUniquelyHeld()` and
//! `purgeProxiesNotUsedSince()` drop proxies decides the order their textures return to the
//! resource cache (and so which ones it purges first).
//!
//! Invalidation: Skia's listener posts an `UniqueKeyInvalidatedMsg_Graphite` on the global
//! `SkMessageBus`, which delivers it to the inbox whose id is the message's recorder id. Each
//! recorder has one proxy cache, so the port's listener posts straight into this cache's inbox
//! (held weakly), with no process-wide state.
//!
//! `RecorderPriv::CreateCachedProxy` (bitmap tables such as the dither look-up table) is
//! [`crate::graphite::recorder::RecorderPriv::create_cached_proxy`], built on
//! [`ProxyCache::find_cache_entry`] and [`ProxyCache::insert_cache_entry`]. The image entry point
//! `findOrCreateCachedProxy(recorder, key, GPUGeneratorFn)` is
//! [`find_or_create_cached_proxy_from_image`].

use std::sync::{Arc, Mutex, Weak};

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::id_change_listener::IdChangeListener;
use skia_rust_core::rect::IRect;
use skia_rust_core::t_hash::THashMap;

use crate::gpu::gpu_types::StdSteadyClockTimePoint;
use crate::gpu::resource_key::{UniqueKey, UniqueKeyBuilder, UniqueKeyInvalidatedMsgGraphite};
use crate::graphite::texture_proxy::TextureProxy;

/// One cached proxy and the listener that invalidates it (`None` if the source won't change).
// Port of: src/gpu/graphite/ProxyCache.h#L94-L97 (chrome/m156)
#[derive(Clone, Debug)]
struct CacheEntry {
    proxy: Arc<TextureProxy>,
    listener: Option<Arc<IdChangeListener>>,
}

/// `SkMessageBus<UniqueKeyInvalidatedMsg_Graphite, uint32_t>::Inbox` for one recorder.
#[derive(Debug, Default)]
struct Inbox {
    messages: Mutex<Vec<UniqueKeyInvalidatedMsgGraphite>>,
}

impl Inbox {
    // Port of: src/core/SkMessageBus.h#L106-L111 (chrome/m156)
    fn receive(&self, message: UniqueKeyInvalidatedMsgGraphite) {
        self.messages
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(message);
    }

    // Port of: src/core/SkMessageBus.h#L113-L119 (chrome/m156)
    fn poll(&self) -> Vec<UniqueKeyInvalidatedMsgGraphite> {
        std::mem::take(
            &mut *self
                .messages
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner),
        )
    }
}

/// `make_bitmap_key`: the key of a bitmap's pixels (generation id and subset).
// Port of: src/gpu/graphite/ProxyCache.cpp#L28-L41 (chrome/m156)
#[allow(clippy::cast_sign_loss)] // builder[i] = int, as in C++
fn make_bitmap_key(bm: &Bitmap) -> UniqueKey {
    static PROXY_CACHE_DOMAIN: std::sync::LazyLock<u16> =
        std::sync::LazyLock::new(UniqueKey::generate_domain);

    let origin = bm.pixel_ref_origin();
    let dimensions = bm.dimensions();
    let subset = IRect::from_pt_size(origin, dimensions);

    let mut key = UniqueKey::new();
    {
        let mut builder =
            UniqueKeyBuilder::new(&mut key, *PROXY_CACHE_DOMAIN, 5, Some("ProxyCache"));
        builder[0] = bm.pixel_ref().map_or(0, |pr| pr.generation_id());
        builder[1] = subset.left as u32;
        builder[2] = subset.top as u32;
        builder[3] = subset.right as u32;
        builder[4] = subset.bottom as u32;
    }
    key
}

/// The recorder-local cache of utility proxies. It does not create mipmapped proxies.
#[doc(alias = "skgpu::graphite::ProxyCache")]
pub struct ProxyCache {
    cache: THashMap<UniqueKey, CacheEntry>,
    recorder_id: u32,
    invalid_unique_key_inbox: Arc<Inbox>,
}

impl std::fmt::Debug for ProxyCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProxyCache")
            .field("recorder_id", &self.recorder_id)
            .field("num_cached", &self.cache.count())
            .finish_non_exhaustive()
    }
}

impl ProxyCache {
    /// `ProxyCache(recorderID)`.
    // Port of: src/gpu/graphite/ProxyCache.cpp#L65-L67 (chrome/m156)
    #[must_use]
    pub fn new(recorder_id: u32) -> Self {
        debug_assert_ne!(recorder_id, 0);
        Self {
            cache: THashMap::new(|key: &UniqueKey| key.hash()),
            recorder_id,
            invalid_unique_key_inbox: Arc::new(Inbox::default()),
        }
    }

    /// `findOrCreateCacheEntry()`: the cached proxy for `key` (refreshing its texture's access
    /// time), or the proxy `create_entry` makes for the final label (the key's tag if `label` is
    /// empty), which is then cached with its invalidation listener.
    // Port of: src/gpu/graphite/ProxyCache.cpp#L75-L95 (chrome/m156)
    pub fn find_or_create_cache_entry(
        &mut self,
        key: &UniqueKey,
        label: &str,
        create_entry: impl FnOnce(&str) -> Option<(Arc<TextureProxy>, Option<Arc<IdChangeListener>>)>,
    ) -> Option<Arc<TextureProxy>> {
        if let Some(proxy) = self.find_cache_entry(key) {
            return Some(proxy);
        }

        let final_label = if label.is_empty() {
            key.tag().unwrap_or("")
        } else {
            label
        };
        let (proxy, listener) = create_entry(final_label)?;
        // Success, add it to the cache
        self.insert_cache_entry(key, Arc::clone(&proxy), listener);
        Some(proxy)
    }

    /// The first half of `findOrCreateCacheEntry()`: the cached proxy for `key`, refreshing its
    /// texture's access time, if there is one. Callers that must create a proxy with the
    /// resource provider, which the cache lives in, look up here and then call
    /// [`Self::insert_cache_entry`].
    // Port of: src/gpu/graphite/ProxyCache.h#L60-L69 (chrome/m156), the cache-hit half
    pub fn find_cache_entry(&mut self, key: &UniqueKey) -> Option<Arc<TextureProxy>> {
        self.process_invalid_key_msgs();

        let cached = self.cache.find(key)?;
        cached.proxy.with_texture(|texture| {
            if let Some(texture) = texture {
                texture.base().update_access_time();
            }
        });
        Some(Arc::clone(&cached.proxy))
    }

    /// Caches `proxy` for `key` with its invalidation `listener` (the cache-miss half of
    /// `findOrCreateCacheEntry()`).
    // Port of: src/gpu/graphite/ProxyCache.cpp#L90-L94 (chrome/m156)
    pub fn insert_cache_entry(
        &mut self,
        key: &UniqueKey,
        proxy: Arc<TextureProxy>,
        listener: Option<Arc<IdChangeListener>>,
    ) {
        self.cache.set(key.clone(), CacheEntry { proxy, listener });
    }

    /// `make_unique_key_invalidation_listener(key, recorderID)`: a listener that invalidates the
    /// entry for `key` in this cache when the source's id changes.
    // Port of: src/gpu/graphite/ProxyCache.cpp#L43-L59 (chrome/m156)
    #[must_use]
    pub fn make_unique_key_invalidation_listener(&self, key: &UniqueKey) -> Arc<IdChangeListener> {
        let msg = UniqueKeyInvalidatedMsgGraphite::new(key, self.recorder_id);
        let inbox: Weak<Inbox> = Arc::downgrade(&self.invalid_unique_key_inbox);
        IdChangeListener::new(move || {
            // SkMessageBus::Post: delivered to the inbox of the recorder that made the listener,
            // if it still exists.
            if let Some(inbox) = inbox.upgrade() {
                inbox.receive(msg.clone());
            }
        })
    }

    /// `purgeAll()`.
    // Port of: src/gpu/graphite/ProxyCache.cpp#L169-L177 (chrome/m156)
    #[doc(alias = "purgeAll")]
    pub fn purge_all(&mut self) {
        // removeEntriesAndListeners() without having to copy out all of the keys
        self.cache.foreach(|_, entry| {
            if let Some(listener) = &entry.listener {
                listener.mark_should_deregister();
            }
        });
        self.cache.reset();
    }

    // Port of: src/gpu/graphite/ProxyCache.cpp#L179-L197 (chrome/m156)
    fn process_invalid_key_msgs(&mut self) {
        let invalid_key_msgs = self.invalid_unique_key_inbox.poll();

        for msg in &invalid_key_msgs {
            // NOTE(crbug.com/1480570): A change listener is only invoked once, but the entry may
            // have been removed for other reasons while the bitmap listener owner was alive.
            if self.cache.find(msg.key()).is_some() {
                self.cache.remove(msg.key());
            }
        }
    }

    // Port of: src/gpu/graphite/ProxyCache.cpp#L199-L221 (chrome/m156)
    fn remove_entries_and_listeners(
        &mut self,
        to_remove: &[UniqueKey],
        quit_purging_time: Option<StdSteadyClockTimePoint>,
    ) {
        // This assumes that the entry removal is coming from not polling the invalid key messages,
        // so it's necessary to mark the listeners as done.
        let time_remains_before_stop_time =
            || quit_purging_time.is_none_or(|quit| StdSteadyClockTimePoint::now() < quit);

        for k in to_remove {
            if let Some(e) = self.cache.find(k)
                && let Some(listener) = &e.listener
            {
                listener.mark_should_deregister();
            }
            let removed = self.cache.remove(k);
            drop(removed);
            if !time_remains_before_stop_time() {
                return;
            }
        }
    }

    // Port of: src/gpu/graphite/ProxyCache.cpp#L223-L236 (chrome/m156)
    pub(crate) fn free_uniquely_held(&mut self) {
        self.process_invalid_key_msgs();

        let mut to_remove = Vec::new();

        self.cache.foreach(|key, entry| {
            if Arc::strong_count(&entry.proxy) == 1 {
                to_remove.push(key.clone());
            }
        });

        self.remove_entries_and_listeners(&to_remove, None);
    }

    // Port of: src/gpu/graphite/ProxyCache.cpp#L238-L255 (chrome/m156)
    pub(crate) fn purge_proxies_not_used_since(
        &mut self,
        purge_time: Option<StdSteadyClockTimePoint>,
        quit_purging_time: Option<StdSteadyClockTimePoint>,
    ) {
        self.process_invalid_key_msgs();

        let mut to_remove = Vec::new();

        self.cache.foreach(|key, entry| {
            let purge = entry.proxy.with_texture(|texture| {
                texture.is_some_and(|resource| {
                    purge_time.is_none_or(|t| resource.base().last_access_time() < t)
                })
            });
            if purge {
                to_remove.push(key.clone());
            }
        });

        self.remove_entries_and_listeners(&to_remove, quit_purging_time);
    }

    // ---- GPU_TEST_UTILS -----------------------------------------------------------------------

    /// `numCached()` (test utility).
    // Port of: src/gpu/graphite/ProxyCache.cpp#L258-L260 (chrome/m156)
    #[doc(alias = "numCached")]
    #[must_use]
    pub fn num_cached(&self) -> usize {
        self.cache.count()
    }

    /// `find(bitmap)` (test utility).
    // Port of: src/gpu/graphite/ProxyCache.cpp#L262-L274 (chrome/m156)
    #[must_use]
    pub fn find(&self, bitmap: &Bitmap) -> Option<Arc<TextureProxy>> {
        let key = make_bitmap_key(bitmap);
        self.cache
            .find(&key)
            .map(|cached| Arc::clone(&cached.proxy))
    }

    /// `forceProcessInvalidKeyMsgs()` (test utility).
    #[doc(alias = "forceProcessInvalidKeyMsgs")]
    pub fn force_process_invalid_key_msgs(&mut self) {
        self.process_invalid_key_msgs();
    }

    /// `forceFreeUniquelyHeld()` (test utility).
    #[doc(alias = "forceFreeUniquelyHeld")]
    pub fn force_free_uniquely_held(&mut self) {
        self.free_uniquely_held();
    }

    /// `forcePurgeProxiesNotUsedSince()` (test utility): not limited in how long it may take.
    // Port of: src/gpu/graphite/ProxyCache.cpp#L284-L287 (chrome/m156)
    #[doc(alias = "forcePurgeProxiesNotUsedSince")]
    pub fn force_purge_proxies_not_used_since(&mut self, purge_time: StdSteadyClockTimePoint) {
        self.purge_proxies_not_used_since(Some(purge_time), None);
    }

    /// The key of `bitmap`'s pixels, as `findOrCreateCachedProxy(recorder, bitmap)` uses.
    #[must_use]
    pub fn bitmap_key(bitmap: &Bitmap) -> UniqueKey {
        make_bitmap_key(bitmap)
    }
}

/// `ProxyCache::findOrCreateCachedProxy(recorder, key, context, GPUGeneratorFn)`: the proxy cached
/// for `key`, or the texture of the image `generator` makes for it, which is then cached. Images
/// the GPU generates never have invalidation listeners.
///
/// The resource provider is locked only while the cache is consulted and while the image's
/// texture is instantiated: the generator records draws, which lock it again.
// Port of: src/gpu/graphite/ProxyCache.cpp#L142-L170 (chrome/m156)
#[doc(alias = "findOrCreateCachedProxy")]
pub fn find_or_create_cached_proxy_from_image(
    recorder: &crate::graphite::recorder::Recorder,
    key: &UniqueKey,
    generator: impl FnOnce(&crate::graphite::recorder::Recorder) -> Option<skia_rust_core::image::Image>,
) -> Option<Arc<TextureProxy>> {
    let shared = Arc::clone(recorder.priv_().resource_provider());
    {
        let mut provider = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = provider
            .proxy_cache()
            .and_then(|cache| cache.find_cache_entry(key))
        {
            return Some(cached);
        }
    }

    // Cache miss: make the image without holding the provider's lock.
    let image = generator(recorder)?;
    let view = crate::graphite::texture_utils::as_view(Some(&image));
    let proxy = view.ref_proxy()?;
    {
        let mut provider = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        // Force `textureImage`'s TextureProxy to be instantiated so that it's not treated by a
        // Recorder as a scratch image that can have a temporary scratch texture assignment.
        proxy.instantiate(&mut provider);
        provider
            .proxy_cache()?
            .insert_cache_entry(key, Arc::clone(&proxy), None);
    }
    Some(proxy)
}

/// `ProxyCache::findOrCreateCachedProxy(recorder, key, context, BitmapGeneratorFn, label)`: the
/// proxy cached for `key`, or the texture of the bitmap `generator` makes for it, uploaded with
/// `MakeBitmapProxyView`. An empty bitmap, or a view that cannot be made, caches nothing. A bitmap
/// whose pixels are shared (not unique) gets an invalidation listener, as in C++.
///
/// The provider is locked only to consult the cache and to insert the entry: the generator and the
/// upload lock it again.
// Port of: src/gpu/graphite/ProxyCache.cpp#L101-L140 (chrome/m156)
#[doc(alias = "findOrCreateCachedProxy")]
pub fn find_or_create_cached_proxy_from_bitmap(
    recorder: &crate::graphite::recorder::Recorder,
    key: &UniqueKey,
    generator: impl FnOnce() -> Bitmap,
    label: &str,
) -> Option<Arc<TextureProxy>> {
    let shared = Arc::clone(recorder.priv_().resource_provider());
    {
        let mut provider = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(cached) = provider
            .proxy_cache()
            .and_then(|cache| cache.find_cache_entry(key))
        {
            return Some(cached);
        }
    }

    // Cache miss: make the bitmap and its texture without holding the provider's lock.
    let bitmap = generator();
    if bitmap.is_empty() {
        return None;
    }
    let final_label = if label.is_empty() {
        key.tag().unwrap_or("")
    } else {
        label
    };
    let view = crate::graphite::texture_utils::make_bitmap_proxy_view(
        recorder,
        &bitmap,
        None,
        crate::gpu::gpu_types::Mipmapped::No,
        crate::gpu::gpu_types::Budgeted::Yes,
        final_label,
    )?;
    let proxy = view.ref_proxy()?;

    // Since if the bitmap is held by more than just this function call (e.g. it likely came from
    // an existing SkBitmap), it's worth adding a listener to remove the entry automatically when
    // no one holds on to it anymore.
    let add_listener = !bitmap.pixel_ref_is_unique();
    let listener = {
        let mut provider = shared
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let cache = provider.proxy_cache()?;
        let listener = add_listener.then(|| cache.make_unique_key_invalidation_listener(key));
        if let (Some(listener), Some(pixel_ref)) = (&listener, bitmap.pixel_ref()) {
            pixel_ref.add_gen_id_change_listener(Some(Arc::clone(listener)));
        }
        listener
    };

    let mut provider = shared
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    provider
        .proxy_cache()?
        .insert_cache_entry(key, Arc::clone(&proxy), listener);
    Some(proxy)
}
