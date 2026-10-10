// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileImageFilter.h,
// src/gpu/graphite/precompile/PrecompileImageFilter.cpp, PrecompileImageFilterPriv.h and
// PrecompileImageFiltersPriv.h

//! `PrecompileImageFilter` and the `PrecompileImageFilters` factories. Each filter knows which
//! pipelines its draw will need (`createPipelines`), and reports the color filter it reduces to
//! when it is one.

use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;

use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::key_context::KeyContext;
use crate::graphite::precompile::blender::{PrecompileBlender, PrecompileBlenders};
use crate::graphite::precompile::color_filter::PrecompileColorFilter;
use crate::graphite::precompile::paint_options::{PaintOptions, ProcessCombination};
use crate::graphite::precompile::shader::{PrecompileShader, PrecompileShaders};
use crate::graphite::precompile::shader_effects;
use crate::graphite::precompile::shader_image::{ALL_TILE_MODES, ImageShaderFlags};
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::Coverage;

/// The virtual interface of `PrecompileImageFilter`. The base `addToKey` is not valid for image
/// filters, so only the filter-specific parts are here.
pub(crate) trait ImageFilterImpl: Send + Sync {
    /// `isColorFilterNode()`: the color filter this filter is, when it is a color filter node.
    fn is_color_filter_node(&self) -> Option<PrecompileColorFilter> {
        None
    }

    /// `onCreatePipelines(keyContext, renderPassDesc, processCombination)`.
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    );
}

/// `PrecompileImageFilter`: a shared precompile image filter, with its inputs.
#[doc(alias = "SkImageFilter")]
#[derive(Clone)]
pub struct PrecompileImageFilter {
    imp: Arc<dyn ImageFilterImpl>,
    inputs: Vec<Option<PrecompileImageFilter>>,
}

impl std::fmt::Debug for PrecompileImageFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileImageFilter")
            .field("inputs", &self.inputs.len())
            .finish_non_exhaustive()
    }
}

impl PrecompileImageFilter {
    fn new(imp: Arc<dyn ImageFilterImpl>, inputs: Vec<Option<PrecompileImageFilter>>) -> Self {
        Self { imp, inputs }
    }

    /// `priv().isColorFilterNode()`.
    #[must_use]
    pub fn is_color_filter_node(&self) -> Option<PrecompileColorFilter> {
        self.imp.is_color_filter_node()
    }

    /// `priv().countInputs()`.
    #[must_use]
    pub fn count_inputs(&self) -> usize {
        self.inputs.len()
    }

    /// `priv().getInput(index)`.
    #[must_use]
    pub fn get_input(&self, index: usize) -> Option<&PrecompileImageFilter> {
        self.inputs[index].as_ref()
    }

    /// `PrecompileImageFilter::asAColorFilter()`: the color filter this filter reduces to, when
    /// it is a color filter node with no input.
    // Port of: src/gpu/graphite/precompile/PrecompileImageFilter.cpp#L36-L46 (chrome/m156)
    #[must_use]
    pub fn as_a_color_filter(&self) -> Option<PrecompileColorFilter> {
        let tmp = self.is_color_filter_node()?;
        debug_assert_eq!(self.count_inputs(), 1);
        if self.get_input(0).is_some() {
            return None;
        }
        Some(tmp)
    }

    /// `PrecompileImageFilter::createPipelines(keyContext, renderPassDesc, processCombination)`.
    // Port of: src/gpu/graphite/precompile/PrecompileImageFilter.cpp#L48-L58 (chrome/m156)
    pub fn create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        self.imp
            .on_create_pipelines(key_context, render_pass_desc, process_combination);
        for input in self.inputs.iter().flatten() {
            input.create_pipelines(key_context, render_pass_desc, process_combination);
        }
    }
}

/// `PrecompileImageFiltersPriv::CreateBlurImageFilterPipelines`.
// Port of: src/gpu/graphite/precompile/PrecompileImageFilter.cpp#L60-L73 (chrome/m156)
pub(crate) fn create_blur_image_filter_pipelines(
    key_context: &KeyContext<'_>,
    render_pass_desc: &RenderPassDesc,
    process_combination: &ProcessCombination<'_>,
) {
    let mut blur_paint_options = PaintOptions::new();
    let image_shader = no_alpha_no_cubic_image();
    blur_paint_options.set_shaders(&[shader_effects::blur(image_shader)]);
    blur_paint_options.set_blend_modes(&[BlendMode::Src]);
    blur_paint_options.build_combinations(
        key_context,
        DrawTypeFlags::SIMPLE_SHAPE,
        false,
        Coverage::SingleChannel,
        render_pass_desc,
        process_combination,
    );
}

/// The shared image shader of the image-filter pipelines: `Image(kNoAlphaNoCubic)`.
fn no_alpha_no_cubic_image() -> PrecompileShader {
    PrecompileShaders::image(ImageShaderFlags::NO_ALPHA_NO_CUBIC, &[], &ALL_TILE_MODES)
}

/// `PrecompileBlendFilterImageFilter`.
struct BlendFilter {
    blender: PrecompileBlender,
}

impl ImageFilterImpl for BlendFilter {
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let mut paint_options = PaintOptions::new();
        let image_shader = no_alpha_no_cubic_image();
        let blend_shader = PrecompileShaders::blend(
            &[Some(self.blender.clone())],
            std::slice::from_ref(&image_shader),
            std::slice::from_ref(&image_shader),
        );
        paint_options.set_shaders(&[blend_shader]);
        paint_options.build_combinations(
            key_context,
            DrawTypeFlags::SIMPLE_SHAPE,
            false,
            Coverage::SingleChannel,
            render_pass_desc,
            process_combination,
        );
    }
}

/// `PrecompileBlurImageFilter`.
struct BlurFilter;

impl ImageFilterImpl for BlurFilter {
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        create_blur_image_filter_pipelines(key_context, render_pass_desc, process_combination);
    }
}

/// `PrecompileColorFilterImageFilter`.
struct ColorFilterFilter {
    color_filter: Option<PrecompileColorFilter>,
}

impl ImageFilterImpl for ColorFilterFilter {
    fn is_color_filter_node(&self) -> Option<PrecompileColorFilter> {
        self.color_filter.clone()
    }

    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let mut paint_options = PaintOptions::new();
        let image_shader = no_alpha_no_cubic_image();
        let blend_modes = [BlendMode::DstOut];
        paint_options.set_shaders(&[image_shader]);
        paint_options.set_color_filters(std::slice::from_ref(&self.color_filter));
        paint_options.set_blend_modes(&blend_modes);
        paint_options.build_combinations(
            key_context,
            DrawTypeFlags::SIMPLE_SHAPE,
            false,
            Coverage::SingleChannel,
            render_pass_desc,
            process_combination,
        );
    }
}

/// `PrecompileDisplacementMapImageFilter`.
struct DisplacementMapFilter;

impl ImageFilterImpl for DisplacementMapFilter {
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let mut displacement = PaintOptions::new();
        let image_shader = no_alpha_no_cubic_image();
        displacement.set_shaders(&[shader_effects::displacement(
            image_shader.clone(),
            image_shader,
        )]);
        displacement.build_combinations(
            key_context,
            DrawTypeFlags::SIMPLE_SHAPE,
            false,
            Coverage::SingleChannel,
            render_pass_desc,
            process_combination,
        );
    }
}

/// `PrecompileLightingImageFilter`.
struct LightingFilter;

impl ImageFilterImpl for LightingFilter {
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let image_shader = no_alpha_no_cubic_image();
        let mut lighting = PaintOptions::new();
        lighting.set_shaders(&[shader_effects::lighting(image_shader)]);
        lighting.build_combinations(
            key_context,
            DrawTypeFlags::SIMPLE_SHAPE,
            false,
            Coverage::SingleChannel,
            render_pass_desc,
            process_combination,
        );
    }
}

/// `PrecompileMatrixConvolutionImageFilter`.
struct MatrixConvolutionFilter;

impl ImageFilterImpl for MatrixConvolutionFilter {
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let mut matrix_conv = PaintOptions::new();
        let image_shader = no_alpha_no_cubic_image();
        matrix_conv.set_shaders(&[shader_effects::matrix_convolution(image_shader)]);
        matrix_conv.build_combinations(
            key_context,
            DrawTypeFlags::SIMPLE_SHAPE,
            false,
            Coverage::SingleChannel,
            render_pass_desc,
            process_combination,
        );
    }
}

/// `PrecompileMorphologyImageFilter`.
struct MorphologyFilter;

impl ImageFilterImpl for MorphologyFilter {
    fn on_create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let image_shader = no_alpha_no_cubic_image();
        {
            let mut sparse = PaintOptions::new();
            sparse.set_shaders(&[shader_effects::sparse_morphology(image_shader.clone())]);
            sparse.set_blend_modes(&[BlendMode::Src]);
            sparse.build_combinations(
                key_context,
                DrawTypeFlags::SIMPLE_SHAPE,
                false,
                Coverage::SingleChannel,
                render_pass_desc,
                process_combination,
            );
        }
        {
            let mut linear = PaintOptions::new();
            linear.set_shaders(&[shader_effects::linear_morphology(image_shader)]);
            linear.set_blend_modes(&[BlendMode::SrcOver]);
            linear.build_combinations(
                key_context,
                DrawTypeFlags::SIMPLE_SHAPE,
                false,
                Coverage::SingleChannel,
                render_pass_desc,
                process_combination,
            );
        }
    }
}

/// `PrecompileImageFilters`: the factories for precompile image filters.
#[derive(Debug, Clone, Copy)]
pub struct PrecompileImageFilters;

impl PrecompileImageFilters {
    /// `PrecompileImageFilters::Arithmetic(background, foreground)`.
    #[must_use]
    pub fn arithmetic(
        background: Option<PrecompileImageFilter>,
        foreground: Option<PrecompileImageFilter>,
    ) -> Option<PrecompileImageFilter> {
        Self::blend_with(PrecompileBlenders::arithmetic(), background, foreground)
    }

    /// `PrecompileImageFilters::Blend(bm, background, foreground)`.
    #[must_use]
    pub fn blend_mode(
        bm: BlendMode,
        background: Option<PrecompileImageFilter>,
        foreground: Option<PrecompileImageFilter>,
    ) -> Option<PrecompileImageFilter> {
        Self::blend_with(Some(PrecompileBlenders::mode(bm)), background, foreground)
    }

    /// `PrecompileImageFilters::Blend(blender, background, foreground)`.
    // Port of: src/gpu/graphite/precompile/PrecompileImageFilter.cpp#L152-L172 (chrome/m156)
    #[must_use]
    pub fn blend(
        blender: Option<PrecompileBlender>,
        background: Option<PrecompileImageFilter>,
        foreground: Option<PrecompileImageFilter>,
    ) -> Option<PrecompileImageFilter> {
        Self::blend_with(blender, background, foreground)
    }

    /// The shared body of the `Blend` overloads.
    fn blend_with(
        blender: Option<PrecompileBlender>,
        background: Option<PrecompileImageFilter>,
        foreground: Option<PrecompileImageFilter>,
    ) -> Option<PrecompileImageFilter> {
        let blender = blender.unwrap_or_else(|| PrecompileBlenders::mode(BlendMode::SrcOver));
        if let Some(bm) = blender.as_blend_mode() {
            if bm == BlendMode::Src {
                return foreground;
            } else if bm == BlendMode::Dst {
                return background;
            } else if bm == BlendMode::Clear {
                return None; // TODO: actually return PrecompileImageFilters::Empty
            }
        }
        Some(PrecompileImageFilter::new(
            Arc::new(BlendFilter { blender }),
            vec![background, foreground],
        ))
    }

    /// `PrecompileImageFilters::Blur(input)`.
    #[must_use]
    pub fn blur(input: Option<PrecompileImageFilter>) -> PrecompileImageFilter {
        PrecompileImageFilter::new(Arc::new(BlurFilter), vec![input])
    }

    /// `PrecompileImageFilters::ColorFilter(colorFilter, input)`.
    // Port of: src/gpu/graphite/precompile/PrecompileImageFilter.cpp#L187-L201 (chrome/m156)
    #[must_use]
    pub fn color_filter(
        color_filter: Option<PrecompileColorFilter>,
        input: Option<PrecompileImageFilter>,
    ) -> Option<PrecompileImageFilter> {
        let mut color_filter = color_filter;
        let mut input = input;
        if let (Some(cf), Some(in_filter)) = (&color_filter, &input)
            && let Some(input_cf) = in_filter.is_color_filter_node()
        {
            color_filter = cf.make_composed(Some(input_cf));
            input = in_filter.get_input(0).cloned();
        }
        let filter = input;
        match color_filter {
            Some(cf) => Some(PrecompileImageFilter::new(
                Arc::new(ColorFilterFilter {
                    color_filter: Some(cf),
                }),
                vec![filter],
            )),
            None => filter,
        }
    }

    /// `PrecompileImageFilters::DisplacementMap(input)`.
    #[must_use]
    pub fn displacement_map(input: Option<PrecompileImageFilter>) -> PrecompileImageFilter {
        PrecompileImageFilter::new(Arc::new(DisplacementMapFilter), vec![input])
    }

    /// `PrecompileImageFilters::Lighting(input)`.
    #[must_use]
    pub fn lighting(input: Option<PrecompileImageFilter>) -> PrecompileImageFilter {
        PrecompileImageFilter::new(Arc::new(LightingFilter), vec![input])
    }

    /// `PrecompileImageFilters::MatrixConvolution(input)`.
    #[must_use]
    pub fn matrix_convolution(input: Option<PrecompileImageFilter>) -> PrecompileImageFilter {
        PrecompileImageFilter::new(Arc::new(MatrixConvolutionFilter), vec![input])
    }

    /// `PrecompileImageFilters::Morphology(input)`.
    #[must_use]
    pub fn morphology(input: Option<PrecompileImageFilter>) -> PrecompileImageFilter {
        PrecompileImageFilter::new(Arc::new(MorphologyFilter), vec![input])
    }
}
