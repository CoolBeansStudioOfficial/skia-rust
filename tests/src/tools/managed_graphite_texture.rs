// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/gpu/ManagedBackendTexture.cpp (ManagedGraphiteTexture, chrome/m156)

//! `sk_gpu_test::ManagedGraphiteTexture`: a Graphite backend texture the test owns, created and
//! filled from pixmaps, deleted when the value drops.
//!
//! Skia's version also holds a finished-proc reference that keeps the texture alive until the GPU
//! has finished the recording that uses it. The tests here read their results back before the
//! value drops, so the texture is deleted on drop instead. The value holds the context's resource
//! provider (`ContextPriv::resource_provider`), not a borrow of the context, so the test can keep
//! using the context while it lives.

use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

/// `ManagedGraphiteTexture`: a backend texture that is deleted through the context's resource
/// provider on drop.
#[derive(Debug)]
pub struct ManagedGraphiteTexture {
    provider: SharedResourceProvider,
    texture: BackendTexture,
}

impl ManagedGraphiteTexture {
    /// `MakeUnInit(recorder, ii, mipmapped, renderable, isProtected)`: a texture of the pixmap's
    /// size and color type, not yet filled.
    // Port of: tools/gpu/ManagedBackendTexture.cpp#L178-L199 (chrome/m156)
    #[doc(alias = "MakeUnInit")]
    fn make_uninit(
        provider: SharedResourceProvider,
        recorder: &mut Recorder,
        info: &ImageInfo,
        mipmapped: Mipmapped,
        renderable: Renderable,
        is_protected: Protected,
    ) -> Option<Self> {
        let caps = recorder.priv_().caps().clone();
        let texture_info = caps.get_default_sampled_texture_info(
            info.color_type(),
            mipmapped,
            is_protected,
            renderable,
        );
        let texture = recorder.create_backend_texture(info.dimensions(), &texture_info);
        if !texture.is_valid() {
            return None;
        }
        Some(Self { provider, texture })
    }

    /// `MakeFromPixmap(recorder, src, mipmapped, renderable, isProtected)`: a texture holding
    /// `src` (and, when `mipmapped`, its generated mip levels).
    // Port of: tools/gpu/ManagedBackendTexture.cpp#L205-L236 (chrome/m156)
    #[doc(alias = "MakeFromPixmap")]
    #[must_use]
    pub fn make_from_pixmap(
        context: &WgpuContext,
        recorder: &mut Recorder,
        src: &Pixmap<'_>,
        mipmapped: Mipmapped,
        renderable: Renderable,
        is_protected: Protected,
    ) -> Option<Self> {
        let mbet = Self::make_uninit(
            ContextPriv::resource_provider(context).clone(),
            recorder,
            src.info(),
            mipmapped,
            renderable,
            is_protected,
        )?;

        // `levels({src})`: a second pixmap over the same pixels.
        let base = Pixmap::new_readonly(src.info(), src.addr()?, src.row_bytes())?;
        let mut levels = vec![base];
        // `SkMipmap::Build(src, nullptr)` computes the level contents.
        let mm = if mipmapped == Mipmapped::Yes {
            Some(Mipmap::build(src, true)?)
        } else {
            None
        };
        if let Some(mm) = &mm {
            for i in 0..mm.count_levels() {
                levels.push(mm.get_level(i)?.pixmap);
            }
        }

        if !recorder.update_backend_texture(&mbet.texture, &levels, None) {
            return None;
        }
        Some(mbet)
    }

    /// `texture()`: the backend texture.
    #[must_use]
    pub fn texture(&self) -> &BackendTexture {
        &self.texture
    }
}

impl Drop for ManagedGraphiteTexture {
    // Port of: tools/gpu/ManagedBackendTexture.cpp#L172-L176 (chrome/m156), `deleteBackendTexture`
    // with the backend check done by the caller's recorder
    fn drop(&mut self) {
        if self.texture.is_valid() {
            self.provider
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .delete_backend_texture(&self.texture);
        }
    }
}
