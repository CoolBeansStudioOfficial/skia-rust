// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/gpu/BackendTextureImageFactory.cpp (chrome/m156)

//! `sk_gpu_test::MakeBackendTextureImage`: an image over a backend texture holding a pixmap, made
//! for a Graphite recorder. The texture is owned by the image's release callback.

use std::sync::Arc;

use skia_rust_core::color_type::ColorType;
use skia_rust_core::image::Image as CoreImage;
use skia_rust_core::pixmap::Pixmap;
use skia_rust_gpu::gpu::gpu_types::{Mipmapped, Origin, Protected, Renderable};
use skia_rust_gpu::graphite::image_factories::{
    wrap_texture_for_alpha_type, wrap_texture_with_release,
};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::wgpu::WgpuContext;

use crate::tools::managed_graphite_texture::ManagedGraphiteTexture;

/// `MakeBackendTextureImage(recorder, pixmap, isMipmapped, isRenderable, origin, isProtected)`.
// Port of: tools/gpu/BackendTextureImageFactory.cpp#L91-L129 (chrome/m156)
#[must_use]
pub fn make_backend_texture_image(
    context: &WgpuContext,
    recorder: &mut Recorder,
    pixmap: &Pixmap<'_>,
    is_mipmapped: Mipmapped,
    is_renderable: Renderable,
    origin: Origin,
    is_protected: Protected,
) -> Option<CoreImage> {
    let mbet = ManagedGraphiteTexture::make_from_pixmap(
        context,
        recorder,
        pixmap,
        is_mipmapped,
        is_renderable,
        is_protected,
    )?;
    let mbet = Arc::new(mbet);

    let info = pixmap.info();
    let color_space = info.color_info().color_space_ref().cloned();
    // TODO(b/476410476) in Skia: the new WrapTexture API that does not take a color type has no
    // way to be interpreted as gray, so the gray color type is wrapped with its color type.
    if info.color_type() == ColorType::Gray8 {
        wrap_texture_with_release(
            recorder,
            mbet.texture(),
            info.color_type(),
            info.alpha_type(),
            color_space,
            origin,
            Some(mbet.release_context()),
        )
    } else {
        wrap_texture_for_alpha_type(
            recorder,
            mbet.texture(),
            info.alpha_type(),
            color_space,
            origin,
            Some(mbet.release_context()),
        )
    }
}
