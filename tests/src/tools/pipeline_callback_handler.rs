// Copyright 2025 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Port of: tools/graphite/PipelineCallbackHandler.h, .cpp (chrome/m156)

//! A caching-callback client: it collects the pipelines a context reports (their labels and, when
//! they can be serialized, their Android-style keys), so that a test can recreate them later.

use std::sync::{Arc, Mutex};

use skia_rust_core::data::Data;
use skia_rust_gpu::graphite::context_options::{
    Callback, PipelineCacheOp, PipelineCachingCallbackFn,
};

// Port of: tools/graphite/PipelineCallbackHandler.h#L35-L37 (chrome/m156), `PipelineData`
#[derive(Debug)]
struct PipelineData {
    label: String,
    android_style_key: Option<Data>,
    unique_key_hash: u32,
    uses: u32,
}

/// `skiatools::graphite::PipelineCallBackHandler`.
///
/// Skia keeps the pipelines in a hash table keyed by `(label, uniqueKeyHash)`; here a `Vec` keeps
/// the same entries in insertion order, and the lookup is a linear scan.
// Port of: tools/graphite/PipelineCallbackHandler.h#L24-L113 (chrome/m156)
#[derive(Debug, Default)]
pub struct PipelineCallBackHandler {
    map: Mutex<Vec<PipelineData>>,
}

impl PipelineCallBackHandler {
    /// A handler with no pipelines.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    /// `PipelineCallBackHandler::CallBack`, as the caching callback of a context: the handler
    /// adds every pipeline the context reports.
    // Port of: tools/graphite/PipelineCallbackHandler.h#L28-L36 (chrome/m156)
    #[must_use]
    pub fn caching_callback(self: &Arc<Self>) -> Callback<PipelineCachingCallbackFn> {
        let handler = Arc::clone(self);
        let callback: Arc<PipelineCachingCallbackFn> = Arc::new(
            move |op: PipelineCacheOp,
                  label: &str,
                  unique_key_hash: u32,
                  from_precompile: bool,
                  android_style_key: Option<&Data>| {
                handler.add(
                    op,
                    label,
                    unique_key_hash,
                    from_precompile,
                    android_style_key.cloned(),
                );
            },
        );
        Callback(callback)
    }

    /// `add(op, label, uniqueKeyHash, fromPrecompile, androidStyleKey)`.
    // Port of: tools/graphite/PipelineCallbackHandler.cpp#L12-L31 (chrome/m156)
    pub fn add(
        &self,
        op: PipelineCacheOp,
        label: &str,
        unique_key_hash: u32,
        from_precompile: bool,
        android_style_key: Option<Data>,
    ) {
        let mut map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(found) = map
            .iter_mut()
            .find(|d| d.unique_key_hash == unique_key_hash && d.label == label)
        {
            if found.android_style_key.is_none() && android_style_key.is_some() {
                found.android_style_key = android_style_key;
            }
            if op == PipelineCacheOp::PipelineFound {
                found.uses += 1;
            }
        } else {
            map.push(PipelineData {
                label: label.to_owned(),
                android_style_key,
                unique_key_hash,
                uses: u32::from(!from_precompile),
            });
        }
    }

    /// `retrieveKeys(result)`: the Android-style keys of the pipelines that have one. Not every
    /// pipeline is serializable, so the ones without a key are left out.
    // Port of: tools/graphite/PipelineCallbackHandler.h#L49-L58 (chrome/m156)
    #[must_use]
    pub fn retrieve_keys(&self) -> Vec<Data> {
        let map = self.map.lock().unwrap_or_else(|e| e.into_inner());
        map.iter()
            .filter_map(|d| d.android_style_key.clone())
            .collect()
    }

    /// `reset()`: forgets every pipeline.
    // Port of: tools/graphite/PipelineCallbackHandler.h#L60-L64 (chrome/m156)
    pub fn reset(&self) {
        self.map.lock().unwrap_or_else(|e| e.into_inner()).clear();
    }
}
