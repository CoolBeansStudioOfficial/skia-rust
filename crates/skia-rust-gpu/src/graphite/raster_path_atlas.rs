// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RasterPathAtlas.h, src/gpu/graphite/RasterPathAtlas.cpp

//! [`RasterPathAtlas`]: the path atlas whose masks are rasterized on the CPU (through the raster
//! backend), and packed into atlas textures. Shapes are cached for future frames, so the raster
//! work is done once for each mask. Three managers hold the masks: the cached one, the small-path
//! one for masks of at most 162 pixels, and an uncached one for shapes that cannot be cached.

use std::sync::Arc;

use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::stroke_rec::StrokeRec;

use crate::graphite::caps::Caps;
use crate::graphite::draw_atlas::UseStorageTextures;
use crate::graphite::draw_context::DrawContext;
use crate::graphite::geom::shape::Shape;
use crate::graphite::geom::transform::Transform;
use crate::graphite::path_atlas::{DrawAtlasMgr, K_ENTRY_PADDING, PathAtlas, path_atlas_dimensions};
use crate::graphite::proxy_cache::find_or_create_cached_proxy_from_bitmap;
use crate::graphite::raster_path_utils::{Half2, RasterMaskHelper, generate_path_mask_key};
use crate::graphite::recorder::Recorder;
use crate::graphite::texture_proxy::TextureProxy;

/// `kDefaultAtlasDim`.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L22 (chrome/m156)
const K_DEFAULT_ATLAS_DIM: u32 = 2048;
/// `kSmallPathPlotWidth`.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L23 (chrome/m156)
const K_SMALL_PATH_PLOT_WIDTH: u32 = 512;
/// `kSmallPathPlotHeight`.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L24 (chrome/m156)
const K_SMALL_PATH_PLOT_HEIGHT: u32 = 256;
/// `kUncachedAtlasDim`.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L25 (chrome/m156)
const K_UNCACHED_ATLAS_DIM: u32 = 2048;
/// `kMaxSmallPathSize`: the largest mask (per side) that goes to the small-path atlas.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L58 (chrome/m156)
const K_MAX_SMALL_PATH_SIZE: u16 = 162;

/// `RasterPathAtlas`: CPU-rasterized path masks, packed into atlases.
// Port of: src/gpu/graphite/RasterPathAtlas.h#L17-L80 (chrome/m156)
#[doc(alias = "skgpu::graphite::RasterPathAtlas")]
#[derive(Debug)]
pub struct RasterPathAtlas {
    width: u32,
    height: u32,
    /// `fCachedAtlasMgr`.
    cached_atlas_mgr: DrawAtlasMgr,
    /// `fSmallPathAtlasMgr`.
    small_path_atlas_mgr: DrawAtlasMgr,
    /// `fUncachedAtlasMgr`.
    uncached_atlas_mgr: DrawAtlasMgr,
}

impl RasterPathAtlas {
    /// `RasterPathAtlas(recorder)`.
    // Port of: src/gpu/graphite/RasterPathAtlas.cpp#L27-L42 (chrome/m156)
    #[must_use]
    pub fn new(caps: &dyn Caps) -> Self {
        let (width, height) = path_atlas_dimensions(caps, K_DEFAULT_ATLAS_DIM, K_DEFAULT_ATLAS_DIM);
        Self {
            width,
            height,
            cached_atlas_mgr: DrawAtlasMgr::new(
                width,
                height,
                width / 2,
                height / 2,
                UseStorageTextures::No,
                "RasterPathAtlas",
                caps,
            ),
            small_path_atlas_mgr: DrawAtlasMgr::new(
                width.max(K_SMALL_PATH_PLOT_WIDTH),
                height.max(K_SMALL_PATH_PLOT_HEIGHT),
                K_SMALL_PATH_PLOT_WIDTH,
                K_SMALL_PATH_PLOT_HEIGHT,
                UseStorageTextures::No,
                "RasterPathAtlas",
                caps,
            ),
            uncached_atlas_mgr: DrawAtlasMgr::new(
                K_UNCACHED_ATLAS_DIM,
                K_UNCACHED_ATLAS_DIM,
                K_UNCACHED_ATLAS_DIM / 2,
                K_UNCACHED_ATLAS_DIM / 2,
                UseStorageTextures::No,
                "RasterPathAtlas",
                caps,
            ),
        }
    }

    /// `recordUploads(dc)`: records the uploads of the three atlases.
    // Port of: src/gpu/graphite/RasterPathAtlas.cpp#L44-L48 (chrome/m156)
    pub fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) {
        self.cached_atlas_mgr.record_uploads(dc, recorder);
        self.small_path_atlas_mgr.record_uploads(dc, recorder);
        self.uncached_atlas_mgr.record_uploads(dc, recorder);
    }

    /// `compact()`.
    // Port of: src/gpu/graphite/RasterPathAtlas.h#L35-L40 (chrome/m156)
    pub fn compact(&mut self, recorder: &Recorder) {
        self.cached_atlas_mgr.compact(recorder);
        self.small_path_atlas_mgr.compact(recorder);
        self.uncached_atlas_mgr.compact(recorder);
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/RasterPathAtlas.h#L41-L46 (chrome/m156)
    pub fn free_gpu_resources(&mut self, recorder: &Recorder) {
        self.cached_atlas_mgr.free_gpu_resources(recorder);
        self.small_path_atlas_mgr.free_gpu_resources(recorder);
        self.uncached_atlas_mgr.free_gpu_resources(recorder);
    }

    /// `evictAtlases()`.
    // Port of: src/gpu/graphite/RasterPathAtlas.h#L47-L52 (chrome/m156)
    pub fn evict_atlases(&mut self) {
        self.cached_atlas_mgr.evict_all();
        self.small_path_atlas_mgr.evict_all();
        self.uncached_atlas_mgr.evict_all();
    }
}

impl PathAtlas for RasterPathAtlas {
    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }

    // Port of: src/gpu/graphite/RasterPathAtlas.cpp#L50-L115 (chrome/m156)
    fn on_add_shape(
        &mut self,
        recorder: &Recorder,
        shape: &Shape,
        local_to_device: &Transform,
        stroke_rec: &StrokeRec,
        mask_origin: Half2,
        mask_size: Half2,
        transformed_mask_offset: IPoint,
        out_pos: &mut Half2,
    ) -> Option<Arc<TextureProxy>> {
        let mut proxy = None;
        if !shape.is_volatile_path() {
            // Try to locate or add to cached DrawAtlas.
            if mask_size.0 <= K_MAX_SMALL_PATH_SIZE && mask_size.1 <= K_MAX_SMALL_PATH_SIZE {
                proxy = self.small_path_atlas_mgr.find_or_create_entry(
                    recorder,
                    shape,
                    local_to_device,
                    stroke_rec,
                    mask_origin,
                    mask_size,
                    transformed_mask_offset,
                    out_pos,
                );
            }
            if proxy.is_none() {
                proxy = self.cached_atlas_mgr.find_or_create_entry(
                    recorder,
                    shape,
                    local_to_device,
                    stroke_rec,
                    mask_origin,
                    mask_size,
                    transformed_mask_offset,
                    out_pos,
                );
            }
        }

        // Try to add to uncached DrawAtlas.
        if proxy.is_none() {
            let mut locator = crate::graphite::draw_atlas::AtlasLocator::default();
            proxy = self.uncached_atlas_mgr.add_to_atlas(
                recorder,
                shape,
                local_to_device,
                stroke_rec,
                mask_size,
                transformed_mask_offset,
                out_pos,
                &mut locator,
            );
        }
        if proxy.is_some() {
            return proxy;
        }

        // Failed to add to atlases, try to add to ProxyCache.
        let mask_key = generate_path_mask_key(
            shape,
            local_to_device,
            stroke_rec,
            mask_origin,
            mask_size,
        );
        // SkIRect::MakeSize({maskSize.x(), maskSize.y()}).makeOffset(kEntryPadding, kEntryPadding)
        let shape_bounds = IRect::from_xywh(
            K_ENTRY_PADDING,
            K_ENTRY_PADDING,
            i32::from(mask_size.0),
            i32::from(mask_size.1),
        );
        let cached_proxy = find_or_create_cached_proxy_from_bitmap(
            recorder,
            &mask_key,
            || {
                // RasterMaskHelper::Allocate(shapeBounds.size(), -transformedMaskOffset, pad)
                let mut buffer = RasterMaskHelper::allocate(
                    shape_bounds.size(),
                    K_ENTRY_PADDING,
                    0,
                );
                {
                    let mut helper = RasterMaskHelper::over(
                        &mut buffer,
                        IPoint::new(-transformed_mask_offset.x, -transformed_mask_offset.y),
                    );
                    helper.draw_shape(shape, local_to_device, stroke_rec);
                }
                buffer.to_bitmap()
            },
            "",
        );
        *out_pos = (K_ENTRY_PADDING as u16, K_ENTRY_PADDING as u16);
        cached_proxy
    }
}
