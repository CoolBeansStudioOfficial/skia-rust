// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RuntimeEffectDictionary.h (placeholder)

//! The seam for `skgpu::graphite::RuntimeEffectDictionary`.
//!
//! The dictionary maps code snippet ids to the runtime effects and mesh specifications a
//! `Recording` uses. It is ported with G5a (it needs `SkRuntimeEffect` and `SkMeshSpecification`
//! ports); until then the recorder creates one per recording and the tasks pass it on without
//! reading it.

/// Keeps a live reference to every runtime effect that a `Recording` is going to paint, and a
/// way to retrieve its shader text from its code snippet id.
#[doc(alias = "skgpu::graphite::RuntimeEffectDictionary")]
#[derive(Debug, Default)]
pub struct RuntimeEffectDictionary {}

impl RuntimeEffectDictionary {
    /// An empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
