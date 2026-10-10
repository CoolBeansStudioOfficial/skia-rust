// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/precompile/PrecompileMaskFilter.h and
// src/gpu/graphite/precompile/PrecompileMaskFilter.cpp

//! `PrecompileMaskFilter` and the `PrecompileMaskFilters` factories.

use std::sync::Arc;

use skia_rust_core::blend_mode::BlendMode;
use skia_rust_core::color_type::ColorType;

use crate::gpu::gpu_types::{Mipmapped, Protected, Renderable};
use crate::graphite::draw_types::DrawTypeFlags;
use crate::graphite::graphite_types::DepthStencilFlags;
use crate::graphite::key_context::KeyContext;
use crate::graphite::precompile::image_filter::create_blur_image_filter_pipelines;
use crate::graphite::precompile::paint_options::{PaintOptions, ProcessCombination};
use crate::graphite::precompile::shader::PrecompileShaders;
use crate::graphite::render_pass_desc::RenderPassDesc;
use crate::graphite::render_step::Coverage;
use crate::graphite::resource_types::{LoadOp, StoreOp};
use crate::graphite::texture_info::TextureInfo;
use skia_rust_core::swizzle::Swizzle;

/// The virtual interface of `PrecompileMaskFilter`: `createPipelines`.
pub(crate) trait MaskFilterImpl: Send + Sync {
    /// `createPipelines(keyContext, paintOptions, renderPassDesc, processCombination)`.
    fn create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        paint_options: &PaintOptions,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    );
}

/// `PrecompileMaskFilter`: a shared precompile mask filter.
#[doc(alias = "SkMaskFilter")]
#[derive(Clone)]
pub struct PrecompileMaskFilter {
    pub(crate) imp: Arc<dyn MaskFilterImpl>,
}

impl std::fmt::Debug for PrecompileMaskFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileMaskFilter")
            .finish_non_exhaustive()
    }
}

impl PrecompileMaskFilter {
    /// `PrecompileMaskFilter::createPipelines`.
    pub fn create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        paint_options: &PaintOptions,
        render_pass_desc: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        self.imp.create_pipelines(
            key_context,
            paint_options,
            render_pass_desc,
            process_combination,
        );
    }
}

/// `PrecompileBlurMaskFilter`.
struct BlurMaskFilter;

impl MaskFilterImpl for BlurMaskFilter {
    // Port of: PrecompileBlurMaskFilter::createPipelines (chrome/m156)
    fn create_pipelines(
        &self,
        key_context: &KeyContext<'_>,
        paint_options: &PaintOptions,
        render_pass_desc_in: &RenderPassDesc,
        process_combination: &ProcessCombination<'_>,
    ) {
        let caps = key_context.caps();
        let info: TextureInfo = caps.get_default_sampled_texture_info(
            ColorType::Alpha8,
            Mipmapped::No,
            Protected::No,
            Renderable::Yes,
        );
        let coverage_render_pass_desc = RenderPassDesc::make(
            caps,
            &info,
            LoadOp::Clear,
            StoreOp::Store,
            DepthStencilFlags::Depth,
            [0.0, 0.0, 0.0, 0.0],
            false,
            Swizzle::new("a000"),
            caps.get_dst_read_strategy(),
        );

        create_blur_image_filter_pipelines(
            key_context,
            &coverage_render_pass_desc,
            process_combination,
        );
        {
            let mut restore_options = paint_options.clone();
            restore_options.set_mask_filters(&[]);
            restore_options.build_combinations(
                key_context,
                DrawTypeFlags::INTERNAL_COVERAGE_MASK,
                false,
                Coverage::SingleChannel,
                render_pass_desc_in,
                process_combination,
            );
        }
        {
            let mut coverage_options = PaintOptions::new();
            coverage_options.set_shaders(&[PrecompileShaders::color()]);
            coverage_options.set_blend_modes(&[BlendMode::SrcOver]);
            coverage_options.build_combinations(
                key_context,
                DrawTypeFlags::ANALYTIC_RRECT,
                false,
                Coverage::SingleChannel,
                &coverage_render_pass_desc,
                process_combination,
            );
        }
    }
}

/// `PrecompileMaskFilters`: the factories for precompile mask filters.
#[derive(Debug, Clone, Copy)]
pub struct PrecompileMaskFilters;

impl PrecompileMaskFilters {
    /// `PrecompileMaskFilters::Blur()`.
    #[must_use]
    pub fn blur() -> PrecompileMaskFilter {
        PrecompileMaskFilter {
            imp: Arc::new(BlurMaskFilter),
        }
    }
}
