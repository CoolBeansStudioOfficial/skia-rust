// Copyright 2023 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: src/shaders/SkCoordClampShader.{h,cpp}

//! `SkCoordClampShader`: samples a shader with its coordinates clamped to a subset rectangle.

use crate::effect_priv::StageRec;
use crate::matrix::Matrix;
use crate::raster_pipeline::Stage;
use crate::raster_pipeline::contexts::CoordClampCtx;
use crate::rect::Rect;
use crate::shader::Shader;
use crate::shaders::shader_base::{MatrixRec, ShaderBase, ShaderType};
use crate::write_buffer::BinaryWriteBuffer;

/// A shader whose coordinates are clamped to `subset` before it samples `shader`
/// (`SkCoordClampShader`).
// Port of: src/shaders/SkCoordClampShader.h#L23-L46 (chrome/m156)
#[doc(alias = "SkCoordClampShader")]
#[derive(Clone, Debug)]
pub struct CoordClampShader {
    shader: Shader,
    subset: Rect,
}

impl CoordClampShader {
    /// `SkShaders::CoordClamp(shader, subset)`: `None` if there is no shader or the subset is not
    /// sorted.
    // Port of: src/shaders/SkCoordClampShader.cpp#L55-L67 (chrome/m156)
    #[doc(alias = "CoordClamp")]
    #[must_use]
    pub fn make(shader: Option<Shader>, subset: Rect) -> Option<Shader> {
        let shader = shader?;
        if !subset.is_sorted() {
            return None;
        }
        Some(Shader::from_base(CoordClampShader { shader, subset }))
    }

    /// The shader whose coordinates are clamped (`shader()`).
    // Port of: src/shaders/SkCoordClampShader.h#L30 (chrome/m156)
    #[must_use]
    pub fn shader(&self) -> &Shader {
        &self.shader
    }

    /// The subset the coordinates are clamped to (`subset()`).
    // Port of: src/shaders/SkCoordClampShader.h#L31 (chrome/m156)
    #[must_use]
    pub fn subset(&self) -> Rect {
        self.subset
    }
}

impl ShaderBase for CoordClampShader {
    // Port of: src/shaders/SkCoordClampShader.cpp#L28-L33 (chrome/m156), `flatten`
    fn flatten(&self, buffer: &mut BinaryWriteBuffer) {
        buffer.write_shader(Some(&self.shader));
        buffer.write_rect(&self.subset);
    }

    // Port of: src/shaders/SkCoordClampShader.h#L28 (chrome/m156)
    fn is_opaque(&self) -> bool {
        self.shader.is_opaque()
    }

    fn shader_type(&self) -> ShaderType {
        ShaderType::CoordClamp
    }

    // Port of: src/shaders/SkCoordClampShader.cpp#L35-L53 (chrome/m156), `appendStages`
    fn append_stages(&self, rec: &mut StageRec<'_, '_>, m_rec: &MatrixRec) -> bool {
        // The child's matrix is only valid inside the subset rectangle, but it is not marked as
        // such, so that SkImageShader's "total matrix is valid" filtering still applies.
        let Some(child_m_rec) = m_rec.apply(rec, &Matrix::default()) else {
            return false;
        };

        let clamp_ctx = rec.alloc.make(CoordClampCtx {
            min_x: self.subset.left,
            min_y: self.subset.top,
            max_x: self.subset.right,
            max_y: self.subset.bottom,
        });
        rec.pipeline.append(Stage::ClampXAndY(clamp_ctx));
        self.shader.as_base().append_stages(rec, &child_m_rec)
    }
}
