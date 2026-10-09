// Copyright 2012 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkMorphologyImageFilter.cpp

//! `SkMorphologyImageFilter`: dilates (max) or erodes (min) its input over a rectangular kernel.
//! Each axis is one pass of the `LinearMorphology` known runtime effect, followed by passes of the
//! `SparseMorphology` effect that double the radius each step.

use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{Context, Mapping, irect_intersect_in_place, size_round};
use skia_rust_core::known_runtime_effects::{StableKey, get_known_runtime_effect};
use skia_rust_core::matrix::Matrix;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{IRect, Rect};
use skia_rust_core::runtime_effect::{ChildPtr, RuntimeShaderBuilder};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::{ISize, Size};

use crate::image_filters::crop_filter::crop;
use skia_rust_core::tile_mode::TileMode;

/// `MorphType`: erode takes the minimum of the kernel, dilate the maximum.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L16-L19 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MorphType {
    /// `kErode`.
    Erode,
    /// `kDilate`.
    Dilate,
}

/// `MorphDirection`.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L56 (chrome/m156)
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum MorphDirection {
    X,
    Y,
}

/// `kMaxRadii`: the largest layer-space radius, to avoid slow draw calls (crbug.com/1123035).
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L56-L57 (chrome/m156)
const K_MAX_RADII: i32 = 256;

/// `kMaxLinearRadius`: the linear kernel does (2R+1) texture samples per pixel, so R is kept small.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L179-L181 (chrome/m156)
const K_MAX_LINEAR_RADIUS: i32 = 14;

/// The morphology image filter (`SkMorphologyImageFilter`).
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L35-L62 (chrome/m156)
#[doc(alias = "SkMorphologyImageFilter")]
#[derive(Debug)]
pub struct MorphologyImageFilter {
    common: ImageFilterCommon,
    /// `fType`.
    morph_type: MorphType,
    /// `fRadii`: the parameter-space radii.
    radii: Size,
}

impl MorphologyImageFilter {
    /// `SkMorphologyImageFilter(type, radii, input)`.
    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L35-L39 (chrome/m156)
    #[must_use]
    pub fn new(morph_type: MorphType, radii: Size, input: Option<ImageFilter>) -> Self {
        MorphologyImageFilter {
            common: ImageFilterCommon::new(vec![input], None),
            morph_type,
            radii,
        }
    }

    /// `radii(mapping)`: the layer-space radii, rounded and capped at 256.
    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L49-L58 (chrome/m156)
    fn radii(&self, mapping: &Mapping) -> ISize {
        let radii = size_round(mapping.param_to_layer_size(self.radii));
        debug_assert!(radii.width >= 0 && radii.height >= 0);

        // We limit the radius to something small, to avoid slow draw calls: crbug.com/1123035
        ISize::new(radii.width.min(K_MAX_RADII), radii.height.min(K_MAX_RADII))
    }

    /// `requiredInput`: the input for a morphology filter is always the kernel outset, regardless
    /// of morph type.
    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L63-L70 (chrome/m156)
    fn required_input(&self, mapping: &Mapping, mut bounds: IRect) -> IRect {
        let radii = self.radii(mapping);
        bounds.outset((radii.width, radii.height));
        bounds
    }

    /// `kernelOutputBounds`.
    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L71-L84 (chrome/m156)
    fn kernel_output_bounds(&self, mapping: &Mapping, mut bounds: IRect) -> IRect {
        let radii = self.radii(mapping);
        if self.morph_type == MorphType::Dilate {
            // Transparent pixels up to the kernel radius away will be overridden by kDilate's "max"
            // function and be set to the input's boundary pixel colors, thus expanding the output.
            bounds.outset((radii.width, radii.height));
        } else {
            // Pixels closer than the kernel radius to the input image's edges are overridden by
            // kErode's "min" function and will be set to transparent black, contracting the output.
            bounds.inset((radii.width, radii.height));
        }
        bounds
    }
}

/// `make_linear_morphology(input, type, direction, radius)`: one linear pass of the
/// `LinearMorphology` known effect.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L182-L197 (chrome/m156)
fn make_linear_morphology(
    input: Option<Shader>,
    morph_type: MorphType,
    direction: MorphDirection,
    radius: i32,
) -> Option<Shader> {
    debug_assert!(radius <= K_MAX_LINEAR_RADIUS);

    let effect = get_known_runtime_effect(StableKey::LinearMorphology)?;
    let mut builder = RuntimeShaderBuilder::new(effect.clone());
    builder
        .child("child")
        .assign(input.map_or(ChildPtr::Empty, ChildPtr::Shader));
    let offset = if direction == MorphDirection::X {
        [1.0_f32, 0.0]
    } else {
        [0.0, 1.0]
    };
    builder.uniform("offset").set_f32(&offset);
    builder
        .uniform("flip")
        .set_f32(&[if morph_type == MorphType::Dilate {
            1.0
        } else {
            -1.0
        }]);
    builder.uniform("radius").set_i32(&[radius]);

    builder.make_shader(None::<&Matrix>)
}

/// `make_sparse_morphology(input, type, direction, radius)`: one sparse pass of the
/// `SparseMorphology` known effect, which doubles the kernel size of its input.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L199-L214 (chrome/m156)
fn make_sparse_morphology(
    input: Option<Shader>,
    morph_type: MorphType,
    direction: MorphDirection,
    radius: i32,
) -> Option<Shader> {
    let effect = get_known_runtime_effect(StableKey::SparseMorphology)?;
    let mut builder = RuntimeShaderBuilder::new(effect.clone());
    builder
        .child("child")
        .assign(input.map_or(ChildPtr::Empty, ChildPtr::Shader));
    #[allow(clippy::cast_precision_loss)] // radius is at most 256: exact in f32
    let r = radius as f32;
    let offset = if direction == MorphDirection::X {
        [r, 0.0]
    } else {
        [0.0, r]
    };
    builder.uniform("offset").set_f32(&offset);
    builder
        .uniform("flip")
        .set_f32(&[if morph_type == MorphType::Dilate {
            1.0
        } else {
            -1.0
        }]);

    builder.make_shader(None::<&Matrix>)
}

/// `morphology_pass(ctx, input, type, dir, radius)`: applies a 1D morphology of `radius` along
/// `dir`, in as many steps as the linear and sparse effects need.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L216-L269 (chrome/m156)
fn morphology_pass(
    ctx: &Context<'_>,
    input: &FilterResult,
    morph_type: MorphType,
    dir: MorphDirection,
    radius: i32,
) -> FilterResult {
    let axis_delta = |step: i32| -> IPoint {
        IPoint::new(
            if dir == MorphDirection::X { step } else { 0 },
            if dir == MorphDirection::Y { step } else { 0 },
        )
    };

    // The first iteration will sample a full kernel outset from the final output.
    let mut sample_bounds = ctx.desired_output();
    sample_bounds.outset(axis_delta(radius));

    let mut child_output = input.clone();
    let mut applied_radius = 0;
    while radius > applied_radius {
        if !child_output.has_image() {
            return FilterResult::default(); // Eroded or dilated transparent black is still transparent black
        }

        // The first iteration uses up to kMaxLinearRadius with a linear accumulation pass.
        // After that we double the radius each step until we can finish with the target radius.
        let step_radius = if applied_radius == 0 {
            K_MAX_LINEAR_RADIUS.min(radius)
        } else {
            (radius - applied_radius).min(applied_radius)
        };

        let step_ctx = if applied_radius + step_radius < radius {
            // Intermediate steps need to output what will be sampled on the next iteration
            let mut output_bounds = sample_bounds;
            output_bounds.inset(axis_delta(step_radius));
            ctx.with_new_desired_output(output_bounds)
        } else {
            // else the last iteration should output what was originally requested
            ctx.with_new_desired_output(ctx.desired_output())
        };

        let mut builder = Builder::new(&step_ctx);
        builder.add(
            child_output.clone(),
            Some(sample_bounds),
            ShaderFlags::SAMPLED_REPEATEDLY,
            default_sampling(),
        );
        child_output = builder.eval(
            |inputs| {
                let input = inputs[0].clone();
                if applied_radius == 0 {
                    make_linear_morphology(input, morph_type, dir, step_radius)
                } else {
                    make_sparse_morphology(input, morph_type, dir, step_radius)
                }
            },
            None,
            false,
        );

        sample_bounds = step_ctx.desired_output();
        applied_radius += step_radius;
        debug_assert!(applied_radius <= radius); // Our last iteration should hit 'radius' exactly.
    }

    child_output
}

impl ImageFilterBase for MorphologyImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L271-L300 (chrome/m156)
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let required_input = self.required_input(ctx.mapping(), ctx.desired_output());
        let child_output = self.get_child_output(0, &ctx.with_new_desired_output(required_input));

        // If childOutput completely fulfilled requiredInput, maxOutput will match the context's
        // desired output, but if the output image is smaller, this will restrict the morphology
        // output to what is actual produceable.
        let mut max_output = self.kernel_output_bounds(ctx.mapping(), child_output.layer_bounds());
        if !irect_intersect_in_place(&mut max_output, &ctx.desired_output()) {
            return FilterResult::default();
        }

        // The X pass has to preserve the extra rows to later be consumed by the Y pass.
        let radii = self.radii(ctx.mapping());
        let mut max_output_x = max_output;
        max_output_x.outset((0, radii.height));
        let child_output = morphology_pass(
            &ctx.with_new_desired_output(max_output_x),
            &child_output,
            self.morph_type,
            MorphDirection::X,
            radii.width,
        );
        morphology_pass(
            &ctx.with_new_desired_output(max_output),
            &child_output,
            self.morph_type,
            MorphDirection::Y,
            radii.height,
        )
    }

    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L302-L308 (chrome/m156)
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let required_input = self.required_input(mapping, desired_output);
        self.get_child_input_layer_bounds(0, mapping, required_input, content_bounds)
    }

    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L310-L318 (chrome/m156)
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        // An unbounded child output is an unbounded morphology output.
        self.get_child_output_layer_bounds(0, mapping, content_bounds)
            .map(|child| self.kernel_output_bounds(mapping, child))
    }

    // Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L320-L329 (chrome/m156)
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        // See kernelOutputBounds() for rationale
        let mut bounds = match self.get_input(0) {
            Some(input) => input.compute_fast_bounds(src),
            None => *src,
        };
        if self.morph_type == MorphType::Dilate {
            bounds.outset((self.radii.width, self.radii.height));
        } else {
            bounds.inset((self.radii.width, self.radii.height));
        }
        bounds
    }
}

/// `make_morphology(type, radii, input, cropRect)`.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L86-L107 (chrome/m156)
fn make_morphology(
    morph_type: MorphType,
    radii: (f32, f32),
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    if radii.0 < 0.0 || radii.1 < 0.0 {
        return None; // invalid
    }
    let mut filter = input;
    if radii.0 > 0.0 || radii.1 > 0.0 {
        filter = Some(ImageFilter::from_base(MorphologyImageFilter::new(
            morph_type,
            Size::new(radii.0, radii.1),
            filter,
        )));
    }
    // otherwise both radii are 0, so the kernel is always the identity function, in which case
    // we just need to apply the 'cropRect' to the 'input'.

    if let Some(crop_rect) = crop_rect {
        filter = crop(&crop_rect, TileMode::Decal, filter);
    }
    filter
}

/// `SkImageFilters::Dilate(radiusX, radiusY, input, cropRect)`.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L299-L303 (chrome/m156)
#[doc(alias = "Dilate")]
#[must_use]
pub fn dilate(
    radius: (f32, f32),
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_morphology(MorphType::Dilate, radius, input, crop_rect)
}

/// `SkImageFilters::Erode(radiusX, radiusY, input, cropRect)`.
// Port of: src/effects/imagefilters/SkMorphologyImageFilter.cpp#L305-L309 (chrome/m156)
#[doc(alias = "Erode")]
#[must_use]
pub fn erode(
    radius: (f32, f32),
    input: Option<ImageFilter>,
    crop_rect: Option<Rect>,
) -> Option<ImageFilter> {
    make_morphology(MorphType::Erode, radius, input, crop_rect)
}
