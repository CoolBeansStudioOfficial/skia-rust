// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/RuntimeEffectDictionary.{h,cpp}

//! [`RuntimeEffectDictionary`]: the runtime effects and mesh specifications a `Recording` uses.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};

use skia_rust_core::mesh::{MeshSpecification, mesh_priv};
use skia_rust_core::runtime_effect::RuntimeEffect;
use skia_rust_core::runtime_effect_priv;

/// Keeps a live reference to every runtime effect and mesh specification that a `Recording` is
/// going to paint, and a way to retrieve its shader text from its code snippet id.
///
/// Each runtime effect dictionary lives for just one `Recording`. While recording, it is filled
/// with runtime effects. In `snap()`, ownership of it is assumed by the `PipelineCreationTask`s
/// that could require its contents and a new one takes its place in the `Recorder`.
// Port of: src/gpu/graphite/RuntimeEffectDictionary.h#L30-L54 (chrome/m156)
#[doc(alias = "skgpu::graphite::RuntimeEffectDictionary")]
#[derive(Debug, Default)]
pub struct RuntimeEffectDictionary {
    // `fSpinLock` guards both maps.
    state: Mutex<State>,
}

#[derive(Debug, Default)]
struct State {
    dict: HashMap<i32, RuntimeEffect>,
    mesh_spec_dict: HashMap<i32, Arc<MeshSpecification>>,
}

impl RuntimeEffectDictionary {
    /// An empty dictionary.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// The runtime effect with `code_snippet_id`, if there is one (`find`).
    // Port of: src/gpu/graphite/RuntimeEffectDictionary.h#L32-L37 (chrome/m156)
    #[must_use]
    pub fn find(&self, code_snippet_id: i32) -> Option<RuntimeEffect> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.dict.get(&code_snippet_id).cloned()
    }

    /// The mesh specification with `code_snippet_id`, if there is one (`findMeshSpec`).
    // Port of: src/gpu/graphite/RuntimeEffectDictionary.h#L38-L43 (chrome/m156)
    #[doc(alias = "findMeshSpec")]
    #[must_use]
    pub fn find_mesh_spec(&self, code_snippet_id: i32) -> Option<Arc<MeshSpecification>> {
        let state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.mesh_spec_dict.get(&code_snippet_id).cloned()
    }

    /// Records that `effect` has `code_snippet_id` (`set`).
    ///
    /// The same code snippet id must never refer to two different effects.
    // Port of: src/gpu/graphite/RuntimeEffectDictionary.cpp#L15-L22 (chrome/m156)
    pub fn set(&self, code_snippet_id: i32, effect: RuntimeEffect) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);

        debug_assert!(state.dict.get(&code_snippet_id).is_none_or(|existing| {
            runtime_effect_priv::hash(existing) == runtime_effect_priv::hash(&effect)
        }));
        state.dict.insert(code_snippet_id, effect);
    }

    /// Records that `spec` has `code_snippet_id` (`set`).
    // Port of: src/gpu/graphite/RuntimeEffectDictionary.cpp#L24-L31 (chrome/m156)
    #[doc(alias = "set")]
    pub fn set_mesh_spec(&self, code_snippet_id: i32, spec: Arc<MeshSpecification>) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);

        debug_assert!(
            state
                .mesh_spec_dict
                .get(&code_snippet_id)
                .is_none_or(|existing| mesh_priv::hash(existing) == mesh_priv::hash(&spec))
        );
        state.mesh_spec_dict.insert(code_snippet_id, spec);
    }
}
