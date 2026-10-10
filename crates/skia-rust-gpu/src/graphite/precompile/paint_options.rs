// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PaintOptions.h,
// src/gpu/graphite/precompile/PaintOptions.cpp and PaintOptionsPriv.h

//! `PaintOptions`: the precompile analog of `SkPaint`. Each slot (shader, color filter, blend
//! mode, blender, clip shader, image filter, mask filter) lists its options; `build_combinations`
//! walks every combination of them and reports the pipeline key each one produces.

use skia_rust_core::blend_mode::BlendMode;

use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::key_context::{KeyContext, KeyGenFlags};
use crate::graphite::precompile::base::select_option;
use crate::graphite::precompile::blender::{PrecompileBlender, PrecompileBlenders};
use crate::graphite::precompile::color_filter::{PrecompileColorFilter, PrecompileColorFilters};
use crate::graphite::precompile::image_filter::PrecompileImageFilter;
use crate::graphite::precompile::mask_filter::PrecompileMaskFilter;
use crate::graphite::precompile::paint_option::PaintOption;
use crate::graphite::precompile::shader::{PrecompileShader, PrecompileShaders};
use crate::graphite::precompile::shader_effects::ctm;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::Coverage;
use crate::graphite::texture_format::TextureFormat;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// `PaintOptions::ProcessCombination`: called once per combination with its pipeline key ID.
pub type ProcessCombination<'a> =
    dyn Fn(UniquePaintParamsID, DrawTypeFlags, bool, Coverage, &RenderPassDesc) + 'a;

/// `PaintOptions`: the options for each slot of a paint, from which precompilation combinations
/// are generated.
// Port of: include/gpu/graphite/precompile/PaintOptions.h#L28-L200 (chrome/m156)
#[derive(Clone, Debug)]
pub struct PaintOptions {
    shader_options: Vec<PrecompileShader>,
    color_filter_options: Vec<Option<PrecompileColorFilter>>,
    blend_mode_options: Vec<BlendMode>,
    blender_options: Vec<PrecompileBlender>,
    clip_shader_options: Vec<Option<PrecompileShader>>,
    image_filter_options: Vec<Option<PrecompileImageFilter>>,
    mask_filter_options: Vec<Option<PrecompileMaskFilter>>,
    primitive_blend_mode: BlendMode,
    skip_color_xform: bool,
    dither: bool,
    paint_color_is_opaque: bool,
}

impl Default for PaintOptions {
    fn default() -> Self {
        Self::new()
    }
}

impl PaintOptions {
    /// `PaintOptions()`: default initialized, with no options in any slot.
    #[must_use]
    pub fn new() -> Self {
        Self {
            shader_options: Vec::new(),
            color_filter_options: Vec::new(),
            blend_mode_options: Vec::new(),
            blender_options: Vec::new(),
            clip_shader_options: Vec::new(),
            image_filter_options: Vec::new(),
            mask_filter_options: Vec::new(),
            primitive_blend_mode: BlendMode::SrcOver,
            skip_color_xform: false,
            dither: false,
            paint_color_is_opaque: true,
        }
    }

    /// `setShaders(shaders)`: the shader options.
    #[doc(alias = "setShaders")]
    pub fn set_shaders(&mut self, shaders: &[PrecompileShader]) {
        self.shader_options = shaders.to_vec();
    }

    /// `getShaders()`.
    #[must_use]
    #[doc(alias = "getShaders")]
    pub fn get_shaders(&self) -> &[PrecompileShader] {
        &self.shader_options
    }

    /// `setImageFilters(imageFilters)`: the image filter options (a `None` is no filter).
    #[doc(alias = "setImageFilters")]
    pub fn set_image_filters(&mut self, image_filters: &[Option<PrecompileImageFilter>]) {
        self.image_filter_options = image_filters.to_vec();
    }

    /// `getImageFilters()`.
    #[must_use]
    #[doc(alias = "getImageFilters")]
    pub fn get_image_filters(&self) -> &[Option<PrecompileImageFilter>] {
        &self.image_filter_options
    }

    /// `setMaskFilters(maskFilters)`: the mask filter options (a `None` is no filter).
    #[doc(alias = "setMaskFilters")]
    pub fn set_mask_filters(&mut self, mask_filters: &[Option<PrecompileMaskFilter>]) {
        self.mask_filter_options = mask_filters.to_vec();
    }

    /// `getMaskFilters()`.
    #[must_use]
    #[doc(alias = "getMaskFilters")]
    pub fn get_mask_filters(&self) -> &[Option<PrecompileMaskFilter>] {
        &self.mask_filter_options
    }

    /// `setColorFilters(colorFilters)`: the color filter options (a `None` is no filter).
    #[doc(alias = "setColorFilters")]
    pub fn set_color_filters(&mut self, color_filters: &[Option<PrecompileColorFilter>]) {
        self.color_filter_options = color_filters.to_vec();
    }

    /// `getColorFilters()`.
    #[must_use]
    #[doc(alias = "getColorFilters")]
    pub fn get_color_filters(&self) -> &[Option<PrecompileColorFilter>] {
        &self.color_filter_options
    }

    /// `setBlendModes(blendModes)`.
    #[doc(alias = "setBlendModes")]
    pub fn set_blend_modes(&mut self, blend_modes: &[BlendMode]) {
        self.blend_mode_options = blend_modes.to_vec();
    }

    /// `getBlendModes()`.
    #[must_use]
    #[doc(alias = "getBlendModes")]
    pub fn get_blend_modes(&self) -> &[BlendMode] {
        &self.blend_mode_options
    }

    /// `addBlendMode(bm)`.
    #[doc(alias = "addBlendMode")]
    pub fn add_blend_mode(&mut self, bm: BlendMode) {
        self.blend_mode_options.push(bm);
    }

    /// `setBlenders(blenders)`: a blender that is a blend mode goes into the blend-mode options.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L68-L76 (chrome/m156)
    #[doc(alias = "setBlenders")]
    pub fn set_blenders(&mut self, blenders: &[PrecompileBlender]) {
        for b in blenders {
            if let Some(bm) = b.as_blend_mode() {
                self.blend_mode_options.push(bm);
            } else {
                self.blender_options.push(b.clone());
            }
        }
    }

    /// `getBlenders()`: the blenders that are not blend modes.
    #[must_use]
    #[doc(alias = "getBlenders")]
    pub fn get_blenders(&self) -> &[PrecompileBlender] {
        &self.blender_options
    }

    /// `setDither(dither)`.
    #[doc(alias = "setDither")]
    pub fn set_dither(&mut self, dither: bool) {
        self.dither = dither;
    }

    /// `isDither()`.
    #[must_use]
    #[doc(alias = "isDither")]
    pub fn is_dither(&self) -> bool {
        self.dither
    }

    /// `setPaintColorIsOpaque(paintColorIsOpaque)`.
    #[doc(alias = "setPaintColorIsOpaque")]
    pub fn set_paint_color_is_opaque(&mut self, paint_color_is_opaque: bool) {
        self.paint_color_is_opaque = paint_color_is_opaque;
    }

    /// `isPaintColorOpaque()`.
    #[must_use]
    #[doc(alias = "isPaintColorOpaque")]
    pub fn is_paint_color_opaque(&self) -> bool {
        self.paint_color_is_opaque
    }

    /// `PaintOptionsPriv::addColorFilter(cf)`.
    #[doc(hidden)]
    pub fn add_color_filter(&mut self, cf: Option<PrecompileColorFilter>) {
        self.color_filter_options.push(cf);
    }

    /// `PaintOptionsPriv::setClipShaders(clipShaders)`: each clip shader is wrapped in a CTM
    /// shader, followed by its inverted (difference-clip) variant.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L78-L92 (chrome/m156)
    #[doc(hidden)]
    pub fn set_clip_shaders(&mut self, clip_shaders: &[PrecompileShader]) {
        self.clip_shader_options.clear();
        for cs in clip_shaders {
            // All clipShaders get wrapped in a CTMShader ...
            let with_ctm = ctm(std::slice::from_ref(cs));
            // ... and, if it is a SkClipOp::kDifference clip, an additional ColorFilterShader.
            let inverted = with_ctm.make_with_color_filter(Some(PrecompileColorFilters::blend()));
            self.clip_shader_options.push(Some(with_ctm));
            self.clip_shader_options.push(Some(inverted));
        }
    }

    /// `PaintOptionsPriv::setPrimitiveBlendMode(bm)`.
    #[doc(hidden)]
    pub fn set_primitive_blend_mode(&mut self, bm: BlendMode) {
        self.primitive_blend_mode = bm;
    }

    /// `PaintOptionsPriv::setSkipColorXform(skipColorXform)`.
    #[doc(hidden)]
    pub fn set_skip_color_xform(&mut self, skip_color_xform: bool) {
        self.skip_color_xform = skip_color_xform;
    }

    /// `numShaderCombinations()`: the solid color counts as one when there are no shaders.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L94-L102 (chrome/m156)
    fn num_shader_combinations(&self) -> i32 {
        let n: i32 = self
            .shader_options
            .iter()
            .map(PrecompileShader::num_combinations)
            .sum();
        if n > 0 { n } else { 1 }
    }

    /// `numColorFilterCombinations()`.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L104-L116 (chrome/m156)
    fn num_color_filter_combinations(&self) -> i32 {
        let mut n = 0;
        for cf in &self.color_filter_options {
            match cf {
                None => n += 1,
                Some(cf) => n += cf.num_combinations(),
            }
        }
        if n > 0 { n } else { 1 }
    }

    /// `numBlendCombinations()`.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L118-L131 (chrome/m156)
    fn num_blend_combinations(&self) -> i32 {
        let mut n = i32::try_from(self.blend_mode_options.len()).unwrap_or(i32::MAX);
        for b in &self.blender_options {
            debug_assert!(b.as_blend_mode().is_none());
            n += b.num_child_combinations();
        }
        if n > 0 { n } else { 1 } // a kSrcOver blend when none is specified
    }

    /// `numClipShaderCombinations()`.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L133-L145 (chrome/m156)
    fn num_clip_shader_combinations(&self) -> i32 {
        let mut n = 0;
        for cs in &self.clip_shader_options {
            match cs {
                Some(cs) => n += cs.num_child_combinations(),
                None => n += 1,
            }
        }
        if n > 0 { n } else { 1 }
    }

    /// `PaintOptionsPriv::numCombinations()`: the product of the slot counts.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L147-L152 (chrome/m156)
    #[must_use]
    #[doc(hidden)]
    pub fn num_combinations(&self) -> i32 {
        self.num_shader_combinations()
            * self.num_color_filter_combinations()
            * self.num_blend_combinations()
            * self.num_clip_shader_combinations()
    }

    /// `createKey(keyContext, targetFormat, desiredCombination, addPrimitiveBlender,
    /// addAnalyticClip, coverage)`: builds the key of one combination into the key builder.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L154-L211 (chrome/m156)
    #[allow(clippy::fn_params_excessive_bools)] // mirrors the C++ signature
    fn create_key(
        &self,
        key_context: &KeyContext<'_>,
        target_format: TextureFormat,
        desired_combination: i32,
        add_primitive_blender: bool,
        add_analytic_clip: bool,
        coverage: Coverage,
    ) {
        debug_assert!(desired_combination < self.num_combinations());
        let num_clip_shader_combos = self.num_clip_shader_combinations();
        let num_blend_mode_combos = self.num_blend_combinations();
        let num_color_filter_combinations = self.num_color_filter_combinations();

        let desired_clip_shader_combination = desired_combination % num_clip_shader_combos;
        let mut remaining = desired_combination / num_clip_shader_combos;
        let desired_blend_combination = remaining % num_blend_mode_combos;
        remaining /= num_blend_mode_combos;
        let desired_color_filter_combination = remaining % num_color_filter_combinations;
        remaining /= num_color_filter_combinations;
        let desired_shader_combination = remaining;
        debug_assert!(desired_shader_combination < self.num_shader_combinations());

        let clip_shader = select_option(&self.clip_shader_options, desired_clip_shader_combination);

        let blend_mode_count = i32::try_from(self.blend_mode_options.len()).unwrap_or(i32::MAX);
        let mut final_blender = if desired_blend_combination < blend_mode_count {
            let bm =
                self.blend_mode_options[usize::try_from(desired_blend_combination).unwrap_or(0)];
            (Some(PrecompileBlenders::mode(bm)), 0)
        } else {
            let blender_options: Vec<Option<PrecompileBlender>> =
                self.blender_options.iter().cloned().map(Some).collect();
            select_option(
                &blender_options,
                desired_blend_combination - blend_mode_count,
            )
        };
        if final_blender.0.is_none() {
            final_blender = (Some(PrecompileBlenders::mode(BlendMode::SrcOver)), 0);
        }

        let shader_options: Vec<Option<PrecompileShader>> =
            self.shader_options.iter().cloned().map(Some).collect();
        let option = PaintOption::new(
            self.paint_color_is_opaque,
            final_blender,
            select_option(&shader_options, desired_shader_combination),
            select_option(&self.color_filter_options, desired_color_filter_combination),
            add_primitive_blender,
            self.primitive_blend_mode,
            self.skip_color_xform,
            clip_shader,
            coverage,
            target_format,
            self.dither,
            add_analytic_clip,
        );
        option.to_key(key_context);
    }

    /// `PaintOptions::buildCombinations`: runs `process_combination` on the pipeline key of every
    /// combination of these options.
    // Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L239-L326 (chrome/m156)
    #[doc(hidden)]
    pub fn build_combinations(
        &self,
        key_context: &KeyContext<'_>,
        draw_types: DrawTypeFlags,
        with_primitive_blender: bool,
        coverage: Coverage,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        if !self.image_filter_options.is_empty() || !self.mask_filter_options.is_empty() {
            // When image filtering, the original blend mode is taken over by the restore paint.
            let mut tmp = self.clone();
            tmp.set_image_filters(&[]);
            tmp.set_mask_filters(&[]);
            tmp.add_blend_mode(BlendMode::SrcOver);

            if !self.image_filter_options.is_empty() {
                let mut new_cfs: Vec<Option<PrecompileColorFilter>> =
                    tmp.color_filter_options.clone();
                if new_cfs.is_empty() {
                    // Port note: this null is an over-generation that Skia makes for the
                    // SkPaint the PaintParamsKeyTest builds without color filters.
                    new_cfs.push(None);
                }
                // As in SkCanvasPriv::ImageToColorFilter, we fuse CFIFs into the base draw's CFs.
                for o in &self.image_filter_options {
                    let image_filters_cf = o
                        .as_ref()
                        .and_then(PrecompileImageFilter::as_a_color_filter);
                    if let Some(image_filters_cf) = image_filters_cf {
                        if tmp.color_filter_options.is_empty() {
                            new_cfs.push(Some(image_filters_cf));
                        } else {
                            for cf in &tmp.color_filter_options {
                                let new_cf = image_filters_cf.make_composed(cf.clone());
                                new_cfs.push(new_cf);
                            }
                        }
                    }
                }
                tmp.set_color_filters(&new_cfs);
            }

            tmp.build_combinations(
                key_context,
                draw_types,
                with_primitive_blender,
                coverage,
                render_pass_desc,
                process_combination,
            );
            create_image_drawing_pipelines(
                key_context,
                self,
                render_pass_desc,
                process_combination,
            );
            for o in self.image_filter_options.iter().flatten() {
                o.create_pipelines(key_context, render_pass_desc, process_combination);
            }
            for o in self.mask_filter_options.iter().flatten() {
                o.create_pipelines(key_context, self, render_pass_desc, process_combination);
            }
        } else {
            let num_combinations = self.num_combinations();
            // This matches the logic in Device::drawGeometry() that optimizes inner-fill capable
            // and non-AA draws to disable HW blending when possible.
            let owned_context;
            let final_context: &KeyContext<'_> = if draw_types.contains(DrawTypeFlags::SIMPLE_SHAPE)
                || coverage == Coverage::None
            {
                owned_context = key_context.with_extra_flags(KeyGenFlags::PREFER_FIXED_SRC_BLEND);
                &owned_context
            } else {
                key_context
            };
            for i in 0..num_combinations {
                // Since the precompilation path's uniforms aren't used and don't change the key,
                // the exact layout doesn't matter.
                key_context
                    .pipeline_data_gatherer()
                    .borrow_mut()
                    .reset_for_draw();
                key_context
                    .paint_params_key_builder()
                    .borrow_mut()
                    .reset_for_draw();
                self.create_key(
                    final_context,
                    render_pass_desc.color_attachment.format,
                    i,
                    with_primitive_blender,
                    draw_types.contains(DrawTypeFlags::ANALYTIC_CLIP),
                    coverage,
                );
                // Reset the builder after we get the paint ID; the key is not needed afterwards.
                let paint_id = final_context.dict().find_or_create_for_builder(
                    &mut final_context.paint_params_key_builder().borrow_mut(),
                );
                process_combination(
                    paint_id,
                    draw_types,
                    with_primitive_blender,
                    coverage,
                    render_pass_desc,
                );
            }
        }
    }
}

/// `create_image_drawing_pipelines`: the image drawing pipelines a filtered paint also needs.
// Port of: src/gpu/graphite/precompile/PaintOptions.cpp#L213-L237 (chrome/m156)
fn create_image_drawing_pipelines(
    key_context: &KeyContext<'_>,
    orig: &PaintOptions,
    render_pass_desc: &RenderPassDesc,
    process_combination: &ProcessCombination<'_>,
) {
    let mut image_paint_options = PaintOptions::new();
    // For imagefilters we know we don't have alpha-only textures and don't need cubic filtering.
    let image_shader = PrecompileShaders::image(
        crate::graphite::precompile::shader_image::ImageShaderFlags::NO_ALPHA_NO_CUBIC,
        &[],
        &crate::graphite::precompile::shader_image::ALL_TILE_MODES,
    );
    image_paint_options.set_shaders(&[image_shader]);
    image_paint_options.set_blend_modes(orig.get_blend_modes());
    image_paint_options.set_blenders(orig.get_blenders());
    image_paint_options.set_color_filters(orig.get_color_filters());
    image_paint_options.add_color_filter(None);
    image_paint_options.build_combinations(
        key_context,
        DrawTypeFlags::SIMPLE_SHAPE,
        false,
        Coverage::SingleChannel,
        render_pass_desc,
        process_combination,
    );
}
