// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkRuntimeColorFilter.{h,cpp}

//! `SkRuntimeColorFilter`: a color filter that runs a [`RuntimeEffect`] through the Raster
//! Pipeline.
//!
//! skia-rust: `asRuntimeEffect` is not ported.

use core::fmt;

use crate::capabilities::Capabilities;
use crate::color_filter::{ColorFilterBase, ColorFilterType};
use crate::data::Data;
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::matrix::Matrix;
use crate::read_buffer::ReadBuffer;
use crate::runtime_effect::{ChildPtr, RuntimeEffect};
use crate::runtime_effect_priv::{self as priv_, RuntimeEffectRpCallbacks};
use crate::shaders::shader_base::MatrixRec;
use crate::write_buffer::BinaryWriteBuffer;

/// A color filter that runs a [`RuntimeEffect`] (`SkRuntimeColorFilter`).
// Port of: src/effects/colorfilters/SkRuntimeColorFilter.h#L24-L51 (chrome/m156)
#[doc(alias = "SkRuntimeColorFilter")]
pub struct RuntimeColorFilter {
    effect: RuntimeEffect,
    uniforms: Data,
    children: Vec<ChildPtr>,
}

/// `SkRuntimeColorFilter::CreateProc`: rebuilds a runtime color filter from its effect, uniforms
/// and children.
// Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L89-L125 (chrome/m156), CreateProc
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    registry: &FlattenableRegistry,
) -> Option<crate::color_filter::ColorFilter> {
    let effect = priv_::read_effect(buffer, |sksl, options| {
        RuntimeEffect::make_for_color_filter(sksl, options)
    })?;
    let uniforms = buffer
        .read_byte_array_as_data()
        .unwrap_or_else(Data::new_empty);
    let children = priv_::read_child_effects(buffer, &effect, registry)?;
    effect.make_color_filter(uniforms, &children)
}

impl fmt::Debug for RuntimeColorFilter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RuntimeColorFilter")
            .field("effect", &self.effect)
            .field("uniforms", &self.uniforms)
            .field("children", &self.children)
            .finish()
    }
}

impl RuntimeColorFilter {
    /// A color filter of `effect` with `uniforms` and `children`.
    // Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L36-L41 (chrome/m156)
    #[must_use]
    pub fn new(effect: RuntimeEffect, uniforms: Data, children: &[ChildPtr]) -> Self {
        RuntimeColorFilter {
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

impl ColorFilterBase for RuntimeColorFilter {
    // Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L89 (chrome/m156), SK_FLATTENABLE_HOOKS
    fn type_name(&self) -> &'static str {
        "SkRuntimeColorFilter"
    }

    // Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L75-L85 (chrome/m156), flatten
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        priv_::write_stable_key_or_source(buffer, &self.effect);
        buffer.write_byte_array(self.uniforms.as_bytes());
        priv_::write_child_effects(buffer, &self.children);
    }

    // Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L43-L63 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, _shader_is_opaque: bool) -> bool {
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

    // Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L65-L67 (chrome/m156)
    fn on_is_alpha_unchanged(&self) -> bool {
        priv_::is_alpha_unchanged(&self.effect)
    }

    // Port of: src/effects/colorfilters/SkRuntimeColorFilter.h#L36 (chrome/m156)
    fn color_filter_type(&self) -> ColorFilterType {
        ColorFilterType::Runtime
    }
}
