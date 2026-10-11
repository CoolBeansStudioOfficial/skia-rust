// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tests/StrikeForGPUTest.cpp (chrome/m156)

#![cfg(test)]

use std::sync::Arc;

use skia_rust_core::read_buffer::ReadBuffer;
use skia_rust_core::strike::{StrikePinner, StrikePromise};
use skia_rust_core::strike_cache::StrikeCache;
use skia_rust_core::strike_spec::StrikeSpec;
use skia_rust_core::write_buffer::BinaryWriteBuffer;
use skia_rust_tools::font_tool_utils::default_font;

use crate::{def_test, reporter_assert};

// Port of: tests/StrikeForGPUTest.cpp#L31-L36 (chrome/m156)
struct Pinner;

impl StrikePinner for Pinner {
    // Changing canDelete to return true causes this test to expectedly fail.
    fn can_delete(&mut self) -> bool {
        false
    }
}

// Port of: tests/StrikeForGPUTest.cpp#L27-L61 (chrome/m156)
def_test!(SkStrikePromise_Basic, |reporter| {
    let strike_cache = StrikeCache::new();
    let (strike_spec, _) = StrikeSpec::make_canonicalized(&default_font(), None);

    let to_compare_with: usize;
    let data;

    // Ensure that the ref in srcPromise is dropped.
    {
        // Make a strike with a Pinner.
        let strike = strike_cache.create_strike(&strike_spec, None, Some(Box::new(Pinner)));
        to_compare_with = Arc::as_ptr(&strike) as usize;
        let src_promise = StrikePromise::from_strike(strike);
        let mut write_buffer = BinaryWriteBuffer::new();
        src_promise.flatten(&mut write_buffer);
        data = write_buffer.snapshot_as_data();
    }

    // Remove all unpinned strikes.
    strike_cache.purge_all();

    let mut read_buffer = ReadBuffer::new(data.as_bytes());
    let dst_promise = StrikePromise::make_from_buffer(&mut read_buffer, None, &strike_cache);

    reporter_assert!(reporter, dst_promise.is_some());
    if let Some(mut dst_promise) = dst_promise {
        reporter_assert!(
            reporter,
            Arc::as_ptr(dst_promise.strike()) as usize == to_compare_with
        );
    }
});
