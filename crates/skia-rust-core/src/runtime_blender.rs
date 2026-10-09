// Copyright 2019 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/core/SkRuntimeBlender.{h,cpp}

//! `SkRuntimeBlender`: a blender that runs a [`RuntimeEffect`] through the Raster Pipeline.
//!
//! skia-rust: flattening is not ported.

use core::fmt;

use crate::blender::{BlenderBase, BlenderType};
use crate::capabilities::Capabilities;
use crate::data::Data;
use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::runtime_effect::{ChildPtr, RuntimeEffect};
use crate::runtime_effect_priv::{self as priv_, RuntimeEffectRpCallbacks};
use crate::shaders::shader_base::MatrixRec;

/// A blender that runs a [`RuntimeEffect`] (`SkRuntimeBlender`).
// Port of: src/core/SkRuntimeBlender.h#L24-L51 (chrome/m156)
#[doc(alias = "SkRuntimeBlender")]
pub struct RuntimeBlender {
    effect: RuntimeEffect,
    uniforms: Data,
    children: Vec<ChildPtr>,
}

impl fmt::Debug for RuntimeBlender {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeBlender")
            .field("effect", &self.effect)
            .field("uniforms", &self.uniforms)
            .field("children", &self.children)
            .finish()
    }
}

impl RuntimeBlender {
    /// A blender of `effect` with `uniforms` and `children`.
    // Port of: src/core/SkRuntimeBlender.h#L27-L32 (chrome/m156)
    #[must_use]
    pub fn new(effect: RuntimeEffect, uniforms: Data, children: &[ChildPtr]) -> Self {
        RuntimeBlender {
            effect,
            uniforms,
            children: children.to_vec(),
        }
    }

    /// The effect (`effect`).
    #[must_use]
    pub fn effect(&self) -> &RuntimeEffect {
        &self.effect
    }

    /// The uniforms (`uniforms`).
    #[must_use]
    pub fn uniforms(&self) -> &Data {
        &self.uniforms
    }

    /// The children (`children`).
    #[must_use]
    pub fn children(&self) -> &[ChildPtr] {
        &self.children
    }
}

impl BlenderBase for RuntimeBlender {
    // Port of: src/core/SkRuntimeBlender.cpp#L104-L125 (chrome/m156)
    fn on_append_stages(&self, rec: &mut StageRec<'_, '_>) -> bool {
        if !priv_::can_draw(Capabilities::raster_backend(), &self.effect) {
            // SkRP has support for many parts of #version 300 already, but for now, we restrict
            // its usage in runtime effects to just #version 100.
            return false;
        }
        if let Some(program) = self.effect.rp_program() {
            let uniforms =
                priv_::uniforms_as_span(self.effect.uniforms(), &self.uniforms, rec.dst_cs);
            let mut matrix = MatrixRec::new(Matrix::i());
            matrix.mark_ctm_applied();
            let mut callbacks = RuntimeEffectRpCallbacks::new(
                rec,
                &matrix,
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

    // Port of: src/core/SkRuntimeBlender.h#L35 (chrome/m156)
    fn blender_type(&self) -> BlenderType {
        BlenderType::Runtime
    }
}
