// Copyright 2006 The Android Open Source Project
// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.
// Ported from Skia: include/core/SkShader.h, src/shaders/SkShader.cpp

//! `SkShader`: what a paint fills geometry with (a color, gradient, image, ...).
//!
//! [`Shader`] is the shared, cheaply clonable handle (`sk_sp<SkShader>`); the implementations
//! provide [`ShaderBase`] (`SkShaderBase`'s virtuals, in [`crate::shaders::shader_base`]). The
//! factories (`SkShaders::Color`, `Empty`, ...) are in [`crate::shaders`].

use core::fmt;
use std::sync::Arc;

use crate::color_filter::ColorFilter;
use crate::shaders::color_filter_shader::ColorFilterShader;
use crate::shaders::shader_base::ShaderBase;

/// A shared shader (`sk_sp<SkShader>`): a cheaply clonable handle to a [`ShaderBase`].
///
/// Shaders specify the source color(s) for what is being drawn. If a paint has no shader, then
/// the paint's color is used. If the paint has a shader, then the shader's color(s) are used
/// instead, but they are modulated by the paint's alpha.
///
/// Equality is identity, as Skia compares `sk_sp`s ([`Shader::ptr_eq`]).
///
/// skia-rust: `isAImage`, `makeWithLocalMatrix` and `makeWithWorkingColorSpace` need shaders that
/// are not ported yet (image, local-matrix and working-color-space shaders, Phase 3).
// Port of: include/core/SkShader.h#L36-L96 (chrome/m156)
#[doc(alias = "SkShader")]
#[derive(Clone)]
pub struct Shader(Arc<dyn ShaderBase>);

impl Shader {
    /// Wraps a shader implementation.
    #[must_use]
    pub fn from_base(base: impl ShaderBase) -> Shader {
        Shader(Arc::new(base))
    }

    /// The implementation (`as_SB`).
    #[doc(alias = "as_SB")]
    #[must_use]
    pub fn as_base(&self) -> &dyn ShaderBase {
        &*self.0
    }

    /// True if the shader is guaranteed to produce only opaque colors, subject to the paint
    /// using the shader to apply an opaque alpha value (`isOpaque`). Subclasses should override
    /// this to allow some optimizations.
    #[doc(alias = "isOpaque")]
    #[must_use]
    pub fn is_opaque(&self) -> bool {
        self.0.is_opaque()
    }

    /// A shader that runs this shader's colors through `color_filter` (`makeWithColorFilter`).
    // Port of: src/shaders/SkShader.cpp#L43-L45 (chrome/m156)
    #[doc(alias = "makeWithColorFilter")]
    #[must_use]
    pub fn with_color_filter(&self, color_filter: impl Into<ColorFilter>) -> Shader {
        ColorFilterShader::make(self.clone(), 1.0, Some(color_filter.into()))
    }

    /// True if `self` and `other` are the same shader (Skia's `sk_sp` comparison).
    #[must_use]
    pub fn ptr_eq(&self, other: &Shader) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// True if this is the only handle to the shader (`SkRefCntBase::unique`).
    #[doc(alias = "unique")]
    #[must_use]
    pub fn is_unique(&self) -> bool {
        Arc::strong_count(&self.0) == 1
    }
}

impl PartialEq for Shader {
    /// Identity, as Skia's `sk_sp<SkShader>` `operator==`.
    fn eq(&self, other: &Shader) -> bool {
        self.ptr_eq(other)
    }
}

impl fmt::Debug for Shader {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Shader").field(&self.0).finish()
    }
}
