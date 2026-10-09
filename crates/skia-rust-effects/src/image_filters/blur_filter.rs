// Copyright 2011 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkBlurImageFilter.cpp

//! `SkBlurImageFilter`: a Gaussian blur of its input, evaluated by the backend's blur engine
//! through [`FilterResult::Builder::blur`](skia_rust_core::image_filter_result::Builder::blur).

use skia_rust_core::blur_engine::is_effectively_identity;
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{Context, Mapping, irect_intersect_in_place, size_ceil};
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::crop;

/// This rather arbitrary-looking value results in a maximum box blur kernel size of 1000 pixels on
/// the raster path, which matches the `WebKit` and `Firefox` implementations.
// Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L183-L188 (chrome/m156)
const K_MAX_SIGMA: f32 = 532.0;

/// `std::min(a, b)` for floats: `b` only when `b < a`.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// The blur image filter (`SkBlurImageFilter`).
// Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L16-L44 (chrome/m156)
#[doc(alias = "SkBlurImageFilter")]
#[derive(Debug)]
pub struct BlurImageFilter {
    common: ImageFilterCommon,
    /// The parameter-space sigma (`fSigma`).
    sigma: Size,
    /// kDecal means no legacy tiling; it is handled by a crop filter instead (`fLegacyTileMode`).
    legacy_tile_mode: TileMode,
}

impl BlurImageFilter {
    /// `SkBlurImageFilter(sigma, input)`.
    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L16-L19 (chrome/m156)
    #[must_use]
    pub fn new(sigma: Size, input: Option<ImageFilter>) -> Self {
        BlurImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            sigma,
            legacy_tile_mode: TileMode::Decal,
        }
    }

    /// `SkBlurImageFilter(sigma, legacyTileMode, input)`.
    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L21-L25 (chrome/m156)
    #[must_use]
    pub fn new_legacy(sigma: Size, legacy_tile_mode: TileMode, input: Option<ImageFilter>) -> Self {
        BlurImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            sigma,
            legacy_tile_mode,
        }
    }

    /// `mapSigma`: the layer-space sigma, clamped, with the axes that would not blur set to 0.
    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L178-L200 (chrome/m156)
    fn map_sigma(&self, mapping: &Mapping) -> Size {
        let mut sigma = mapping.param_to_layer_size(self.sigma);
        // Clamp to the maximum sigma
        sigma = Size::new(
            std_min(sigma.width, K_MAX_SIGMA),
            std_min(sigma.height, K_MAX_SIGMA),
        );
        // Disable bluring on axes that are not finite, or that are small enough that the blur is
        // effectively an identity.
        if !sigma.width.is_finite() || is_effectively_identity(sigma.width) {
            sigma = Size::new(0.0, sigma.height);
        }
        if !sigma.height.is_finite() || is_effectively_identity(sigma.height) {
            sigma = Size::new(sigma.width, 0.0);
        }
        sigma
    }

    /// `kernelBounds`: `bounds` outset by three times the sigma.
    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L59-L65 (chrome/m156)
    fn kernel_bounds(&self, mapping: &Mapping, mut bounds: IRect) -> IRect {
        let sigma = self.map_sigma(mapping);
        let radii = size_ceil(Size::new(3.0 * sigma.width, 3.0 * sigma.height));
        bounds.outset((radii.width, radii.height));
        bounds
    }
}

impl ImageFilterBase for BlurImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L202-L232 (chrome/m156)
    #[allow(clippy::float_cmp)] // mirrors `sigma.width() == 0.f && sigma.height() == 0.f`
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let input_ctx =
            ctx.with_new_desired_output(self.kernel_bounds(ctx.mapping(), ctx.desired_output()));
        let mut child_output = self.get_child_output(0, &input_ctx);

        let sigma = self.map_sigma(ctx.mapping());
        if sigma.width == 0.0 && sigma.height == 0.0 {
            // No actual blur, so just return the input unmodified
            return child_output;
        }

        // By default, FilterResult::blur() will calculate a more optimal output automatically, so
        // convey the original output to it.
        let mut max_output = ctx.desired_output();
        if self.legacy_tile_mode != TileMode::Decal {
            // Legacy tiling output is also dependent on the original child output bounds ignoring
            // the tile mode's effect.
            max_output = self.kernel_bounds(ctx.mapping(), child_output.layer_bounds());
            if !irect_intersect_in_place(&mut max_output, &ctx.desired_output()) {
                return FilterResult::default();
            }
            // Legacy tiling applied to the input image when there was no explicit crop rect. Use
            // the child's output image's layer bounds as the crop rectangle to adjust the edge tile
            // mode without restricting the image.
            child_output = child_output.apply_crop(
                &input_ctx,
                child_output.layer_bounds(),
                self.legacy_tile_mode,
            );
        }

        // For non-legacy tiling, 'maxOutput' is equal to the desired output. For decal's it matches
        // what Builder::blur() calculates internally. For legacy tiling, however, it's dependent on
        // the original child output's bounds ignoring the tile mode's effect.
        let cropped_output = ctx.with_new_desired_output(max_output);
        let mut builder = Builder::new(&cropped_output);
        builder.add(child_output, None, ShaderFlags::NONE, default_sampling());
        builder.blur(sigma)
    }

    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L240-L247 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let required_input = self.kernel_bounds(mapping, desired_output);
        self.get_child_input_layer_bounds(0, mapping, required_input, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L249-L258 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        let child_output = self.get_child_output_layer_bounds(0, mapping, content_bounds);
        // An unbounded child output is an unbounded blur output.
        child_output.map(|child| self.kernel_bounds(mapping, child))
    }

    // Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L260-L265 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        let mut bounds = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(src),
            None => *src,
        };
        bounds.outset((self.sigma.width * 3.0, self.sigma.height * 3.0));
        bounds
    }
}

/// `SkImageFilters::Blur(sigmaX, sigmaY, tileMode, input, cropRect)`.
// Port of: src/effects/imagefilters/SkBlurImageFilter.cpp#L67-L96 (chrome/m156)
#[doc(alias = "Blur")]
#[must_use]
pub fn blur(
    sigma_x: f32,
    sigma_y: f32,
    tile_mode: TileMode,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    if !sigma_x.is_finite() || !sigma_y.is_finite() || sigma_x < 0.0 || sigma_y < 0.0 {
        // Non-finite or negative sigmas are error conditions. We allow 0 sigma for X and/or Y for
        // 1D blurs; onFilterImage() will detect when no visible blurring would occur based on the
        // Context mapping.
        return None;
    }
    let sigma = Size::new(sigma_x, sigma_y);

    // Temporarily allow tiling with no crop rect
    if tile_mode != TileMode::Decal && crop_rect.is_none() {
        return Some(ImageFilter::from_base(BlurImageFilter::new_legacy(
            sigma, tile_mode, input,
        )));
    }

    // The 'tileMode' behavior is not well-defined if there is no crop. We only apply it if there is
    // a provided 'cropRect'.
    let mut filter = input;
    if let Some(rect) = crop_rect
        && tile_mode != TileMode::Decal
    {
        // Historically the input image was restricted to the cropRect when tiling was not
        // kDecal, so that the kernel evaluated the tiled edge conditions, while a kDecal crop only
        // affected the output.
        filter = crop(&rect, tile_mode, filter);
    }
    filter = Some(ImageFilter::from_base(BlurImageFilter::new(sigma, filter)));
    if let Some(rect) = crop_rect {
        // But regardless of the tileMode, the output is always decal cropped
        filter = crop(&rect, TileMode::Decal, filter);
    }
    filter
}
