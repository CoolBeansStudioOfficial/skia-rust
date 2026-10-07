// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkBlendShader.{h,cpp}

//! `SkBlendShader`: a shader that blends two shaders with a blend mode.
//!
//! Only what `SkClipStack` needs (`SkShaders::Blend(SkBlendMode, ...)`, [`ShaderBase::is_opaque`]
//! and the shader accessors) is ported. Flattening is not ported (no `SkWriteBuffer`), nor the
//! `SkBlender` overload (it needs runtime effects).
//!
//! skia-rust: [`append_stages`](ShaderBase::append_stages) is not ported. Skia's version stores the
//! first shader's output in arena memory that the pipeline reads and writes at run time
//! (`store_src`/`load_dst` on a context the shader allocates); here writable pipeline memory is
//! named by a [`MemSlot`](crate::raster_pipeline::MemSlot) bound per run, and shaders have no way
//! to reserve one yet. Until the first shader that needs it is ported (Phase 3), a blend shader
//! draws nothing (its stages fail to append, like [`EmptyShader`](super::EmptyShader)'s).

use crate::blend_mode::{BlendMode, BlendModeCoeff};
use crate::effect_priv::StageRec;
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};

/// A shader of `src` blended over `dst` with a blend mode (`SkBlendShader`). Create one with
/// [`shaders::blend`](crate::shaders::blend).
// Port of: src/shaders/SkBlendShader.h#L23-L47 (chrome/m156)
#[doc(alias = "SkBlendShader")]
#[derive(Clone, Debug)]
pub struct BlendShader {
    dst: Shader,
    src: Shader,
    mode: BlendMode,
}

impl BlendShader {
    /// `SkBlendShader(mode, dst, src)`.
    // Port of: src/shaders/SkBlendShader.h#L26-L27 (chrome/m156)
    #[must_use]
    pub fn new(mode: BlendMode, dst: Shader, src: Shader) -> BlendShader {
        BlendShader { dst, src, mode }
    }

    /// The destination shader (`dst()`).
    #[must_use]
    pub fn dst(&self) -> &Shader {
        &self.dst
    }

    /// The source shader (`src()`).
    #[must_use]
    pub fn src(&self) -> &Shader {
        &self.src
    }

    /// The blend mode (`mode()`).
    #[must_use]
    pub fn mode(&self) -> BlendMode {
        self.mode
    }
}

impl ShaderBase for BlendShader {
    // Port of: src/shaders/SkBlendShader.cpp#L51-L68 (chrome/m156)
    fn is_opaque(&self) -> bool {
        let Some((src_coeff, dst_coeff)) = self.mode.as_coeff() else {
            return false; // pessimistic
        };
        // The result of the blend is opaque if the sum of "srcCoeff*src + dstCoeff*dst"'s alpha is
        // 1. This is definitely the case if either term of the addition will always be 1.
        let src_is_opaque = self.src.is_opaque();
        let dst_is_opaque = self.dst.is_opaque();
        let coeff_is_opaque = |coeff: BlendModeCoeff| {
            let src_alpha = coeff == BlendModeCoeff::SA || coeff == BlendModeCoeff::SC;
            let dst_alpha = coeff == BlendModeCoeff::DA || coeff == BlendModeCoeff::DC;
            coeff == BlendModeCoeff::One
                || (src_alpha && src_is_opaque)
                || (dst_alpha && dst_is_opaque)
        };
        (src_is_opaque && coeff_is_opaque(src_coeff))
            || (dst_is_opaque && coeff_is_opaque(dst_coeff))
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::Blend
    }

    // skia-rust: not ported, see the module docs.
    fn append_stages(&self, _rec: &mut StageRec<'_, '_>, _m_rec: &MatrixRec) -> bool {
        false
    }
}
