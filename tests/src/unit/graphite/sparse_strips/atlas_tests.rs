// Copyright 2026 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/graphite/sparse_strips/AtlasTests.cpp (chrome/m156)

#![cfg(test)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_lines
)]

use std::sync::Arc;

use skia_rust_gpu::graphite::geom::end_caps::EndCaps;
use skia_rust_gpu::graphite::sparse_strips::alpha_atlas_manager::AlphaAtlasManager;
use skia_rust_gpu::graphite::sparse_strips::config::SparseStripConfig;
use skia_rust_gpu::graphite::texture_proxy::TextureProxy;

use crate::{def_graphite_test_for_all_contexts, reporter_assert};

/// `a == b` for two `sk_sp<TextureProxy>`: the same proxy (or both null).
fn same_proxy(a: Option<&Arc<TextureProxy>>, b: Option<&Arc<TextureProxy>>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
}

/// The value of a `finalizeRun()` result that failed the `has_value()` assertion before it.
const NO_ALLOC: (i32, u16) = (i32::MIN, u16::MAX);

def_graphite_test_for_all_contexts!(
    SparseStrips_AtlasDimensionsAndStride,
    |reporter, context| {
        // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L17-L54 (chrome/m156)
        let recorder = context.make_recorder(None);
        let mut atlas_manager = AlphaAtlasManager::new();
        // Initial allocation (e.g. 512 bytes = 8 tiles of 8x8)
        reporter_assert!(
            reporter,
            atlas_manager.request_alpha_space(&recorder, 512).is_some()
        );
        {
            let alloc = atlas_manager.finalize_run(&recorder);
            reporter_assert!(reporter, alloc.is_some());
            let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
            reporter_assert!(reporter, alpha_index == 0);
            reporter_assert!(reporter, tex_page == 0);
        }
        reporter_assert!(reporter, atlas_manager.num_pages() == 1);

        // Initial page dimensions: 8192 x 1
        let proxy = atlas_manager.get_page_proxy(0);
        reporter_assert!(reporter, proxy.is_some());
        if let Some(proxy) = &proxy {
            reporter_assert!(
                reporter,
                proxy.dimensions().width == SparseStripConfig::K_ATLAS_WIDTH
            );
            reporter_assert!(reporter, proxy.dimensions().height == 1);
        }

        // Verify row byte stride alignment (32,768 bytes/row = 8192 pixels * 4 bytes/pixel)
        let stride_bytes = SparseStripConfig::K_ATLAS_WIDTH_BYTES;
        reporter_assert!(reporter, stride_bytes == 32768);

        // Subsequent allocation on same row/page
        reporter_assert!(
            reporter,
            atlas_manager.request_alpha_space(&recorder, 1024).is_some()
        );
        {
            let alloc = atlas_manager.finalize_run(&recorder);
            reporter_assert!(reporter, alloc.is_some());
            let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
            reporter_assert!(reporter, alpha_index == 512);
            reporter_assert!(reporter, tex_page == 0);
        }
        reporter_assert!(reporter, atlas_manager.num_pages() == 1);
    }
);

def_graphite_test_for_all_contexts!(
    SparseStrips_AtlasDynamicGrowthAndDoubling,
    |reporter, context| {
        // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L56-L85 (chrome/m156)
        let recorder = context.make_recorder(None);
        let mut atlas_manager = AlphaAtlasManager::new();

        // Page 0: allocate exactly 1 row (32,768 bytes)
        reporter_assert!(
            reporter,
            atlas_manager
                .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES)
                .is_some()
        );
        {
            let alloc = atlas_manager.finalize_run(&recorder);
            reporter_assert!(reporter, alloc.is_some());
            let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
            reporter_assert!(reporter, tex_page == 0);
        }
        reporter_assert!(
            reporter,
            atlas_manager
                .get_page_proxy(0)
                .is_some_and(|proxy| proxy.dimensions().height == 1)
        );

        // Page 1: allocating 1 more tile exceeds Page 0, triggers doubling -> 2 rows (65,536
        // bytes)
        reporter_assert!(
            reporter,
            atlas_manager.request_alpha_space(&recorder, 64).is_some()
        );
        {
            let alloc = atlas_manager.finalize_run(&recorder);
            reporter_assert!(reporter, alloc.is_some());
            let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
            reporter_assert!(reporter, alpha_index == 0); // Atomic on new page, starts at 0
            reporter_assert!(reporter, tex_page == 1);
        }
        reporter_assert!(reporter, atlas_manager.num_pages() == 2);
        reporter_assert!(
            reporter,
            atlas_manager
                .get_page_proxy(1)
                .is_some_and(|proxy| proxy.dimensions().height == 2)
        );
    }
);

def_graphite_test_for_all_contexts!(
    SparseStrips_AtlasNullBufferAndNullCaps,
    |reporter, context| {
        // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L87-L184 (chrome/m156)
        let recorder = context.make_recorder(None);
        let mut atlas_manager = AlphaAtlasManager::new();

        // Page 0: 1 row
        reporter_assert!(
            reporter,
            atlas_manager
                .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES)
                .is_some()
        );
        {
            let alloc = atlas_manager.finalize_run(&recorder);
            reporter_assert!(reporter, alloc.is_some());
            let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
            reporter_assert!(reporter, tex_page == 0);
        }
        reporter_assert!(reporter, atlas_manager.num_pages() == 1);

        // Page 1: 2 rows
        reporter_assert!(
            reporter,
            atlas_manager
                .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES * 2)
                .is_some()
        );
        {
            let alloc = atlas_manager.finalize_run(&recorder);
            reporter_assert!(reporter, alloc.is_some());
            let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
            reporter_assert!(reporter, tex_page == 1);
        }
        reporter_assert!(reporter, atlas_manager.num_pages() == 2);

        // Verify EndCaps recording of NullCaps
        let mut ends = EndCaps::new();
        ends.add_cap(0, 0, 16, 0, 0);
        reporter_assert!(reporter, !ends.has_null_caps());
        reporter_assert!(reporter, ends.first_null_cap_index() == -1);

        ends.add_cap(16, 0, 16, 0, 1);
        reporter_assert!(reporter, !ends.has_null_caps());
        reporter_assert!(reporter, ends.first_null_cap_index() == -1);

        // Exceeding Page 1: NullCaps handling allows MakeStrips to run to completion!
        // Transitions to the 3rd nullbuffer (kNullSlot).
        {
            let null_ptr = atlas_manager.request_alpha_space(&recorder, 64);
            reporter_assert!(reporter, null_ptr.is_some());
            if let Some(null_ptr) = null_ptr {
                null_ptr[0] = 0xAB;
            }
        }
        let alloc3 = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc3.is_some());
        let alloc3 = alloc3.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, alloc3.1 == AlphaAtlasManager::K_NULL_SLOT);
        reporter_assert!(reporter, alloc3.0 == 0);
        reporter_assert!(reporter, atlas_manager.has_null_buffer());
        reporter_assert!(
            reporter,
            atlas_manager
                .get_page_proxy(usize::from(alloc3.1))
                .is_none()
        );
        reporter_assert!(
            reporter,
            atlas_manager
                .get_page_data(usize::from(alloc3.1))
                .is_some_and(|data| data[0] == 0xAB)
        );
        reporter_assert!(reporter, atlas_manager.num_pages() == 2);

        if alloc3.1 == AlphaAtlasManager::K_NULL_SLOT {
            ends.mark_first_null_cap();
        }
        ends.add_cap(32, 0, 16, alloc3.0, alloc3.1);
        reporter_assert!(reporter, ends.has_null_caps());
        reporter_assert!(reporter, ends.first_null_cap_index() == 2);

        // Subsequent nullcap does not overwrite firstNullCapIndex
        reporter_assert!(
            reporter,
            atlas_manager.request_alpha_space(&recorder, 32).is_some()
        );
        let alloc4 = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc4.is_some());
        let alloc4 = alloc4.unwrap_or(NO_ALLOC);
        if alloc4.1 == AlphaAtlasManager::K_NULL_SLOT {
            ends.mark_first_null_cap();
        }
        ends.add_cap(48, 0, 16, alloc4.0, alloc4.1);
        reporter_assert!(reporter, ends.has_null_caps());
        reporter_assert!(reporter, ends.first_null_cap_index() == 2);

        // Flush: recordUploads retires older slot 0
        atlas_manager.record_uploads(None, &recorder);
        reporter_assert!(reporter, atlas_manager.num_pages() == 1);
        reporter_assert!(reporter, atlas_manager.get_page_proxy(0).is_none());
        reporter_assert!(reporter, atlas_manager.get_page_proxy(1).is_some());
        reporter_assert!(reporter, atlas_manager.has_null_buffer());

        // Resolve nullcaps: migrates nullbuffer data to freed slot 0 and updates endcaps
        let resolved = atlas_manager.resolve_null_caps(&recorder, &mut ends);
        reporter_assert!(reporter, resolved);
        reporter_assert!(reporter, !ends.has_null_caps());
        reporter_assert!(reporter, ends.first_null_cap_index() == -1);
        reporter_assert!(reporter, !atlas_manager.has_null_buffer());
        reporter_assert!(reporter, atlas_manager.num_pages() == 2);
        reporter_assert!(reporter, atlas_manager.get_page_proxy(0).is_some());

        // Endcaps 2 and 3 now point to slot 0 and have valid proxies
        reporter_assert!(reporter, ends.caps()[2].tex_page == 0);
        reporter_assert!(reporter, ends.caps()[3].tex_page == 0);
        reporter_assert!(
            reporter,
            same_proxy(
                ends.proxies()[0].as_ref(),
                atlas_manager.get_page_proxy(0).as_ref()
            )
        );
        reporter_assert!(
            reporter,
            same_proxy(
                ends.proxies()[1].as_ref(),
                atlas_manager.get_page_proxy(1).as_ref()
            )
        );
        reporter_assert!(reporter, ends.draw_start_index() == 2);
        reporter_assert!(reporter, ends.draw_end_index() == 4);
        reporter_assert!(
            reporter,
            atlas_manager
                .get_page_data(0)
                .is_some_and(|data| data[0] == 0xAB)
        );
    }
);

def_graphite_test_for_all_contexts!(
    SparseStrips_AtlasStraddleToNullBuffer,
    |reporter, context| {
        // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L186-L234 (chrome/m156)
        let recorder = context.make_recorder(None);
        let mut atlas_manager = AlphaAtlasManager::new();

        // Fill Page 0
        reporter_assert!(
            reporter,
            atlas_manager
                .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES)
                .is_some()
        );
        let alloc0 = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc0.is_some());

        // Fill Page 1 almost full, leaving 64 bytes
        let page1_capacity = SparseStripConfig::K_ATLAS_WIDTH_BYTES * 2;
        reporter_assert!(
            reporter,
            atlas_manager
                .request_alpha_space(&recorder, page1_capacity - 64)
                .is_some()
        );
        let alloc1 = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc1.is_some());
        reporter_assert!(reporter, alloc1.unwrap_or(NO_ALLOC).1 == 1);

        // Now start a run that straddles past Page 1's capacity:
        // Chunk 1: 64 bytes (fills Page 1)
        let p1 = atlas_manager.request_alpha_space(&recorder, 64);
        reporter_assert!(reporter, p1.is_some());
        if let Some(p1) = p1 {
            for (i, byte) in p1.iter_mut().enumerate().take(64) {
                *byte = (i + 1) as u8;
            }
        }

        // Chunk 2: 64 bytes (overruns Page 1)
        let p2 = atlas_manager.request_alpha_space(&recorder, 64);
        reporter_assert!(reporter, p2.is_some());
        if let Some(p2) = p2 {
            for (i, byte) in p2.iter_mut().enumerate().take(64) {
                *byte = (i + 65) as u8;
            }
        }

        // Finalize run: should memmove the entire 128 bytes to kNullSlot!
        let straddle_alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, straddle_alloc.is_some());
        let straddle_alloc = straddle_alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, straddle_alloc.1 == AlphaAtlasManager::K_NULL_SLOT);
        reporter_assert!(reporter, straddle_alloc.0 == 0);
        reporter_assert!(
            reporter,
            atlas_manager.active_slot() == i32::from(AlphaAtlasManager::K_NULL_SLOT)
        );
        reporter_assert!(reporter, atlas_manager.has_null_buffer());

        let null_data = atlas_manager.get_page_data(usize::from(AlphaAtlasManager::K_NULL_SLOT));
        reporter_assert!(reporter, null_data.is_some());
        if let Some(null_data) = null_data {
            for (i, byte) in null_data.iter().enumerate().take(128) {
                reporter_assert!(reporter, *byte == (i + 1) as u8);
            }
        }
    }
);

def_graphite_test_for_all_contexts!(SparseStrips_AtlasEndCapAtomicity, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L236-L266 (chrome/m156)
    let recorder = context.make_recorder(None);
    let mut atlas_manager = AlphaAtlasManager::new();

    // Allocate almost all of Page 0, leaving 64 bytes
    reporter_assert!(
        reporter,
        atlas_manager
            .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES - 64)
            .is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 0);
        reporter_assert!(reporter, alpha_index == 0);
    }

    // Request 128 bytes (which cannot fit in the remaining 64 bytes of Page 0).
    // The allocation must be atomic to Page 1: entire 128 bytes at offset 0 of Page 1.
    reporter_assert!(
        reporter,
        atlas_manager.request_alpha_space(&recorder, 128).is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 1);
        reporter_assert!(reporter, alpha_index == 0);
    }
    reporter_assert!(reporter, atlas_manager.num_pages() == 2);
});

def_graphite_test_for_all_contexts!(SparseStrips_AtlasFlipFlopAtFlush, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L268-L338 (chrome/m156)
    let recorder = context.make_recorder(None);
    let mut atlas_manager = AlphaAtlasManager::new();

    // Slot 0: 1 row (32,768 bytes)
    reporter_assert!(
        reporter,
        atlas_manager
            .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES)
            .is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 0);
    }
    reporter_assert!(reporter, atlas_manager.active_slot() == 0);
    let _initial_slot0 = atlas_manager.get_page_proxy(0);
    reporter_assert!(reporter, atlas_manager.num_pages() == 1);

    // Slot 1: 2 rows (65,536 bytes) via XOR flip-flop (0 ^ 1 = 1)
    reporter_assert!(
        reporter,
        atlas_manager.request_alpha_space(&recorder, 64).is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 1);
    }
    reporter_assert!(reporter, atlas_manager.active_slot() == 1);
    let initial_slot1 = atlas_manager.get_page_proxy(1);
    reporter_assert!(reporter, atlas_manager.num_pages() == 2);

    // Flush: recordUploads(nullptr) retires older slot 0 (1 ^ 1 = 0).
    // Slot 1 stays right in place in slot 1 (no moving needed)!
    atlas_manager.record_uploads(None, &recorder);
    reporter_assert!(reporter, atlas_manager.num_pages() == 1);
    reporter_assert!(reporter, atlas_manager.get_page_proxy(0).is_none());
    reporter_assert!(
        reporter,
        same_proxy(
            atlas_manager.get_page_proxy(1).as_ref(),
            initial_slot1.as_ref()
        )
    );
    reporter_assert!(reporter, atlas_manager.active_slot() == 1);

    // Continue allocating in current active slot (Slot 1):
    reporter_assert!(
        reporter,
        atlas_manager.request_alpha_space(&recorder, 64).is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 1);
    }
    reporter_assert!(reporter, atlas_manager.num_pages() == 1);

    // Fill Slot 1 and trigger flip-flop to Slot 0 (1 ^ 1 = 0) -> doubled to 4 rows!
    reporter_assert!(
        reporter,
        atlas_manager
            .request_alpha_space(&recorder, SparseStripConfig::K_ATLAS_WIDTH_BYTES * 2)
            .is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (_alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 0);
    }
    reporter_assert!(reporter, atlas_manager.active_slot() == 0);
    reporter_assert!(reporter, atlas_manager.num_pages() == 2);
    reporter_assert!(
        reporter,
        atlas_manager
            .get_page_proxy(0)
            .is_some_and(|proxy| proxy.dimensions().height == 4)
    );

    // Second flush: retires older slot 1 (0 ^ 1 = 1). Slot 0 remains active!
    atlas_manager.record_uploads(None, &recorder);
    reporter_assert!(reporter, atlas_manager.num_pages() == 1);
    reporter_assert!(reporter, atlas_manager.get_page_proxy(1).is_none());
    reporter_assert!(
        reporter,
        atlas_manager
            .get_page_proxy(0)
            .is_some_and(|proxy| proxy.dimensions().height == 4)
    );
    reporter_assert!(reporter, atlas_manager.active_slot() == 0);
});

def_graphite_test_for_all_contexts!(SparseStrips_AtlasStraddleAndMemmove, |reporter, context| {
    // Port of: tests/graphite/sparse_strips/AtlasTests.cpp#L340-L396 (chrome/m156)
    let recorder = context.make_recorder(None);
    let mut atlas_manager = AlphaAtlasManager::new();

    // Page 0 has 1 row (32,768 bytes).
    // Fill Page 0 up to 64 bytes before the end.
    let almost_full = SparseStripConfig::K_ATLAS_WIDTH_BYTES - 64;
    reporter_assert!(
        reporter,
        atlas_manager
            .request_alpha_space(&recorder, almost_full)
            .is_some()
    );
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 0);
        reporter_assert!(reporter, alpha_index == 0);
    }

    // Now start a run of 128 bytes (two 64-byte chunks).
    // Chunk 1 (64 bytes): fills the rest of Page 0.
    let ptr1 = atlas_manager.request_alpha_space(&recorder, 64);
    reporter_assert!(reporter, ptr1.is_some());
    if let Some(ptr1) = ptr1 {
        for (i, byte) in ptr1.iter_mut().enumerate().take(64) {
            *byte = (i + 1) as u8;
        }
    }

    // Chunk 2 (64 bytes): straddles past Page 0's capacity into overrun!
    let ptr2 = atlas_manager.request_alpha_space(&recorder, 64);
    reporter_assert!(reporter, ptr2.is_some());
    if let Some(ptr2) = ptr2 {
        for (i, byte) in ptr2.iter_mut().enumerate().take(64) {
            *byte = (i + 65) as u8;
        }
    }

    // Finalize the run: should detect straddle, create Page 1, and copy 128 bytes to offset 0 of
    // Page 1!
    {
        let alloc = atlas_manager.finalize_run(&recorder);
        reporter_assert!(reporter, alloc.is_some());
        let (alpha_index, tex_page) = alloc.unwrap_or(NO_ALLOC);
        reporter_assert!(reporter, tex_page == 1);
        reporter_assert!(reporter, alpha_index == 0);
    }
    reporter_assert!(reporter, atlas_manager.num_pages() == 2);
    reporter_assert!(reporter, atlas_manager.active_slot() == 1);

    // Verify that the written bytes were accurately moved into Page 1 at offset 0
    let proxy1 = atlas_manager.get_page_proxy(1);
    reporter_assert!(reporter, proxy1.is_some());
    reporter_assert!(
        reporter,
        proxy1
            .as_ref()
            .is_some_and(|proxy| proxy.dimensions().height == 2)
    ); // doubled rows

    let page1_data = atlas_manager.get_page_data(1);
    reporter_assert!(reporter, page1_data.is_some());
    if let Some(page1_data) = page1_data {
        for (i, byte) in page1_data.iter().enumerate().take(128) {
            reporter_assert!(reporter, *byte == (i + 1) as u8);
        }
    }
});
