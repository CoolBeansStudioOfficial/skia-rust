// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkRuntimeShader.{h,cpp}

//! `SkRuntimeShader`: a shader that runs a [`RuntimeEffect`] through the Raster Pipeline.
//!
//! skia-rust: flattening is not ported.

use core::fmt;
use std::sync::Arc;

use crate::capabilities::Capabilities;
use crate::color_space::ColorSpace;
use crate::data::Data;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use skia_rust_sksl::codegen::rp;
use skia_rust_sksl::tracing::DebugTracePriv;

use crate::point::IPoint;
use crate::runtime_effect::{ChildPtr, RuntimeEffect, TracedShader};
use crate::runtime_effect_priv::{
    self as priv_, RuntimeEffectRpCallbacks, UniformsCallback, UniformsCallbackContext,
};
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};

/// A shader that runs a [`RuntimeEffect`] (`SkRuntimeShader`).
// Port of: src/shaders/SkRuntimeShader.h#L26-L67 (chrome/m156)
#[doc(alias = "SkRuntimeShader")]
pub struct RuntimeShader {
    effect: RuntimeEffect,
    uniform_data: Option<Data>,
    uniforms_callback: Option<UniformsCallback>,
    children: Vec<ChildPtr>,
}

impl fmt::Debug for RuntimeShader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeShader")
            .field("effect", &self.effect)
            .field("uniform_data", &self.uniform_data)
            .field("has_uniforms_callback", &self.uniforms_callback.is_some())
            .field("children", &self.children)
            .finish()
    }
}

impl RuntimeShader {
    /// A shader of `effect` with `uniforms` and `children`.
    // Port of: src/shaders/SkRuntimeShader.cpp#L43-L51 (chrome/m156)
    #[must_use]
    pub fn new(effect: RuntimeEffect, uniforms: Data, children: &[ChildPtr]) -> Self {
        RuntimeShader {
            effect,
            uniform_data: Some(uniforms),
            uniforms_callback: None,
            children: children.to_vec(),
        }
    }

    /// A shader of `effect` whose uniforms come from `uniforms_callback` at draw time.
    // Port of: src/shaders/SkRuntimeShader.cpp#L53-L60 (chrome/m156)
    #[must_use]
    pub fn new_deferred(
        effect: RuntimeEffect,
        uniforms_callback: UniformsCallback,
        children: &[ChildPtr],
    ) -> Self {
        RuntimeShader {
            effect,
            uniform_data: None,
            uniforms_callback: Some(uniforms_callback),
            children: children.to_vec(),
        }
    }

    /// The effect (`effect`).
    #[must_use]
    pub fn effect(&self) -> &RuntimeEffect {
        &self.effect
    }

    /// The children (`children`).
    #[must_use]
    pub fn children(&self) -> &[ChildPtr] {
        &self.children
    }

    /// The uniform data, invoking the uniforms callback (if any) for `dst_cs` (`uniformData`).
    ///
    /// # Panics
    /// Never for a shader made by the constructors: it has uniform data or a callback.
    // Port of: src/shaders/SkRuntimeShader.cpp#L125-L136 (chrome/m156)
    #[doc(alias = "uniformData")]
    #[must_use]
    pub fn uniform_data(&self, dst_cs: Option<&ColorSpace>) -> Data {
        if let Some(data) = &self.uniform_data {
            return data.clone();
        }

        // We want to invoke the uniforms-callback each time a paint occurs.
        let callback = self
            .uniforms_callback
            .as_ref()
            .expect("a shader has uniform data or a uniforms callback");
        let uniforms = callback(&UniformsCallbackContext {
            dst_color_space: dst_cs,
        });
        debug_assert_eq!(uniforms.size(), self.effect.uniform_size());
        uniforms
    }

    /// `makeTracedClone`: a copy of this shader on an unoptimized copy of its effect, which
    /// records a debug trace of the pixel at `coord`.
    // Port of: src/shaders/SkRuntimeShader.cpp#L63-L77 (chrome/m156)
    pub(crate) fn make_traced_clone(&self, coord: IPoint) -> TracedShader {
        let unoptimized = self.effect.make_unoptimized_clone();
        let source = unoptimized.source().as_bytes();
        // The program is compiled now, with its trace ops recording into this trace (the copy is
        // new, so its program is not compiled yet).
        let debug_trace = unoptimized
            .rp_program_traced(new_debug_trace(source, coord))
            .and_then(rp::Program::debug_trace_handle)
            // A program that does not compile has no trace to record into.
            .unwrap_or_else(|| Arc::new(new_debug_trace(source, coord)));
        let shader = RuntimeShader::new(unoptimized, self.uniform_data(None), &self.children);
        TracedShader {
            shader: Shader::from_base(shader),
            debug_trace,
        }
    }

    // Port of: src/shaders/SkRuntimeShader.cpp#L95-L123 (chrome/m156)
    fn append_stages_impl(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        if !priv_::can_draw(Capabilities::raster_backend(), &self.effect) {
            // SkRP has support for many parts of #version 300 already, but for now, we restrict
            // its usage in runtime effects to just #version 100.
            return false;
        }
        if let Some(program) = self.effect.rp_program() {
            let Some(new_m_rec) = m_rec.apply(rec, Matrix::i()) else {
                return false;
            };
            let uniforms = priv_::uniforms_as_span(
                self.effect.uniforms(),
                &self.uniform_data(rec.dst_cs),
                rec.dst_cs,
            );
            let mut callbacks = RuntimeEffectRpCallbacks::new(
                rec,
                &new_m_rec,
                &self.children,
                &self.effect.0.sample_usages,
            );
            let alloc = rec.alloc;
            return program.append_stages(
                &mut *rec.pipeline,
                alloc,
                Some(&mut callbacks),
                &uniforms,
            );
        }
        false
    }
}

/// A debug trace of the source of `source` for the pixel at `coord` (`makeDebugTrace`).
// Port of: src/shaders/SkRuntimeShader.cpp#L63-L69 (chrome/m156)
fn new_debug_trace(source: &[u8], coord: IPoint) -> DebugTracePriv {
    let mut debug_trace = DebugTracePriv::default();
    debug_trace.set_source(source);
    debug_trace.set_trace_coord(coord.x, coord.y);
    debug_trace
}

impl ShaderBase for RuntimeShader {
    // Port of: src/shaders/SkRuntimeShader.h#L40 (chrome/m156)
    fn is_opaque(&self) -> bool {
        priv_::always_opaque(&self.effect)
    }

    // Port of: src/shaders/SkRuntimeShader.h#L42 (chrome/m156)
    fn shader_type(&self) -> ShaderType {
        ShaderType::Runtime
    }

    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        self.append_stages_impl(rec, m_rec)
    }
}
