// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/dawn/DawnBackendTexture.cpp,
//                   include/gpu/graphite/dawn/DawnGraphiteTypes.h (BackendTextures::MakeDawn)

//! `DawnBackendTexture` on wgpu: [`BackendTexture`]s that wrap a `wgpu::Texture` or a
//! `wgpu::TextureView`.
//!
//! Dawn's `BackendTexture` does not retain the `WGPUTexture`; the client keeps it alive. The wgpu
//! handles are reference counted, so the `BackendTexture` here keeps the texture alive.

use std::any::Any;

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::BackendApi;
use crate::graphite::backend_texture::{BackendTexture, BackendTextureData, backend_texture_priv};
use crate::graphite::wgpu::texture_info::{WgpuTextureInfo, texture_infos};

/// The wgpu data of a [`BackendTexture`]: a texture, or a texture view.
// Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L16-L44 (chrome/m156)
#[derive(Clone, Debug)]
pub struct WgpuBackendTextureData {
    texture: Option<wgpu::Texture>,
    texture_view: Option<wgpu::TextureView>,
}

impl BackendTextureData for WgpuBackendTextureData {
    fn backend(&self) -> BackendApi {
        BackendApi::Dawn
    }

    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L35-L43 (chrome/m156)
    fn equal(&self, that: &dyn BackendTextureData) -> bool {
        debug_assert_eq!(that.backend(), BackendApi::Dawn);
        that.as_any().downcast_ref::<Self>().is_some_and(|other| {
            self.texture == other.texture && self.texture_view == other.texture_view
        })
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

/// `skgpu::graphite::BackendTextures` for the wgpu back end.
pub mod backend_textures {
    use super::{
        BackendApi, BackendTexture, ISize, WgpuBackendTextureData, WgpuTextureInfo,
        backend_texture_priv, texture_infos,
    };

    /// `BackendTextures::MakeDawn(WGPUTexture)`: a `BackendTexture` from a `wgpu::Texture`. The
    /// texture info is queried from the texture.
    ///
    /// This is the recommended way of specifying a `BackendTexture` for wgpu. Any `Image` or
    /// `Surface` that wraps the `BackendTexture` keeps the texture alive.
    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L58-L66 (chrome/m156)
    #[doc(alias = "MakeDawn")]
    #[must_use]
    #[allow(clippy::cast_possible_wrap)] // texture sizes are at most 2^31 - 1
    pub fn make_wgpu(texture: &wgpu::Texture) -> BackendTexture {
        backend_texture_priv::make(
            ISize::new(texture.width() as i32, texture.height() as i32),
            texture_infos::make_wgpu(&WgpuTextureInfo::from_texture(texture)),
            WgpuBackendTextureData {
                texture: Some(texture.clone()),
                texture_view: None,
            },
        )
    }

    /// `BackendTextures::MakeDawn(planeDimensions, info, WGPUTexture)`: a `BackendTexture` for
    /// `texture` with the given dimensions and info.
    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L68-L82 (chrome/m156)
    #[doc(alias = "MakeDawn")]
    #[must_use]
    pub fn make_wgpu_with_info(
        plane_dimensions: ISize,
        info: &WgpuTextureInfo,
        texture: &wgpu::Texture,
    ) -> BackendTexture {
        // wgpu has no multiplanar textures.
        debug_assert_eq!(info.aspect, wgpu::TextureAspect::All);
        backend_texture_priv::make(
            plane_dimensions,
            texture_infos::make_wgpu(info),
            WgpuBackendTextureData {
                texture: Some(texture.clone()),
                texture_view: None,
            },
        )
    }

    /// `BackendTextures::MakeDawn(dimensions, info, WGPUTextureView)`: a `BackendTexture` from
    /// a `wgpu::TextureView`. Texture dimensions and info have to be provided.
    ///
    /// Using a texture view rather than a texture is less efficient for operations that require
    /// buffer transfers to or from the texture: an intermediate copy is required. Use it only
    /// where a texture is unavailable, in particular when rendering to a surface texture.
    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L84-L91 (chrome/m156)
    #[doc(alias = "MakeDawn")]
    #[must_use]
    pub fn make_wgpu_from_view(
        dimensions: ISize,
        info: &WgpuTextureInfo,
        texture_view: &wgpu::TextureView,
    ) -> BackendTexture {
        backend_texture_priv::make(
            dimensions,
            texture_infos::make_wgpu(&strip_copy_usage(info)),
            WgpuBackendTextureData {
                texture: None,
                texture_view: Some(texture_view.clone()),
            },
        )
    }

    /// When we only have a texture view we can't actually take advantage of these usage bits
    /// because they require having the texture.
    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L50-L56 (chrome/m156)
    fn strip_copy_usage(info: &WgpuTextureInfo) -> WgpuTextureInfo {
        let mut result = *info;
        result.usage &= !(wgpu::TextureUsages::COPY_DST | wgpu::TextureUsages::COPY_SRC);
        result
    }

    /// `BackendTextures::GetDawnTexturePtr()`: the wgpu texture of a wgpu `BackendTexture`.
    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L93-L101 (chrome/m156)
    #[doc(alias = "GetDawnTexturePtr")]
    #[must_use]
    pub fn get_wgpu_texture(tex: &BackendTexture) -> Option<wgpu::Texture> {
        data(tex)?.texture.clone()
    }

    /// `BackendTextures::GetDawnTextureViewPtr()`: the wgpu texture view of a wgpu
    /// `BackendTexture`.
    // Port of: src/gpu/graphite/dawn/DawnBackendTexture.cpp#L103-L111 (chrome/m156)
    #[doc(alias = "GetDawnTextureViewPtr")]
    #[must_use]
    pub fn get_wgpu_texture_view(tex: &BackendTexture) -> Option<wgpu::TextureView> {
        data(tex)?.texture_view.clone()
    }

    fn data(tex: &BackendTexture) -> Option<&WgpuBackendTextureData> {
        if !tex.is_valid() || tex.backend() != BackendApi::Dawn {
            return None;
        }
        backend_texture_priv::get_data(tex)?
            .as_any()
            .downcast_ref::<WgpuBackendTextureData>()
    }
}
