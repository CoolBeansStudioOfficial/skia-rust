// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/TextureUtils.cpp (`skif::GraphiteBackend`,
// `skif::MakeGraphiteBackend`)

//! Graphite's image filter backend (`skif::MakeGraphiteBackend`): filter results are Graphite
//! devices and special images, and blurs are the shader blur algorithm
//! (`SkShaderBlurAlgorithm`, core's [`ShaderBlurAlgorithm`]) drawn on the GPU.
//!
//! skia-rust: the backend holds its recorder weakly (C++ holds a raw `Recorder*`); once the
//! recorder is gone the backend makes nothing. The image filter cache
//! (`SkImageFilterCache::Create`) is not ported, as for the raster backend.

use std::rc::Weak;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::bitmap::Bitmap;
use skia_rust_core::blur_engine::{BlurAlgorithm, BlurEngine};
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::device::Device as CoreDevice;
use skia_rust_core::image::Image as CoreImage;
use skia_rust_core::image_filter_types::Backend;
use skia_rust_core::image_info::ImageInfo;
use skia_rust_core::rect::IRect;
use skia_rust_core::shader_blur_algorithm::{MAX_LINEAR_SIGMA, ShaderBlurAlgorithm};
use skia_rust_core::size::{ISize, Size};
use skia_rust_core::special_image::SpecialImage;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::tile_mode::TileMode;

use crate::gpu::backing_fit::BackingFit;
use crate::gpu::gpu_types::{Budgeted, Mipmapped};
use crate::graphite::device::Device;
use crate::graphite::image_graphite::Image;
use crate::graphite::recorder::{Recorder, RecorderInner, RecorderPriv};
use crate::graphite::resource_types::LoadOp;
use crate::graphite::special_image::make_graphite;
use crate::graphite::texture_format::read_swizzle_for_color_type;
use crate::graphite::texture_proxy_view::TextureProxyView;

/// `skif::GraphiteBackend`: the backend, its blur engine and its only blur algorithm.
// Port of: src/gpu/graphite/TextureUtils.cpp#L735-L807 (chrome/m156)
#[derive(Debug)]
pub struct GraphiteBackend {
    recorder: Weak<RecorderInner>,
    surface_props: SurfaceProps,
    color_type: ColorType,
}

impl GraphiteBackend {
    fn recorder(&self) -> Option<Recorder> {
        self.recorder.upgrade().map(Recorder::from_inner)
    }

    // `Device::Make(fRecorder, imageInfo, kYes, kNo, kApprox, props, kDiscard, label)`.
    fn make_scratch_device(
        &self,
        image_info: &ImageInfo,
        props: &SurfaceProps,
        label: &str,
    ) -> Option<Box<dyn CoreDevice>> {
        let recorder = self.recorder()?;
        Device::make_with_info(
            Some(&recorder),
            image_info,
            Budgeted::Yes,
            Mipmapped::No,
            BackingFit::Approx,
            props,
            LoadOp::Discard,
            label,
            /*register_with_recorder=*/ true,
            /*allow_unpremul=*/ false,
        )
        .map(|device| Box::new(device) as Box<dyn CoreDevice>)
    }
}

impl Backend for GraphiteBackend {
    // Port of: src/gpu/graphite/TextureUtils.cpp#L750-L766 (chrome/m156)
    fn make_device(
        &self,
        size: ISize,
        color_space: Option<ColorSpace>,
        props: Option<&SurfaceProps>,
    ) -> Option<Box<dyn CoreDevice>> {
        let image_info = ImageInfo::new(size, self.color_type, AlphaType::Premul, color_space);
        let props = props.copied().unwrap_or(self.surface_props);
        self.make_scratch_device(&image_info, &props, "ImageFilterResult")
    }

    // Port of: src/gpu/graphite/TextureUtils.cpp#L768-L770 (chrome/m156)
    fn make_image(&self, subset: &IRect, image: &CoreImage) -> Option<SpecialImage> {
        let recorder = self.recorder();
        make_graphite(
            recorder.as_ref(),
            subset,
            Some(image.clone()),
            &self.surface_props,
        )
    }

    // Port of: src/gpu/graphite/TextureUtils.cpp#L772-L786 (chrome/m156)
    fn get_cached_bitmap(&self, data: &Bitmap) -> Option<CoreImage> {
        let recorder = self.recorder();
        let proxy =
            RecorderPriv::create_cached_proxy(recorder.as_ref(), data, "ImageFilterCachedBitmap")?;

        let color_info = data.info().color_info();
        let swizzle = read_swizzle_for_color_type(color_info.color_type(), proxy.format());
        Some(Image::new(TextureProxyView::new(Some(proxy), swizzle), color_info).into_core())
    }

    fn surface_props(&self) -> &SurfaceProps {
        &self.surface_props
    }

    fn color_type(&self) -> ColorType {
        self.color_type
    }

    // Port of: src/gpu/graphite/TextureUtils.cpp#L788 (chrome/m156)
    fn blur_engine(&self) -> Option<&dyn BlurEngine> {
        Some(self)
    }
}

impl BlurEngine for GraphiteBackend {
    // Port of: src/gpu/graphite/TextureUtils.cpp#L790-L795 (chrome/m156)
    fn find_algorithm(&self, _sigma: Size, _color_type: ColorType) -> Option<&dyn BlurAlgorithm> {
        // The runtime effect blurs handle all tilemodes and color types
        Some(self)
    }
}

impl BlurAlgorithm for GraphiteBackend {
    // Port of: src/core/SkBlurEngine.h#L136-L137 (chrome/m156)
    fn max_sigma(&self) -> f32 {
        MAX_LINEAR_SIGMA
    }

    fn supports_only_decal_tiling(&self) -> bool {
        false
    }

    fn blur(
        &self,
        sigma: Size,
        src: &SpecialImage,
        src_rect: IRect,
        tile_mode: TileMode,
        dst_rect: IRect,
    ) -> Option<SpecialImage> {
        self.shader_blur(sigma, src, src_rect, tile_mode, dst_rect)
    }
}

impl ShaderBlurAlgorithm for GraphiteBackend {
    // Port of: src/gpu/graphite/TextureUtils.cpp#L798-L807 (chrome/m156)
    fn make_device(&self, image_info: &ImageInfo) -> Option<Box<dyn CoreDevice>> {
        self.make_scratch_device(image_info, &self.surface_props, "EvalBlurTexture")
    }
}

/// `skif::MakeGraphiteBackend(recorder, surfaceProps, colorType)`.
// Port of: src/gpu/graphite/TextureUtils.cpp#L812-L817 (chrome/m156)
#[doc(alias = "MakeGraphiteBackend")]
#[must_use]
pub fn make_graphite_backend(
    recorder: &Recorder,
    surface_props: &SurfaceProps,
    color_type: ColorType,
) -> Arc<dyn Backend> {
    // The backend is `!Send` like the recorder it holds; core shares backends as `Arc<dyn
    // Backend>` (the raster backend is `Send + Sync`).
    #[allow(clippy::arc_with_non_send_sync)]
    Arc::new(GraphiteBackend {
        recorder: recorder.downgrade(),
        surface_props: *surface_props,
        color_type,
    })
}
