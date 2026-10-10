// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/AtlasProvider.h, src/gpu/graphite/AtlasProvider.cpp

//! [`AtlasProvider`]: the atlases a recorder shares between its draws: the raster path atlas, the
//! glyph atlases, and the clip atlas when the raster path strategy is in use.
//!
//! Not ported yet (G13/G17): the compute path atlases, the sparse-strip alpha atlas, and
//! `getAtlasTexture()`, whose only callers are those.

use crate::gpu::sk_log::skia_log_e;
use crate::graphite::caps::Caps;
use crate::graphite::clip_atlas_manager::ClipAtlasManager;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::raster_path_atlas::RasterPathAtlas;
use crate::graphite::recorder::Recorder;
use crate::graphite::text::text_atlas_manager::TextAtlasManager;

/// `use_clip_atlas(recorder)`: only the raster atlas strategy routes clips through the atlas.
// Port of: src/gpu/graphite/AtlasProvider.cpp#L23-L27 (chrome/m156)
fn use_clip_atlas(raster_strategy: bool) -> bool {
    // Currently only the raster atlas strategy utilizes the clip atlas.
    raster_strategy
}

/// `AtlasProvider`: groups the atlas management algorithms of a recorder.
// Port of: src/gpu/graphite/AtlasProvider.h#L28-L113 (chrome/m156)
#[doc(alias = "skgpu::graphite::AtlasProvider")]
#[derive(Debug)]
pub struct AtlasProvider {
    /// `fRasterPathAtlas`: the CPU-rasterized path masks.
    raster_path_atlas: RasterPathAtlas,
    /// `fClipAtlasManager`: the clip masks, if the clip atlas is in use.
    clip_atlas_manager: Option<ClipAtlasManager>,
    /// `fTextAtlasManager`: the glyph masks.
    text_atlas_manager: TextAtlasManager,
}

impl AtlasProvider {
    /// `AtlasProvider(recorder)`. `raster_path_strategy` is whether the renderer provider uses
    /// `PathRendererStrategy::kRasterAtlas`.
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L29-L38 (chrome/m156)
    #[must_use]
    pub fn new(caps: &dyn Caps, raster_path_strategy: bool) -> Self {
        Self {
            raster_path_atlas: RasterPathAtlas::new(caps),
            clip_atlas_manager: use_clip_atlas(raster_path_strategy)
                .then(|| ClipAtlasManager::new(caps)),
            text_atlas_manager: TextAtlasManager::new(caps),
        }
    }

    /// `getRasterPathAtlas()`.
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L44 (chrome/m156)
    #[must_use]
    pub fn raster_path_atlas(&mut self) -> &mut RasterPathAtlas {
        &mut self.raster_path_atlas
    }

    /// `textAtlasManager()`.
    // Port of: src/gpu/graphite/AtlasProvider.h#L48 (chrome/m156)
    #[must_use]
    pub fn text_atlas_manager(&self) -> &TextAtlasManager {
        &self.text_atlas_manager
    }

    /// `textAtlasManager()`, to change the atlases.
    // Port of: src/gpu/graphite/AtlasProvider.h#L48 (chrome/m156)
    #[must_use]
    pub fn text_atlas_manager_mut(&mut self) -> &mut TextAtlasManager {
        &mut self.text_atlas_manager
    }

    /// `getClipAtlasManager()`: the clip atlas, or `None` when clips are not atlased.
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L45 (chrome/m156)
    #[must_use]
    pub fn clip_atlas_manager(&mut self) -> Option<&mut ClipAtlasManager> {
        self.clip_atlas_manager.as_mut()
    }

    /// `freeGpuResources()`: frees the pages not in use or needed by pending work.
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L97-L121 (chrome/m156)
    pub fn free_gpu_resources(&mut self, recorder: &Recorder) {
        self.text_atlas_manager.free_gpu_resources(recorder);
        self.raster_path_atlas.free_gpu_resources(recorder);
        if let Some(clip) = &mut self.clip_atlas_manager {
            clip.free_gpu_resources(recorder);
        }
    }

    /// `recordUploads(dc)`: pushes the pending uploads of the atlases onto the draw context.
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L123-L136 (chrome/m156)
    pub fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) {
        if !self.text_atlas_manager.record_uploads(dc, recorder) {
            skia_log_e!("TextAtlasManager uploads have failed -- may see invalid results.");
        }
        self.raster_path_atlas.record_uploads(dc, recorder);
        if let Some(clip) = &mut self.clip_atlas_manager
            && clip.record_uploads(dc, recorder)
        {
            skia_log_e!("ClipAtlasManager uploads have failed -- may see invalid results.");
        }
    }

    /// `compact()`: garbage collection after a flush.
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L138-L146 (chrome/m156)
    pub fn compact(&mut self, recorder: &Recorder) {
        self.text_atlas_manager.compact(recorder);
        self.raster_path_atlas.compact(recorder);
        if let Some(clip) = &mut self.clip_atlas_manager {
            clip.compact(recorder);
        }
    }

    /// `invalidateAtlases()`: the atlases are evicted, after a failed recording (the failed tasks
    /// can include uploads that the atlases depend on).
    // Port of: src/gpu/graphite/AtlasProvider.cpp#L148-L165 (chrome/m156)
    pub fn invalidate_atlases(&mut self) {
        self.text_atlas_manager.evict_atlases();
        self.raster_path_atlas.evict_atlases();
        if let Some(clip) = &mut self.clip_atlas_manager {
            clip.evict_atlases();
        }
    }
}
