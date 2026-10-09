// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/gpu/graphite/ShaderInfo.h (the members G5a reads)

//! The seam for `skgpu::graphite::ShaderInfo`.
//!
//! `ShaderInfo` (the snippet tree to `SkSL` generator, blend and coverage code, labels) is ported
//! with G6. The snippet preamble generators of the `ShaderCodeDictionary` (G5a) read three things
//! from it, and this struct holds exactly those; G6 adds the rest of Skia's members to it.

use std::sync::Arc;

use crate::graphite::runtime_effect_dictionary::RuntimeEffectDictionary;
use crate::graphite::shader_code_dictionary::ShaderCodeDictionary;

/// Holds all root `ShaderNode`s defined for a `PaintParams` as well as the extracted fixed
/// function blending parameters and other aggregate requirements for the effect trees that have
/// been linked into a single fragment program.
///
/// Only the members the snippet preamble generators use exist so far (see the module docs).
// Port of: src/gpu/graphite/ShaderInfo.h#L33-L146 (chrome/m156)
#[doc(alias = "skgpu::graphite::ShaderInfo")]
#[derive(Debug)]
pub struct ShaderInfo {
    shader_code_dictionary: ShaderCodeDictionary,
    runtime_effect_dictionary: Arc<RuntimeEffectDictionary>,
    uniform_ssbo_index: Option<&'static str>,
}

impl ShaderInfo {
    /// Name used in-shader for storage buffer uniform (`kStorageBufferName`).
    // Port of: src/gpu/graphite/ShaderInfo.h#L91 (chrome/m156)
    #[doc(alias = "kStorageBufferName")]
    pub const STORAGE_BUFFER_NAME: &'static str = "fsStorageBuffer";

    /// The `ShaderInfo(dict, rteDict, uniformSsboIndex, ...)` constructor.
    ///
    /// `uniform_ssbo_index` is the `SkSL` expression that indexes the storage buffer of combined
    /// uniforms, or `None` when the uniforms are in a regular uniform block.
    // Port of: src/gpu/graphite/ShaderInfo.h#L96-L99 (chrome/m156)
    #[must_use]
    pub fn new(
        shader_code_dictionary: &ShaderCodeDictionary,
        runtime_effect_dictionary: Arc<RuntimeEffectDictionary>,
        uniform_ssbo_index: Option<&'static str>,
    ) -> Self {
        Self {
            shader_code_dictionary: shader_code_dictionary.clone(),
            runtime_effect_dictionary,
            uniform_ssbo_index,
        }
    }

    /// `shaderCodeDictionary()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L57-L59 (chrome/m156)
    #[doc(alias = "shaderCodeDictionary")]
    #[must_use]
    pub fn shader_code_dictionary(&self) -> &ShaderCodeDictionary {
        &self.shader_code_dictionary
    }

    /// `runtimeEffectDictionary()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L60-L62 (chrome/m156)
    #[doc(alias = "runtimeEffectDictionary")]
    #[must_use]
    pub fn runtime_effect_dictionary(&self) -> &RuntimeEffectDictionary {
        &self.runtime_effect_dictionary
    }

    /// `uniformSsboIndex()`.
    // Port of: src/gpu/graphite/ShaderInfo.h#L64 (chrome/m156)
    #[doc(alias = "uniformSsboIndex")]
    #[must_use]
    pub fn uniform_ssbo_index(&self) -> Option<&str> {
        self.uniform_ssbo_index
    }
}
