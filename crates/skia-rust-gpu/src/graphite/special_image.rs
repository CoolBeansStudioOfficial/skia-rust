// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/SpecialImage_Graphite.{h,cpp}

//! `SkSpecialImages::MakeGraphite`: a special image over a Graphite-backed image.
//!
//! skia-rust: `skgpu::graphite::SpecialImage` is the texture flavor of core's
//! [`SpecialImage`] (`docs/design/gpu.md` §5.5): it only wraps a Graphite-backed `SkImage`, so the
//! flavor is in core and this module converts the image first.

use skia_rust_core::image::Image as CoreImage;
use skia_rust_core::rect::{Contains, IRect};
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;

use crate::graphite::recorder::Recorder;
use crate::graphite::texture_utils::get_graphite_backed;

/// `SkSpecialImages::MakeGraphite(recorder, subset, image, props)`: a special image of `subset`
/// of `image`, converted to a Graphite-backed image with the recorder's image provider when it is
/// not one already.
///
/// `recorder` can be `None` when wrapping a Graphite-backed image, since there's no work that
/// needs to be added (snapping a special image from a device that has been marked immutable and
/// abandoned its recorder).
// Port of: src/gpu/graphite/SpecialImage_Graphite.cpp#L56-L88 (chrome/m156)
#[doc(alias = "MakeGraphite")]
#[must_use]
pub fn make_graphite(
    recorder: Option<&Recorder>,
    subset: &IRect,
    image: Option<CoreImage>,
    props: &SurfaceProps,
) -> Option<SpecialImage> {
    let mut image = image?;
    if subset.is_empty() {
        return None;
    }
    debug_assert!(image.bounds().contains(subset));

    // Use the Recorder's client ImageProvider to convert to a graphite-backed image when
    // possible, but this does not necessarily mean the provider will produce a valid image.
    if !image.as_base().is_graphite_backed() {
        let recorder = recorder?;
        let (graphite_image, _) = get_graphite_backed(recorder, &image, SamplingOptions::default());
        image = graphite_image?;
    }
    SpecialImage::make_from_texture_image(subset, image, props)
}
