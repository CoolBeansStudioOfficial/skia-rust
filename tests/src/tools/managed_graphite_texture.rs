// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/gpu/ManagedBackendTexture.cpp (ManagedGraphiteTexture, chrome/m156)

//! `sk_gpu_test::ManagedGraphiteTexture`: a Graphite backend texture the test owns, created and
//! filled from pixmaps, deleted when the value drops.
//!
//! As in Skia, `MakeUnInit` adds a finished proc to the recorder that holds a reference to the
//! texture, so the texture is not deleted until the GPU has finished the recording that uploads to
//! it, even if the wrapping image (and its release callback) is gone by then. The value is a
//! reference-counted handle (`sk_sp<ManagedGraphiteTexture>`) and holds the context's resource
//! provider (`ContextPriv::resource_provider`), not a borrow of the context, so the test can keep
//! using the context while it lives.

use std::sync::Arc;

use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::mipmap::Mipmap;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use skia_rust_gpu::gpu::ref_cnted_callback::CallbackProc;
use skia_rust_gpu::graphite::backend_texture::BackendTexture;
use skia_rust_gpu::graphite::context_priv::{ContextPriv, SharedResourceProvider};
use skia_rust_gpu::graphite::graphite_types::InsertFinishInfo;
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

/// `ManagedGraphiteTexture`: a backend texture that is deleted through the context's resource
/// provider on drop.
#[derive(Debug, Clone)]
pub struct ManagedGraphiteTexture {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
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
        let mbet = Self {
            inner: Arc::new(Inner { provider, texture }),
        };
        // `recorder->addFinishInfo({mbet->releaseContext(), FinishedProc})`
        let keep_alive = mbet.clone();
        recorder.add_finish_info(InsertFinishInfo::new(Box::new(move |_| drop(keep_alive))));
        Some(mbet)
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

        if !recorder.update_backend_texture(&mbet.inner.texture, &levels, None) {
            return None;
        }
        Some(mbet)
    }

    /// `MakeMipmappedFromPixmaps(recorder, levels, renderable, isProtected)`: a mipmapped texture
    /// of the size and color type of `levels[0]`, with each pixmap of `levels` written to the
    /// mip level of its index.
    // Port of: tools/gpu/ManagedBackendTexture.cpp#L238-L260 (chrome/m156)
    #[doc(alias = "MakeMipmappedFromPixmaps")]
    #[must_use]
    pub fn make_mipmapped_from_pixmaps(
        context: &WgpuContext,
        recorder: &mut Recorder,
        levels: &[Pixmap<'_>],
        renderable: Renderable,
        is_protected: Protected,
    ) -> Option<Self> {
        let first = levels.first()?;
        let mbet = Self::make_uninit(
            ContextPriv::resource_provider(context).clone(),
            recorder,
            first.info(),
            Mipmapped::Yes,
            renderable,
            is_protected,
        )?;
        if !recorder.update_backend_texture(&mbet.inner.texture, levels, None) {
            return None;
        }
        Some(mbet)
    }

    /// `releaseContext()`: a release callback for a wrapped texture that keeps this texture alive
    /// until the wrapping image's texture is released.
    // Port of: tools/gpu/ManagedBackendTexture.cpp#L168-L170 (chrome/m156)
    #[doc(alias = "releaseContext")]
    #[must_use]
    pub fn release_context(&self) -> CallbackProc {
        let keep_alive = self.clone();
        CallbackProc::Result(Box::new(move |_| drop(keep_alive)))
    }

    /// `texture()`: the backend texture.
    #[must_use]
    pub fn texture(&self) -> &BackendTexture {
        &self.inner.texture
    }
}

impl Drop for Inner {
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
