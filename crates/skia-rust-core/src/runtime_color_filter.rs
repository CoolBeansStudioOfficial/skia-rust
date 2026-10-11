// Copyright 2023 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/effects/colorfilters/SkRuntimeColorFilter.{h,cpp}

//! `SkRuntimeColorFilter`: a color filter that runs a [`RuntimeEffect`] through the Raster
//! Pipeline.
//!
//! skia-rust: flattening and `asRuntimeEffect` are not ported.

use core::fmt;

use crate::capabilities::Capabilities;
use crate::color_filter::{ColorFilterBase, ColorFilterType};
use crate::data::Data;
use crate::effect_priv::StageRec;
use crate::flattenable::FlattenableRegistry;
use crate::known_runtime_effects::{is_skia_known_runtime_effect, maybe_get_known_runtime_effect};
use crate::matrix::Matrix;
use crate::picture_priv;
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

/// `SkRuntimeColorFilter::CreateProc`. An effect that is not a known stable key carries `SkSL`
/// source, which is not compiled here, so such a filter is not read.
// Port of: src/effects/colorfilters/SkRuntimeColorFilter.cpp#L89-L125 (chrome/m156), CreateProc
pub fn create_proc(
    buffer: &mut ReadBuffer<'_>,
    _registry: &FlattenableRegistry,
) -> Option<crate::color_filter::ColorFilter> {
    // `buffer.allowSkSL()` is true by default, and the reader never turns it off.
    let mut effect = None;
    if !buffer.is_version_lt(picture_priv::VERSION_SERIALIZE_STABLE_KEYS) {
        let candidate = buffer.read_uint();
        effect = maybe_get_known_runtime_effect(candidate);
        if effect.is_none() && !buffer.validate(candidate == 0) {
            return None;
        }
    }
    let Some(effect) = effect else {
        // The SkSL source is read and compiled by Skia; the compiler is not ported.
        let _ = buffer.read_string();
        return None;
    };
    let uniforms = buffer
        .read_byte_array_as_data()
        .unwrap_or_else(Data::new_empty);

    // `SkRuntimeEffectPriv::ReadChildEffects`: the count must match the effect's children.
    let child_count = buffer.read32();
    let matches = usize::try_from(child_count).ok() == Some(effect.children().len());
    if !buffer.validate(matches) {
        return None;
    }
    if child_count != 0 {
        // Child effects are read with `readRawFlattenable`, which is not ported.
        buffer.validate(false);
        return None;
    }
    if !buffer.is_valid() {
        return None;
    }
    effect.make_color_filter(uniforms, &[])
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
    // skia-rust: only the Skia-internal effects are written by stable key. An effect that is
    // not one of them is written with its SkSL source, which is not ported, so its flatten fails.
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        let stable_key = self
            .effect
            .0
            .stable_key
            .load(std::sync::atomic::Ordering::Relaxed);
        if is_skia_known_runtime_effect(stable_key.cast_signed()) {
            buffer.write_uint(stable_key);
        } else {
            buffer.write_uint(0);
            buffer.mark_flatten_failed();
        }
        buffer.write_byte_array(self.uniforms.as_bytes());
        // `WriteChildEffects`: the count, then each child (none are written here).
        let child_count = u32::try_from(self.children.len()).unwrap_or(u32::MAX);
        buffer.write_uint(child_count);
        if !self.children.is_empty() {
            buffer.mark_flatten_failed();
        }
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
