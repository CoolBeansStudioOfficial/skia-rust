// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureProxy.h, src/gpu/graphite/TextureProxy.cpp

//! `TextureProxy`: a texture that may not exist yet. It is instantiated with a [`Texture`] when the
//! recording is prepared (or immediately, for unbudgeted proxies), or lazily through a callback.
//!
//! Proxies are shared (`sk_sp`) by images, devices and tasks, and images are `Send + Sync`, so a
//! proxy is an `Arc<TextureProxy>` whose instantiation state sits behind a `Mutex`
//! (`docs/design/gpu.md` §5.1).

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Budgeted, Mipmapped, Protected};
use crate::graphite::caps::Caps;
use crate::graphite::graphite_types::{SampleCount, Volatile};
use crate::graphite::resource::ResourceRef;
use crate::graphite::resource_provider::ResourceProvider;
use crate::graphite::scratch_resource_manager::ScratchResourceManager;
use crate::graphite::texture::Texture;
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};
use crate::graphite::texture_utils::compute_size;

/// `TextureProxy::LazyInstantiateCallback`.
pub type LazyInstantiateCallback =
    Box<dyn Fn(&mut ResourceProvider) -> Option<ResourceRef<Texture>> + Send + Sync>;

/// A possibly uninstantiated texture.
#[doc(alias = "skgpu::graphite::TextureProxy")]
pub struct TextureProxy {
    // In the following, `volatile` and `lazy_instantiate_callback` can be accessed from multiple
    // threads so need to remain immutable.
    dimensions: ISize,
    info: TextureInfo,

    budgeted: Mutex<Budgeted>,
    volatile: Volatile,

    // String used to describe the current use of this TextureProxy. It will be set on its Texture
    // object when the proxy gets instantiated.
    label: String,

    texture: Mutex<Option<ResourceRef<Texture>>>,

    lazy_instantiate_callback: Option<LazyInstantiateCallback>,
}

impl fmt::Debug for TextureProxy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TextureProxy")
            .field("dimensions", &self.dimensions)
            .field("info", &self.info)
            .field("label", &self.label)
            .field("is_lazy", &self.is_lazy())
            .field("is_instantiated", &self.is_instantiated())
            .finish_non_exhaustive()
    }
}

// Port of: src/gpu/graphite/TextureProxy.cpp#L162-L168 (chrome/m156)
fn texture_info_and_size_are_valid(
    caps: &dyn Caps,
    dimensions: ISize,
    texture_info: &TextureInfo,
) -> bool {
    dimensions.width >= 1
        && dimensions.height >= 1
        && dimensions.width <= caps.max_texture_size()
        && dimensions.height <= caps.max_texture_size()
        && texture_info.is_valid()
}

impl TextureProxy {
    // Port of: src/gpu/graphite/TextureProxy.cpp#L22-L33 (chrome/m156)
    fn new_scratch(dimensions: ISize, info: &TextureInfo, budgeted: Budgeted, label: &str) -> Self {
        debug_assert!(info.is_valid());
        Self {
            dimensions,
            info: info.clone(),
            budgeted: Mutex::new(budgeted),
            volatile: Volatile::No,
            label: label.to_owned(),
            texture: Mutex::new(None),
            lazy_instantiate_callback: None,
        }
    }

    // Port of: src/gpu/graphite/TextureProxy.cpp#L35-L44 (chrome/m156)
    fn new_wrapped(texture: ResourceRef<Texture>) -> Self {
        let info = texture.texture_info().clone();
        debug_assert!(info.is_valid());
        Self {
            dimensions: texture.dimensions(),
            info,
            budgeted: Mutex::new(texture.base().budgeted()),
            volatile: Volatile::No,
            label: texture.base().label(),
            texture: Mutex::new(Some(texture)),
            lazy_instantiate_callback: None,
        }
    }

    // Port of: src/gpu/graphite/TextureProxy.cpp#L46-L59 (chrome/m156)
    fn new_lazy(
        dimensions: ISize,
        texture_info: &TextureInfo,
        budgeted: Budgeted,
        is_volatile: Volatile,
        callback: LazyInstantiateCallback,
    ) -> Self {
        debug_assert!(texture_info.is_valid());
        Self {
            dimensions,
            info: texture_info.clone(),
            budgeted: Mutex::new(budgeted),
            volatile: is_volatile,
            label: String::new(),
            texture: Mutex::new(None),
            lazy_instantiate_callback: Some(callback),
        }
    }

    fn texture_slot(&self) -> MutexGuard<'_, Option<ResourceRef<Texture>>> {
        self.texture
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// `sampleCount()`.
    #[must_use]
    pub fn sample_count(&self) -> SampleCount {
        self.info.sample_count()
    }

    /// `mipmapped()`.
    #[must_use]
    pub fn mipmapped(&self) -> Mipmapped {
        self.info.mipmapped()
    }

    /// `format()`.
    #[must_use]
    pub fn format(&self) -> TextureFormat {
        texture_info_priv::view_format(&self.info)
    }

    /// `dimensions()`: the texture's dimensions once instantiated, else the proxy's.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L63-L66 (chrome/m156)
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        debug_assert!(!self.is_fully_lazy() || self.is_instantiated());
        match &*self.texture_slot() {
            Some(texture) => texture.dimensions(),
            None => self.dimensions,
        }
    }

    /// `textureInfo()`.
    #[doc(alias = "textureInfo")]
    #[must_use]
    pub fn texture_info(&self) -> &TextureInfo {
        &self.info
    }

    /// `label()`.
    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    /// `isLazy()`.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L68-L70 (chrome/m156)
    #[doc(alias = "isLazy")]
    #[must_use]
    pub fn is_lazy(&self) -> bool {
        self.lazy_instantiate_callback.is_some()
    }

    /// `isFullyLazy()`: lazy without known dimensions.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L72-L77 (chrome/m156)
    #[doc(alias = "isFullyLazy")]
    #[must_use]
    pub fn is_fully_lazy(&self) -> bool {
        let result = self.dimensions.width < 0;
        debug_assert_eq!(result, self.dimensions.height < 0);
        debug_assert!(!result || self.is_lazy());
        result
    }

    /// `isVolatile()`.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L79-L83 (chrome/m156)
    #[doc(alias = "isVolatile")]
    #[must_use]
    pub fn is_volatile(&self) -> bool {
        debug_assert!(self.volatile == Volatile::No || self.lazy_instantiate_callback.is_some());

        self.volatile == Volatile::Yes
    }

    /// `isProtected()`.
    #[must_use]
    pub fn is_protected(&self) -> Protected {
        self.info.is_protected()
    }

    /// `uninstantiatedGpuMemorySize()`.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L85-L87 (chrome/m156)
    #[doc(alias = "uninstantiatedGpuMemorySize")]
    #[must_use]
    pub fn uninstantiated_gpu_memory_size(&self) -> usize {
        compute_size(self.dimensions, &self.info)
    }

    /// `instantiate()`: finds or creates the texture (not for lazy proxies).
    // Port of: src/gpu/graphite/TextureProxy.cpp#L89-L105 (chrome/m156)
    pub fn instantiate(&self, resource_provider: &mut ResourceProvider) -> bool {
        debug_assert!(!self.is_lazy());

        if self.is_instantiated() {
            return true;
        }

        // TODO(389908374): Once all tasks use the ScratchResourceManager, this can be updated to
        // just finding and creating a non-shareable AND non-budgeted texture.
        let budgeted = *self
            .budgeted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(texture) = resource_provider.find_or_create_non_shareable_texture(
            self.dimensions,
            &self.info,
            &self.label,
            budgeted,
        ) else {
            return false;
        };
        self.validate_texture(&texture);
        *self.texture_slot() = Some(texture);
        true
    }

    /// `lazyInstantiate()`: runs the lazy callback (lazy proxies only).
    ///
    /// Unlike Ganesh, the proxy's dimensions are not updated, so a fully-lazy proxy goes back to
    /// being fully lazy when deinstantiated.
    ///
    /// # Panics
    /// If the proxy is not lazy.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L107-L120 (chrome/m156)
    #[doc(alias = "lazyInstantiate")]
    pub fn lazy_instantiate(&self, resource_provider: &mut ResourceProvider) -> bool {
        debug_assert!(self.is_lazy());

        if self.is_instantiated() {
            return true;
        }

        // The slot is not locked while the callback runs, so it may inspect this proxy.
        let callback = self
            .lazy_instantiate_callback
            .as_ref()
            .expect("lazy proxy has a callback");
        let Some(texture) = callback(resource_provider) else {
            return false;
        };
        self.validate_texture(&texture);
        *self.texture_slot() = Some(texture);
        true
    }

    /// `InstantiateIfNotLazy(ResourceProvider*, TextureProxy*)`: true for lazy proxies, else the
    /// result of [`instantiate`](Self::instantiate).
    // Port of: src/gpu/graphite/TextureProxy.cpp#L122-L129 (chrome/m156)
    #[doc(alias = "InstantiateIfNotLazy")]
    pub fn instantiate_if_not_lazy(
        resource_provider: &mut ResourceProvider,
        texture_proxy: &TextureProxy,
    ) -> bool {
        if texture_proxy.is_lazy() {
            return true;
        }

        texture_proxy.instantiate(resource_provider)
    }

    /// `InstantiateIfNotLazy(ScratchResourceManager*, TextureProxy*)`: instantiates a scratch
    /// proxy with a texture from the scratch manager. Lazy and instantiated proxies return true.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L131-L145 (chrome/m156)
    #[doc(alias = "InstantiateIfNotLazy")]
    pub fn instantiate_if_not_lazy_scratch(
        scratch_manager: &mut ScratchResourceManager,
        resource_provider: &mut ResourceProvider,
        texture_proxy: &TextureProxy,
    ) -> bool {
        if texture_proxy.is_lazy() || texture_proxy.is_instantiated() {
            return true;
        }

        let Some(texture) = scratch_manager.get_scratch_texture(
            resource_provider,
            texture_proxy.dimensions(),
            texture_proxy.texture_info(),
            &texture_proxy.label,
        ) else {
            return false;
        };
        texture_proxy.validate_texture(&texture);
        *texture_proxy.texture_slot() = Some(texture);
        true
    }

    /// `isInstantiated()`.
    #[doc(alias = "isInstantiated")]
    #[must_use]
    pub fn is_instantiated(&self) -> bool {
        self.texture_slot().is_some()
    }

    /// `deinstantiate()`: drops the texture (volatile lazy proxies only).
    // Port of: src/gpu/graphite/TextureProxy.cpp#L148-L152 (chrome/m156)
    pub fn deinstantiate(&self) {
        debug_assert!(self.volatile == Volatile::Yes && self.lazy_instantiate_callback.is_some());

        let texture = self.texture_slot().take();
        drop(texture);
    }

    /// `refTexture()`: the texture, with a new usage ref.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L154-L156 (chrome/m156)
    #[doc(alias = "refTexture")]
    #[must_use]
    pub fn ref_texture(&self) -> Option<ResourceRef<Texture>> {
        self.texture_slot().clone()
    }

    /// `texture()`: calls `f` with the texture without adding a ref.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L158-L160 (chrome/m156)
    pub fn with_texture<R>(&self, f: impl FnOnce(Option<&ResourceRef<Texture>>) -> R) -> R {
        f(self.texture_slot().as_ref())
    }

    /// `setBudgeted()`: only for uninstantiated, non-lazy proxies.
    // Port of: src/gpu/graphite/TextureProxy.h#L92-L96 (chrome/m156)
    #[doc(alias = "setBudgeted")]
    pub fn set_budgeted(&self, budgeted: Budgeted) {
        debug_assert!(!self.is_instantiated());
        debug_assert!(!self.is_lazy());
        *self
            .budgeted
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = budgeted;
    }

    /// `Make()`: a proxy that is instantiated immediately when unbudgeted (to avoid races if the
    /// client uses the wrapping object on multiple threads).
    // Port of: src/gpu/graphite/TextureProxy.cpp#L170-L189 (chrome/m156)
    #[must_use]
    pub fn make(
        caps: &dyn Caps,
        resource_provider: &mut ResourceProvider,
        dimensions: ISize,
        texture_info: &TextureInfo,
        budgeted: Budgeted,
        label: &str,
    ) -> Option<Arc<TextureProxy>> {
        if !texture_info_and_size_are_valid(caps, dimensions, texture_info) {
            return None;
        }

        let proxy = Arc::new(TextureProxy::new_scratch(
            dimensions,
            texture_info,
            budgeted,
            label,
        ));
        if budgeted == Budgeted::No {
            // Instantiate immediately to avoid races later on if the client starts to use the
            // wrapping object on multiple threads.
            if !proxy.instantiate(resource_provider) {
                return None;
            }
        }
        Some(proxy)
    }

    /// `MakeLazy()`.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L191-L206 (chrome/m156)
    #[doc(alias = "MakeLazy")]
    #[must_use]
    pub fn make_lazy(
        caps: &dyn Caps,
        dimensions: ISize,
        texture_info: &TextureInfo,
        budgeted: Budgeted,
        is_volatile: Volatile,
        callback: LazyInstantiateCallback,
    ) -> Option<Arc<TextureProxy>> {
        if !texture_info_and_size_are_valid(caps, dimensions, texture_info) {
            return None;
        }

        Some(Arc::new(TextureProxy::new_lazy(
            dimensions,
            texture_info,
            budgeted,
            is_volatile,
            callback,
        )))
    }

    /// `MakeFullyLazy()`: a lazy proxy without dimensions.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L208-L219 (chrome/m156)
    #[doc(alias = "MakeFullyLazy")]
    #[must_use]
    pub fn make_fully_lazy(
        texture_info: &TextureInfo,
        budgeted: Budgeted,
        is_volatile: Volatile,
        callback: LazyInstantiateCallback,
    ) -> Arc<TextureProxy> {
        debug_assert!(texture_info.is_valid());

        Arc::new(TextureProxy::new_lazy(
            ISize::new(-1, -1),
            texture_info,
            budgeted,
            is_volatile,
            callback,
        ))
    }

    /// `Wrap()`: a proxy around an existing texture.
    // Port of: src/gpu/graphite/TextureProxy.cpp#L221-L223 (chrome/m156)
    #[must_use]
    pub fn wrap(texture: ResourceRef<Texture>) -> Arc<TextureProxy> {
        Arc::new(TextureProxy::new_wrapped(texture))
    }

    // Port of: src/gpu/graphite/TextureProxy.cpp#L225-L233 (chrome/m156)
    fn validate_texture(&self, texture: &Texture) {
        debug_assert!(self.is_fully_lazy() || self.dimensions == texture.dimensions());
        debug_assert!(
            self.info.can_be_fulfilled_by(texture.texture_info()),
            "proxy->fInfo[{}] incompatible with texture->fInfo[{}]",
            self.info,
            texture.texture_info()
        );
    }
}

/// `AutoDeinstantiateTextureProxy`: deinstantiates a volatile proxy when dropped.
// Port of: src/gpu/graphite/TextureProxy.h#L154-L167 (chrome/m156)
#[derive(Debug)]
pub struct AutoDeinstantiateTextureProxy {
    texture_proxy: Option<Arc<TextureProxy>>,
}

impl AutoDeinstantiateTextureProxy {
    /// Deinstantiates `texture_proxy` (if any) when dropped.
    #[must_use]
    pub fn new(texture_proxy: Option<Arc<TextureProxy>>) -> Self {
        Self { texture_proxy }
    }
}

impl Drop for AutoDeinstantiateTextureProxy {
    fn drop(&mut self) {
        if let Some(proxy) = &self.texture_proxy {
            proxy.deinstantiate();
        }
    }
}
