// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/geom/AnalyticRRectBlurMask.h, AnalyticRRectBlurMask.cpp

//! [`AnalyticRRectBlurMask`]: the shader inputs of the analytic rounded rect blur, and its
//! accessors.
//!
//! Not ported: `AnalyticRRectBlurMask::Make`. Its CDF look-up table is `0.5 + 0.5 * std::erf(x)`
//! over 256 samples, and `std::erf` is not in the standard library (`docs/design/gpu.md`, libm).
//! Without `Make` nothing constructs this type, so `Device::draw_blurred_rrect` falls back to a
//! regular draw for rounded rects.

use std::sync::Arc;

use skia_rust_core::m44::V2;
use skia_rust_core::rrect::RRect;

use crate::graphite::geom::rect::Rect;
use crate::graphite::texture_proxy::TextureProxy;

/// `AnalyticRRectBlurMask`: the rounded rect, its local sigma and its CDF look-up texture.
// Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L17-L60 (chrome/m156)
#[doc(alias = "skgpu::graphite::AnalyticRRectBlurMask")]
#[derive(Clone, Debug)]
pub struct AnalyticRRectBlurMask {
    rrect: RRect,
    local_sigma: V2,
    cdf_lut: Arc<TextureProxy>,
}

impl AnalyticRRectBlurMask {
    /// `rrect()`.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L24 (chrome/m156)
    #[must_use]
    pub const fn rrect(&self) -> &RRect {
        &self.rrect
    }

    /// `localSigma()`.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L25 (chrome/m156)
    #[must_use]
    pub const fn local_sigma(&self) -> V2 {
        self.local_sigma
    }

    /// `refCdfProxy()`.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L26 (chrome/m156)
    #[must_use]
    pub fn ref_cdf_proxy(&self) -> Arc<TextureProxy> {
        Arc::clone(&self.cdf_lut)
    }

    /// `bounds()`: the rrect's bounds outset by the draw pad.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L27-L31 (chrome/m156)
    #[must_use]
    pub fn bounds(&self) -> Rect {
        // `Rect(fRRect.getBounds()).makeOutset(drawPad)`: l - dx, t - dy, r + dx, b + dy.
        let bounds = self.rrect.bounds();
        let pad = self.draw_pad();
        Rect::new(
            bounds.left - pad.x,
            bounds.top - pad.y,
            bounds.right + pad.x,
            bounds.bottom + pad.y,
        )
    }

    /// `drawPadX()`.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L32 (chrome/m156)
    #[must_use]
    pub fn draw_pad_x(&self) -> f32 {
        (3.0 * self.local_sigma.x).ceil()
    }

    /// `drawPadY()`.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L33 (chrome/m156)
    #[must_use]
    pub fn draw_pad_y(&self) -> f32 {
        (3.0 * self.local_sigma.y).ceil()
    }

    /// `drawPad()`.
    // Port of: src/gpu/graphite/geom/AnalyticRRectBlurMask.h#L34 (chrome/m156)
    #[must_use]
    pub fn draw_pad(&self) -> V2 {
        V2::new(self.draw_pad_x(), self.draw_pad_y())
    }
}
