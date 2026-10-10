// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/graphite/PublicPrecompile.cpp (chrome/m156),
//          include/gpu/graphite/precompile/Precompile.h (chrome/m156)

//! `Precompile()`: creates the pipelines for every combination of a set of paint options, draw
//! types and render-pass properties, ahead of the first draw.

use std::cell::RefCell;
use std::sync::Arc;

use skia_rust_core::alpha_type::AlphaType;
use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_space::ColorSpace;
use skia_rust_core::color_type::ColorType;
use skia_rust_core::image_info::ColorInfo;

use crate::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::graphics_pipeline::PipelineCreationFlags;
use crate::graphite::graphics_pipeline_desc::GraphicsPipelineDesc;
use crate::graphite::graphite_types::DepthStencilFlags;
use crate::graphite::key_context::KeyContext;
use crate::graphite::paint_params_key::PaintParamsKeyBuilder;
use crate::graphite::pipeline_data::PipelineDataGatherer;
use crate::graphite::pipeline_manager::PipelineCreationContext;
use crate::graphite::precompile::color_filter::PrecompileColorFilters;
use crate::graphite::precompile::paint_options::PaintOptions;
use crate::graphite::precompile_context::PrecompileContext;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::{Coverage, RenderStepID};
use crate::graphite::resource_types::{Layout, LoadOp, StoreOp};
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::texture_format::write_swizzle_for_color_type;
use crate::graphite::texture_info::texture_info_priv;
use crate::graphite::unique_paint_params_id::UniquePaintParamsID;

/// `skgpu::graphite::RenderPassProperties`: the render-pass properties a precompilation covers.
// Port of: include/gpu/graphite/precompile/Precompile.h#L26-L39 (chrome/m156)
#[derive(Clone, Debug, PartialEq)]
pub struct RenderPassProperties {
    /// `fDSFlags`.
    pub ds_flags: DepthStencilFlags,
    /// `fDstCT`.
    pub dst_ct: ColorType,
    /// `fDstCS`.
    pub dst_cs: Option<ColorSpace>,
    /// `fRequiresMSAA`.
    pub requires_msaa: bool,
}

impl Default for RenderPassProperties {
    // Port of: include/gpu/graphite/precompile/Precompile.h#L35-L38 (chrome/m156)
    fn default() -> Self {
        Self {
            ds_flags: DepthStencilFlags::None,
            dst_ct: ColorType::RGBA8888,
            dst_cs: None,
            requires_msaa: false,
        }
    }
}

// Port of: src/gpu/graphite/PublicPrecompile.cpp#L42-L87 (chrome/m156), the anonymous `compile`
fn compile(
    shared_context: &Arc<dyn PipelineCreationContext>,
    key_context: &KeyContext<'_>,
    unique_id: UniquePaintParamsID,
    draw_types: DrawTypeFlags,
    render_pass_desc: &RenderPassDesc,
    with_primitive_blender: bool,
    coverage: Coverage,
) {
    let base = shared_context.shared_context();
    let renderer_provider = shared_context.renderer_provider();
    let pipeline_manager = base.pipeline_manager();

    for r in renderer_provider.renderers() {
        if (r.draw_types() & draw_types).is_empty() {
            continue;
        }

        if r.emits_primitive_color() != with_primitive_blender {
            // UniqueIDs are explicitly built either w/ or w/o primitiveBlending so must
            // match what the Renderer requires
            continue;
        }

        if r.coverage() != coverage {
            // For now, UniqueIDs are explicitly built with a specific type of coverage so must
            // match what the Renderer requires
            continue;
        }

        for s in r.steps() {
            debug_assert!(
                !s.performs_shading() || s.emits_primitive_color() == with_primitive_blender
            );

            let paint_id = if s.performs_shading() {
                unique_id
            } else {
                UniquePaintParamsID::invalid()
            };

            let _handle = pipeline_manager.create_handle(
                shared_context,
                Some(key_context.rt_effect_dict()),
                &GraphicsPipelineDesc::new(s.render_step_id(), paint_id),
                render_pass_desc,
                PipelineCreationFlags::FOR_PRECOMPILATION,
            );
        }
    }
}

// Port of: src/gpu/graphite/PublicPrecompile.cpp#L339-L358 (chrome/m156), `PrecompileCombinations`

fn precompile_combinations(
    shared_context: &Arc<dyn PipelineCreationContext>,
    options: &PaintOptions,
    key_context: &KeyContext<'_>,
    draw_types: DrawTypeFlags,
    with_primitive_blender: bool,
    coverage: Coverage,
    render_pass_desc_in: &RenderPassDesc,
) {
    if draw_types.is_empty() {
        return;
    }

    options.build_combinations(
        key_context,
        draw_types,
        with_primitive_blender,
        coverage,
        render_pass_desc_in,
        &|unique_id, draw_types, with_primitive_blender, coverage, render_pass_desc| {
            compile(
                shared_context,
                key_context,
                unique_id,
                draw_types,
                render_pass_desc,
                with_primitive_blender,
                coverage,
            );
        },
    );
}

/// `Precompile(precompileContext, options, drawTypes, renderPassProperties)`: creates the
/// pipelines for `options` and `draw_types` in each render pass described by
/// `render_pass_properties`.
// Port of: src/gpu/graphite/PublicPrecompile.cpp#L89-L337 (chrome/m156)
#[doc(alias = "Precompile")]
// One-to-one with Skia's `Precompile()`, which is one function: the loops over the render passes
// and the draw types stay together so that the order of the combinations matches Skia's.
#[allow(clippy::too_many_lines)]
pub fn precompile(
    precompile_context: &PrecompileContext,
    options: &PaintOptions,
    draw_types: DrawTypeFlags,
    render_pass_properties: &[RenderPassProperties],
) {
    let shared_context = precompile_context.shared_context();
    let base = shared_context.shared_context();
    let dict = base.shader_code_dictionary();
    let caps = base.caps();

    let rt_effect_dict = Arc::new(RuntimeEffectDictionary::new());

    for rpp in render_pass_properties {
        // TODO: Allow the client to pass in mipmapping and protection too?
        let info = caps.get_default_sampled_texture_info(
            rpp.dst_ct,
            Mipmapped::No,
            Protected::No,
            Renderable::Yes,
        );
        let Some(write_swizzle) =
            write_swizzle_for_color_type(rpp.dst_ct, texture_info_priv::view_format(&info))
        else {
            continue; // Skip
        };

        // TODO(robertphillips): address mismatches between the MSAA requirements of the Renderers
        // associated w/ the requested drawTypes and the specified MSAA setting

        // On Native Metal, the LoadOp, StoreOp and clearColor fields don't influence
        // the actual RenderPassDescKey.
        // For Dawn, the LoadOp will sometimes matter. We add an extra LoadOp::kLoad combination
        // when necessary.
        let load_ops = [LoadOp::Clear, LoadOp::Load];

        let mut num_load_ops = 1;
        if rpp.requires_msaa
            && !caps.msaa_render_to_single_sampled_support()
            && caps.load_op_affects_msaa_pipelines()
        {
            num_load_ops = 2;
        }

        for load_op in load_ops.into_iter().take(num_load_ops) {
            let render_pass_desc = RenderPassDesc::make(
                caps,
                &info,
                load_op,
                StoreOp::Store,
                rpp.ds_flags,
                /* clear_color= */ [0.0, 0.0, 0.0, 0.0],
                rpp.requires_msaa,
                write_swizzle,
                caps.get_dst_read_strategy(),
            );

            let ci = ColorInfo::new(rpp.dst_ct, AlphaType::Premul, rpp.dst_cs.clone());

            // The PipelineDataGatherer handles uniform data; in the pre-compile case we don't need
            // to record the uniform data but the process of generating it is required to create the
            // correct key.
            let gatherer = RefCell::new(PipelineDataGatherer::new(Layout::Metal));
            let builder = RefCell::new(PaintParamsKeyBuilder::new(dict));
            let key_context = KeyContext::new(
                base.caps_arc().clone(),
                &builder,
                &gatherer,
                dict,
                Arc::clone(&rt_effect_dict),
                &ci,
            );

            for coverage in [Coverage::None, Coverage::SingleChannel] {
                precompile_combinations(
                    shared_context,
                    options,
                    &key_context,
                    draw_types
                        & !(DrawTypeFlags::BITMAP_TEXT_COLOR
                            | DrawTypeFlags::BITMAP_TEXT_LCD
                            | DrawTypeFlags::SDF_TEXT_LCD
                            | DrawTypeFlags::DRAW_VERTICES
                            | DrawTypeFlags::DROP_SHADOWS),
                    /* with_primitive_blender= */ false,
                    coverage,
                    &render_pass_desc,
                );
            }

            if draw_types.contains(DrawTypeFlags::NON_SIMPLE_SHAPE) {
                // Special case handling to pick up the:
                //     "CoverBoundsRenderStep[InverseCover] + (empty)"
                // pipelines.
                let render_step = shared_context
                    .renderer_provider()
                    .lookup(RenderStepID::CoverBounds_InverseCover)
                    .cloned();
                if let Some(render_step) = render_step {
                    let _handle = base.pipeline_manager().create_handle(
                        shared_context,
                        Some(Arc::clone(&rt_effect_dict)),
                        &GraphicsPipelineDesc::new(
                            render_step.render_step_id(),
                            UniquePaintParamsID::invalid(),
                        ),
                        &render_pass_desc,
                        PipelineCreationFlags::FOR_PRECOMPILATION,
                    );
                }
            }

            if draw_types.contains(DrawTypeFlags::BITMAP_TEXT_COLOR) {
                let reduced_types =
                    draw_types & (DrawTypeFlags::BITMAP_TEXT_COLOR | DrawTypeFlags::ANALYTIC_CLIP);
                // For color emoji text, shaders don't affect the final color
                let mut tmp = options.clone();
                tmp.set_shaders(&[]);
                tmp.set_primitive_blend_mode(BlendMode::DstIn);

                // ARGB text doesn't emit coverage and always has a primitive blender
                precompile_combinations(
                    shared_context,
                    &tmp,
                    &key_context,
                    reduced_types,
                    /* with_primitive_blender= */ true,
                    Coverage::None,
                    &render_pass_desc,
                );
            }

            if draw_types.intersects(DrawTypeFlags::BITMAP_TEXT_LCD | DrawTypeFlags::SDF_TEXT_LCD) {
                let reduced_types = draw_types
                    & (DrawTypeFlags::BITMAP_TEXT_LCD
                        | DrawTypeFlags::SDF_TEXT_LCD
                        | DrawTypeFlags::ANALYTIC_CLIP);
                // LCD-based text always emits LCD coverage but never has primitiveBlenders
                precompile_combinations(
                    shared_context,
                    options,
                    &key_context,
                    reduced_types,
                    /* with_primitive_blender= */ false,
                    Coverage::Lcd,
                    &render_pass_desc,
                );
            }

            if draw_types.contains(DrawTypeFlags::DRAW_VERTICES) {
                let reduced_types =
                    draw_types & (DrawTypeFlags::DRAW_VERTICES | DrawTypeFlags::ANALYTIC_CLIP);
                // drawVertices w/ colors use a primitiveBlender while those w/o don't. It never
                // emits coverage.
                for with_primitive_blender in [true, false] {
                    precompile_combinations(
                        shared_context,
                        options,
                        &key_context,
                        reduced_types,
                        with_primitive_blender,
                        Coverage::None,
                        &render_pass_desc,
                    );
                }
            }

            if draw_types.contains(DrawTypeFlags::DROP_SHADOWS) {
                let reduced_types =
                    draw_types & (DrawTypeFlags::DROP_SHADOWS | DrawTypeFlags::ANALYTIC_CLIP);

                let mut new_options = PaintOptions::default();
                new_options.set_blend_modes(&[BlendMode::SrcOver]);

                // Analytic
                {
                    precompile_combinations(
                        shared_context,
                        &new_options,
                        &key_context,
                        reduced_types,
                        /* with_primitive_blender= */ false,
                        Coverage::SingleChannel,
                        &render_pass_desc,
                    );
                }

                // Geometric
                {
                    let cf = PrecompileColorFilters::compose(
                        &[Some(PrecompileColorFilters::blend_modes(&[
                            BlendMode::Modulate,
                        ]))],
                        &[Some(PrecompileColorFilters::gaussian())],
                    );

                    new_options.set_color_filters(&[cf]);
                    new_options.set_primitive_blend_mode(BlendMode::Dst);
                    new_options.set_skip_color_xform(true);

                    precompile_combinations(
                        shared_context,
                        &new_options,
                        &key_context,
                        reduced_types,
                        /* with_primitive_blender= */ true,
                        Coverage::None,
                        &render_pass_desc,
                    );
                }
            }
        }
    }
}
