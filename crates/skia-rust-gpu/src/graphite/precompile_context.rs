// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: src/gpu/graphite/PrecompileContext.cpp (chrome/m156),
//          include/gpu/graphite/PrecompileContext.h (chrome/m156)

//! `PrecompileContext`: the part of a `Context` that precompiles pipelines. It can keep the
//! pipeline machinery alive on its own, so it may outlive the `Context` it was made from.

use std::sync::Arc;
use std::time::Duration;

use skia_rust_core::data::Data;

use crate::gpu::gpu_types::StdSteadyClockTimePoint;
use crate::graphite::graphics_pipeline::PipelineCreationFlags;
use crate::graphite::pipeline_manager::PipelineCreationContext;
use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::serialization_utils::{data_contains_external_format, data_to_pipeline_desc};

/// `PrecompileContext::StatOptions`.
// Port of: include/gpu/graphite/PrecompileContext.h#L37-L48 (chrome/m156)
#[doc(alias = "PrecompileContext::StatOptions")]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatOptions {
    /// Emit histograms for Skia's Precompiled Pipeline usage. The histograms are not recorded in
    /// this port (`GlobalCache::report_precompile_stats`), so this option does nothing.
    #[default]
    Precompile,
    /// Emit histograms for Skia's Pipeline cache usage. Starts a new epoch.
    PipelineCache,
}

/// `PrecompileContext::ExternalFormatResult`.
// Port of: include/gpu/graphite/PrecompileContext.h#L99-L103 (chrome/m156)
#[doc(alias = "PrecompileContext::ExternalFormatResult")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalFormatResult {
    /// The serialized key was invalid.
    Invalid,
    /// The key uses no external format.
    NoExternalFormat,
    /// The key uses an external format.
    HasExternalFormat,
}

/// `skgpu::graphite::PrecompileContext`: precompiles serialized pipeline keys and looks at them.
// Port of: include/gpu/graphite/PrecompileContext.h#L20-L120 (chrome/m156)
#[doc(alias = "skgpu::graphite::PrecompileContext")]
pub struct PrecompileContext {
    /// `fSharedContext`: keeps the shared state (caches, caps, pipeline manager) alive.
    shared_context: Arc<dyn PipelineCreationContext>,
}

impl std::fmt::Debug for PrecompileContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PrecompileContext").finish_non_exhaustive()
    }
}

impl PrecompileContext {
    /// `PrecompileContext(sharedContext)`, made by `Context::makePrecompileContext`.
    // Port of: src/gpu/graphite/PrecompileContext.cpp#L24-L26 (chrome/m156)
    #[must_use]
    pub fn new(shared_context: Arc<dyn PipelineCreationContext>) -> Self {
        Self { shared_context }
    }

    /// The shared context this precompile context keeps alive (`priv().sharedContext()`).
    #[must_use]
    pub fn shared_context(&self) -> &Arc<dyn PipelineCreationContext> {
        &self.shared_context
    }

    /// `purgePipelinesNotUsedInMs(msNotUsed)`: purges the pipelines not used in the last
    /// `ms_not_used`, whether or not the cache is over budget.
    // Port of: src/gpu/graphite/PrecompileContext.cpp#L32-L38 (chrome/m156)
    #[doc(alias = "purgePipelinesNotUsedInMs")]
    pub fn purge_pipelines_not_used_in_ms(&self, ms_not_used: Duration) {
        let purge_time: StdSteadyClockTimePoint = std::time::Instant::now()
            .checked_sub(ms_not_used)
            .unwrap_or_else(std::time::Instant::now);

        self.shared_context
            .shared_context()
            .global_cache()
            .purge_pipelines_not_used_since(purge_time);
    }

    /// `reportPipelineStats(option)`: emits the pipeline histograms for `option`.
    // Port of: src/gpu/graphite/PrecompileContext.cpp#L40-L48 (chrome/m156)
    #[doc(alias = "reportPipelineStats")]
    pub fn report_pipeline_stats(&self, option: StatOptions) {
        if option == StatOptions::PipelineCache {
            self.shared_context
                .shared_context()
                .global_cache()
                .report_cache_stats();
        }
    }

    /// `precompile(serializedPipelineKey)`: creates the pipeline the serialized key describes.
    /// `false` if the key is missing or malformed.
    // Port of: src/gpu/graphite/PrecompileContext.cpp#L50-L79 (chrome/m156)
    #[must_use]
    pub fn precompile(&self, serialized_pipeline_key: Option<&Data>) -> bool {
        let base = self.shared_context.shared_context();
        let caps = base.caps();
        let rt_effect_dict = Arc::new(RuntimeEffectDictionary::new());

        let Some((pipeline_desc, render_pass_desc)) =
            data_to_pipeline_desc(caps, base.shader_code_dictionary(), serialized_pipeline_key)
        else {
            return false;
        };

        let _handle = base.pipeline_manager().create_handle(
            &self.shared_context,
            Some(rt_effect_dict),
            &pipeline_desc,
            &render_pass_desc,
            PipelineCreationFlags::FOR_PRECOMPILATION,
        );

        true
    }

    /// `getPipelineLabel(serializedPipelineKey, uniqueHash)`: a human-readable version of a
    /// serialized key, and optionally its unique hash, which is only valid for the lifetime of
    /// the `Context`. `""` on failure (and `0` for the hash).
    // Port of: src/gpu/graphite/PrecompileContext.cpp#L81-L120 (chrome/m156)
    #[must_use]
    pub fn get_pipeline_label(
        &self,
        serialized_pipeline_key: Option<&Data>,
        mut unique_hash: Option<&mut u32>,
    ) -> String {
        if let Some(hash) = &mut unique_hash {
            **hash = 0;
        }
        let base = self.shared_context.shared_context();
        let caps = base.caps();
        let dict = base.shader_code_dictionary();

        // Deep in deserialize_graphics_pipeline_desc, this will have unpacked the PaintParamsKey
        // and then registered it with the ShaderCodeDictionary to get this session's
        // UniquePaintParamsID for it.
        let Some((pipeline_desc, render_pass_desc)) =
            data_to_pipeline_desc(caps, dict, serialized_pipeline_key)
        else {
            return String::new();
        };

        let renderer_provider = self.shared_context.renderer_provider();
        let Some(render_step) = renderer_provider.lookup(pipeline_desc.render_step_id()) else {
            return String::new();
        };

        if let Some(unique_hash) = unique_hash {
            // This will make use of the UniquePaintParamsID registered in data_to_pipeline_desc.
            let pipeline_key = caps.make_graphics_pipeline_key(&pipeline_desc, &render_pass_desc);
            *unique_hash = pipeline_key.hash();
        }

        crate::graphite::context_utils::get_pipeline_label(
            caps,
            dict,
            &render_pass_desc,
            &**render_step,
            pipeline_desc.paint_params_id(),
        )
    }

    /// `containsExternalFormat(serializedPipelineKey)`: whether the serialized key uses an
    /// external texture format.
    // Port of: src/gpu/graphite/PrecompileContext.cpp#L122-L136 (chrome/m156)
    #[must_use]
    pub fn contains_external_format(
        &self,
        serialized_pipeline_key: Option<&Data>,
    ) -> ExternalFormatResult {
        let base = self.shared_context.shared_context();
        match data_contains_external_format(
            base.caps(),
            base.shader_code_dictionary(),
            serialized_pipeline_key,
        ) {
            None => ExternalFormatResult::Invalid,
            Some(false) => ExternalFormatResult::NoExternalFormat,
            Some(true) => ExternalFormatResult::HasExternalFormat,
        }
    }
}
