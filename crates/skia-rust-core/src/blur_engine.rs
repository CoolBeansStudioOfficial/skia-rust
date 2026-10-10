// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkBlurEngine.h (the backend-agnostic interface and sigma helpers)

//! `SkBlurEngine`: the backend's blur algorithms, used by image-filter blurs through
//! [`crate::image_filter_result::FilterResult`]. The raster engine is in `skia-rust-raster`.

use crate::color_type::ColorType;
use crate::rect::IRect;
use crate::size::Size;
use crate::special_image::SpecialImage;
use crate::tile_mode::TileMode;

/// Any sigmas smaller than this are effectively an identity blur so can skip convolution at a
/// higher level.
// Port of: src/core/SkBlurEngine.h#L38-L46 (chrome/m156)
#[doc(alias = "IsEffectivelyIdentity")]
#[must_use]
pub fn is_effectively_identity(sigma: f32) -> bool {
    sigma <= 0.03
}

/// The pixel radius such that pixels outside it would have an insignificant contribution to the
/// blurred value (`SigmaToRadius`).
// Port of: src/core/SkBlurEngine.h#L69-L73 (chrome/m156)
#[doc(alias = "SigmaToRadius")]
#[must_use]
pub fn sigma_to_radius(sigma: f32) -> i32 {
    // skia-rust: the C++ uses sk_float_ceil2int, which is the same saturating ceiling.
    if is_effectively_identity(sigma) {
        0
    } else {
        crate::floating_point::float_ceil2int(3.0 * sigma)
    }
}

/// The successive box blur window for a given sigma, from the SVG spec
/// (`BoxBlurWindow`).
// Port of: src/core/SkBlurEngine.h#L89-L93 (chrome/m156)
#[doc(alias = "BoxBlurWindow")]
#[must_use]
pub fn box_blur_window(sigma: f32) -> i32 {
    let possible_window = crate::floating_point::float_floor2int(
        sigma * 3.0 * (2.0 * crate::floating_point::FLOAT_PI).sqrt() / 4.0 + 0.5,
    );
    possible_window.max(1)
}

/// One algorithm of a [`BlurEngine`] (`SkBlurEngine::Algorithm`).
// Port of: src/core/SkBlurEngine.h#L95-L124 (chrome/m156)
#[doc(alias = "SkBlurEngine::Algorithm")]
pub trait BlurAlgorithm {
    /// The maximum sigma that can be passed to [`blur`](Self::blur). Larger sigmas must downscale
    /// the input first.
    fn max_sigma(&self) -> f32;

    /// Whether [`blur`](Self::blur) only supports `TileMode::Decal`: other tile modes must be
    /// applied by the caller, which adjusts the source and destination bounds.
    fn supports_only_decal_tiling(&self) -> bool;

    /// A blurred image that fills `dst_rect` (relative to `src`), restricted to the pixels of
    /// `src_rect`. `None` if the algorithm cannot blur `src`.
    fn blur(
        &self,
        sigma: Size,
        src: &SpecialImage,
        src_rect: IRect,
        tile_mode: TileMode,
        dst_rect: IRect,
    ) -> Option<SpecialImage>;
}

/// A backend's blur engine (`SkBlurEngine`).
// Port of: src/core/SkBlurEngine.h#L47-L67 (chrome/m156)
#[doc(alias = "SkBlurEngine")]
pub trait BlurEngine {
    /// The algorithm for blurring `sigma` in an image of `color_type`, or `None` if the engine
    /// has none for it (`findAlgorithm`).
    fn find_algorithm(&self, sigma: Size, color_type: ColorType) -> Option<&dyn BlurAlgorithm>;
}
