// Copyright 2024 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/DrawAtlasTest.cpp (chrome/m156)

#![cfg(test)]
// Mirrors the C++ tests, which declare constants and similarly named bindings inline.
// The index and count casts are the C++ `int`/`uint32_t` arithmetic of the atlas tests, on values
// far below the narrow types; the `for i` loops index `locators` as the C++ loops do.
#![allow(
    clippy::items_after_statements,
    clippy::similar_names,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::needless_range_loop
)]

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use skia_rust_core::size::ISize;
use skia_rust_gpu::gpu::mask_format::MaskFormat;
use skia_rust_gpu::gpu::token::Token;
use skia_rust_gpu::graphite::draw_atlas::{
    AllowMultitexturing, AtlasLocator, DrawAtlas, ErrorCode, GenerationCounter,
    K_MAX_MULTITEXTURE_PAGES, PlotEvictionCallback, PlotLocator, UseStorageTextures,
};
use skia_rust_gpu::graphite::recorder::Recorder;
use skia_rust_gpu::graphite::text_atlas_manager::AtlasConfig;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;

use crate::{Reporter, def_graphite_test_for_all_contexts, def_test, reporter_assert};

const K_NUM_PLOTS: i32 = 2;
const K_PLOT_SIZE: i32 = 32;
const K_ATLAS_SIZE: i32 = K_NUM_PLOTS * K_PLOT_SIZE;

// Port of: tests/graphite/DrawAtlasTest.cpp#L27-L35 (chrome/m156)
// `gEvictCount` is a file global in the C++; each test owns the counter it shares with its evictor.
#[derive(Clone)]
struct PlotEvictionCounter {
    count: Arc<AtomicU32>,
}

impl PlotEvictionCallback for PlotEvictionCounter {
    fn evict(&mut self, _plot_locator: PlotLocator) {
        self.count.fetch_add(1, Ordering::Relaxed);
    }
}

// Port of: tests/graphite/DrawAtlasTest.cpp#L37-L43 (chrome/m156)
fn check(
    reporter: &mut Reporter,
    atlas: &DrawAtlas,
    expected_active: u32,
    expected_evict_count: u32,
    evict_count: &AtomicU32,
) {
    reporter_assert!(reporter, atlas.num_active_pages() == expected_active);
    reporter_assert!(
        reporter,
        evict_count.load(Ordering::Relaxed) == expected_evict_count
    );
    reporter_assert!(reporter, atlas.max_pages() == K_MAX_MULTITEXTURE_PAGES);
}

// Port of: tests/graphite/DrawAtlasTest.cpp#L45-L59 (chrome/m156)
fn fill_plot(
    atlas: &mut DrawAtlas,
    recorder: &Recorder,
    atlas_locator: &mut AtlasLocator,
    alpha: i32,
) -> bool {
    // `SkBitmap::eraseARGB(alpha, 0, 0, 0)` on an A8 bitmap: every texel holds `alpha` (as a byte).
    let data = vec![alpha as u8; (K_PLOT_SIZE * K_PLOT_SIZE) as usize];
    let code = atlas.add_to_atlas(recorder, K_PLOT_SIZE, K_PLOT_SIZE, &data, atlas_locator);
    ErrorCode::Succeeded == code
}

// `recorder->priv().tokenTracker()->nextFlushToken()`.
fn next_flush_token(recorder: &Recorder) -> Token {
    recorder.priv_().token_tracker().borrow().next_flush_token()
}

// This is a basic DrawOpAtlas test. It simply verifies that multitexture atlases correctly
// add and remove pages. Note that this is simulating flush-time behavior.
// Port of: tests/graphite/DrawAtlasTest.cpp#L62-L163 (chrome/m156)
def_graphite_test_for_all_contexts!(BasicDrawAtlas, |reporter, context| {
    let recorder = context.make_recorder(None);

    let evict_count = Arc::new(AtomicU32::new(0));
    let evictor = PlotEvictionCounter {
        count: evict_count.clone(),
    };
    let counter = GenerationCounter::default();
    let atlas_mask_format = MaskFormat::A8;
    let mut atlas = DrawAtlas::make(
        atlas_mask_format,
        K_ATLAS_SIZE,
        K_ATLAS_SIZE,
        K_ATLAS_SIZE / K_NUM_PLOTS,
        K_ATLAS_SIZE / K_NUM_PLOTS,
        &counter,
        AllowMultitexturing::Yes,
        UseStorageTextures::No,
        Some(Box::new(evictor)),
        "BasicDrawAtlasTest",
    );
    check(reporter, &atlas, 0, 0, &evict_count);

    // Fill up the first page
    let mut atlas_locator = AtlasLocator::default();
    let mut test_atlas_locator = AtlasLocator::default();
    for i in 0..K_NUM_PLOTS * K_NUM_PLOTS {
        let result = fill_plot(&mut atlas, &recorder, &mut atlas_locator, i * 32);
        reporter_assert!(reporter, result);
        // We will check drawing with the first plot.
        if i == 0 {
            test_atlas_locator = atlas_locator;
        }
        check(reporter, &atlas, 1, 0, &evict_count);
    }
    reporter_assert!(reporter, atlas.num_allocated_plots() == 4);

    // Force creation of a second page.
    let result = fill_plot(&mut atlas, &recorder, &mut atlas_locator, 4 * 32);
    reporter_assert!(reporter, result);
    check(reporter, &atlas, 2, 0, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 5);

    // Simulate a lot of draws using only the first plot. The last texture should be compacted.
    for _ in 0..512 {
        atlas.set_last_use_token(&test_atlas_locator, next_flush_token(&recorder));
        recorder.priv_().issue_flush_token();
        atlas.compact(next_flush_token(&recorder));
    }
    check(reporter, &atlas, 1, 1, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 4);

    // Simulate a lot of non-atlas draws. We should end up with no textures.
    for _ in 0..512 {
        recorder.priv_().issue_flush_token();
        atlas.compact(next_flush_token(&recorder));
    }
    check(reporter, &atlas, 0, 5, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 0);

    // Fill the atlas all the way up.
    evict_count.store(0, Ordering::Relaxed);
    for p in 1..=4 {
        for i in 0..K_NUM_PLOTS * K_NUM_PLOTS {
            let result = fill_plot(&mut atlas, &recorder, &mut atlas_locator, p * i * 16);
            atlas.set_last_use_token(&atlas_locator, next_flush_token(&recorder));
            // We will check drawing with plot index 2 in the 3rd page
            if p == 3 && i == 2 {
                test_atlas_locator = atlas_locator;
            }
            reporter_assert!(reporter, result);
        }
        check(reporter, &atlas, p as u32, 0, &evict_count);
    }
    // Try one more, it should fail.
    let result = fill_plot(&mut atlas, &recorder, &mut atlas_locator, 0xff);
    reporter_assert!(reporter, !result);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 16);

    // Try to clear everything out. Should fail because there are pending "draws."
    atlas.free_gpu_resources(next_flush_token(&recorder));
    check(reporter, &atlas, 4, 0, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 16);

    // Flush those draws
    recorder.priv_().issue_flush_token();
    atlas.compact(next_flush_token(&recorder));

    // Simulate a draw using only a plot in the 3rd page
    atlas.set_last_use_token(&test_atlas_locator, next_flush_token(&recorder));

    // FreeGpuResources should only remove the 4th page
    atlas.free_gpu_resources(next_flush_token(&recorder));
    check(reporter, &atlas, 3, 4, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 12);

    // Now flush
    recorder.priv_().issue_flush_token();
    atlas.compact(next_flush_token(&recorder));

    // FreeGpuResources should clear everything out
    atlas.free_gpu_resources(next_flush_token(&recorder));
    check(reporter, &atlas, 0, 16, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 0);
});

// Port of: tests/graphite/DrawAtlasTest.cpp#L165-L267 (chrome/m156)
def_graphite_test_for_all_contexts!(ThrashDrawAtlasCache, |reporter, context| {
    let recorder = context.make_recorder(None);
    let evict_count = Arc::new(AtomicU32::new(0));
    let evictor = PlotEvictionCounter {
        count: evict_count.clone(),
    };
    let counter = GenerationCounter::default();

    // Use a 4-page atlas with 4 plots per page (16 total plots)
    const NUM_PLOTS: i32 = 2;
    const PLOT_SIZE: i32 = 32;
    const ATLAS_SIZE: i32 = NUM_PLOTS * PLOT_SIZE;
    const MAX_PAGES: u32 = K_MAX_MULTITEXTURE_PAGES;
    const TOTAL_PLOTS: usize = (NUM_PLOTS * NUM_PLOTS) as usize * MAX_PAGES as usize;
    const _: () = assert!(TOTAL_PLOTS == 16);

    let mut atlas = DrawAtlas::make(
        MaskFormat::A8,
        ATLAS_SIZE,
        ATLAS_SIZE,
        PLOT_SIZE,
        PLOT_SIZE,
        &counter,
        AllowMultitexturing::Yes,
        UseStorageTextures::No,
        Some(Box::new(evictor)),
        "ThrashDrawAtlasTest",
    );
    let mut locators = vec![AtlasLocator::default(); TOTAL_PLOTS];

    // Test kTryAgain failure and recovery
    // Fill the entire atlas and mark all plots as in-use for the current flush
    evict_count.store(0, Ordering::Relaxed);
    for i in 0..TOTAL_PLOTS {
        reporter_assert!(
            reporter,
            fill_plot(&mut atlas, &recorder, &mut locators[i], i as i32)
        );
        atlas.set_last_use_token(&locators[i], next_flush_token(&recorder));
    }
    check(reporter, &atlas, MAX_PAGES, 0, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == TOTAL_PLOTS as u32);

    // Try to add one more plot. All plots are "in-flight", so this must fail.
    let mut first = locators[0];
    reporter_assert!(
        reporter,
        !fill_plot(&mut atlas, &recorder, &mut first, 0xff)
    );

    // Now, simulate a flush. This makes the plots evictable.
    recorder.priv_().issue_flush_token();
    atlas.compact(next_flush_token(&recorder));

    // Try adding again. It should succeed by evicting the least recently used plot.
    reporter_assert!(
        reporter,
        fill_plot(&mut atlas, &recorder, &mut locators[0], 0xff)
    );
    check(reporter, &atlas, MAX_PAGES, 1, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == TOTAL_PLOTS as u32);

    // Reset atlas state
    evict_count.store(0, Ordering::Relaxed);
    atlas.evict_all_plots();
    reporter_assert!(
        reporter,
        evict_count.load(Ordering::Relaxed) == TOTAL_PLOTS as u32
    );

    // After evicting all plots, repeatedly compact until no pages are active.
    recorder.priv_().issue_flush_token();
    while atlas.num_active_pages() > 0 {
        atlas.compact(next_flush_token(&recorder));
        recorder.priv_().issue_flush_token();
    }
    check(reporter, &atlas, 0, TOTAL_PLOTS as u32, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 0);

    // Test partial page compaction
    evict_count.store(0, Ordering::Relaxed);
    // Refill the atlas
    for i in 0..TOTAL_PLOTS {
        reporter_assert!(
            reporter,
            fill_plot(&mut atlas, &recorder, &mut locators[i], i as i32)
        );
        atlas.set_last_use_token(&locators[i], next_flush_token(&recorder));
    }
    reporter_assert!(reporter, atlas.num_allocated_plots() == TOTAL_PLOTS as u32);
    recorder.priv_().issue_flush_token();
    // One compact to settle things
    atlas.compact(next_flush_token(&recorder));

    // Use only one plot on the last page repeatedly, making others on that page stale.
    // The locator for the first plot on the last page is at index (totalPlots - numPlots*numPlots).
    let last_page_plot = locators[TOTAL_PLOTS - (NUM_PLOTS * NUM_PLOTS) as usize];

    // After many flushes, the other 3 plots on the last page should be evicted.
    // We loop one more than kPlotRecentlyUsedCount (32) times to ensure eviction.
    for _ in 0..33 {
        atlas.set_last_use_token(&last_page_plot, next_flush_token(&recorder));
        recorder.priv_().issue_flush_token();
        atlas.compact(next_flush_token(&recorder));
    }

    // Expect 3 evictions from the last page. The page itself should remain active. Not all
    // evictions clear the data on a plot, so check for nonEmptyPlots here rather than
    // numAllocatedPlots.
    let plots_on_page = (NUM_PLOTS * NUM_PLOTS) as u32;
    check(reporter, &atlas, MAX_PAGES, plots_on_page - 1, &evict_count);
    reporter_assert!(
        reporter,
        atlas.num_non_empty_plots() == TOTAL_PLOTS as u32 - (plots_on_page - 1)
    );

    // FreeGpuResources
    atlas.free_gpu_resources(next_flush_token(&recorder));
    check(reporter, &atlas, 0, 16, &evict_count);
    reporter_assert!(reporter, atlas.num_allocated_plots() == 0);
});

// Port of: tests/graphite/DrawAtlasTest.cpp#L269-L330 (chrome/m156)
def_graphite_test_for_all_contexts!(DrawAtlasProxyLifetime, |reporter, context| {
    let evict_count = Arc::new(AtomicU32::new(0));
    let recorder = context.make_recorder(None);
    let evictor = PlotEvictionCounter {
        count: evict_count.clone(),
    };
    let counter = GenerationCounter::default();
    const PLOTS_ON_PAGE: usize = (K_NUM_PLOTS * K_NUM_PLOTS) as usize;
    let mut atlas = DrawAtlas::make(
        MaskFormat::A8,
        K_ATLAS_SIZE,
        K_ATLAS_SIZE,
        K_PLOT_SIZE,
        K_PLOT_SIZE,
        &counter,
        AllowMultitexturing::Yes,
        UseStorageTextures::No,
        Some(Box::new(evictor)),
        "DrawAtlasProxyTest",
    );

    // Fill three pages and collect a shared pointer to each page's TextureProxy.
    let mut proxies: Vec<Arc<TextureProxy>> = Vec::new();
    let mut locators = vec![AtlasLocator::default(); PLOTS_ON_PAGE * 3];

    for i in 0..PLOTS_ON_PAGE * 3 {
        reporter_assert!(
            reporter,
            fill_plot(&mut atlas, &recorder, &mut locators[i], i as i32)
        );
        let page = locators[i].page_index() as usize;
        if page >= proxies.len()
            && let Some(proxy) = atlas.get_proxies()[page].clone()
        {
            proxies.push(proxy);
        }
    }

    reporter_assert!(reporter, atlas.num_active_pages() == 3);
    reporter_assert!(reporter, proxies.len() == 3);

    // All proxies should have a ref count > 1 (one from our vector, one from the atlas).
    for proxy in &proxies {
        reporter_assert!(reporter, Arc::strong_count(proxy) > 1);
    }

    // Simulate many frames where only a plot on the first page (page 0) is ever used. This will
    // make pages 1 and 2 stale and eligible for compaction. Use the first locator we created, which
    // is guaranteed to be on page 0.
    let first_page_locator = locators[0];

    for _ in 0..50 {
        atlas.set_last_use_token(&first_page_locator, next_flush_token(&recorder));
        recorder.priv_().issue_flush_token();
        atlas.compact(next_flush_token(&recorder));
    }

    // The atlas should have compacted away the idle pages (2 and 1).
    reporter_assert!(reporter, atlas.num_active_pages() == 1);
    // The proxy for page 0 should still be shared with the atlas.
    reporter_assert!(reporter, Arc::strong_count(&proxies[0]) > 1);
    // The proxies for pages 1 and 2 should now be unique to our vector
    reporter_assert!(reporter, Arc::strong_count(&proxies[1]) == 1);
    reporter_assert!(reporter, Arc::strong_count(&proxies[2]) == 1);

    atlas.free_gpu_resources(next_flush_token(&recorder));
    reporter_assert!(reporter, atlas.num_allocated_plots() == 0);
});

// Port of: tests/graphite/DrawAtlasTest.cpp#L332-L342 (chrome/m156)
fn test_draw_atlas_config(
    reporter: &mut Reporter,
    max_texture_size: i32,
    max_bytes: usize,
    mask_format: MaskFormat,
    expected_dimensions: ISize,
    expected_plot_dimensions: ISize,
) {
    let config = AtlasConfig::new(max_texture_size, max_bytes);
    reporter_assert!(
        reporter,
        config.atlas_dimensions(mask_format) == expected_dimensions
    );
    reporter_assert!(
        reporter,
        config.plot_dimensions(mask_format) == expected_plot_dimensions
    );
}

// Port of: tests/graphite/DrawAtlasTest.cpp#L344-L398 (chrome/m156)
def_test!(DrawAtlasConfig_Basic, |reporter| {
    // 1/4 MB
    test_draw_atlas_config(
        reporter,
        65536,
        256 * 1024,
        MaskFormat::Argb,
        ISize::new(256, 256),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        256 * 1024,
        MaskFormat::A8,
        ISize::new(512, 512),
        ISize::new(256, 256),
    );
    // 1/2 MB
    test_draw_atlas_config(
        reporter,
        65536,
        512 * 1024,
        MaskFormat::Argb,
        ISize::new(512, 256),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        512 * 1024,
        MaskFormat::A8,
        ISize::new(1024, 512),
        ISize::new(256, 256),
    );
    // 1 MB
    test_draw_atlas_config(
        reporter,
        65536,
        1024 * 1024,
        MaskFormat::Argb,
        ISize::new(512, 512),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        1024 * 1024,
        MaskFormat::A8,
        ISize::new(1024, 1024),
        ISize::new(256, 256),
    );
    // 2 MB
    test_draw_atlas_config(
        reporter,
        65536,
        2 * 1024 * 1024,
        MaskFormat::Argb,
        ISize::new(1024, 512),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        2 * 1024 * 1024,
        MaskFormat::A8,
        ISize::new(2048, 1024),
        ISize::new(512, 256),
    );
    // 4 MB
    test_draw_atlas_config(
        reporter,
        65536,
        4 * 1024 * 1024,
        MaskFormat::Argb,
        ISize::new(1024, 1024),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        4 * 1024 * 1024,
        MaskFormat::A8,
        ISize::new(2048, 2048),
        ISize::new(512, 512),
    );
    // 8 MB
    test_draw_atlas_config(
        reporter,
        65536,
        8 * 1024 * 1024,
        MaskFormat::Argb,
        ISize::new(2048, 1024),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        8 * 1024 * 1024,
        MaskFormat::A8,
        ISize::new(2048, 2048),
        ISize::new(512, 512),
    );
    // 16 MB (should be same as 8 MB)
    test_draw_atlas_config(
        reporter,
        65536,
        16 * 1024 * 1024,
        MaskFormat::Argb,
        ISize::new(2048, 1024),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        16 * 1024 * 1024,
        MaskFormat::A8,
        ISize::new(2048, 2048),
        ISize::new(512, 512),
    );

    // 4MB, restricted texture size
    test_draw_atlas_config(
        reporter,
        1024,
        8 * 1024 * 1024,
        MaskFormat::Argb,
        ISize::new(1024, 1024),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        1024,
        8 * 1024 * 1024,
        MaskFormat::A8,
        ISize::new(1024, 1024),
        ISize::new(256, 256),
    );

    // 3 MB (should be same as 2 MB)
    test_draw_atlas_config(
        reporter,
        65536,
        3 * 1024 * 1024,
        MaskFormat::Argb,
        ISize::new(1024, 512),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        3 * 1024 * 1024,
        MaskFormat::A8,
        ISize::new(2048, 1024),
        ISize::new(512, 256),
    );

    // minimum size
    test_draw_atlas_config(
        reporter,
        65536,
        0,
        MaskFormat::Argb,
        ISize::new(256, 256),
        ISize::new(256, 256),
    );
    test_draw_atlas_config(
        reporter,
        65536,
        0,
        MaskFormat::A8,
        ISize::new(512, 512),
        ISize::new(256, 256),
    );
});
