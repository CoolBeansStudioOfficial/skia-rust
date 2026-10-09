// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/Texture.h, src/gpu/graphite/Texture.cpp

//! `skgpu::graphite::Texture`: the backend-neutral half of a GPU texture resource.
//!
//! The backend half (the wgpu texture, its views and label) implements [`TextureBackend`] and is
//! owned by the texture. `MutableTextureState` (Vulkan/Android layouts) and `uploadDataOnHost`
//! (host-copy uploads) are not ported: the wgpu back end uses neither.

use std::any::Any;
use std::fmt;
use std::sync::{Arc, Mutex};

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::{Mipmapped, Protected};
use crate::graphite::graphite_types::SampleCount;
use crate::graphite::resource::{Resource, ResourceObject, ResourceRef, synchronize_backend_label};
use crate::graphite::resource_types::Ownership;
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::texture_utils::compute_size;

/// The backend half of a [`Texture`] (the virtual functions a backend texture overrides).
pub trait TextureBackend: Send + Sync + fmt::Debug + 'static {
    /// `freeGpuData()`.
    fn free_gpu_data(&self);
    /// `setBackendLabel()`.
    fn set_backend_label(&self, _label: &str) {}
    /// `onUpdateGpuMemorySize()`.
    fn on_update_gpu_memory_size(&self, current: usize) -> usize {
        current
    }
    /// `canUploadOnHost()`.
    fn can_upload_on_host(&self) -> bool {
        false
    }
    /// For downcasting to the concrete backend texture.
    fn as_any(&self) -> &dyn Any;
}

/// `sk_sp<RefCntedCallback>`: a shared callback that runs when its last reference is dropped.
pub type ReleaseCallback = Arc<dyn Any + Send + Sync>;

/// A GPU texture.
#[doc(alias = "skgpu::graphite::Texture")]
pub struct Texture {
    dimensions: ISize,
    info: TextureInfo,
    release_callback: Mutex<Option<ReleaseCallback>>,
    backend: Box<dyn TextureBackend>,
}

impl fmt::Debug for Texture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Texture")
            .field("dimensions", &self.dimensions)
            .field("info", &self.info)
            .field("backend", &self.backend)
            .finish_non_exhaustive()
    }
}

impl Texture {
    /// `Texture(sharedContext, dimensions, info, isTransient, mutableState, ownership, label)`:
    /// creates the texture resource, holding one usage ref.
    ///
    /// For the initial GPU size, this assumes that a transient texture will not have any actual
    /// memory.
    // Port of: src/gpu/graphite/Texture.cpp#L18-L32 (chrome/m156)
    #[must_use]
    pub fn make(
        dimensions: ISize,
        info: &TextureInfo,
        is_transient: bool,
        ownership: Ownership,
        label: &str,
        backend: Box<dyn TextureBackend>,
    ) -> ResourceRef<Texture> {
        let gpu_memory_size = if is_transient {
            0
        } else {
            compute_size(dimensions, info)
        };
        let texture = Resource::new(
            Texture {
                dimensions,
                info: info.clone(),
                release_callback: Mutex::new(None),
                backend,
            },
            ownership,
            gpu_memory_size,
            label,
            false,
            false,
        );
        synchronize_backend_label(&*texture.erased());
        texture
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

    /// `dimensions()`.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.dimensions
    }

    /// `textureInfo()`.
    #[doc(alias = "textureInfo")]
    #[must_use]
    pub fn texture_info(&self) -> &TextureInfo {
        &self.info
    }

    /// `setReleaseCallback()`.
    // Port of: src/gpu/graphite/Texture.cpp#L36-L38 (chrome/m156)
    #[doc(alias = "setReleaseCallback")]
    pub fn set_release_callback(&self, release_callback: Option<ReleaseCallback>) {
        *self
            .release_callback
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = release_callback;
    }

    /// `canUploadOnHost()`.
    #[must_use]
    pub fn can_upload_on_host(&self) -> bool {
        self.backend.can_upload_on_host()
    }

    /// The backend half.
    #[must_use]
    pub fn backend(&self) -> &dyn TextureBackend {
        &*self.backend
    }
}

impl ResourceObject for Texture {
    fn resource_type(&self) -> &'static str {
        "Texture"
    }

    fn free_gpu_data(&self) {
        self.backend.free_gpu_data();
    }

    // Port of: src/gpu/graphite/Texture.cpp#L40-L46 (chrome/m156)
    fn invoke_release_proc(&self) {
        // Depending on the ref count of the release callback this may or may not actually
        // trigger the ReleaseProc to be called.
        drop(
            self.release_callback
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take(),
        );
    }

    fn set_backend_label(&self, label: &str) {
        self.backend.set_backend_label(label);
    }

    fn on_update_gpu_memory_size(&self, current: usize) -> usize {
        self.backend.on_update_gpu_memory_size(current)
    }

    fn is_protected(&self) -> Protected {
        self.info.is_protected()
    }

    fn as_texture(&self) -> Option<&Texture> {
        Some(self)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::graphite::texture_format::TextureFormat;
    use crate::graphite::texture_info::tests::mock_info;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// A backend texture that counts frees.
    #[derive(Debug, Default)]
    pub(crate) struct MockTexture {
        pub(crate) freed: Arc<AtomicUsize>,
    }

    impl TextureBackend for MockTexture {
        fn free_gpu_data(&self) {
            self.freed.fetch_add(1, Ordering::Relaxed);
        }

        fn as_any(&self) -> &dyn Any {
            self
        }
    }

    #[test]
    fn size_and_release_callback() {
        let freed = Arc::new(AtomicUsize::new(0));
        let tex = Texture::make(
            ISize::new(4, 2),
            &mock_info(TextureFormat::RGBA8),
            false,
            Ownership::Owned,
            "T",
            Box::new(MockTexture {
                freed: freed.clone(),
            }),
        );
        assert_eq!(tex.base().gpu_memory_size(), 32);
        assert_eq!(tex.base().label(), "T");
        let callback: ReleaseCallback = Arc::new(());
        tex.set_release_callback(Some(callback.clone()));
        assert_eq!(Arc::strong_count(&callback), 2);
        drop(tex);
        assert_eq!(Arc::strong_count(&callback), 1);
        assert_eq!(freed.load(Ordering::Relaxed), 1);
    }
}
