// Copyright 2018 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/SkRemoteGlyphCacheTest.cpp (chrome/m156)
//
// Not ported: the Ganesh-only tests (`DEF_GANESH_TEST*`, which are `excluded` in the manifest),
// and the helpers they use (`compare_blobs`, `MakeSurface`, `RasterBlob*`, ...).

#![cfg(test)]

use std::collections::HashSet;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use skia_rust_core::descriptor::{AutoDescriptor, Descriptor, REC_TAG};
use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_metrics::FontMetrics;
use skia_rust_core::font_metrics_priv;
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FontHinting, GlyphId};
use skia_rust_core::graphics::{
    purge_font_cache, set_typeface_cache_count_limit, typeface_cache_count_limit,
};
use skia_rust_core::mask::MaskFormat;
use skia_rust_core::paint::Paint;
use skia_rust_core::path::Path;
use skia_rust_core::point::Point;
use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::rect::Rect;
use skia_rust_core::scaler_context::{SCALER_CONTEXT_REC_SIZE, ScalerContextRec};
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::{Typeface, TypefaceId};
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_gpu::text_gpu::chrome_remote_glyph_cache::{
    CacheMissType, DiscardableHandleId, StrikeClient, StrikeClientDiscardableHandleManager,
    StrikeServer, StrikeServerDiscardableHandleManager,
};
use skia_rust_gpu::text_gpu::typeface_remote::TypefaceProxyPrototype;
use skia_rust_tools::font_tool_utils::create_test_typeface;

use crate::{def_test, reporter_assert};

/// The state of the [`DiscardableManager`] that its lock guards.
struct DiscardableManagerState {
    /// `fNextHandleId`.
    next_handle_id: DiscardableHandleId,
    /// `fLastDeletedHandleId`.
    last_deleted_handle_id: DiscardableHandleId,
    /// `fLockedHandles`.
    locked_handles: HashSet<DiscardableHandleId>,
    /// `fCacheMissCount`.
    cache_miss_count: [i32; CacheMissType::LAST as usize + 1],
}

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L72-L158 (chrome/m156)
struct DiscardableManager {
    // The tests below run in parallel on multiple threads and use the same process global
    // SkStrikeCache. So the implementation needs to be thread-safe.
    state: Mutex<DiscardableManagerState>,
}

impl DiscardableManager {
    fn new() -> Self {
        Self {
            state: Mutex::new(DiscardableManagerState {
                next_handle_id: 0,
                last_deleted_handle_id: 0,
                locked_handles: HashSet::new(),
                cache_miss_count: [0; CacheMissType::LAST as usize + 1],
            }),
        }
    }

    fn lock(&self) -> MutexGuard<'_, DiscardableManagerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn unlock_all(&self) {
        self.lock().locked_handles.clear();
    }

    fn unlock_and_delete_all(&self) {
        let mut state = self.lock();
        state.locked_handles.clear();
        state.last_deleted_handle_id = state.next_handle_id;
    }

    fn locked_handles(&self) -> HashSet<DiscardableHandleId> {
        self.lock().locked_handles.clone()
    }

    fn handle_count(&self) -> DiscardableHandleId {
        self.lock().next_handle_id
    }

    #[allow(dead_code)] // used by the Ganesh tests of the C++ file
    fn cache_miss_count(&self, miss_type: usize) -> i32 {
        self.lock().cache_miss_count[miss_type]
    }

    #[allow(dead_code)] // used by the Ganesh tests of the C++ file
    fn has_cache_miss(&self) -> bool {
        self.lock().cache_miss_count.iter().any(|&count| count > 0)
    }
}

impl StrikeServerDiscardableHandleManager for DiscardableManager {
    // Server implementation.
    fn create_handle(&self) -> DiscardableHandleId {
        let mut state = self.lock();

        // Handles starts as locked.
        state.next_handle_id += 1;
        let id = state.next_handle_id;
        state.locked_handles.insert(id);
        id
    }

    fn lock_handle(&self, id: DiscardableHandleId) -> bool {
        let mut state = self.lock();

        if id <= state.last_deleted_handle_id {
            return false;
        }
        state.locked_handles.insert(id);
        true
    }

    fn is_handle_deleted(&self, id: DiscardableHandleId) -> bool {
        let state = self.lock();

        id <= state.last_deleted_handle_id
    }
}

impl StrikeClientDiscardableHandleManager for DiscardableManager {
    // Client implementation.
    fn delete_handle(&self, id: DiscardableHandleId) -> bool {
        let state = self.lock();

        id <= state.last_deleted_handle_id
    }

    fn notify_cache_miss(&self, miss_type: CacheMissType, _font_size: i32) {
        let mut state = self.lock();

        state.cache_miss_count[miss_type as usize] += 1;
    }
}

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L160-L183 (chrome/m156)
fn build_text_blob(tf: &Typeface, glyph_count: usize, text_size: i32) -> TextBlob {
    let mut font = Font::default();
    font.set_typeface(Some(tf.clone()));
    font.set_hinting(FontHinting::Normal);
    #[allow(clippy::cast_precision_loss)] // the sizes are small
    font.set_size(text_size as f32);
    font.set_edging(Edging::AntiAlias);
    font.set_subpixel(true);

    let mut builder = TextBlobBuilder::new();
    let bounds = Rect::from_wh(10.0, 10.0);
    let (glyphs, pos) = builder.alloc_run_pos_h(&font, glyph_count, 0.0, Some(&bounds));

    for i in 0..glyph_count {
        #[allow(clippy::cast_precision_loss, clippy::cast_possible_truncation)] // small indices
        {
            glyphs[i] = i as GlyphId;
            pos[i] = i as f32;
        }
    }
    builder.make().expect("a text blob")
}

/// `ToolUtils::CreateTestTypeface("monospace", SkFontStyle())`.
fn monospace() -> Typeface {
    create_test_typeface(Some("monospace"), FontStyle::default())
}

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L703-L735 (chrome/m156)
def_test!(SkRemoteGlyphCache_StrikeLockingServer, |reporter| {
    let discardable_manager = Arc::new(DiscardableManager::new());
    let server = StrikeServer::new(discardable_manager.clone());
    let _client = StrikeClient::new(discardable_manager.clone(), false, None);

    let server_typeface = monospace();
    let glyph_count = 10;
    let server_blob = build_text_blob(&server_typeface, glyph_count, 1);

    let props = SurfaceProps::default();
    let cache_diff_canvas = server.make_analysis_canvas(10, 10, &props, None, true, true);
    let paint = Paint::default();
    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);

    // The strike from the blob should be locked after it has been drawn on the canvas.
    reporter_assert!(reporter, discardable_manager.handle_count() == 1);
    reporter_assert!(reporter, discardable_manager.locked_handles().len() == 1);

    // Write the strike data and unlock everything. Re-analyzing the blob should lock the handle
    // again.
    let mut font_data = Vec::new();
    server.write_strike_data(&mut font_data);
    discardable_manager.unlock_all();
    reporter_assert!(reporter, discardable_manager.locked_handles().is_empty());

    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);
    reporter_assert!(reporter, discardable_manager.handle_count() == 1);
    reporter_assert!(reporter, discardable_manager.locked_handles().len() == 1);

    // Must unlock everything on termination, otherwise memory leaks can be reported.
    discardable_manager.unlock_and_delete_all();
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L737-L770 (chrome/m156)
def_test!(SkRemoteGlyphCache_StrikeDeletionServer, |reporter| {
    let discardable_manager = Arc::new(DiscardableManager::new());
    let server = StrikeServer::new(discardable_manager.clone());
    let _client = StrikeClient::new(discardable_manager.clone(), false, None);

    let server_typeface = monospace();
    let glyph_count = 10;
    let server_blob = build_text_blob(&server_typeface, glyph_count, 1);

    let props = SurfaceProps::default();
    let cache_diff_canvas = server.make_analysis_canvas(10, 10, &props, None, true, true);
    let paint = Paint::default();
    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);
    reporter_assert!(reporter, discardable_manager.handle_count() == 1);

    // Write the strike data and delete all the handles. Re-analyzing the blob should create new
    // handles.
    let mut font_data = Vec::new();
    server.write_strike_data(&mut font_data);

    // Another analysis pass, to ensure that deleting handles after a complete cache hit still
    // works. This is a regression test for crbug.com/999682.
    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);
    server.write_strike_data(&mut font_data);
    reporter_assert!(reporter, discardable_manager.handle_count() == 1);

    discardable_manager.unlock_and_delete_all();
    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);
    reporter_assert!(reporter, discardable_manager.handle_count() == 2);

    // Must unlock everything on termination, otherwise memory leaks can be reported.
    discardable_manager.unlock_and_delete_all();
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L772-L810 (chrome/m156)
def_test!(SkRemoteGlyphCache_StrikePinningClient, |reporter| {
    let discardable_manager = Arc::new(DiscardableManager::new());
    let server = StrikeServer::new(discardable_manager.clone());
    let mut client = StrikeClient::new(discardable_manager.clone(), false, None);

    // Server.
    let server_typeface = monospace();
    let server_typeface_id: TypefaceId = server_typeface.unique_id();

    let glyph_count = 10;
    let server_blob = build_text_blob(&server_typeface, glyph_count, 1);

    let props = SurfaceProps::default();
    let cache_diff_canvas = server.make_analysis_canvas(10, 10, &props, None, true, true);
    let paint = Paint::default();
    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);

    let mut server_strike_data = Vec::new();
    server.write_strike_data(&mut server_strike_data);

    // Client.
    reporter_assert!(reporter, client.read_strike_data(&server_strike_data));
    // C++ holds a raw pointer, which does not own the typeface. Here the handle is a reference,
    // so the client and the handle own the typeface when nothing else does.
    let client_typeface = client
        .retrieve_typeface_using_server_id_for_test(server_typeface_id)
        .expect("the client has the typeface");

    // The cache remains alive until it is pinned in the discardable manager.
    purge_font_cache();
    reporter_assert!(reporter, client_typeface.strong_count() > 2);

    // Once the strike is unpinned and purged, SkStrikeClient should be the only owner of the
    // clientTf.
    discardable_manager.unlock_and_delete_all();
    purge_font_cache();
    reporter_assert!(reporter, client_typeface.strong_count() == 2);

    // Must unlock everything on termination, otherwise memory leaks can be reported.
    discardable_manager.unlock_and_delete_all();
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L812-L838 (chrome/m156)
def_test!(SkRemoteGlyphCache_ClientMemoryAccounting, |reporter| {
    let discardable_manager = Arc::new(DiscardableManager::new());
    let server = StrikeServer::new(discardable_manager.clone());
    let mut client = StrikeClient::new(discardable_manager.clone(), false, None);

    // Server.
    let server_typeface = monospace();

    let glyph_count = 10;
    let server_blob = build_text_blob(&server_typeface, glyph_count, 1);

    let props = SurfaceProps::default();
    let cache_diff_canvas = server.make_analysis_canvas(10, 10, &props, None, true, true);
    let paint = Paint::default();
    cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);

    let mut server_strike_data = Vec::new();
    server.write_strike_data(&mut server_strike_data);

    // Client.
    reporter_assert!(reporter, client.read_strike_data(&server_strike_data));

    // Must unlock everything on termination, otherwise memory leaks can be reported.
    discardable_manager.unlock_and_delete_all();
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L840-L884 (chrome/m156)
def_test!(SkRemoteGlyphCache_PurgesServerEntries, |reporter| {
    let discardable_manager = Arc::new(DiscardableManager::new());
    let server = StrikeServer::new(discardable_manager.clone());
    server.set_max_entries_in_descriptor_map_for_testing(1);
    let _client = StrikeClient::new(discardable_manager.clone(), false, None);

    {
        let server_typeface = monospace();
        let glyph_count = 10;
        let server_blob = build_text_blob(&server_typeface, glyph_count, 1);

        let props = SurfaceProps::default();
        let cache_diff_canvas = server.make_analysis_canvas(10, 10, &props, None, true, true);
        let paint = Paint::default();
        reporter_assert!(reporter, server.remote_strike_map_size_for_testing() == 0);
        cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);
        reporter_assert!(reporter, server.remote_strike_map_size_for_testing() == 1);
    }

    // Serialize to release the lock from the strike server and delete all current
    // handles.
    let mut font_data = Vec::new();
    server.write_strike_data(&mut font_data);
    discardable_manager.unlock_and_delete_all();

    // Use a different typeface. Creating a new strike should evict the previous
    // one.
    {
        let server_typeface = create_test_typeface(Some("Georgia"), FontStyle::default());
        let glyph_count = 10;
        let server_blob = build_text_blob(&server_typeface, glyph_count, 1);

        let props = SurfaceProps::default();
        let cache_diff_canvas = server.make_analysis_canvas(10, 10, &props, None, true, true);
        let paint = Paint::default();
        reporter_assert!(reporter, server.remote_strike_map_size_for_testing() == 1);
        cache_diff_canvas.draw_text_blob(&server_blob, (0.0, 0.0), &paint);
        reporter_assert!(reporter, server.remote_strike_map_size_for_testing() == 1);
    }

    // Must unlock everything on termination, otherwise memory leaks can be reported.
    discardable_manager.unlock_and_delete_all();
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L1395-L1401 (chrome/m156)
def_test!(SkTypefaceProxy_Basic_Serial, |reporter| {
    // Must be multiple of 4 bytes or the buffer will be invalid.
    let broken_bytes: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut broken_buffer = ReadBuffer::new(&broken_bytes);
    let broken_proto = TypefaceProxyPrototype::make_from_buffer(&mut broken_buffer);
    reporter_assert!(reporter, broken_proto.is_none());
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L1403-L1412 (chrome/m156)
def_test!(SkGraphics_Limits, |reporter| {
    let prev1 = typeface_cache_count_limit();

    let mut prev2 = set_typeface_cache_count_limit(prev1 + 1);
    reporter_assert!(reporter, prev1 == prev2);
    prev2 = typeface_cache_count_limit();
    reporter_assert!(reporter, prev2 == prev1 + 1);

    let _ = set_typeface_cache_count_limit(prev1); // restore orig
});

// Port of: tests/SkRemoteGlyphCacheTest.cpp#L1414-L1494 (chrome/m156)
def_test!(SkRemoteGlyphCache_b513780208, |reporter| {
    // Legitimate path glyphs should only use kBW_Format, kA8_Format, or kLCD16_Format.
    // If a malicious strike provided a path with kARGB32_Format, it could trigger an
    // uninitialized memory issue in SkScalerContext::GenerateImageFromPath.
    //
    // SkGlyph::addPathFromBuffer now rejects any path whose format is not an allowed type.

    const K_W: i32 = 200;
    const K_H: u32 = 4;
    const K_SERVER_TYPEFACE_ID: TypefaceId = 0x6500_0000; // arbitrary
    const K_PACKED_GLYPH_ID: u32 = 0x42; // arbitrary

    let build_malicious_strike_data = || {
        let mut buffer = BinaryWriteBuffer::new();

        // --- typefaces ---
        buffer.write_int(1); // typefaceCount
        buffer.write_uint(K_SERVER_TYPEFACE_ID);
        buffer.write_int(256);
        buffer.write_int(0);
        buffer.write_bool(false);
        buffer.write_bool(false);

        // --- strikes ---
        buffer.write_int(1); // strikeCount
        buffer.write_uint(K_SERVER_TYPEFACE_ID); // serverTypefaceID
        buffer.write_uint(1); // discardableHandleID

        // SkDescriptor: Craft a Rec that triggers GenerateImageFromPath (fFrameWidth >= 0).
        {
            let mut rec = ScalerContextRec::default();
            rec.typeface_id = K_SERVER_TYPEFACE_ID;
            rec.text_size = 16.0;
            rec.pre_scale_x = 1.0;
            rec.post_2x2[0][0] = 1.0;
            rec.post_2x2[1][1] = 1.0;
            rec.frame_width = 0.0; // triggers fGenerateImageFromPath
            rec.mask_format = MaskFormat::A8;
            let mut ad = AutoDescriptor::with_size(
                Descriptor::compute_overhead(1) + SCALER_CONTEXT_REC_SIZE,
            );
            let desc = ad.get_desc_mut();
            desc.add_entry(REC_TAG, SCALER_CONTEXT_REC_SIZE, Some(&rec.to_bytes()));
            desc.compute_checksum();
            desc.flatten(&mut buffer);
        }

        buffer.write_bool(false); // fontMetricsInitialized == false
        {
            let fm = FontMetrics::default();
            font_metrics_priv::flatten(&mut buffer, &fm);
        }

        buffer.write_int(0); // imagesCount

        // Craft a path glyph with an invalid mask format (kARGB32_Format).
        buffer.write_int(1); // pathsCount
        buffer.write_uint(K_PACKED_GLYPH_ID);
        #[allow(clippy::cast_precision_loss)] // 200 is exact
        buffer.write_point(Point::new(K_W as f32, 0.0));
        #[allow(clippy::cast_sign_loss)] // 200 is positive
        buffer.write_uint(((K_W as u32) << 16) | K_H);
        buffer.write_uint(0);
        buffer.write_uint(MaskFormat::Argb32 as u32);
        buffer.write_bool(true); // hasPath
        buffer.write_bool(false);
        buffer.write_bool(false);
        buffer.write_path(&Path::rect(Rect::from_wh(1.0, 1.0), None));

        buffer.write_int(0); // drawablesCount

        buffer.snapshot_as_data()
    };

    let blob = build_malicious_strike_data();
    reporter_assert!(reporter, !blob.as_bytes().is_empty());

    let cache = StrikeCache::new();
    let discardable_manager = Arc::new(DiscardableManager::new());
    let mut client = StrikeClient::new(discardable_manager, /* is_logging */ false, Some(&cache));

    // This should fail to read the strike data because SkGlyph::addPathFromBuffer
    // now validates that path glyphs have an expected mask format.
    reporter_assert!(reporter, !client.read_strike_data(blob.as_bytes()));
});
