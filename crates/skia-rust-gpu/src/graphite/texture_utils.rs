// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureUtils.cpp (ComputeSize and MakeBitmapProxyView)

//! Texture helpers (`TextureUtils.h`): `ComputeSize`, which the resource model needs, and
//! `MakeBitmapProxyView` for unmipmapped bitmaps, which `RecorderPriv::CreateCachedProxy` uses.
//!
//! Not ported yet: `MakeBitmapProxyView` for `Mipmapped::Yes` (it builds an `SkMipmap` of the
//! bitmap), and the image helpers that come with images and surfaces (G10d).

use std::sync::Arc;

use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::compressed_data_utils::compressed_format_data_size;
use skia_rust_core::image_info_priv::image_info_is_valid;
use skia_rust_core::rect::IRect;
use skia_rust_core::size::ISize;
use skia_rust_core::texture_compression_type::TextureCompressionType;

use crate::gpu::gpu_types::{Budgeted, Mipmapped, Renderable};
use crate::gpu::sk_log::skia_log_e;
use crate::graphite::context_priv::SharedResourceProvider;
use crate::graphite::recorder::Recorder;
use crate::graphite::task::upload_task::{ImageUploadContext, MipLevel, UploadSource};
use crate::graphite::texture_format::{
    are_color_type_and_format_compatible, read_swizzle_for_color_type,
    texture_format_bytes_per_block, texture_format_compression_type,
};
use crate::graphite::texture_info::{TextureInfo, texture_info_priv};
use crate::graphite::texture_proxy::TextureProxy;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `ComputeSize`: the approximate GPU memory a texture of `dimensions` described by `info` uses.
// Port of: src/gpu/graphite/TextureUtils.cpp#L370-L393 (chrome/m156)
#[doc(alias = "ComputeSize")]
#[allow(clippy::cast_sign_loss)] // mirrors the (size_t) casts on non-negative dimensions
#[must_use]
pub fn compute_size(dimensions: ISize, info: &TextureInfo) -> usize {
    let format = texture_info_priv::view_format(info);
    let compression = texture_format_compression_type(format);

    let color_size = if compression == TextureCompressionType::None {
        // TODO(b/401016699): Add logic to handle multiplanar formats
        let bytes_per_pixel = texture_format_bytes_per_block(format);

        (dimensions.width as usize)
            .wrapping_mul(dimensions.height as usize)
            .wrapping_mul(bytes_per_pixel as usize)
    } else {
        compressed_format_data_size(compression, dimensions, info.mipmapped() == Mipmapped::Yes)
    };

    // size_t arithmetic wraps in C++.
    let mut final_size = color_size.wrapping_mul(info.sample_count() as usize);

    if info.mipmapped() == Mipmapped::Yes {
        final_size = final_size.wrapping_add(color_size / 3);
    }
    final_size
}

/// `MakeBitmapProxyView()` for an unmipmapped bitmap: a budgeted texture the bitmap's pixels are
/// uploaded to, through the recorder's root upload list (or on the host when that is cheaper).
///
/// The proxy cache always asks for `Mipmapped::kNo`, so the mipmapped path (which builds an
/// `SkMipmap` of the bitmap) is not ported.
// Port of: src/gpu/graphite/TextureUtils.cpp#L254-L343 (chrome/m156), with `mipmapped == kNo`
#[doc(alias = "MakeBitmapProxyView")]
#[must_use]
///
/// The resource provider is locked only to create the proxy: the upload needs it again (its
/// transfer buffers come from the provider), so the caller must not hold the lock.
pub fn make_bitmap_proxy_view(
    recorder: &Recorder,
    resource_provider: &SharedResourceProvider,
    bitmap: &Bitmap,
    label: &str,
) -> Option<TextureProxyView> {
    let priv_ = recorder.priv_();
    let caps = Arc::clone(priv_.caps());
    let ct = bitmap.color_type();

    let texture_info = caps.get_default_sampled_texture_info(
        ct,
        Mipmapped::No,
        priv_.is_protected(),
        Renderable::No,
    );
    if !texture_info.is_valid() {
        return None;
    }
    if !image_info_is_valid(bitmap.info()) {
        return None;
    }

    // Create proxy.
    let proxy = {
        let mut provider = resource_provider
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        TextureProxy::make(
            caps.max_texture_size(),
            &mut provider,
            bitmap.dimensions(),
            &texture_info,
            Budgeted::Yes,
            label,
        )?
    };

    let format = proxy.format();
    debug_assert!(are_color_type_and_format_compatible(ct, format));

    let swizzle = read_swizzle_for_color_type(ct, format);
    let view = TextureProxyView::new(Some(proxy), swizzle);

    // Src and dst colorInfo are the same.
    let color_info = bitmap.info().color_info().clone();
    // Add upload to the root upload list. These bitmaps are uploaded to unique textures so there
    // is no need to coordinate resource sharing. It is better to then group them into a single
    // task at the start of the Recording.
    let dimensions = IRect::from_size(bitmap.dimensions());
    let pixmap = bitmap.pixmap();
    let levels = [MipLevel {
        pixels: pixmap.addr(),
        row_bytes: bitmap.row_bytes(),
    }];
    // The upload source takes the view's proxy as the unique holder while it is uploaded; the
    // C++ moves `view` into it.
    let upload_source =
        UploadSource::make(&*caps, &view, &color_info, &color_info, &levels, dimensions);
    drop(view);
    if !upload_source.is_valid() {
        skia_log_e!("MakeBitmapProxyView: Could not create UploadSource");
        return None;
    }

    if upload_source.attempt_upload_on_host() {
        return Some(upload_source.view().clone());
    }

    // Otherwise it failed or was unavailable, so use a task on the root upload list.
    let upload_buffer_manager = priv_.upload_buffer_manager().clone();
    let mut upload_buffer_manager = upload_buffer_manager.borrow_mut();
    let uploaded = priv_.root_upload_list().borrow_mut().record_upload(
        &*caps,
        &mut upload_buffer_manager,
        &upload_source,
        Some(Box::new(ImageUploadContext)),
    );
    if !uploaded {
        skia_log_e!("MakeBitmapProxyView: Could not create UploadInstance");
        return None;
    }

    Some(upload_source.view().clone())
}
