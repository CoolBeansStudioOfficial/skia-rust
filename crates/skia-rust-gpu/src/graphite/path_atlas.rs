// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/PathAtlas.h, src/gpu/graphite/PathAtlas.cpp

//! [`PathAtlas`]: the transient atlases of coverage masks for path rendering, and the
//! [`DrawAtlasMgr`] that wraps a `DrawAtlas` with a cache of the masks it holds.
//!
//! The atlases of Skia are objects that point back at the recorder that owns them. Here the
//! recorder is passed to each call instead, so the atlases hold no reference to it, and the
//! `DrawAtlas` eviction callback shares the shape cache through an `Arc`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::m44::M44;
use skia_rust_core::math_priv::prev_pow2;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::IRect;
use skia_rust_core::stroke_rec::StrokeRec;

use crate::gpu::mask_format::MaskFormat;
use crate::gpu::resource_key::UniqueKey;
use crate::graphite::caps::Caps;
use crate::graphite::draw_atlas::{
    AllowMultitexturing, AtlasLocator, DrawAtlas, ErrorCode, GenerationCounter,
    PlotEvictionCallback, PlotLocator, UseStorageTextures,
};
use crate::graphite::draw_context::DrawContext;
use crate::graphite::geom::coverage_mask_shape::{CoverageMaskShape, MaskInfo};
use crate::graphite::geom::rect::Rect;
use crate::graphite::geom::shape::Shape;
use crate::graphite::geom::transform::Transform;
use crate::graphite::raster_path_utils::{Half2, generate_path_mask_key};
use crate::graphite::recorder::Recorder;
use crate::graphite::renderer::Renderer;
use crate::graphite::texture_proxy::TextureProxy;

/// `PathAtlas::kEntryPadding`: the padding around each entry, so that linear sampling never reads
/// a neighbouring entry.
// Port of: src/gpu/graphite/PathAtlas.h#L44 (chrome/m156)
pub const K_ENTRY_PADDING: i32 = 1;

/// `kMinAtlasTextureSize`: the smallest the path atlas textures should be, unless the device
/// requires smaller.
// Port of: src/gpu/graphite/PathAtlas.cpp#L17-L18 (chrome/m156)
const K_MIN_ATLAS_TEXTURE_SIZE: i32 = 512;

/// The dimensions of a path atlas, as `PathAtlas::PathAtlas()` computes them: the requested size
/// rounded down to a power of two, and no larger than the largest path atlas (or texture) the
/// caps allow.
// Port of: src/gpu/graphite/PathAtlas.cpp#L24-L33 (chrome/m156)
#[must_use]
pub fn path_atlas_dimensions(caps: &dyn Caps, requested_width: u32, requested_height: u32) -> (u32, u32) {
    let max_texture_size = caps
        .max_path_atlas_texture_size()
        .max(K_MIN_ATLAS_TEXTURE_SIZE)
        .min(caps.max_texture_size());
    let max = u32::try_from(max_texture_size).unwrap_or(0);
    let width = prev_pow2(requested_width.min(max) as i32) as u32;
    let height = prev_pow2(requested_height.min(max) as i32) as u32;
    (width, height)
}

/// `float` to `uint16_t` as `skvx::cast<uint16_t>` converts it: truncated toward zero, then
/// narrowed.
// The C++ converts through `int`, so a negative value wraps as the narrowing does on the host.
fn half_from_f32(v: f32) -> u16 {
    (v as i32) as u16
}

/// `PathAtlas`: the interface of the path atlases. The subclass supplies `on_add_shape` (where
/// the mask is rendered, and its entry in the atlas); `add_shape` is shared by all of them.
// Port of: src/gpu/graphite/PathAtlas.h#L26-L94 (chrome/m156)
#[doc(alias = "skgpu::graphite::PathAtlas")]
pub trait PathAtlas {
    /// `width()`.
    #[must_use]
    fn width(&self) -> u32;

    /// `height()`.
    #[must_use]
    fn height(&self) -> u32;

    /// `onAddShape()`: renders `shape` into an atlas entry, and returns the texture holding it.
    /// `mask_origin` and `mask_size` are the clipped mask's, `transformed_mask_offset` the
    /// device-space offset of the mask, and `out_pos` receives the entry's position in the
    /// texture (after its padding).
    // Port of: src/gpu/graphite/PathAtlas.h#L140-L146 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors PathAtlas::onAddShape
    fn on_add_shape(
        &mut self,
        recorder: &Recorder,
        shape: &Shape,
        local_to_device: &Transform,
        style: &StrokeRec,
        mask_origin: Half2,
        mask_size: Half2,
        transformed_mask_offset: IPoint,
        out_pos: &mut Half2,
    ) -> Option<Arc<TextureProxy>>;

    /// `isSuitableForAtlasing(transformedShapeBounds, clipBounds)`: whether a coverage mask of
    /// these bounds benefits from atlasing without causing too many atlas renders.
    // Port of: src/gpu/graphite/PathAtlas.h#L92-L97 (chrome/m156)
    fn is_suitable_for_atlasing(&self, _transformed_shape_bounds: &Rect, _clip_bounds: &Rect) -> bool {
        true
    }

    /// `addShape(transformedShapeBounds, shape, localToDevice, style)`: schedules `shape` to be
    /// rendered into the atlas, and returns the renderer that draws the coverage mask (the
    /// coverage mask renderer) and the mask that the draw samples. `None` if the shape does not
    /// fit.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L35-L66 (chrome/m156)
    fn add_shape<'r>(
        &mut self,
        recorder: &'r Recorder,
        transformed_shape_bounds: &Rect,
        shape: &Shape,
        local_to_device: &Transform,
        style: &StrokeRec,
    ) -> Option<(&'r Renderer, CoverageMaskShape)> {
        // It is possible for the transformed shape bounds to be fully clipped out while the draw
        // still produces coverage due to an inverse fill. In this case, don't render any mask;
        // CoverageMaskShapeRenderStep will automatically handle the simple fill. We'll handle this
        // by adding an empty mask.
        let empty_mask = transformed_shape_bounds.is_empty_negative_or_nan();

        // Round out the shape bounds to preserve any fractional offset so that it is present in
        // the translation that we use when deriving the atlas-space transform later.
        let mask_bounds = transformed_shape_bounds.make_round_out();

        // This size does *not* include any padding that the atlas may place around the mask. This
        // size represents the area the shape can actually modify.
        let mask_size: Half2 = if empty_mask {
            (0, 0)
        } else {
            let size = mask_bounds.size();
            (half_from_f32(size[0]), half_from_f32(size[1]))
        };

        // We use the origin of the clipped mask bounds relative to the full mask to distinguish
        // between clips of the same size.
        let shape_dev_bounds = local_to_device.map_rect(&shape.bounds());
        let clipped_mask_origin = mask_bounds.top_left() - shape_dev_bounds.top_left();
        let mask_origin_half = (
            half_from_f32(clipped_mask_origin[0]),
            half_from_f32(clipped_mask_origin[1]),
        );
        let top_left = mask_bounds.top_left();
        let transformed_mask_offset = IPoint::new(top_left[0] as i32, top_left[1] as i32);

        let mut texture_origin: Half2 = (0, 0);
        let atlas_proxy = self.on_add_shape(
            recorder,
            shape,
            local_to_device,
            style,
            mask_origin_half,
            mask_size,
            transformed_mask_offset,
            &mut texture_origin,
        )?;

        let mask_info = MaskInfo {
            texture_origin,
            mask_size,
        };
        let renderer = recorder.priv_().renderer_provider().coverage_mask();
        let mask = CoverageMaskShape::new(
            shape,
            atlas_proxy,
            M44::translate(mask_bounds.left(), mask_bounds.top(), 0.0),
            mask_info,
        );
        Some((renderer, mask))
    }
}

/// `PathAtlas::DrawAtlasMgr::ShapeCache` and the per-plot key lists, which the eviction callback
/// shares with the manager.
#[derive(Debug, Default)]
struct ShapeCache {
    /// `fShapeCache`: the locator of each cached mask.
    shapes: HashMap<UniqueKey, AtlasLocator>,
    /// `fKeyLists`: the keys of the masks in each plot, indexed by the plot's list index.
    key_lists: Vec<Vec<UniqueKey>>,
    /// The plots per page, to find a plot's list index.
    num_plots: u32,
}

/// The eviction callback of a [`DrawAtlasMgr`]: removes the cache entries of an evicted plot.
#[derive(Debug)]
struct ShapeCacheEvictor {
    cache: Arc<Mutex<ShapeCache>>,
}

impl PlotEvictionCallback for ShapeCacheEvictor {
    // Port of: src/gpu/graphite/PathAtlas.cpp#L163-L177 (chrome/m156)
    fn evict(&mut self, plot_locator: PlotLocator) {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        // Remove all entries for this Plot from the ShapeCache.
        let index = plot_locator.page_index() * cache.num_plots + plot_locator.plot_index();
        let keys = std::mem::take(&mut cache.key_lists[index as usize]);
        for key in &keys {
            cache.shapes.remove(key);
        }
    }
}

/// `PathAtlas::DrawAtlasMgr`: a `DrawAtlas` for path masks, with the cache of the masks it holds
/// (so that a shape drawn again reuses its entry).
// Port of: src/gpu/graphite/PathAtlas.h#L100-L184 (chrome/m156)
#[doc(alias = "skgpu::graphite::PathAtlas::DrawAtlasMgr")]
#[derive(Debug)]
pub struct DrawAtlasMgr {
    /// `fDrawAtlas`.
    draw_atlas: Box<DrawAtlas>,
    /// The cache, shared with the eviction callback of `draw_atlas`.
    cache: Arc<Mutex<ShapeCache>>,
    /// Keeps the generation counter the atlas draws its ids from.
    _generation: GenerationCounter,
}

impl DrawAtlasMgr {
    /// `DrawAtlasMgr(width, height, plotWidth, plotHeight, useStorageTextures, label, caps)`.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L68-L84 (chrome/m156)
    #[must_use]
    pub fn new(
        width: u32,
        height: u32,
        plot_width: u32,
        plot_height: u32,
        use_storage_textures: UseStorageTextures,
        label: &str,
        caps: &dyn Caps,
    ) -> Self {
        const MASK_FORMAT: MaskFormat = MaskFormat::A8;
        let cache = Arc::new(Mutex::new(ShapeCache::default()));
        let generation = GenerationCounter::default();
        let allow = if caps.allow_multiple_atlas_textures() {
            AllowMultitexturing::Yes
        } else {
            AllowMultitexturing::No
        };
        let draw_atlas = DrawAtlas::make(
            MASK_FORMAT,
            i32::try_from(width).unwrap_or(i32::MAX),
            i32::try_from(height).unwrap_or(i32::MAX),
            i32::try_from(plot_width).unwrap_or(i32::MAX),
            i32::try_from(plot_height).unwrap_or(i32::MAX),
            &generation,
            allow,
            use_storage_textures,
            Some(Box::new(ShapeCacheEvictor {
                cache: Arc::clone(&cache),
            })),
            label,
        );
        {
            // fKeyLists.resize(numPlots * maxPages), each list empty.
            let mut state = cache.lock().unwrap_or_else(PoisonError::into_inner);
            state.num_plots = draw_atlas.num_plots();
            let lists = (draw_atlas.num_plots() * draw_atlas.max_pages()) as usize;
            state.key_lists = vec![Vec::new(); lists];
        }
        Self {
            draw_atlas,
            cache,
            _generation: generation,
        }
    }

    /// `findOrCreateEntry(recorder, shape, localToDevice, strokeRec, maskOrigin, maskSize,
    /// transformedMaskOffset, outPos)`: the atlas texture holding the mask of the shape, from
    /// the cache if it is there, or added to the atlas (and cached).
    // Port of: src/gpu/graphite/PathAtlas.cpp#L129-L166 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors DrawAtlasMgr::findOrCreateEntry
    pub fn find_or_create_entry(
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
        let mask_key = generate_path_mask_key(
            shape,
            local_to_device,
            stroke_rec,
            mask_origin,
            mask_size,
        );

        let cached = self
            .cache
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .shapes
            .get(&mask_key)
            .copied();
        if let Some(locator) = cached {
            let top_left = locator.top_left();
            *out_pos = (
                (top_left.x + K_ENTRY_PADDING) as u16,
                (top_left.y + K_ENTRY_PADDING) as u16,
            );
            let token = recorder.priv_().token_tracker().borrow().next_flush_token();
            self.draw_atlas.set_last_use_token(&locator, token);
            return self.proxy_of(&locator);
        }

        let mut locator = AtlasLocator::default();
        let proxy = self.add_to_atlas(
            recorder,
            shape,
            local_to_device,
            stroke_rec,
            mask_size,
            transformed_mask_offset,
            out_pos,
            &mut locator,
        )?;

        // Add locator to ShapeCache.
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache.shapes.insert(mask_key.clone(), locator);
        // Add key to Plot's ShapeKeyList.
        let index = self.draw_atlas.get_list_index(&locator.plot_locator());
        cache.key_lists[index as usize].push(mask_key);
        Some(proxy)
    }

    /// `addToAtlas(recorder, shape, localToDevice, strokeRec, maskSize, transformedMaskOffset,
    /// outPos, locator)`: adds the mask of the shape to the atlas, without caching it. The mask
    /// is rendered by `rasterize` (`onAddToAtlas`), which gets the atlas and the locator of the
    /// entry.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L168-L208 (chrome/m156)
    #[allow(clippy::too_many_arguments)] // mirrors DrawAtlasMgr::addToAtlas
    pub fn add_to_atlas(
        &mut self,
        recorder: &Recorder,
        shape: &Shape,
        local_to_device: &Transform,
        stroke_rec: &StrokeRec,
        mask_size: Half2,
        transformed_mask_offset: IPoint,
        out_pos: &mut Half2,
        locator: &mut AtlasLocator,
    ) -> Option<Arc<TextureProxy>> {
        self.add_to_atlas_with(
            recorder,
            mask_size,
            out_pos,
            locator,
            |draw_atlas, locator, shape_bounds| {
                on_add_to_atlas_raster(
                    draw_atlas,
                    shape,
                    local_to_device,
                    stroke_rec,
                    shape_bounds,
                    transformed_mask_offset,
                    locator,
                )
            },
        )
    }

    /// `addToAtlas` with the rendering of the mask supplied as `on_add_to_atlas`, which returns
    /// false if the mask could not be rendered. `shape_bounds` is the mask's bounds in the entry
    /// (without its padding).
    // Port of: src/gpu/graphite/PathAtlas.cpp#L168-L208 (chrome/m156), `onAddToAtlas` supplied
    pub fn add_to_atlas_with(
        &mut self,
        recorder: &Recorder,
        mask_size: Half2,
        out_pos: &mut Half2,
        locator: &mut AtlasLocator,
        on_add_to_atlas: impl FnOnce(&mut DrawAtlas, &AtlasLocator, IRect) -> bool,
    ) -> Option<Arc<TextureProxy>> {
        // Render mask.
        let i_shape_bounds = IRect::from_xywh(0, 0, i32::from(mask_size.0), i32::from(mask_size.1));
        // Outset to take padding into account.
        let mut i_atlas_bounds = i_shape_bounds;
        i_atlas_bounds.outset((K_ENTRY_PADDING, K_ENTRY_PADDING));

        // Request space in DrawAtlas.
        let error_code = self.draw_atlas.add_rect(
            recorder,
            i_atlas_bounds.width(),
            i_atlas_bounds.height(),
            locator,
        );
        if error_code != ErrorCode::Succeeded {
            return None;
        }

        let top_left = locator.top_left();
        *out_pos = (
            (top_left.x + K_ENTRY_PADDING) as u16,
            (top_left.y + K_ENTRY_PADDING) as u16,
        );

        // If the mask is empty, just return.
        if mask_size.0 == 0 || mask_size.1 == 0 {
            let token = recorder.priv_().token_tracker().borrow().next_flush_token();
            self.draw_atlas.set_last_use_token(locator, token);
            return self.proxy_of(locator);
        }

        if !on_add_to_atlas(&mut self.draw_atlas, locator, i_shape_bounds) {
            return None;
        }

        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        self.draw_atlas.set_last_use_token(locator, token);
        self.proxy_of(locator)
    }

    /// The texture of the page `locator` is in.
    fn proxy_of(&self, locator: &AtlasLocator) -> Option<Arc<TextureProxy>> {
        self.draw_atlas
            .get_proxies()
            .get(locator.page_index() as usize)
            .and_then(Clone::clone)
    }

    /// `recordUploads(dc, recorder)`.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L210-L212 (chrome/m156)
    pub fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) -> bool {
        self.draw_atlas.record_uploads(dc, recorder)
    }

    /// `compact(recorder)`.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L214-L216 (chrome/m156)
    pub fn compact(&mut self, recorder: &Recorder) {
        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        self.draw_atlas.compact(token);
    }

    /// `freeGpuResources(recorder)`.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L218-L220 (chrome/m156)
    pub fn free_gpu_resources(&mut self, recorder: &Recorder) {
        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        self.draw_atlas.free_gpu_resources(token);
    }

    /// `evictAll()`.
    // Port of: src/gpu/graphite/PathAtlas.cpp#L157-L161 (chrome/m156)
    pub fn evict_all(&mut self) {
        self.draw_atlas.evict_all_plots();
        debug_assert!(
            self.cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .shapes
                .is_empty()
        );
    }

    /// The atlas this manager wraps.
    #[must_use]
    pub fn draw_atlas(&self) -> &DrawAtlas {
        &self.draw_atlas
    }
}

/// `RasterAtlasMgr::onAddToAtlas`: draws the shape's coverage into the atlas entry, through the
/// raster backend. Shared with the path atlas that rasterizes its masks.
// Port of: src/gpu/graphite/RasterPathAtlas.cpp#L127-L137 (chrome/m156)
pub(crate) fn on_add_to_atlas_raster(
    draw_atlas: &mut DrawAtlas,
    shape: &Shape,
    local_to_device: &Transform,
    stroke_rec: &StrokeRec,
    _shape_bounds: IRect,
    transformed_mask_offset: IPoint,
    locator: &AtlasLocator,
) -> bool {
    let Some(mut pixmap) = draw_atlas.prep_for_render(locator, K_ENTRY_PADDING, None) else {
        return false;
    };
    let mut helper = crate::graphite::raster_path_utils::RasterMaskHelper::over_pixmap(
        &mut pixmap,
        IPoint::new(-transformed_mask_offset.x, -transformed_mask_offset.y),
    );
    helper.draw_shape(shape, local_to_device, stroke_rec);
    true
}

/// `UniqueKey` of a mask is the cache key; kept as a type alias for readers of the C++.
pub type MaskKey = UniqueKey;
