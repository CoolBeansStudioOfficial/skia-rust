// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/gpu/graphite/ContextOptions.h

//! `ContextOptions`: the options a Graphite `Context` is created with.
//!
//! Only the options the caps and the resource layer read are ported (G11a). The callbacks
//! (`fPipelineCachingCallback`, `fPipelineCallback`), the executor, the persistent pipeline
//! storage, the shader error handler, the user-defined runtime effects and `fOptionsPriv` come
//! with the `Context` and `PipelineManager` (G9b, G6), whose types they name.

use std::fmt;
use std::sync::Arc;

use skia_rust_core::data::Data;
use skia_rust_core::size::ISize;

use crate::graphite::graphite_types::SampleCount;

/// `ContextOptions::kDefaultContextBudget`.
pub const DEFAULT_CONTEXT_BUDGET: usize = 256 * (1 << 20);

/// `ContextOptions::PipelineCacheOp`: why a pipeline callback runs.
// Port of: include/gpu/graphite/ContextOptions.h#L152-L155 (chrome/m156)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PipelineCacheOp {
    /// `kAddingPipeline`: a pipeline was compiled and added to the cache.
    AddingPipeline,
    /// `kPipelineFound`: a cached pipeline was requested again.
    PipelineFound,
}

/// `ContextOptions::PipelineCachingCallback`, as a closure. Skia passes a client `void*` context
/// separately; the closure captures it instead. The arguments are the operation, the pipeline's
/// label, its unique key hash, whether it came from precompilation, and the serialized key (only
/// when the key is serializable).
// Port of: include/gpu/graphite/ContextOptions.h#L157-L161 (chrome/m156)
pub type PipelineCachingCallbackFn =
    dyn Fn(PipelineCacheOp, &str, u32, bool, Option<&Data>) + Send + Sync;

/// `ContextOptions::PipelineCallback` (deprecated), as a closure over the serialized key.
// Port of: include/gpu/graphite/ContextOptions.h#L188 (chrome/m156)
pub type PipelineCallbackFn = dyn Fn(&Data) + Send + Sync;

/// A shared client callback, compared by identity.
pub struct Callback<F: ?Sized>(pub Arc<F>);

impl<F: ?Sized> Clone for Callback<F> {
    fn clone(&self) -> Self {
        Self(Arc::clone(&self.0))
    }
}

impl<F: ?Sized> PartialEq for Callback<F> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<F: ?Sized> fmt::Debug for Callback<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Callback(..)")
    }
}

/// Options for creating a Graphite `Context`.
// Port of: include/gpu/graphite/ContextOptions.h#L34-L252 (chrome/m156)
#[doc(alias = "skgpu::graphite::ContextOptions")]
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::struct_excessive_bools)] // mirrors the C++ options struct
pub struct ContextOptions {
    /// `fDisableDriverCorrectnessWorkarounds`: disables workarounds for driver bugs.
    pub disable_driver_correctness_workarounds: bool,
    /// `fInternalMultisampleCount`: the sample count of the internal MSAA attachments.
    pub internal_multisample_count: SampleCount,
    /// `fInternalMSAATileSize`.
    pub internal_msaa_tile_size: Option<ISize>,
    /// `fMinimumPathSizeForMSAA`: paths smaller than this are not drawn with MSAA.
    pub minimum_path_size_for_msaa: f32,
    /// `fGlyphCacheTextureMaximumBytes`.
    pub glyph_cache_texture_maximum_bytes: usize,
    /// `fMinDistanceFieldFontSize`.
    pub min_distance_field_font_size: f32,
    /// `fGlyphsAsPathsFontSize`.
    pub glyphs_as_paths_font_size: f32,
    /// `fMaxPathAtlasTextureSize`: oversized, the `PathAtlas` will likely be smaller.
    pub max_path_atlas_texture_size: i32,
    /// `fAllowMultipleAtlasTextures`.
    pub allow_multiple_atlas_textures: bool,
    /// `fSupportBilerpFromGlyphAtlas`.
    pub support_bilerp_from_glyph_atlas: bool,
    /// `fRequireOrderedRecordings`.
    pub require_ordered_recordings: bool,
    /// `fGpuBudgetInBytes`: the budget of the `Context`'s resource cache.
    pub gpu_budget_in_bytes: usize,
    /// `fSetBackendLabels`: labels backend objects (on by default in debug builds, as
    /// `SK_DEBUG` does).
    pub set_backend_labels: bool,
    /// `fUseDrawListLayer`: switches Graphite from the sort-based draw ordering to the
    /// layer-based one.
    pub use_draw_list_layer: bool,
    /// `fEnableCapture`.
    pub enable_capture: bool,
    /// `fAvoidDepthMode`.
    pub avoid_depth_mode: bool,
    /// `fPipelineCachingCallback`: called when a pipeline is added to the cache and when a cached
    /// pipeline is found. Preempts `pipeline_callback`.
    pub pipeline_caching_callback: Option<Callback<PipelineCachingCallbackFn>>,
    /// `fPipelineCallback` (deprecated): called for added pipelines whose key is serializable.
    /// Ignored when `pipeline_caching_callback` is set.
    pub pipeline_callback: Option<Callback<PipelineCallbackFn>>,
}

impl Default for ContextOptions {
    fn default() -> Self {
        Self {
            disable_driver_correctness_workarounds: false,
            internal_multisample_count: SampleCount::Four,
            internal_msaa_tile_size: None,
            minimum_path_size_for_msaa: 0.0,
            glyph_cache_texture_maximum_bytes: 2048 * 1024 * 4,
            min_distance_field_font_size: 18.0,
            // The Linux/Windows value; Android uses 384 and macOS 256 (`SK_BUILD_FOR_*`).
            glyphs_as_paths_font_size: if cfg!(target_os = "android") {
                384.0
            } else if cfg!(target_os = "macos") {
                256.0
            } else {
                324.0
            },
            max_path_atlas_texture_size: 8192,
            allow_multiple_atlas_textures: true,
            support_bilerp_from_glyph_atlas: false,
            require_ordered_recordings: false,
            gpu_budget_in_bytes: DEFAULT_CONTEXT_BUDGET,
            set_backend_labels: cfg!(debug_assertions),
            use_draw_list_layer: false,
            enable_capture: false,
            avoid_depth_mode: false,
            pipeline_caching_callback: None,
            pipeline_callback: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_the_header() {
        let options = ContextOptions::default();
        assert_eq!(options.internal_multisample_count, SampleCount::Four);
        assert_eq!(options.gpu_budget_in_bytes, 256 << 20);
        assert_eq!(options.max_path_atlas_texture_size, 8192);
        assert!(options.allow_multiple_atlas_textures);
        assert!(!options.avoid_depth_mode);
    }
}
