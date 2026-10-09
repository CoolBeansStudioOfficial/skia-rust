// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/ImageProvider.h (the interface), and the
// `DefaultImageProvider` of src/gpu/graphite/Recorder.cpp#L84-L98 (chrome/m156), which the recorder
// uses when the client gives no provider.

//! `ImageProvider`: the hook that turns an image which is not Graphite-backed into one that is, as
//! a draw needs it (`GetGraphiteBacked`).

use skia_rust_core::image::{Image as CoreImage, RequiredProperties};
use skia_rust_core::image_info_priv::color_type_channel_flags;

use crate::gpu::gpu_types::Origin;
use crate::gpu::sk_log::skia_log_e;

use crate::graphite::image_graphite::Image;
use crate::graphite::recorder::Recorder;

/// `skgpu::graphite::ImageProvider`: returns a Graphite-backed version of an image that is not
/// Graphite-backed.
///
/// A returned image must have the original's dimensions and alpha type, a superset of its channels
/// and a top-left origin; otherwise the draw is dropped. It may have different mipmaps from the ones
/// requested: if mipmaps were requested and not returned, the sampling is reduced to linear.
// Port of: include/gpu/graphite/ImageProvider.h#L18-L70 (chrome/m156)
#[doc(alias = "skgpu::graphite::ImageProvider")]
pub trait ImageProvider: std::fmt::Debug + Send + Sync {
    /// `findOrCreate(recorder, image, requiredProps)`: `None` if the provider cannot make one.
    #[doc(alias = "findOrCreate")]
    fn find_or_create(
        &self,
        recorder: &Recorder,
        image: &CoreImage,
        required_props: RequiredProperties,
    ) -> Option<CoreImage>;
}

/// The default `ImageProvider` the recorder uses. It makes no Graphite-backed image from one that
/// is not, so the draw that needed it is dropped.
// Port of: src/gpu/graphite/Recorder.cpp#L84-L98 (chrome/m156), `DefaultImageProvider`
#[derive(Debug, Clone, Copy, Default)]
pub struct DefaultImageProvider;

impl ImageProvider for DefaultImageProvider {
    // Port of: src/gpu/graphite/Recorder.cpp#L91-L95 (chrome/m156), `findOrCreate`
    fn find_or_create(
        &self,
        _recorder: &Recorder,
        image: &CoreImage,
        _required_props: RequiredProperties,
    ) -> Option<CoreImage> {
        debug_assert!(!image.as_base().is_graphite_backed());
        None
    }
}

/// `valid_client_provided_image(clientProvided, original, requiredProps)`: whether a provider's
/// image may stand in for `original`.
// Port of: src/gpu/graphite/TextureUtils.cpp#L100-L124 (chrome/m156)
#[must_use]
pub fn valid_client_provided_image(
    client_provided: Option<&CoreImage>,
    original: &CoreImage,
) -> bool {
    let Some(client_provided) = client_provided else {
        return false;
    };
    let Some(graphite) = Image::from_core(client_provided) else {
        return false;
    };
    if original.dimensions() != client_provided.dimensions()
        || original.alpha_type() != client_provided.alpha_type()
    {
        return false;
    }
    let orig_channels = color_type_channel_flags(original.color_type());
    let client_channels = color_type_channel_flags(client_provided.color_type());
    if (orig_channels & client_channels) != orig_channels {
        return false;
    }
    if graphite.texture_proxy_view().origin() != Origin::TopLeft {
        skia_log_e!("Client provided image must have a TopLeft origin.");
        return false;
    }
    true
}
