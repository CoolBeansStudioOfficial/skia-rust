// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/imagefilters/SkRuntimeImageFilter.cpp

//! `SkImageFilters::RuntimeShader`: evaluates a runtime shader (`SkSL`) over the output of its
//! child image filters, one child shader per input.

use std::sync::Mutex;

use skia_rust_core::image_filter::{ImageFilter, ImageFilterBase, ImageFilterCommon};
use skia_rust_core::image_filter_result::{Builder, FilterResult, ShaderFlags, default_sampling};
use skia_rust_core::image_filter_types::{Context, Mapping, MatrixCapability, size_ceil};
use skia_rust_core::rect::{IRect, Rect, rect_priv};
use skia_rust_core::runtime_effect::{ChildPtr, ChildType, RuntimeShaderBuilder};
use skia_rust_core::shader::Shader;
use skia_rust_core::size::Size;

/// `SkRuntimeImageFilter`: the builder is kept behind a lock because the child shaders are set on
/// it while the filter is evaluated (`fRuntimeEffectLock` in Skia).
// Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L40-L100 (chrome/m156)
#[derive(Debug)]
struct RuntimeImageFilter {
    common: ImageFilterCommon,
    runtime_effect_builder: Mutex<RuntimeShaderBuilder>,
    child_shader_names: Vec<String>,
    max_sample_radius: f32,
    restrict_output_to_input_bounds: bool,
}

impl RuntimeImageFilter {
    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L43-L57 (chrome/m156)
    fn new(
        builder: RuntimeShaderBuilder,
        max_sample_radius: f32,
        child_shader_names: &[&str],
        inputs: &[Option<ImageFilter>],
        restrict_output_to_input_bounds: bool,
    ) -> Self {
        debug_assert!(max_sample_radius >= 0.0);
        RuntimeImageFilter {
            common: ImageFilterCommon::new(inputs.to_vec(), None),
            runtime_effect_builder: Mutex::new(builder),
            child_shader_names: child_shader_names
                .iter()
                .map(|s| (*s).to_string())
                .collect(),
            max_sample_radius,
            restrict_output_to_input_bounds,
        }
    }

    /// `applyMaxSampleRadius(mapping, bounds)`: `bounds` outset by the sample radius in layer
    /// space.
    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L86-L93 (chrome/m156)
    fn apply_max_sample_radius(&self, mapping: &Mapping, mut bounds: IRect) -> IRect {
        let max_sample_radius = size_ceil(
            mapping.param_to_layer_size(Size::new(self.max_sample_radius, self.max_sample_radius)),
        );
        bounds.outset((max_sample_radius.width, max_sample_radius.height));
        bounds
    }
}

impl ImageFilterBase for RuntimeImageFilter {
    fn common(&self) -> &ImageFilterCommon {
        &self.common
    }

    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L65-L66 (chrome/m156),
    // onAffectsTransparentBlack
    fn on_affects_transparent_black(&self) -> bool {
        !self.restrict_output_to_input_bounds
    }

    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L70-L71 (chrome/m156),
    // onGetCTMCapability: the geometric uniforms only respond to translation (skbug.com/40044507).
    fn on_get_ctm_capability(&self) -> MatrixCapability {
        MatrixCapability::Translate
    }

    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L253-L297 (chrome/m156),
    // onFilterImage
    fn on_filter_image(&self, ctx: &Context<'_>) -> FilterResult {
        let input_count = self.count_inputs();
        debug_assert_eq!(input_count, self.child_shader_names.len());

        let input_ctx = ctx.with_new_desired_output(
            self.apply_max_sample_radius(ctx.mapping(), ctx.desired_output()),
        );

        let mut builder = Builder::new(ctx);
        let mut actual_input_bounds = IRect::default(); // LayerSpace<SkIRect>::Empty()
        for i in 0..input_count {
            // Record the input context's desired output as the sample bounds for the child shaders
            // since the runtime shader can go up to max sample radius away from its desired output
            // (which is the default sample bounds if we didn't override it here).
            let child_output = self.get_child_output(i, &input_ctx);
            actual_input_bounds = IRect::join(&actual_input_bounds, &child_output.layer_bounds());
            builder.add(
                child_output,
                Some(input_ctx.desired_output()),
                ShaderFlags::NON_TRIVIAL_SAMPLING,
                default_sampling(),
            );
        }

        let explicit_output = if self.restrict_output_to_input_bounds {
            Some(self.apply_max_sample_radius(ctx.mapping(), actual_input_bounds))
        } else {
            None
        };
        builder.eval(
            |inputs: &[Option<Shader>]| {
                // Lock the mutation of the builder and creation of the shader so that the
                // builder's state is const and is safe for multi-threaded access.
                let mut effect_builder = self
                    .runtime_effect_builder
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                for (i, input) in inputs.iter().enumerate().take(input_count) {
                    effect_builder
                        .child(&self.child_shader_names[i])
                        .assign(input.clone().map_or(ChildPtr::Empty, ChildPtr::Shader));
                }
                let shader = effect_builder.make_shader(None::<&skia_rust_core::matrix::Matrix>);
                // Remove the inputs from the builder to avoid unnecessarily prolonging the input
                // shaders' lifetimes.
                for name in &self.child_shader_names {
                    effect_builder.child(name).assign(ChildPtr::Empty);
                }
                shader
            },
            explicit_output,
            true, // evaluateInParameterSpace
        )
    }

    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L299-L318 (chrome/m156),
    // onGetInputLayerBounds
    fn on_get_input_layer_bounds(
        &self,
        mapping: &Mapping,
        desired_output: IRect,
        content_bounds: Option<IRect>,
    ) -> IRect {
        let input_count = self.count_inputs();
        if input_count == 0 {
            return IRect::default(); // LayerSpace<SkIRect>::Empty()
        }
        // Provide 'maxSampleRadius' pixels (in layer space) to the child shaders.
        let required_input = self.apply_max_sample_radius(mapping, desired_output);
        // Union of all child input bounds so that one source image can provide for all of them.
        (0..input_count).fold(IRect::default(), |acc, i| {
            IRect::join(
                &acc,
                &self.get_child_input_layer_bounds(i, mapping, required_input, content_bounds),
            )
        })
    }

    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L320-L344 (chrome/m156),
    // onGetOutputLayerBounds
    fn on_get_output_layer_bounds(
        &self,
        mapping: &Mapping,
        content_bounds: Option<IRect>,
    ) -> Option<IRect> {
        if self.restrict_output_to_input_bounds {
            let input_count = self.count_inputs();
            let mut child_is_unbounded = false;
            let mut child_output = IRect::default();
            for i in 0..input_count {
                match self.get_child_output_layer_bounds(i, mapping, content_bounds) {
                    Some(o) => child_output = IRect::join(&child_output, &o),
                    None => {
                        child_is_unbounded = true;
                        // This value doesn't matter once child_is_unbounded is true
                    }
                }
            }
            if !child_is_unbounded {
                return Some(self.apply_max_sample_radius(mapping, child_output));
            }
        }
        // Pessimistically assume it can cover anything
        None
    }

    // Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L346-L353 (chrome/m156),
    // computeFastBounds
    fn compute_fast_bounds(&self, src: &Rect) -> Rect {
        if self.restrict_output_to_input_bounds {
            // SkImageFilter::computeFastBounds(src), then outset by the sample radius.
            let mut bounds = if self.count_inputs() == 0 {
                *src
            } else {
                let mut combined = match self.get_input(0) {
                    Some(input) => input.compute_fast_bounds(src),
                    None => *src,
                };
                for i in 1..self.count_inputs() {
                    match self.get_input(i) {
                        Some(input) => combined.join(input.compute_fast_bounds(src)),
                        None => combined.join(src),
                    }
                }
                combined
            };
            bounds.outset((self.max_sample_radius, self.max_sample_radius));
            return bounds;
        }
        // Can't predict what the RT Shader will generate (see onGetOutputLayerBounds).
        rect_priv::make_large_s32()
    }
}

/// `SkImageFilters::RuntimeShader(builder, maxSampleRadius, childShaderNames, inputs, count,
/// restrictOutputToInputBounds)`: one input per child shader name. Returns `None` for a negative
/// sample radius, or if a name is empty, duplicated or not a shader child of the effect.
// Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L121-L153 (chrome/m156)
#[doc(alias = "RuntimeShader")]
#[must_use]
pub fn runtime_shader_children(
    builder: &RuntimeShaderBuilder,
    max_sample_radius: f32,
    child_shader_names: &[&str],
    inputs: &[Option<ImageFilter>],
    restrict_output_to_input_bounds: bool,
) -> Option<ImageFilter> {
    debug_assert_eq!(child_shader_names.len(), inputs.len());
    if max_sample_radius < 0.0 {
        return None; // invalid sample radius
    }
    let child_is_shader = |name: &str| {
        builder
            .effect()
            .find_child(name)
            .is_some_and(|child| child.ty() == ChildType::Shader)
    };
    for (i, name) in child_shader_names.iter().enumerate() {
        // All names must be non-empty, and present as a child shader in the effect:
        if name.is_empty() || !child_is_shader(name) {
            return None;
        }
        // We don't allow duplicates, either:
        if child_shader_names[..i].contains(name) {
            return None;
        }
    }
    Some(ImageFilter::from_base(RuntimeImageFilter::new(
        builder.clone(),
        max_sample_radius,
        child_shader_names,
        inputs,
        restrict_output_to_input_bounds,
    )))
}

/// `SkImageFilters::RuntimeShader(builder, sampleRadius, childShaderName, input,
/// restrictOutputToInputBounds)`. An empty `child_shader_name` uses the effect's only child, and
/// fails if the effect has more or fewer children.
// Port of: src/effects/imagefilters/SkRuntimeImageFilter.cpp#L102-L119 (chrome/m156)
#[doc(alias = "RuntimeShader")]
#[must_use]
pub fn runtime_shader(
    builder: &RuntimeShaderBuilder,
    sample_radius: f32,
    child_shader_name: &str,
    input: Option<ImageFilter>,
    restrict_output_to_input_bounds: bool,
) -> Option<ImageFilter> {
    // If no childShaderName is provided, check to see if we can implicitly assign it to the only
    // child in the effect.
    let implicit_name;
    let child_shader_name = if child_shader_name.is_empty() {
        let children = builder.effect().children();
        if children.len() != 1 {
            return None;
        }
        implicit_name = children[0].name().to_string();
        implicit_name.as_str()
    } else {
        child_shader_name
    };
    runtime_shader_children(
        builder,
        sample_radius,
        &[child_shader_name],
        &[input],
        restrict_output_to_input_bounds,
    )
}
