// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ClipAtlasManager.h, src/gpu/graphite/ClipAtlasManager.cpp

//! [`ClipAtlasManager`]: the atlas of rasterized clip masks. The elements of a clip that are not
//! drawn analytically are flattened (on the CPU) into one A8 mask, which the draws sample. Two
//! managers hold the masks: one for masks that can be keyed by their elements, and a smaller one
//! for those that can only be keyed by their save record.

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss
)]
// The atlas positions and sizes are small integers, converted as the C++ converts them
// (`SkIPoint` to `skvx::half2`, `int` to `float`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::clip_op::ClipOp;
use skia_rust_core::point::IPoint;
use skia_rust_core::rect::{Contains, IRect};

use crate::gpu::mask_format::MaskFormat;
use crate::gpu::resource_key::UniqueKey;
use crate::graphite::caps::Caps;
use crate::graphite::clip_stack::{ClipAtlasManager as ClipAtlasSeam, ClipElement};
use crate::graphite::draw_atlas::{
    AllowMultitexturing, AtlasLocator, DrawAtlas, ErrorCode, GenerationCounter,
    PlotEvictionCallback, PlotLocator, UseStorageTextures,
};
use crate::graphite::draw_context::DrawContext;
use crate::graphite::proxy_cache::find_or_create_cached_proxy_from_bitmap;
use crate::graphite::raster_path_utils::{RasterMaskHelper, generate_clip_mask_key};
use crate::graphite::recorder::Recorder;
use crate::graphite::texture_proxy::TextureProxy;

/// `kClipAtlasWidth`.
// Port of: src/gpu/graphite/ClipAtlasManager.cpp#L19-L20 (chrome/m156)
const K_CLIP_ATLAS_WIDTH: i32 = 2048;
/// `kClipAtlasHeight`.
const K_CLIP_ATLAS_HEIGHT: i32 = 2048;

/// `kEntryPadding` of the clip atlas (the anonymous-namespace copy): the surrounding context
/// that inverse clips need.
// Port of: src/gpu/graphite/ClipAtlasManager.cpp#L41-L42 (chrome/m156)
const K_CLIP_ENTRY_PADDING: i32 = 1;

/// `initial_alpha_for_elements(elements)`: if the first element is an intersect, the mask is
/// cleared to 0 and that element is drawn directly with coverage 1 (later intersects are
/// inverse-filled and draw 0 outside). If the first is a difference, the mask is cleared to 1.
// Port of: src/gpu/graphite/ClipAtlasManager.cpp#L44-L50 (chrome/m156)
fn initial_alpha_for_elements(elements: &[&ClipElement]) -> u8 {
    debug_assert!(!elements.is_empty());
    if elements[0].op == ClipOp::Intersect {
        0x00
    } else {
        0xFF
    }
}

/// `render_elements(helper, elements)`: draws every element into the mask with the replace
/// operation. An intersect after the first draws the inverse shape with coverage 0, which erases
/// what lies outside it; a difference subtracts its shape.
// Port of: src/gpu/graphite/ClipAtlasManager.cpp#L52-L87 (chrome/m156)
fn render_elements(helper: &mut RasterMaskHelper<'_>, elements: &[&ClipElement]) {
    debug_assert!(!elements.is_empty());
    let mut is_first = true;
    for element in elements {
        let (alpha, invert) = if element.op == ClipOp::Intersect {
            // Intersect modifies pixels outside of its geometry. If this is the first element, we
            // can draw directly with coverage 1 since we cleared to 0. Otherwise we draw the
            // inverse-filled shape with 0 coverage to erase everything outside the element.
            if is_first {
                (0xFF, false)
            } else {
                (0x00, true)
            }
        } else {
            // For difference ops, can always just subtract the shape directly by drawing 0
            // coverage.
            debug_assert_eq!(element.op, ClipOp::Difference);
            (0x00, false)
        };

        // Draw the shape; based on how we've initialized the buffer and chosen alpha+invert,
        // every element is drawn with the kReplace_Op.
        if invert == element.shape.inverted() {
            helper.draw_clip(&element.shape, &element.local_to_device, alpha);
        } else {
            let mut inverted = element.shape.clone();
            inverted.set_inverted(invert);
            helper.draw_clip(&inverted, &element.local_to_device, alpha);
        }
        is_first = false;
    }
}

/// One mask in the cache: its bounds (relative to the full transformed mask) and its locator.
// Port of: src/gpu/graphite/ClipAtlasManager.h#L60-L66 (chrome/m156), `MaskHashEntry`
#[derive(Clone, Debug)]
struct MaskHashEntry {
    bounds: IRect,
    locator: AtlasLocator,
}

/// The cache and the per-plot key lists, which the eviction callback shares with the manager.
#[derive(Debug, Default)]
struct MaskCache {
    /// `fMaskCache`: the masks of each key, in the order they were added (smallest bounds
    /// first, as the lookup takes the first one that contains the clip).
    masks: HashMap<UniqueKey, Vec<MaskHashEntry>>,
    /// `fKeyLists`: the (key, bounds) of the masks in each plot, by the plot's list index.
    key_lists: Vec<Vec<(UniqueKey, IRect)>>,
    /// The plots per page, to find a plot's list index.
    num_plots: u32,
}

/// The eviction callback of a [`ClipDrawAtlasMgr`]: removes the cache entries of an evicted plot.
#[derive(Debug)]
struct MaskCacheEvictor {
    cache: Arc<Mutex<MaskCache>>,
}

impl PlotEvictionCallback for MaskCacheEvictor {
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L286-L316 (chrome/m156), `evict`
    fn evict(&mut self, plot_locator: PlotLocator) {
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        let index =
            (plot_locator.page_index() * cache.num_plots + plot_locator.plot_index()) as usize;
        let keys = std::mem::take(&mut cache.key_lists[index]);
        for (key, bounds) in keys {
            // Remove the entry with these bounds from the key's list (the first one matches).
            let mut remove_key = false;
            if let Some(list) = cache.masks.get_mut(&key) {
                if let Some(pos) = list.iter().position(|entry| entry.bounds == bounds) {
                    list.remove(pos);
                }
                remove_key = list.is_empty();
            }
            if remove_key {
                cache.masks.remove(&key);
            }
        }
    }
}

/// `ClipAtlasManager::DrawAtlasMgr`: a `DrawAtlas` of clip masks, with the cache of the masks
/// it holds.
// Port of: src/gpu/graphite/ClipAtlasManager.h#L68-L115 (chrome/m156)
#[derive(Debug)]
struct ClipDrawAtlasMgr {
    /// `fDrawAtlas`.
    draw_atlas: Box<DrawAtlas>,
    /// The cache, shared with the eviction callback of `draw_atlas`.
    cache: Arc<Mutex<MaskCache>>,
    /// Keeps the generation counter the atlas draws its ids from.
    _generation: GenerationCounter,
}

impl ClipDrawAtlasMgr {
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L342-L360 (chrome/m156)
    fn new(
        width: i32,
        height: i32,
        plot_width: i32,
        plot_height: i32,
        use_storage_textures: UseStorageTextures,
        label: &str,
        caps: &dyn Caps,
    ) -> Self {
        let cache = Arc::new(Mutex::new(MaskCache::default()));
        let generation = GenerationCounter::default();
        let allow = if caps.allow_multiple_atlas_textures() {
            AllowMultitexturing::Yes
        } else {
            AllowMultitexturing::No
        };
        let draw_atlas = DrawAtlas::make(
            MaskFormat::A8,
            width,
            height,
            plot_width,
            plot_height,
            &generation,
            allow,
            use_storage_textures,
            Some(Box::new(MaskCacheEvictor {
                cache: Arc::clone(&cache),
            })),
            label,
        );
        {
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

    /// `findOrCreateEntry(recorder, maskKey, elementList, maskDeviceBounds, keyBounds, outPos)`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L213-L255 (chrome/m156)
    fn find_or_create_entry(
        &mut self,
        recorder: &Recorder,
        mask_key: &UniqueKey,
        elements: &[&ClipElement],
        mask_device_bounds: IRect,
        key_bounds: IRect,
        out_pos: &mut IPoint,
    ) -> Option<Arc<TextureProxy>> {
        let cached = {
            let cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
            cache.masks.get(mask_key).and_then(|list| {
                // If this entry is large enough to contain the clip, use it. The list is ordered
                // from smallest bounds to largest.
                list.iter()
                    .find(|entry| entry.bounds.contains(key_bounds))
                    .cloned()
            })
        };
        if let Some(entry) = cached {
            let top_left = entry.locator.top_left();
            // We need to adjust the returned outPos to reflect the subset we're using.
            let subset_relative_pos = IPoint::new(
                key_bounds.left - entry.bounds.left,
                key_bounds.top - entry.bounds.top,
            );
            *out_pos = IPoint::new(
                top_left.x + K_CLIP_ENTRY_PADDING + subset_relative_pos.x,
                top_left.y + K_CLIP_ENTRY_PADDING + subset_relative_pos.y,
            );
            let token = recorder.priv_().token_tracker().borrow().next_flush_token();
            self.draw_atlas.set_last_use_token(&entry.locator, token);
            return self.proxy_of(&entry.locator);
        }

        let mut locator = AtlasLocator::default();
        let proxy = self.add_to_atlas(
            recorder,
            elements,
            mask_device_bounds,
            out_pos,
            &mut locator,
        )?;

        // Add locator and bounds to the cache, at the end of the key's list.
        let mut cache = self.cache.lock().unwrap_or_else(PoisonError::into_inner);
        cache
            .masks
            .entry(mask_key.clone())
            .or_default()
            .push(MaskHashEntry {
                bounds: key_bounds,
                locator,
            });

        // Add key to Plot's MaskKeyList.
        let index = self.draw_atlas.get_list_index(&locator.plot_locator()) as usize;
        cache.key_lists[index].push((mask_key.clone(), key_bounds));
        Some(proxy)
    }

    /// `addToAtlas(recorder, elementsForMask, maskDeviceBounds, outPos, locator)`: renders the
    /// elements into a new entry of the atlas, with padding.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L257-L284 (chrome/m156)
    fn add_to_atlas(
        &mut self,
        recorder: &Recorder,
        elements: &[&ClipElement],
        mask_device_bounds: IRect,
        out_pos: &mut IPoint,
        locator: &mut AtlasLocator,
    ) -> Option<Arc<TextureProxy>> {
        // Render mask.
        let mut mask_w = mask_device_bounds.width();
        let mut mask_h = mask_device_bounds.height();
        if mask_w == 0 || mask_h == 0 {
            return None;
        }
        // Expand to include padding as well (so we clear correctly for inverse clip).
        mask_w += 2 * K_CLIP_ENTRY_PADDING;
        mask_h += 2 * K_CLIP_ENTRY_PADDING;

        // Request space in DrawAtlas, including padding.
        if self.draw_atlas.add_rect(recorder, mask_w, mask_h, locator) != ErrorCode::Succeeded {
            return None;
        }

        // Rasterize path to the record's pixmap with an inset.
        let alpha = initial_alpha_for_elements(elements);
        // SkColor clearColor = alpha << 24 (only if alpha is non-zero).
        let clear_color = (alpha != 0).then(|| u32::from(alpha) << 24);
        let translate = IPoint::new(
            -mask_device_bounds.left + K_CLIP_ENTRY_PADDING,
            -mask_device_bounds.top + K_CLIP_ENTRY_PADDING,
        );
        {
            let mut pixmap = self.draw_atlas.prep_for_render(locator, 0, clear_color)?;
            let mut helper = RasterMaskHelper::over_pixmap(&mut pixmap, translate);
            render_elements(&mut helper, elements);
        }

        let top_left = locator.top_left();
        *out_pos = IPoint::new(
            top_left.x + K_CLIP_ENTRY_PADDING,
            top_left.y + K_CLIP_ENTRY_PADDING,
        );
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

    /// `recordUploads(dc, recorder)`: returns `true` if the uploads failed (the C++ returns
    /// `fDrawAtlas && !fDrawAtlas->recordUploads(dc, recorder)`).
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L318-L320 (chrome/m156)
    fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) -> bool {
        !self.draw_atlas.record_uploads(dc, recorder)
    }

    /// `compact(recorder)`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L331-L336 (chrome/m156)
    fn compact(&mut self, recorder: &Recorder) {
        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        self.draw_atlas.compact(token);
    }

    /// `freeGpuResources(recorder)`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L338-L341 (chrome/m156)
    fn free_gpu_resources(&mut self, recorder: &Recorder) {
        let token = recorder.priv_().token_tracker().borrow().next_flush_token();
        self.draw_atlas.free_gpu_resources(token);
    }

    /// `evictAll()`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L362-L366 (chrome/m156)
    fn evict_all(&mut self) {
        self.draw_atlas.evict_all_plots();
        debug_assert!(
            self.cache
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .masks
                .is_empty()
        );
    }
}

/// `ClipAtlasManager`: the clip masks in atlases, keyed by the clip elements or the save record.
// Port of: src/gpu/graphite/ClipAtlasManager.h#L27-L56 (chrome/m156)
#[doc(alias = "skgpu::graphite::ClipAtlasManager")]
#[derive(Debug)]
pub struct ClipAtlasManager {
    /// `fPathKeyAtlasMgr`: masks keyed by their elements.
    path_key_atlas_mgr: ClipDrawAtlasMgr,
    /// `fSaveRecordKeyAtlasMgr`: masks keyed by the save record. Those are far more transient, so
    /// the atlas is smaller.
    save_record_key_atlas_mgr: ClipDrawAtlasMgr,
}

impl ClipAtlasManager {
    /// `ClipAtlasManager(recorder)`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L22-L35 (chrome/m156)
    #[must_use]
    pub fn new(caps: &dyn Caps) -> Self {
        Self {
            path_key_atlas_mgr: ClipDrawAtlasMgr::new(
                K_CLIP_ATLAS_WIDTH,
                K_CLIP_ATLAS_HEIGHT,
                K_CLIP_ATLAS_WIDTH / 2,
                K_CLIP_ATLAS_HEIGHT / 2,
                UseStorageTextures::No,
                "PathKeyClipAtlas",
                caps,
            ),
            // Examining the results from the top 20 or so webpages, the SaveRecord keyed clips tend
            // to be considerably smaller and rarer, so we use a smaller atlas here.
            save_record_key_atlas_mgr: ClipDrawAtlasMgr::new(
                K_CLIP_ATLAS_WIDTH / 2,
                K_CLIP_ATLAS_HEIGHT / 2,
                K_CLIP_ATLAS_WIDTH / 2,
                K_CLIP_ATLAS_HEIGHT / 2,
                UseStorageTextures::No,
                "SaveRecordKeyClipAtlas",
                caps,
            ),
        }
    }

    /// `findOrCreateEntry(stackRecordID, elementList, maskDeviceBounds, outPos)`: the texture
    /// holding the clip mask of `elements`, and where the mask starts in it. The mask is
    /// rendered on first use, and reused when the same elements come again. If it does not fit in
    /// the atlas, it is cached in the `ProxyCache` instead.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L84-L127 (chrome/m156)
    #[doc(alias = "findOrCreateEntry")]
    pub fn find_or_create_entry(
        &mut self,
        recorder: &Recorder,
        stack_record_id: u32,
        elements: &[&ClipElement],
        mask_device_bounds: IRect,
        out_pos: &mut IPoint,
    ) -> Option<Arc<TextureProxy>> {
        // For the ClipAtlas cache, we don't include the bounds in the key.
        let key = generate_clip_mask_key(stack_record_id, elements, mask_device_bounds, false);

        let atlas_proxy = if key.uses_path_key {
            self.path_key_atlas_mgr.find_or_create_entry(
                recorder,
                &key.key,
                elements,
                mask_device_bounds,
                key.key_bounds,
                out_pos,
            )
        } else {
            self.save_record_key_atlas_mgr.find_or_create_entry(
                recorder,
                &key.key,
                elements,
                mask_device_bounds,
                key.key_bounds,
                out_pos,
            )
        };
        if atlas_proxy.is_some() {
            return atlas_proxy;
        }

        // We need to include the bounds in the key when using the ProxyCache.
        let mask_key = generate_clip_mask_key(stack_record_id, elements, mask_device_bounds, true);
        let proxy = find_or_create_cached_proxy_from_bitmap(
            recorder,
            &mask_key.key,
            || {
                let translate = IPoint::new(-mask_device_bounds.left, -mask_device_bounds.top);
                let mut buffer = RasterMaskHelper::allocate(
                    mask_device_bounds.size(),
                    K_CLIP_ENTRY_PADDING,
                    initial_alpha_for_elements(elements),
                );
                {
                    let mut helper = RasterMaskHelper::over(&mut buffer, translate);
                    render_elements(&mut helper, elements);
                }
                buffer.to_bitmap()
            },
            "",
        );
        *out_pos = IPoint::new(K_CLIP_ENTRY_PADDING, K_CLIP_ENTRY_PADDING);
        proxy
    }

    /// `recordUploads(dc)`: returns `true` if an upload failed (the C++ short-circuits the second
    /// atlas after a failure).
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L129-L132 (chrome/m156)
    pub fn record_uploads(&mut self, dc: &mut DrawContext, recorder: &Recorder) -> bool {
        self.path_key_atlas_mgr.record_uploads(dc, recorder)
            || self.save_record_key_atlas_mgr.record_uploads(dc, recorder)
    }

    /// `compact()`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L134-L137 (chrome/m156)
    pub fn compact(&mut self, recorder: &Recorder) {
        self.path_key_atlas_mgr.compact(recorder);
        self.save_record_key_atlas_mgr.compact(recorder);
    }

    /// `freeGpuResources()`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L139-L143 (chrome/m156)
    pub fn free_gpu_resources(&mut self, recorder: &Recorder) {
        self.path_key_atlas_mgr.free_gpu_resources(recorder);
        self.save_record_key_atlas_mgr.free_gpu_resources(recorder);
    }

    /// `evictAtlases()`.
    // Port of: src/gpu/graphite/ClipAtlasManager.cpp#L145-L149 (chrome/m156)
    pub fn evict_atlases(&mut self) {
        self.path_key_atlas_mgr.evict_all();
        self.save_record_key_atlas_mgr.evict_all();
    }
}

/// The [`ClipAtlasManager`] as the clip stack sees it (`ClipStack::visitClipStackForDraw` calls
/// `findOrCreateEntry` on the manager, which needs the recorder that owns it): the recorder
/// comes with the manager for the one call.
#[derive(Debug)]
pub struct RecorderClipAtlas<'a> {
    /// The recorder that owns the atlas.
    pub recorder: &'a Recorder,
    /// The atlas manager of that recorder.
    pub manager: &'a mut ClipAtlasManager,
}

impl ClipAtlasSeam for RecorderClipAtlas<'_> {
    // Port of: src/gpu/graphite/ClipStack.cpp (the atlas call in visitClipStackForDraw)
    fn find_or_create_entry(
        &mut self,
        stack_record_id: u32,
        element_list: &[&ClipElement],
        mask_bounds: IRect,
        out_pos: &mut IPoint,
    ) -> Option<Arc<TextureProxy>> {
        self.manager.find_or_create_entry(
            self.recorder,
            stack_record_id,
            element_list,
            mask_bounds,
            out_pos,
        )
    }
}
