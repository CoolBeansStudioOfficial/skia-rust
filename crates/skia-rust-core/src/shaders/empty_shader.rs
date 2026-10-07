// Copyright 2022 Google LLC
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkEmptyShader.{h,cpp}

//! `SkEmptyShader`: a shader that draws nothing.

use crate::effect_priv::StageRec;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};

/// A shader that always draws nothing (`SkEmptyShader`): its stages fail to append. Create one
/// with [`shaders::empty`](crate::shaders::empty).
// Port of: src/shaders/SkEmptyShader.h#L22-L43 (chrome/m156)
#[doc(alias = "SkEmptyShader")]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct EmptyShader;

impl ShaderBase for EmptyShader {
    fn is_opaque(&self) -> bool {
        false
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::Empty
    }

    fn append_stages(&self, _rec: &mut StageRec<'_, '_>, _m_rec: &MatrixRec) -> bool {
        false
    }
}
