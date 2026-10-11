// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! T23: the remote glyph cache. A slug made by the strike server's analysis canvas is serialized,
//! the strike data is read by the client, and the client reads the slug back with the typeface
//! ids translated (the Ganesh-only `SkRemoteGlyphCache_StrikeSerializationSlug*` tests of Skia
//! check the same path with pixels).

use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::font::{Edging, Font};
use skia_rust_core::font_style::FontStyle;
use skia_rust_core::font_types::{FontHinting, GlyphId};
use skia_rust_core::paint::Paint;
use skia_rust_core::rect::Rect;
use skia_rust_core::surface_props::SurfaceProps;
use skia_rust_core::text_blob::{TextBlob, TextBlobBuilder};
use skia_rust_core::typeface::Typeface;
use skia_rust_gpu::text_gpu::chrome_remote_glyph_cache::{
    CacheMissType, DiscardableHandleId, StrikeClient, StrikeClientDiscardableHandleManager,
    StrikeServer, StrikeServerDiscardableHandleManager,
};
use skia_rust_tools::font_tool_utils::create_test_typeface;

/// A manager that keeps every handle locked and counts the cache misses.
#[derive(Default)]
struct Manager {
    state: Mutex<(DiscardableHandleId, u32)>,
}

impl StrikeServerDiscardableHandleManager for Manager {
    fn create_handle(&self) -> DiscardableHandleId {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.0 += 1;
        state.0
    }

    fn lock_handle(&self, _id: DiscardableHandleId) -> bool {
        true
    }

    fn is_handle_deleted(&self, _id: DiscardableHandleId) -> bool {
        false
    }
}

impl StrikeClientDiscardableHandleManager for Manager {
    fn delete_handle(&self, _id: DiscardableHandleId) -> bool {
        false
    }

    fn notify_cache_miss(&self, _miss_type: CacheMissType, _font_size: i32) {
        self.state.lock().unwrap_or_else(PoisonError::into_inner).1 += 1;
    }
}

fn build_text_blob(typeface: &Typeface, glyph_count: usize, text_size: f32) -> TextBlob {
    let mut font = Font::default();
    font.set_typeface(Some(typeface.clone()));
    font.set_hinting(FontHinting::Normal);
    font.set_size(text_size);
    font.set_edging(Edging::AntiAlias);
    font.set_subpixel(true);

    let mut builder = TextBlobBuilder::new();
    let bounds = Rect::from_wh(10.0, 10.0);
    let (glyphs, pos) = builder.alloc_run_pos_h(&font, glyph_count, 0.0, Some(&bounds));
    for i in 0..glyph_count {
        glyphs[i] = GlyphId::try_from(i).expect("a small glyph id");
        pos[i] = f32::from(u16::try_from(i).expect("a small index"));
    }
    builder.make().expect("a text blob")
}

/// Makes a slug on the server, and reads it back on the client.
fn slug_round_trip(text_size: f32) {
    let manager = Arc::new(Manager::default());
    let server = StrikeServer::new(manager.clone());
    let mut client = StrikeClient::new(manager.clone(), false, None);

    let server_typeface = create_test_typeface(Some("monospace"), FontStyle::default());
    let server_blob = build_text_blob(&server_typeface, 10, text_size);
    let paint = Paint::default();

    let analysis_canvas =
        server.make_analysis_canvas(10, 10, &SurfaceProps::default(), None, true, true);
    let slug = analysis_canvas
        .convert_blob_to_slug(&server_blob, (0.0, 0.0), &paint)
        .expect("the server makes a slug");
    let slug_bytes = slug.serialize();

    let mut strike_data = Vec::new();
    server.write_strike_data(&mut strike_data);
    assert!(client.read_strike_data(&strike_data));

    let client_slug = client
        .deserialize_slug_for_test(slug_bytes.as_bytes())
        .expect("the client reads the slug");
    assert_eq!(client_slug.source_bounds(), slug.source_bounds());
    assert_eq!(
        client_slug.source_bounds_with_origin(),
        slug.source_bounds_with_origin()
    );

    // A slug that is cut short is refused.
    let bytes = slug_bytes.as_bytes();
    assert!(
        client
            .deserialize_slug_for_test(&bytes[..(bytes.len() / 2) & !3])
            .is_none()
    );
}

#[test]
fn slug_round_trips_as_direct_masks() {
    slug_round_trip(1.0);
}

#[test]
fn slug_round_trips_as_paths() {
    slug_round_trip(360.0);
}

#[test]
fn slug_round_trips_as_distance_field_text() {
    slug_round_trip(30.0);
}

#[test]
fn a_second_analysis_pass_sends_only_the_new_glyphs() {
    let manager = Arc::new(Manager::default());
    let server = StrikeServer::new(manager.clone());
    let mut client = StrikeClient::new(manager.clone(), false, None);

    let typeface = create_test_typeface(Some("monospace"), FontStyle::default());
    let canvas = server.make_analysis_canvas(10, 10, &SurfaceProps::default(), None, true, true);
    let paint = Paint::default();

    canvas.draw_text_blob(&build_text_blob(&typeface, 5, 1.0), (0.0, 0.0), &paint);
    let mut first = Vec::new();
    server.write_strike_data(&mut first);
    assert!(client.read_strike_data(&first));

    canvas.draw_text_blob(&build_text_blob(&typeface, 10, 1.0), (0.0, 0.0), &paint);
    let mut second = Vec::new();
    server.write_strike_data(&mut second);
    assert!(client.read_strike_data(&second));
    assert!(second.len() < first.len() * 2);

    // Nothing new: nothing is written.
    canvas.draw_text_blob(&build_text_blob(&typeface, 10, 1.0), (0.0, 0.0), &paint);
    let mut third = vec![1, 2, 3, 4];
    server.write_strike_data(&mut third);
    assert_eq!(third, [1, 2, 3, 4]);
}
