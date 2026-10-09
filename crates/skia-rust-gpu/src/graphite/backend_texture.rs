// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/BackendTexture.h, src/gpu/graphite/BackendTexture.cpp,
//                   src/gpu/graphite/BackendTexturePriv.h

//! `BackendTexture`: a handle to a client-owned (or Graphite-created) backend texture.
//!
//! Skia stores the backend's `BackendTextureData` subclass inline (`SkAnySubclass`). The port
//! keeps it behind the [`BackendTextureData`] trait in an `Arc` (the data are immutable, so a
//! copy shares them). The wgpu back end's data hold `wgpu::Texture` / `wgpu::TextureView`
//! handles, which are reference counted: unlike Dawn's raw `WGPUTexture`, a `BackendTexture`
//! keeps the wgpu object alive.

use std::any::Any;
use std::fmt;
use std::sync::Arc;

use skia_rust_core::size::ISize;

use crate::gpu::gpu_types::BackendApi;
use crate::graphite::texture_info::TextureInfo;

/// The backend half of a [`BackendTexture`].
// Port of: src/gpu/graphite/BackendTexturePriv.h#L20-L40 (chrome/m156)
pub trait BackendTextureData: Send + Sync + fmt::Debug + 'static {
    /// `type()`.
    fn backend(&self) -> BackendApi;
    /// `equal(that)`: `that` has data of the same backend.
    fn equal(&self, that: &dyn BackendTextureData) -> bool;
    /// For downcasting to the concrete backend data.
    fn as_any(&self) -> &dyn Any;
}

/// A handle to a backend texture.
#[doc(alias = "skgpu::graphite::BackendTexture")]
#[derive(Clone, Debug, Default)]
pub struct BackendTexture {
    dimensions: ISize,
    info: TextureInfo,
    data: Option<Arc<dyn BackendTextureData>>,
}

impl BackendTexture {
    /// `BackendTexture()`: an invalid texture.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `isValid()`.
    #[doc(alias = "isValid")]
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.info.is_valid()
    }

    /// `backend()`.
    #[must_use]
    pub fn backend(&self) -> BackendApi {
        self.info.backend()
    }

    /// `dimensions()`.
    #[must_use]
    pub fn dimensions(&self) -> ISize {
        self.dimensions
    }

    /// `info()`.
    #[must_use]
    pub fn info(&self) -> TextureInfo {
        self.info.clone()
    }
}

impl PartialEq for BackendTexture {
    // Port of: src/gpu/graphite/BackendTexture.cpp#L40-L52 (chrome/m156)
    fn eq(&self, that: &Self) -> bool {
        if !self.is_valid() || !that.is_valid() {
            return false;
        }

        if self.dimensions != that.dimensions || self.info != that.info {
            return false;
        }
        match (&self.data, &that.data) {
            (Some(a), Some(b)) => a.equal(&**b),
            _ => false,
        }
    }
}

/// `BackendTexturePriv`: Graphite-internal construction and access of [`BackendTexture`].
// Port of: src/gpu/graphite/BackendTexturePriv.h#L42-L58 (chrome/m156)
pub mod backend_texture_priv {
    use super::{Arc, BackendTexture, BackendTextureData, ISize, TextureInfo};

    /// `BackendTexturePriv::Make`.
    #[doc(alias = "Make")]
    #[must_use]
    pub fn make(
        dimensions: ISize,
        info: TextureInfo,
        data: impl BackendTextureData,
    ) -> BackendTexture {
        BackendTexture {
            dimensions,
            info,
            data: Some(Arc::new(data)),
        }
    }

    /// `BackendTexturePriv::GetData`.
    #[doc(alias = "GetData")]
    #[must_use]
    pub fn get_data(texture: &BackendTexture) -> Option<&dyn BackendTextureData> {
        texture.data.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_textures_are_never_equal() {
        let a = BackendTexture::new();
        assert!(!a.is_valid());
        assert_eq!(a.backend(), BackendApi::Unsupported);
        // An invalid texture is not even equal to itself.
        assert_ne!(a, a.clone());
    }
}
