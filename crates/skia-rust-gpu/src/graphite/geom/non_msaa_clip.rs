// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/NonMSAAClip.h (the non-legacy branch)

//! `skgpu::graphite::AnalyticClip`: a rect or rrect clip with any circular or rectangular corners
//! under an affine transformation.
//!
//! [`AtlasClip`] is a clip whose mask lives in an atlas texture and [`NonMSAAClip`] holds both.
//! The atlas half is filled by the recorder's `ClipAtlasManager` (G12a); the
//! `ClipStack` produces a non-empty `AtlasClip` only with a clip atlas (see `clip_stack`).

use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect as SkRect};
use skia_rust_simd::vx::Float4;

use crate::graphite::texture_proxy::TextureProxy;

/// `AnalyticClip`: the shader inputs for an analytic clip. The defaults produce no clip.
// Port of: src/gpu/graphite/geom/NonMSAAClip.h#L51-L62 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticClip")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AnalyticClip {
    /// Bounds of the clip (`fBounds`).
    pub bounds: SkRect,
    /// Corner radii (`fRadii`, an `SkV4`).
    pub radii: Float4,
    /// The device-to-clip local 2x2 (`fXform`, an `SkV4`).
    pub xform: Float4,
    /// Inversion matches the `ClipStack`'s convention for depth-only draws: an inverse fill is used
    /// for intersection clips and a regular fill is for difference clips (`fInverted`).
    pub inverted: bool,
}

impl Default for AnalyticClip {
    // Port of: src/gpu/graphite/geom/NonMSAAClip.h#L53-L58 (chrome/m156)
    fn default() -> Self {
        Self {
            bounds: SkRect {
                left: 0.0,
                top: 0.0,
                right: 0.0,
                bottom: 0.0,
            },
            radii: Float4::new(0.0, 0.0, 0.0, 0.0),
            xform: Float4::new(1.0, 0.0, 0.0, 1.0),
            inverted: false,
        }
    }
}

impl AnalyticClip {
    /// `isEmpty()`: a clip with empty bounds and no inversion produces no coverage.
    // Port of: src/gpu/graphite/geom/NonMSAAClip.h#L61 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bounds.is_empty() && !self.inverted
    }
}

/// `AtlasClip`: a clip that uses a mask in an atlas.
// Port of: src/gpu/graphite/geom/NonMSAAClip.h#L68-L75 (chrome/m156)
#[doc(alias = "skgpu::graphite::AtlasClip")]
#[derive(Clone, Debug, Default)]
pub struct AtlasClip {
    /// `fMaskBounds`: the bounds of the mask area, in device space.
    pub mask_bounds: IRect,
    /// `fOutPos`: where the mask was placed in the atlas.
    pub out_pos: IPoint,
    /// `fAtlasTexture`.
    pub atlas_texture: Option<Arc<TextureProxy>>,
}

impl AtlasClip {
    /// `isEmpty()`: no atlas texture means no atlas clip.
    // Port of: src/gpu/graphite/geom/NonMSAAClip.h#L73 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.atlas_texture.is_none()
    }
}

/// `NonMSAAClip`: the combined non-MSAA clip structure.
// Port of: src/gpu/graphite/geom/NonMSAAClip.h#L80-L86 (chrome/m156)
#[doc(alias = "skgpu::graphite::NonMSAAClip")]
#[derive(Clone, Debug, Default)]
pub struct NonMSAAClip {
    /// `fAnalyticClip`.
    pub analytic_clip: AnalyticClip,
    /// `fAtlasClip`.
    pub atlas_clip: AtlasClip,
}

impl NonMSAAClip {
    /// `isEmpty()`.
    // Port of: src/gpu/graphite/geom/NonMSAAClip.h#L85 (chrome/m156)
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.analytic_clip.is_empty() && self.atlas_clip.is_empty()
    }
}
