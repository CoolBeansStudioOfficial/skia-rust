// Copyright 2021 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkBlendShader.{h,cpp}

//! `SkBlendShader`: a shader that blends two shaders with a blend mode.
//!
//! Flattening is not ported (no `SkWriteBuffer`), nor the `SkBlender` overload (it needs runtime
//! effects).
//!
//! skia-rust: [`append_stages`](ShaderBase::append_stages) stores the first shader's output in
//! memory that the pipeline reads and writes at run time (`store_src`/`load_dst`). Skia allocates
//! it in the arena; here writable pipeline memory is named by a
//! [`MemSlot`](crate::raster_pipeline::MemSlot) bound per run, so the shader reserves bytes of
//! [`SHADER_SCRATCH`] with `ArenaAlloc::alloc_scratch` and whoever runs the pipeline binds the
//! buffer.

use crate::blend_mode::{BlendMode, BlendModeCoeff};
use crate::blend_mode_priv;
use crate::effect_priv::{SHADER_SCRATCH, StageRec};
use crate::raster_pipeline::contexts::MAX_STRIDE;
use crate::raster_pipeline::{MemPtr, Stage};
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

/// Returns the output of `s0` (in scratch memory), and leaves the output of `s1` in `r,g,b,a`
/// (`append_two_shaders`).
// Port of: src/shaders/SkBlendShader.cpp#L75-L110 (chrome/m156)
fn append_two_shaders(
    rec: &mut StageRec<'_, '_>,
    m_rec: &MatrixRec,
    s0: &Shader,
    s1: &Shader,
) -> Option<MemPtr> {
    // struct Storage { float fCoords[2 * kMaxStride]; float fRes0[4 * kMaxStride]; };
    // (`make<Storage>()`: see `ArenaAlloc::alloc_scratch`.)
    #[allow(clippy::cast_possible_truncation)] // a small constant
    const COORDS_BYTES: u32 = 2 * MAX_STRIDE as u32 * 4;
    #[allow(clippy::cast_possible_truncation)] // a small constant
    const RES0_BYTES: u32 = 4 * MAX_STRIDE as u32 * 4;
    let base = rec
        .alloc
        .alloc_scratch((COORDS_BYTES + RES0_BYTES) as usize);
    let coords = MemPtr::new(SHADER_SCRATCH, base);
    let res0 = MemPtr::new(SHADER_SCRATCH, base + COORDS_BYTES);

    // Note we cannot simply apply mRec here and then unconditionally store the coordinates. When
    // building for Android Framework it would interrupt the backwards local matrix
    // concatenation if mRec had a pending local matrix and either of the children also had a
    // local matrix. b/256873449
    if m_rec.raster_pipeline_coords_are_seeded() {
        rec.pipeline.append(Stage::StoreSrcRg(coords));
    }
    if !s0.as_base().append_stages(rec, m_rec) {
        return None;
    }
    rec.pipeline.append(Stage::StoreSrc(res0));

    if m_rec.raster_pipeline_coords_are_seeded() {
        rec.pipeline.append(Stage::LoadSrcRg(coords));
    }
    if !s1.as_base().append_stages(rec, m_rec) {
        return None;
    }
    Some(res0)
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

    // Port of: src/shaders/SkBlendShader.cpp#L112-L123 (chrome/m156)
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        let Some(res0) = append_two_shaders(rec, m_rec, &self.dst, &self.src) else {
            return false;
        };

        rec.pipeline.append(Stage::LoadDst(res0));
        blend_mode_priv::append_stages(self.mode, rec.pipeline);
        true
    }
}
