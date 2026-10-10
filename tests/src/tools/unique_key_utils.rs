// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/graphite/UniqueKeyUtils.h, .cpp (chrome/m156)

//! Helpers over the graphics pipelines a `PrecompileContext`'s global cache holds.

use skia_rust_gpu::gpu::resource_key::UniqueKey;
use skia_rust_gpu::graphite::precompile_context::PrecompileContext;

/// `UniqueKeyUtils::FetchUniqueKeys(precompileContext, keys)`: the key of every graphics pipeline
/// in the global cache, in the cache's iteration order.
// Port of: tools/graphite/UniqueKeyUtils.cpp#L11-L20 (chrome/m156)
#[doc(alias = "FetchUniqueKeys")]
#[must_use]
pub fn fetch_unique_keys(precompile_context: &PrecompileContext) -> Vec<UniqueKey> {
    let global_cache = precompile_context
        .shared_context()
        .shared_context()
        .global_cache();
    let mut keys = Vec::with_capacity(global_cache.num_graphics_pipelines());
    global_cache.for_each_graphics_pipeline(|key, _pipeline| keys.push(key.clone()));
    keys
}
