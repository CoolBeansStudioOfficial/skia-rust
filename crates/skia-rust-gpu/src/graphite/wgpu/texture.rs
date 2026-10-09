// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnTexture.h, DawnTexture.cpp

//! `DawnTexture` on wgpu: the backend half of a Graphite [`Texture`].
//!
//! Differences from Dawn:
//! - wgpu textures and views cannot be relabeled after creation, so the label is set when the
//!   object is created (`setBackendLabel` is a no-op).
//! - Wrapping a bare `wgpu::TextureView` (`MakeWrapped(…, TextureView, …)`) works as in Dawn: the
//!   texture is absent and both views are the one view.
//! - YCbCr and multiplanar textures do not exist in wgpu.

use std::any::Any;
use std::sync::{Mutex, MutexGuard, PoisonError};

use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::Mipmapped;
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::resource::{ResourceRef, ResourceUniqueId};
use crate::graphite::resource_types::Ownership;
use crate::graphite::texture::{Texture, TextureBackend};
use crate::graphite::texture_info::TextureInfo;
use crate::graphite::wgpu::async_wait::create_checked;
use crate::graphite::wgpu::shared_context::WgpuSharedContext;
use crate::graphite::wgpu::texture_info::WgpuTextureInfoData;

/// The wgpu objects of a texture; `None` once freed (`freeGpuData()`).
#[derive(Debug)]
struct Objects {
    texture: Option<wgpu::Texture>,
    sample_texture_view: wgpu::TextureView,
    render_texture_view: wgpu::TextureView,
}

/// The wgpu half of a [`Texture`].
// Port of: src/gpu/graphite/dawn/DawnTexture.h#L22-L70 (chrome/m156)
#[doc(alias = "DawnTexture")]
#[derive(Debug)]
pub struct WgpuTexture {
    ownership: Ownership,
    objects: Mutex<Option<Objects>>,
    // The sampler is identified by its resource's unique id, as `getCachedSingleTextureBindGroup`
    // compares ids rather than pointers.
    cached_single_texture_bind_groups: Mutex<Vec<(ResourceUniqueId, wgpu::BindGroup)>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl WgpuTexture {
    /// `MakeDawnTexture()`: creates the wgpu texture described by `info`, or `None` if the
    /// description is invalid.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L22-L86 (chrome/m156)
    #[doc(alias = "MakeDawnTexture")]
    #[must_use]
    pub fn make_wgpu_texture(
        shared_context: &WgpuSharedContext,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<wgpu::Texture> {
        let caps = shared_context.caps();
        if dimensions.width > caps.max_texture_size() || dimensions.height > caps.max_texture_size()
        {
            skia_log_e!(
                "Texture creation failure: dimensions {} x {} too large.",
                dimensions.width,
                dimensions.height
            );
            return None;
        }
        if dimensions.width <= 0 || dimensions.height <= 0 {
            skia_log_e!(
                "Texture creation failure: dimensions {} x {} are empty.",
                dimensions.width,
                dimensions.height
            );
            return None;
        }

        let wgpu_info = info.get::<WgpuTextureInfoData>()?;
        let format = wgpu_info.format?;

        if wgpu_info
            .usage
            .contains(wgpu::TextureUsages::TEXTURE_BINDING)
            && !caps.is_texturable(info, /* allowMSAA= */ true)
            && !caps.is_readable(info, /* allowMSAA= */ true)
        {
            return None;
        }

        if wgpu_info
            .usage
            .contains(wgpu::TextureUsages::RENDER_ATTACHMENT)
            && !caps.is_renderable(info)
        {
            return None;
        }

        if wgpu_info
            .usage
            .contains(wgpu::TextureUsages::STORAGE_BINDING)
            && !caps.is_storage(info)
        {
            return None;
        }

        let mut num_mip_levels = 1;
        if info.mipmapped() == Mipmapped::Yes {
            num_mip_levels = Mipmap::compute_level_count_size(dimensions) + 1;
        }

        #[allow(clippy::cast_sign_loss)] // dimensions and level counts are positive (checked)
        let desc = wgpu::TextureDescriptor {
            label: backend_label(shared_context, label),
            size: wgpu::Extent3d {
                width: dimensions.width as u32,
                height: dimensions.height as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: num_mip_levels as u32,
            sample_count: info.sample_count() as u32,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu_info.usage,
            view_formats: &[],
        };

        create_checked(
            shared_context.device(),
            caps.allow_scoped_error_checks(),
            || shared_context.device().create_texture(&desc),
        )
    }

    /// `CreateTextureViews()`: the views to sample and to render to the texture.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L111-L162 (chrome/m156)
    fn create_texture_views(
        texture: &wgpu::Texture,
        info: &TextureInfo,
        label: Option<&str>,
    ) -> Option<(wgpu::TextureView, wgpu::TextureView)> {
        let wgpu_info = info.get::<WgpuTextureInfoData>()?;
        // Multiplanar textures do not exist in wgpu.
        if wgpu_info.aspect != wgpu::TextureAspect::All {
            return None;
        }
        let mut view_desc = wgpu::TextureViewDescriptor {
            label,
            dimension: Some(wgpu::TextureViewDimension::D2),
            base_array_layer: wgpu_info.slice,
            array_layer_count: Some(1),
            ..Default::default()
        };
        let sample_texture_view = texture.create_view(&view_desc);
        let render_texture_view = if info.mipmapped() == Mipmapped::Yes {
            view_desc.base_mip_level = 0;
            view_desc.mip_level_count = Some(1);
            texture.create_view(&view_desc)
        } else {
            sample_texture_view.clone()
        };
        Some((sample_texture_view, render_texture_view))
    }

    /// `Make()`: creates a texture resource.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L164-L183 (chrome/m156)
    #[must_use]
    pub fn make(
        shared_context: &WgpuSharedContext,
        dimensions: ISize,
        info: &TextureInfo,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        let texture = Self::make_wgpu_texture(shared_context, dimensions, info, label)?;
        let (sample_texture_view, render_texture_view) =
            Self::create_texture_views(&texture, info, backend_label(shared_context, label))?;
        Some(Self::wrap_objects(
            dimensions,
            info,
            Some(texture),
            sample_texture_view,
            render_texture_view,
            Ownership::Owned,
            label,
        ))
    }

    /// `MakeWrapped(…, wgpu::Texture, …)`: wraps a client texture.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L185-L204 (chrome/m156)
    #[doc(alias = "MakeWrapped")]
    #[must_use]
    pub fn make_wrapped(
        shared_context: &WgpuSharedContext,
        dimensions: ISize,
        info: &TextureInfo,
        texture: wgpu::Texture,
        label: &str,
    ) -> Option<ResourceRef<Texture>> {
        let (sample_texture_view, render_texture_view) =
            Self::create_texture_views(&texture, info, backend_label(shared_context, label))?;
        Some(Self::wrap_objects(
            dimensions,
            info,
            Some(texture),
            sample_texture_view,
            render_texture_view,
            Ownership::Wrapped,
            label,
        ))
    }

    /// `MakeWrapped(…, wgpu::TextureView, …)`: wraps a client texture view.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L206-L222 (chrome/m156)
    #[doc(alias = "MakeWrapped")]
    #[must_use]
    pub fn make_wrapped_view(
        dimensions: ISize,
        info: &TextureInfo,
        texture_view: wgpu::TextureView,
        label: &str,
    ) -> ResourceRef<Texture> {
        Self::wrap_objects(
            dimensions,
            info,
            None,
            texture_view.clone(),
            texture_view,
            Ownership::Wrapped,
            label,
        )
    }

    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L88-L109 (chrome/m156)
    fn wrap_objects(
        dimensions: ISize,
        info: &TextureInfo,
        texture: Option<wgpu::Texture>,
        sample_texture_view: wgpu::TextureView,
        render_texture_view: wgpu::TextureView,
        ownership: Ownership,
        label: &str,
    ) -> ResourceRef<Texture> {
        let backend = Self {
            ownership,
            objects: Mutex::new(Some(Objects {
                texture,
                sample_texture_view,
                render_texture_view,
            })),
            cached_single_texture_bind_groups: Mutex::new(Vec::new()),
        };
        Texture::make(
            dimensions,
            info,
            /* isTransient= */ has_transient_usage(info),
            ownership,
            label,
            Box::new(backend),
        )
    }

    /// `dawnTexture()`: the wgpu texture, absent for a wrapped view or once freed.
    #[doc(alias = "dawnTexture")]
    #[must_use]
    pub fn wgpu_texture(&self) -> Option<wgpu::Texture> {
        lock(&self.objects).as_ref()?.texture.clone()
    }

    /// `sampleTextureView()`: absent once freed.
    #[doc(alias = "sampleTextureView")]
    #[must_use]
    pub fn sample_texture_view(&self) -> Option<wgpu::TextureView> {
        Some(lock(&self.objects).as_ref()?.sample_texture_view.clone())
    }

    /// `renderTextureView()`: absent once freed.
    #[doc(alias = "renderTextureView")]
    #[must_use]
    pub fn render_texture_view(&self) -> Option<wgpu::TextureView> {
        Some(lock(&self.objects).as_ref()?.render_texture_view.clone())
    }

    /// `getCachedSingleTextureBindGroup()`: the bind group previously cached for the sampler
    /// with this unique id.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L246-L255 (chrome/m156)
    #[doc(alias = "getCachedSingleTextureBindGroup")]
    #[must_use]
    pub fn get_cached_single_texture_bind_group(
        &self,
        sampler: ResourceUniqueId,
    ) -> Option<wgpu::BindGroup> {
        lock(&self.cached_single_texture_bind_groups)
            .iter()
            .find(|(id, _)| *id == sampler)
            .map(|(_, bind_group)| bind_group.clone())
    }

    /// `addCachedSingleTextureBindGroup()`.
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L257-L261 (chrome/m156)
    #[doc(alias = "addCachedSingleTextureBindGroup")]
    pub fn add_cached_single_texture_bind_group(
        &self,
        bind_group: wgpu::BindGroup,
        sampler: ResourceUniqueId,
    ) {
        lock(&self.cached_single_texture_bind_groups).push((sampler, bind_group));
    }
}

/// The label to give wgpu objects, if the context sets backend labels.
pub(crate) fn backend_label<'a>(
    shared_context: &WgpuSharedContext,
    label: &'a str,
) -> Option<&'a str> {
    (shared_context.caps().set_backend_labels() && !label.is_empty()).then_some(label)
}

// Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L88-L96 (chrome/m156)
fn has_transient_usage(info: &TextureInfo) -> bool {
    info.get::<WgpuTextureInfoData>().is_some_and(|info| {
        info.usage
            .contains(wgpu::TextureUsages::TRANSIENT_ATTACHMENT)
    })
}

impl TextureBackend for WgpuTexture {
    // Port of: src/gpu/graphite/dawn/DawnTexture.cpp#L224-L236 (chrome/m156)
    fn free_gpu_data(&self) {
        let objects = lock(&self.objects).take();
        if let Some(objects) = objects
            && self.ownership != Ownership::Wrapped
            && let Some(texture) = &objects.texture
        {
            // Destroy the texture even if it is still referenced by other BindGroup or views.
            // Graphite should already guarantee that all command buffers using this texture
            // (indirectly via BindGroup or views) are already completed.
            texture.destroy();
        }
        lock(&self.cached_single_texture_bind_groups).clear();
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// The wgpu half of `texture`, if it is a wgpu texture.
#[must_use]
pub fn as_wgpu_texture(texture: &Texture) -> Option<&WgpuTexture> {
    texture.backend().as_any().downcast_ref::<WgpuTexture>()
}
