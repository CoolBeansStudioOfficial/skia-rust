// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/CoverageMaskShape.h

//! [`CoverageMaskShape`]: a shape whose per-pixel coverage comes from a texture (a path atlas
//! entry, or a mask image drawn by `Device::drawCoverageMask`).

use std::sync::Arc;

use skia_rust_core::m44::M44;

use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::raster_path_utils::Half2;
use crate::graphite::texture_proxy::TextureProxy;

/// `CoverageMaskShape::MaskInfo`: where the mask sits in its texture, and its size.
// Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L36-L46 (chrome/m156)
#[doc(alias = "CoverageMaskShape::MaskInfo")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MaskInfo {
    /// The texture-relative integer UV coordinates of the top-left corner of the mask bounds,
    /// including a 1-pixel border.
    pub texture_origin: Half2,
    /// The width and height of the mask bounds in device coordinates, including the border.
    pub mask_size: Half2,
}

/// `skgpu::graphite::CoverageMaskShape`.
// Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L48-L112 (chrome/m156)
#[doc(alias = "skgpu::graphite::CoverageMaskShape")]
#[derive(Clone, Debug)]
pub struct CoverageMaskShape {
    /// Keeps the texture alive.
    texture_proxy: Arc<TextureProxy>,
    mask_to_device: M44,
    inverted: bool,
    mask_info: MaskInfo,
}

impl CoverageMaskShape {
    /// `CoverageMaskShape(shape, proxy, maskToDevice, maskInfo)`.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L56-L63 (chrome/m156)
    #[must_use]
    pub fn new(
        shape: &Shape,
        proxy: Arc<TextureProxy>,
        mask_to_device: M44,
        mask_info: MaskInfo,
    ) -> Self {
        Self {
            texture_proxy: proxy,
            mask_to_device,
            inverted: shape.inverted(),
            mask_info,
        }
    }

    /// `bounds()`: the mask-space bounds, which are the mask size.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L76-L78 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        let (w, h) = self.mask_size();
        Rect::new(0.0, 0.0, f32::from(w), f32::from(h))
    }

    /// `maskToDevice()`: the transform from the mask's texture space to device space.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L82 (chrome/m156)
    #[must_use]
    pub fn mask_to_device(&self) -> &M44 {
        &self.mask_to_device
    }

    /// `textureOrigin()`.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L86 (chrome/m156)
    #[must_use]
    pub fn texture_origin(&self) -> Half2 {
        self.mask_info.texture_origin
    }

    /// `maskSize()`.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L90 (chrome/m156)
    #[must_use]
    pub fn mask_size(&self) -> Half2 {
        self.mask_info.mask_size
    }

    /// `textureProxy()`: the texture the shape is rendered to.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L94 (chrome/m156)
    #[must_use]
    pub fn texture_proxy(&self) -> &TextureProxy {
        &self.texture_proxy
    }

    /// The shared handle to the texture the shape is rendered to.
    #[must_use]
    pub fn texture_proxy_arc(&self) -> &Arc<TextureProxy> {
        &self.texture_proxy
    }

    /// `inverted()`: whether the shape is painted according to an inverse fill rule.
    // Port of: src/gpu/graphite/geom/CoverageMaskShape.h#L98 (chrome/m156)
    #[must_use]
    pub fn inverted(&self) -> bool {
        self.inverted
    }
}
