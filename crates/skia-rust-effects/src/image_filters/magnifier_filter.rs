// Copyright 2012 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkMagnifierImageFilter.cpp

//! `SkMagnifierImageFilter`: magnifies a lens-shaped region of its input, with a rounded inset
//! (the `Magnifier` known runtime effect) or as a plain rect-to-rect transform when the inset is
//! zero.

use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags};
use skia_rust_core::image_filter_types::{Context, Mapping, irect_intersect_in_place, map_rect};
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::matrix::{Matrix, ScaleToFit};
use skia_rust_core::rect::{Contains, IRect, Rect, RoundOut};
use skia_rust_core::runtime_effect::RuntimeEffectBuilder;
use skia_rust_core::sampling_options::SamplingOptions;
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::crop;

/// `std::min(a, b)` for floats: `b` only when `b < a`.
fn std_min(a: f32, b: f32) -> f32 {
    if b < a { b } else { a }
}

/// `std::max(a, b)` for floats: `b` when `a < b`.
fn std_max(a: f32, b: f32) -> f32 {
    if a < b { b } else { a }
}

/// `SkTPin(x, lo, hi)`: `std::max(lo, std::min(x, hi))`.
// Port of: include/private/SkTPin.h#L19-L21 (chrome/m156)
fn t_pin(x: f32, lo: f32, hi: f32) -> f32 {
    std_max(lo, std_min(x, hi))
}

/// `skif::LayerSpace<SkMatrix>::RectToRect`: the matrix from `from` to `to`, or the identity.
// Port of: src/core/SkImageFilterTypes.h#L498-L501 (chrome/m156)
fn rect_to_rect(from: &Rect, to: &Rect) -> Matrix {
    Matrix::rect_to_rect_or_identity(from, to, ScaleToFit::Fill)
}

/// `make_magnifier_shader`: the `Magnifier` known runtime effect over `input`.
// Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L140-L161 (chrome/m156)
fn make_magnifier_shader(
    input: Shader,
    lens_bounds: &Rect,
    zoom_xform: &Matrix,
    inset: Size,
) -> Option<Shader> {
    let magnifier_effect = get_known_runtime_effect(StableKey::Magnifier)?;
    let mut builder = RuntimeEffectBuilder::new(magnifier_effect.clone());
    builder.child("src").assign(input);
    builder.uniform("lensBounds").set_f32(&[
        lens_bounds.left(),
        lens_bounds.top(),
        lens_bounds.right(),
        lens_bounds.bottom(),
    ]);
    // Tx, Ty, Sx, Sy: rc(0,2), rc(1,2), rc(0,0), rc(1,1) of the row-major matrix.
    builder.uniform("zoomXform").set_f32(&[
        zoom_xform.get(2usize),
        zoom_xform.get(5usize),
        zoom_xform.get(0usize),
        zoom_xform.get(4usize),
    ]);
    builder
        .uniform("invInset")
        .set_f32(&[1.0 / inset.width, 1.0 / inset.height]);
    builder.make_shader(None)
}

/// The magnifier image filter (`SkMagnifierImageFilter`).
// Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L17-L47 (chrome/m156)
#[doc(alias = "SkMagnifierImageFilter")]
#[derive(Debug)]
pub struct MagnifierImageFilter {
    common: ImageFilterCommon,
    lens_bounds: Rect,
    zoom_amount: f32,
    inset: f32,
    sampling: SamplingOptions,
}

impl ImageFilterBase for MagnifierImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L163-L227 (chrome/m156)
    fn on_filter_image(&self, context: &Context<'_>) -> FilterResult {
        let mapping = context.mapping();

        // These represent the full lens bounds and the ideal zoom center if everything is visible.
        let lens_bounds = mapping.param_to_layer_rect(&self.lens_bounds);
        let mut zoom_center = lens_bounds.center();

        // When magnifying near the edge of the screen, part of the lens bounds may be offscreen.
        // The auto-sizing's goal is to cover the visible portion of the lens bounds.
        let mut visible_lens_bounds = lens_bounds;
        if !visible_lens_bounds.intersect(Rect::from_irect(context.desired_output())) {
            return FilterResult::default();
        }

        // We pre-emptively fit the zoomed-in src rect to what we expect the child input filter to
        // produce.
        let mut expected_child_output = lens_bounds;
        if let Some(output) =
            self.get_child_output_layer_bounds(0, mapping, Some(context.source().layer_bounds()))
        {
            expected_child_output = Rect::from_irect(output);
        }

        // Clamp the zoom center to be within the childOutput image
        zoom_center.x = t_pin(
            zoom_center.x,
            expected_child_output.left(),
            expected_child_output.right(),
        );
        zoom_center.y = t_pin(
            zoom_center.y,
            expected_child_output.top(),
            expected_child_output.bottom(),
        );

        // The zoom we want to apply in layer-space is fZoomAmount: this filter only supports
        // scale+translate matrices, so the paramToLayer transform of the parameter-space scale is
        // a no-op. But also clamp the maximum amount of zoom to scale half of a layer pixel to the
        // entire lens.
        let max_lens_size = std_max(1.0, std_max(lens_bounds.width(), lens_bounds.height()));
        let inv_zoom = 1.0 / std_min(self.zoom_amount, 2.0 * max_lens_size);

        // The srcRect is the bounding box of the pixels that are linearly scaled up, about
        // zoomCenter.
        let mut src_rect = Rect::new(
            lens_bounds.left() * inv_zoom + zoom_center.x * (1.0 - inv_zoom),
            lens_bounds.top() * inv_zoom + zoom_center.y * (1.0 - inv_zoom),
            lens_bounds.right() * inv_zoom + zoom_center.x * (1.0 - inv_zoom),
            lens_bounds.bottom() * inv_zoom + zoom_center.y * (1.0 - inv_zoom),
        );

        // The above adjustment helps to account for offscreen, but when the magnifier is combined
        // with backdrop offsets, more significant fitting needs to be performed to pin the visible
        // src rect to what's available.
        let mut zoom_xform = rect_to_rect(&lens_bounds, &src_rect);
        if !expected_child_output.contains(visible_lens_bounds) {
            // We need to pick a new srcRect such that srcRect is contained within fitRect and fills
            // visibleLens, while maintaining the aspect ratio of the original srcRect -> lensBounds.
            src_rect = map_rect(&visible_lens_bounds, &zoom_xform);

            if expected_child_output.width() >= src_rect.width()
                && expected_child_output.height() >= src_rect.height()
            {
                let left = if src_rect.left() < expected_child_output.left() {
                    expected_child_output.left()
                } else {
                    std_min(src_rect.right(), expected_child_output.right()) - src_rect.width()
                };
                let top = if src_rect.top() < expected_child_output.top() {
                    expected_child_output.top()
                } else {
                    std_min(src_rect.bottom(), expected_child_output.bottom()) - src_rect.height()
                };

                // Update transform to reflect fitted src
                src_rect = Rect::from_xywh(left, top, src_rect.width(), src_rect.height());
                zoom_xform = rect_to_rect(&visible_lens_bounds, &src_rect);
            } // Else not enough of the target is available to cover, so don't try adjusting
        }

        // When there is no SkSL support, or there's a 0 inset, the magnifier is equivalent to a
        // rect->rect transform and crop.
        let inset = mapping.param_to_layer_size(Size {
            width: self.inset,
            height: self.inset,
        });
        if inset.width <= 0.0 || inset.height <= 0.0 {
            // When applying the zoom as a direct transform, we only require the visibleSrcRect as
            // input from the child filter, and transform it by the inverse of zoomXform.
            let Some(inv_zoom_xform) = zoom_xform.invert() else {
                return FilterResult::default(); // pathological input
            };
            let child_output =
                self.get_child_output(0, &context.with_new_desired_output(src_rect.round_out()));
            return child_output
                .apply_transform(context, &inv_zoom_xform, self.sampling)
                .apply_crop(context, lens_bounds.round_out(), TileMode::Decal);
        }

        // While the zoomed in portion reflects `srcRect`, we need to request as much of
        // `lensBounds` as the child can produce (which is what `expectedChildOutput` is set to).
        let mut builder = Builder::new(context);
        let actual_output = self.get_child_output(
            0,
            &context.with_new_desired_output(expected_child_output.round_out()),
        );

        // Since the actual output is restricted to lensBounds and the builder is outputting up to
        // `lensBounds`, there's no need to provide explicit sample bounds.
        builder.add(
            actual_output,
            None,
            ShaderFlags::NON_TRIVIAL_SAMPLING,
            self.sampling,
        );
        builder.eval(
            |inputs| {
                // If the input resolved to a null shader, the magnified output will be transparent
                // too.
                inputs[0].clone().and_then(|input| {
                    make_magnifier_shader(input, &lens_bounds, &zoom_xform, inset)
                })
            },
            Some(lens_bounds.round_out()),
            false,
        )
    }

    // Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L229-L239 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        _desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        // The required input is always the lens bounds.
        let required_input = mapping.param_to_layer_rect(&self.lens_bounds).round_out();
        // Our required input is the desired output for our child image filter.
        self.get_child_input_layer_bounds(0, mapping, required_input, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L241-L254 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        // The output of this filter is fLensBounds intersected with its child's output.
        let output = self.get_child_output_layer_bounds(0, mapping, content_bounds);
        let mut lens_bounds = mapping.param_to_layer_rect(&self.lens_bounds).round_out();
        match output {
            None => Some(lens_bounds),
            Some(output) => {
                if irect_intersect_in_place(&mut lens_bounds, &output) {
                    Some(lens_bounds)
                } else {
                    // Nothing to magnify
                    Some(IRect::new_empty())
                }
            }
        }
    }

    // Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L256-L263 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        let mut bounds = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(src),
            None => *src,
        };
        if bounds.intersect(self.lens_bounds) {
            bounds
        } else {
            Rect::new_empty()
        }
    }
}

/// `SkImageFilters::Magnifier`: a zoom of `lens_bounds` by `zoom_amount`, with an `inset` border
/// that is blended into the lens. `crop_rect` applies to the input only.
///
/// Returns `None` for an invalid lens or amount. A zoom of 1 or less is the identity, so the input
/// is returned.
// Port of: src/effects/imagefilters/SkMagnifierImageFilter.cpp#L49-L77 (chrome/m156)
#[doc(alias = "Magnifier")]
#[must_use]
pub fn magnifier(
    lens_bounds: &Rect,
    zoom_amount: f32,
    inset: f32,
    sampling: SamplingOptions,
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    if lens_bounds.is_empty()
        || !lens_bounds.is_finite()
        || zoom_amount <= 0.0
        || inset < 0.0
        || !zoom_amount.is_finite()
        || !inset.is_finite()
    {
        return None; // invalid
    }
    // The magnifier automatically restricts its output based on the size of the image it receives
    // as input, so 'cropRect' only applies to its input.
    let input = match &crop_rect {
        Some(rect) => crop(rect, TileMode::Decal, input),
        None => input,
    };

    if zoom_amount > 1.0 {
        Some(ImageFilter::from_base(MagnifierImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            lens_bounds: *lens_bounds,
            zoom_amount,
            inset,
            sampling,
        }))
    } else {
        // Zooming with a value less than 1 is technically a downscaling, which "works" but the
        // non-linear distortion behaves unintuitively. At zoomAmount = 1, this filter is an
        // expensive identity function so treat zoomAmount <= 1 as a no-op.
        input
    }
}
