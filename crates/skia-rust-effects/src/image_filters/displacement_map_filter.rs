// Copyright 2013 Google Inc.
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp

//! `SkDisplacementMapImageFilter`: moves each pixel of its color input by a vector read from the
//! channels of its displacement input, evaluated by the `Displacement` known runtime effect.

use skia_rust_core::color::{Color, ColorChannel};
use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{Context, Mapping, irect_intersect_in_place, size_ceil};
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeShaderBuilder};
use skia_rust_core::sampling_options::{FilterMode, SamplingOptions};
use skia_rust_core::shader::Shader;
use skia_rust_core::shaders;
use skia_rust_core::size::Size;
use skia_rust_core::tile_mode::TileMode;

use crate::image_filters::crop_filter::crop;

/// Index of the displacement input (`kDisplacement`).
// Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L24 (chrome/m156)
const DISPLACEMENT: usize = 0;
/// Index of the color input (`kColor`).
// Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L25 (chrome/m156)
const COLOR: usize = 1;

/// The displacement map image filter (`SkDisplacementMapImageFilter`).
// Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L17-L62 (chrome/m156)
#[doc(alias = "SkDisplacementMapImageFilter")]
#[derive(Debug)]
pub struct DisplacementMapImageFilter {
    common: ImageFilterCommon,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
    /// `fScale`: the parameter-space scale; the vector is `(fScale, fScale)` in parameter space.
    scale: f32,
}

impl DisplacementMapImageFilter {
    /// `SkDisplacementMapImageFilter(xChannel, yChannel, scale, inputs)`.
    // Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L24-L35 (chrome/m156)
    #[must_use]
    pub fn new(
        x_channel: ColorChannel,
        y_channel: ColorChannel,
        scale: f32,
        inputs: [Option<ImageFilter>; 2],
    ) -> Self {
        debug_assert!(scale.is_finite());
        DisplacementMapImageFilter {
            common: ImageFilterCommon::new(inputs.into(), None),
            x_channel,
            y_channel,
            scale,
        }
    }

    /// `outsetByMaxDisplacement(mapping, bounds)`: `bounds` outset by the largest displacement the
    /// scale allows. The scale is a size here, so its magnitude is taken into account.
    // Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L42-L51 (chrome/m156)
    fn outset_by_max_displacement(&self, mapping: &Mapping, mut bounds: IRect) -> IRect {
        // For max displacement, we treat 'scale' as a size instead of a vector. The vector offset
        // maps a [0,1] channel value to [-scale/2, scale/2], and treating it as a size
        // automatically accounts for the absolute magnitude when transforming from param to layer.
        let max_displacement =
            mapping.param_to_layer_size(Size::new(0.5 * self.scale, 0.5 * self.scale));
        let ceil = size_ceil(max_displacement);
        bounds.outset((ceil.width, ceil.height));
        bounds
    }
}

/// `make_displacement_shader(displacement, color, scale, xChannel, yChannel)`.
// Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L56-L84 (chrome/m156)
fn make_displacement_shader(
    displacement: Option<Shader>,
    color: Option<Shader>,
    scale: skia_rust_core::point::Vector,
    x_channel: ColorChannel,
    y_channel: ColorChannel,
) -> Option<Shader> {
    let color = color?; // Color is fully transparent, so no point in displacing it

    // Somehow we had a valid displacement image but failed to produce a shader (e.g. an internal
    // resolve to a new image failed). Treat the displacement as transparent, but it's too late to
    // switch to the applyTransform() optimization.
    let displacement = displacement.unwrap_or_else(|| shaders::color(Color::TRANSPARENT));

    let effect = get_known_runtime_effect(StableKey::Displacement)?;
    let channel_selector = |c: ColorChannel| -> [f32; 4] {
        [
            if c == ColorChannel::R { 1.0 } else { 0.0 },
            if c == ColorChannel::G { 1.0 } else { 0.0 },
            if c == ColorChannel::B { 1.0 } else { 0.0 },
            if c == ColorChannel::A { 1.0 } else { 0.0 },
        ]
    };

    let mut builder = RuntimeShaderBuilder::new(effect.clone());
    builder
        .child("displMap")
        .assign(ChildPtr::Shader(displacement));
    builder.child("colorMap").assign(ChildPtr::Shader(color));
    builder.uniform("scale").set_f32(&[scale.x, scale.y]);
    builder
        .uniform("xSelect")
        .set_f32(&channel_selector(x_channel));
    builder
        .uniform("ySelect")
        .set_f32(&channel_selector(y_channel));
    builder.make_shader(None::<&Matrix>)
}

impl ImageFilterBase for DisplacementMapImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L121-L164 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let required_color_input =
            self.outset_by_max_displacement(ctx.mapping(), ctx.desired_output());
        let color_output =
            self.get_child_output(COLOR, &ctx.with_new_desired_output(required_color_input));
        if !color_output.has_image() {
            return FilterResult::default(); // No non-transparent black colors to displace
        }

        // When the color image filter is unrestricted, its output will be 'maxDisplacement' larger
        // than this filter's desired output. However, if it is cropped, we can restrict this
        // filter's final output. However it's not simply colorOutput intersected with desiredOutput
        // since we have to account for how the clipped colorOutput might still be displaced.
        let mut output_bounds =
            self.outset_by_max_displacement(ctx.mapping(), color_output.layer_bounds());

        // 'outputBounds' has double the max displacement for edges where colorOutput had not been
        // clipped, but that's fine since we intersect with 'desiredOutput'. For edges that were
        // cropped the second max displacement represents how far they can be displaced, which
        // might be inside the original 'desiredOutput'.
        if !irect_intersect_in_place(&mut output_bounds, &ctx.desired_output()) {
            // None of the non-transparent black colors can be displaced into the desired bounds.
            return FilterResult::default();
        }

        // Creation of the displacement map should happen in a non-colorspace aware context. This
        // texture is a purely mathematical construct, so we want to just operate on the stored
        // values. With a more complex DAG attached to this input, it's not clear that working in
        // ANY specific color space makes sense, so we ignore color spaces (and gamma) entirely.
        let displacement_output = self.get_child_output(
            DISPLACEMENT,
            &ctx.with_new_desired_output(output_bounds)
                .with_new_color_space(None),
        );

        // NOTE: The scale is a "vector" not a "size" since we want to preserve negations on the
        // final displacement vector.
        let scale = ctx
            .mapping()
            .param_to_layer_vector(skia_rust_core::point::Vector::new(self.scale, self.scale));

        if !displacement_output.has_image() {
            // A null displacement map means its transparent black, but (0,0,0,0) becomes the vector
            // (-scale/2, -scale/2) applied to the color image, so represent the displacement as a
            // simple transform.
            let constant_displacement = Matrix::translate((-0.5 * scale.x, -0.5 * scale.y));
            return color_output.apply_transform(
                ctx,
                &constant_displacement,
                SamplingOptions::from(FilterMode::Nearest),
            );
        }

        // If we made it this far, then we actually have per-pixel displacement affecting the color
        // image. We need to evaluate each pixel within 'outputBounds'.
        let mut builder = Builder::new(ctx);
        builder.add(
            displacement_output,
            Some(output_bounds),
            ShaderFlags::NONE,
            default_sampling(),
        );
        builder.add(
            color_output,
            Some(required_color_input),
            ShaderFlags::NON_TRIVIAL_SAMPLING,
            SamplingOptions::from(FilterMode::Nearest),
        );
        builder.eval(
            |inputs| {
                make_displacement_shader(
                    inputs[DISPLACEMENT].clone(),
                    inputs[COLOR].clone(),
                    scale,
                    self.x_channel,
                    self.y_channel,
                )
            },
            Some(output_bounds),
            false,
        )
    }

    // Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L166-L180 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        // Pixels up to the maximum displacement away from 'desiredOutput' can be moved into those
        // bounds, depending on how the displacement map renders. To ensure those colors are
        // defined, we require that outset buffer around 'desiredOutput' from the color map.
        let required_input = self.outset_by_max_displacement(mapping, desired_output);
        let required_color =
            self.get_child_input_layer_bounds(COLOR, mapping, required_input, content_bounds);
        // Accumulate the required input for the displacement filter to cover the original desired
        // output.
        let required_displacement = self.get_child_input_layer_bounds(
            DISPLACEMENT,
            mapping,
            desired_output,
            content_bounds,
        );
        IRect::join(&required_color, &required_displacement)
    }

    // Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L182-L192 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        // An unbounded color output is an unbounded displacement output.
        self.get_child_output_layer_bounds(COLOR, mapping, content_bounds)
            .map(|color_output| self.outset_by_max_displacement(mapping, color_output))
    }

    // Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L194-L200 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        let mut color_bounds = match self.get_input(COLOR) {
            Some(input) => input.compute_fast_bounds(src),
            None => *src,
        };
        let max_displacement = 0.5 * self.scale.abs();
        color_bounds.outset((max_displacement, max_displacement));
        color_bounds
    }
}

/// `SkImageFilters::DisplacementMap(xChannelSelector, yChannelSelector, scale, displacement,
/// color, cropRect)`: `None` if the scale is not finite. Every `ColorChannel` is a valid selector
/// (Skia rejects only out-of-range values, which the enum cannot hold).
// Port of: src/effects/imagefilters/SkDisplacementMapImageFilter.cpp#L217-L238 (chrome/m156)
#[doc(alias = "DisplacementMap")]
#[must_use]
pub fn displacement_map(
    (x_channel_selector, y_channel_selector): (ColorChannel, ColorChannel),
    scale: f32,
    displacement: Option<ImageFilter>,
    color: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    if !scale.is_finite() {
        return None;
    }
    let mut filter = Some(ImageFilter::from_base(DisplacementMapImageFilter::new(
        x_channel_selector,
        y_channel_selector,
        scale,
        [displacement, color],
    )));
    if let Some(rect) = crop_rect {
        filter = crop(&rect, TileMode::Decal, filter);
    }
    filter
}
